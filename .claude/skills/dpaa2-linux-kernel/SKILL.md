---
name: dpaa2-linux-kernel
description: Use when working with the Freescale/NXP DPAA2 (Data Path Acceleration Architecture Gen2) Linux kernel drivers — the fsl-mc bus, MC firmware objects (DPRC, DPNI, DPMAC, DPIO, DPBP, DPCON, DPMCP, DPSW), the dpaa2-eth Ethernet driver, dpaa2-mac/PHY (phylink) integration, and the dpaa2-switch driver on QorIQ SoCs such as LS1088A, LS2080A/LS2088A, and LX2160A.
---

# DPAA2 Linux Kernel Skill

Guidance for understanding and working with the **DPAA2 (Data Path Acceleration Architecture Gen2)** Linux kernel drivers used on NXP/Freescale QorIQ SoCs (e.g. LS1088A, LS2080A, LS2088A, LX2160A). DPAA2 replaces the monolithic-NIC model with a set of composable hardware objects managed by an on-chip **Management Complex (MC)** firmware and exposed to Linux over the **fsl-mc bus**.

> **Sources:** This skill synthesizes the official Linux kernel networking documentation
> (docs.kernel.org) across four DPAA2 topics: architecture overview, DPIO driver, MAC/PHY
> support, and the switch driver, plus the dpaa2-eth Ethernet driver page. All content is
> from a single, consistent source type (official kernel docs), so the sources **agree** and
> no conflicts were detected. See [Source Map](#source-map) for the confidence breakdown.

## When to Use This Skill

Use this skill when you need to:

- **Understand DPAA2 architecture** — how a network interface is composed from separate MC objects (DPNI + DPMAC + DPIO + DPBP + DPCON) rather than one NIC block.
- **Map objects to Linux drivers** — figure out which driver binds to which object (`dpaa2-eth` ↔ DPNI, `dpaa2-mac` ↔ DPMAC, `dpio-driver` ↔ DPIO, `dpaa2-switch` ↔ DPSW, `fsl_mc_allocator` ↔ DPMCP/DPBP).
- **Bring up or debug a DPAA2 network interface** — trace the `ip link set dev ethN up` → phylink → `dpmac_set_link_state()` → `netif_carrier_on()` flow.
- **Work with the fast path** — enqueue/dequeue frames and manage buffer pools through the DPIO service APIs (`dpaa2_io_service_*`).
- **Configure the DPAA2 switch** — set up bridging, ACL-based redirect/trap/drop, and port/VLAN mirroring with `bridge` and `tc-flower`.
- **Provision objects** — decide between static provisioning (Datapath Layout / DPL binary parsed by MC at boot) versus dynamic runtime creation (DPAA2 object APIs / `restool`).
- **Debug link or PHY issues** — understand DPMAC link modes (`DPMAC_LINK_TYPE_FIXED` vs `DPMAC_LINK_TYPE_PHY`) and how phylink interacts with MC firmware.
- **Consult the fsl-mc bus device-tree binding** (`compatible = "fsl,qoriq-mc"`) or sysfs bind/unbind interfaces.

## Key Concepts

### The Management Complex (MC)
An on-chip hardware component that owns all DPAA2 resources (queues, buffer pools, ports) and exposes them to software as **objects**. Software talks to the MC through memory-mapped **MC portals (DPMCP)**. The MC mediates *control-plane* operations — create, discover, connect, configure, destroy — but **not** the fast path: packet TX/RX happens directly via DPIO MMIO regions, bypassing the MC.

### DPAA2 Objects (the building blocks)

| Object | Full name | Role | Bound Linux driver |
|--------|-----------|------|--------------------|
| **DPRC** | Datapath Resource Container | Plug-and-play "bus" holding all other objects; IOMMU isolation is per-DPRC | `fsl_mc_bus` / DPRC driver |
| **DPNI** | Datapath Network Interface | A network interface (TX/RX queues, offload/classify config). One DPNI = one Linux netdev | `dpaa2-eth` |
| **DPMAC** | Datapath Ethernet MAC | Connects to an Ethernet PHY; physical TX/RX of frames | `dpaa2-mac` (phylink) |
| **DPIO** | Datapath I/O (QBman SW portal) | Enqueue/dequeue frames + HW buffer-pool ops; ~1 per CPU | `dpio-driver` |
| **DPBP** | Datapath Buffer Pool | HW buffer pool for storing ingress frame data | `fsl_mc_allocator` |
| **DPCON** | Datapath Concentrator | Groups queues into channels; distributes ingress traffic across CPUs | used by `dpaa2-eth` |
| **DPMCP** | Datapath MC Portal | Command portal for sending commands to the MC | `fsl_mc_allocator` |
| **DPSW** | Datapath Switch | L2 switch; each port exposed as a netdev | `dpaa2-switch` |

### Composing a network interface
A single Linux net device is built on a **DPNI**, and additionally uses **DPBPs** (buffer pools it seeds with kernel-allocated buffers), **DPIOs** (per-CPU I/O portals), and **DPCONs** (one channel per CPU that services a queue). Ingress data-availability notifications are raised **per channel** and must be **explicitly re-armed** after firing.

### DPNI ↔ PHY decoupling
DPNIs have no direct 1:1 mapping to PHYs. A DPNI connects either to a **DPMAC** (→ external PHY) or to **another DPNI** (internal link). The connection is made by the MC and is transparent to `dpaa2-eth`.

### Static vs dynamic provisioning
Objects can be added to a DPRC two ways:
1. **Statically** — via a **Datapath Layout (DPL)** binary parsed by the MC at boot.
2. **Dynamically** — at runtime via the DPAA2 object APIs, typically through the `restool` userspace tool.

## Quick Reference

The DPAA2 docs are architectural, so the highest-signal "examples" are the driver APIs,
device-tree bindings, sysfs paths, and `bridge`/`tc` command patterns. All items below are
**from official kernel docs**.

### fsl-mc bus device-tree binding *(from official docs — overview)*
The MC-bus driver is a platform driver probed from a device-tree node supplied by boot firmware:
```dts
/* Documentation/devicetree/bindings/misc/fsl,qoriq-mc.yaml */
soc {
    fsl_mc: fsl-mc@80c000000 {
        compatible = "fsl,qoriq-mc";
        /* MC portal + registers; children discovered dynamically via DPRC scan */
    };
};
```

### DPIO service API — fast-path enqueue/dequeue *(from official docs — dpio-driver)*
Public APIs exported by `dpio-service.c` (declared in `dpaa2-io.h`) for other DPAA2 drivers:
```c
/* Notifications */
dpaa2_io_service_register();      /* register data-availability callback */
dpaa2_io_service_deregister();
dpaa2_io_service_rearm();         /* re-arm a channel after a notification */

/* Dequeue (pull) */
dpaa2_io_service_pull_fq();       /* pull from a frame queue   */
dpaa2_io_service_pull_channel();  /* pull from a channel       */
dpaa2_io_store_create();          /* dequeue result storage    */
dpaa2_io_store_next();
dpaa2_io_store_destroy();

/* Enqueue */
dpaa2_io_service_enqueue_fq();    /* enqueue to a frame queue     */
dpaa2_io_service_enqueue_qd();    /* enqueue to a queuing dest.   */

/* Buffer pool management */
dpaa2_io_service_release();       /* release (seed) buffers to a pool */
dpaa2_io_service_acquire();       /* acquire buffers from a pool      */
```
> Frame-descriptor / scatter-gather helpers live in `dpaa2-fd.h`; dequeue-result parsing in
> `dpaa2-global.h`. The low-level `qbman-portal.c` APIs are **private** — only `dpio-service`
> may call them.

### MAC/PHY (phylink) integration API *(from official docs — mac-phy-support)*
`dpaa2-eth`/`dpaa2-ethsw` use these to attach a DPMAC to phylink:
```c
/* Connect a DPNI's peer DPMAC to phylink at probe / endpoint-change time */
dpaa2_mac_connect();     /* looks up phy-handle, creates phylink, phylink_of_phy_connect() */
dpaa2_mac_disconnect();  /* at unbind / disconnect: tears down phylink instance */

/* Only integrate phylink when the peer DPMAC is NOT of TYPE_FIXED
 * (i.e. TYPE_PHY or TYPE_BACKPLANE). Use the provided helper to check. */
```
Implemented `phylink_mac_ops` callbacks: `.validate()`, `.mac_config()`, `.mac_link_up()`,
`.mac_link_down()` — all program the HW MAC via the MC firmware API `dpmac_set_link_state()`.

### Link-up sequence for a DPNI–DPMAC connection *(from official docs — mac-phy-support)*
```text
ip link set dev eth0 up
  → phylink_start()                 # from .dev_open()
  → .mac_config() / .mac_link_up()  # called by PHYLINK
  → dpmac_set_link_state()          # MC firmware programs the HW MAC
  → netif_carrier_on()              # called directly by PHYLINK
# dpaa2-eth then handles the LINK_STATE_CHANGE irq (Rx taildrop / pause frames)
```

### Link-up sequence for a DPNI–DPNI (internal link) connection *(from official docs)*
```text
ip link set dev eth0 up   →  dpni_enable() on eth0's fsl_mc_device
ip link set dev eth1 up   →  dpni_enable() on eth1's fsl_mc_device
# LINK_STATE_CHANGED irq received by both dpaa2-eth instances once link is up
# netif_carrier_on() called from link_state_update()
```

### Switch: ACL redirect / trap / drop with tc-flower *(from official docs — switch-driver)*
```bash
# Trap: send frames on eth4 with a given source MAC to the CPU
tc filter add dev eth4 ingress flower src_mac 00:01:02:03:04:05 action trap

# Drop: drop frames on eth4 with VID 100 and PCP 3
tc filter add dev eth4 ingress protocol 802.1Q flower vlan_id 100 vlan_prio 3 action drop

# Redirect: send all frames received on eth4 to eth1
tc filter add dev eth4 ingress matchall action mirred egress redirect dev eth1
```
> Supported flow keys: Ethernet `dst_mac`/`src_mac`; IPv4 `dst_ip`/`src_ip`/`ip_proto`/`tos`;
> VLAN `vlan_id`/`vlan_prio`/`vlan_tpid`/`vlan_dei`; L4 `dst_port`/`src_port`. Each ACL entry
> supports **exactly one** action. Shared filter *blocks* let one ACL table cover multiple ports.

### Switch: mirroring *(from official docs — switch-driver)*
```bash
# Per-VLAN mirror (only vlan_id is accepted from 802.1q; other fields rejected).
# The VLAN must be installed on the port (via bridge or a VLAN upper device),
# and the switch supports only ONE mirror destination for all rules.
tc filter add dev eth5 ingress protocol 802.1Q flower vlan_id 100 \
    action mirred egress mirror dev eth1
```

### Switch: FDB learning and broadcast flooding *(from official docs — switch-driver)*
```bash
# HW FDB learning is configured per switch port; disabling it fast-ages learnt addresses
bridge link set dev eth5 learning off

# Broadcast flooding can be toggled per port via brport sysfs
bridge link set dev eth5 flood off      # unknown-unicast/multicast flooding
```
> The DPAA2 switch HW is **not** configurable for VLAN awareness — ports must be used with a
> **VLAN-aware bridge**. STP topology/loop detection needs `stp_state 1` at bridge creation.

## Reference Files

Detailed documentation lives in `references/`. All files are **official Linux kernel docs
(docs.kernel.org)** at **medium** confidence.

- **`references/dpaa2.md`** *(4 pages, ~20 KB — official kernel docs, medium confidence)* — the core reference. Combines four sub-topics:
  - **Architecture Overview** — MC, DPRC, and every object type (DPMAC/DPNI/DPIO/DPBP/DPMCP), plus the Linux driver stack (MC-bus, DPRC, allocator, DPIO, Ethernet, MAC drivers) and the fsl-mc device-tree binding.
  - **DPIO Driver** — object driver vs service vs QBman portal layering, and the full `dpaa2_io_service_*` API list.
  - **MAC / PHY Support** — phylink integration, `DPMAC_LINK_TYPE_FIXED` vs `TYPE_PHY`/`TYPE_BACKPLANE`, `dpaa2_mac_connect/disconnect()`, and the link-up call sequences.
  - **Switch Driver** — DPSW probing/requirements, bridging, ACL offloads, and mirroring.
- **`references/ethernet.md`** *(1 page, ~4.6 KB — official kernel docs, medium confidence)* — the `dpaa2-eth` driver: supported platforms (LS2080A/LS2088A/LS1088A), how a netdev is built from DPNI+DPBP+DPIO+DPCON, and features/offloads (HW checksum for TCP/UDP over IPv4/6, unicast/multicast MAC filtering, scatter-gather, up to 10K jumbo frames, static 5-tuple RSS hash, ethtool `-S` stats).
- **`references/index.md`** *(category index)* — maps the categories above to their files/page counts.

Use `view` (or `Read`) on a reference file when you need the full prose behind a summary here.

## Working with This Skill

### Beginner — "What *is* DPAA2 and why isn't there one NIC?"
1. Read the [Key Concepts](#key-concepts) section, then `references/dpaa2.md` (Overview part).
2. Internalize the object table: a netdev = **DPNI** + supporting **DPBP/DPIO/DPCON**, connected to a **DPMAC** for a physical port.
3. Note the control-plane (MC-mediated) vs fast-path (DPIO direct) split — this is the single most important architectural idea.

### Intermediate — "I'm bringing up / debugging an interface"
1. Confirm the fsl-mc bus probed (device tree `compatible = "fsl,qoriq-mc"`; sysfs `bus/fsl-mc`).
2. Check which objects exist in the DPRC (statically via DPL, or dynamically via `restool`).
3. Trace link bring-up using the [link-up sequences](#link-up-sequence-for-a-dpnidpmac-connection-from-official-docs--mac-phy-support). If `netif_carrier` never comes up, inspect the DPMAC link mode (`FIXED` bypasses `dpaa2-eth` binding entirely).
4. For offload/feature questions consult `references/ethernet.md` and `ethtool -S ethN`.

### Advanced — "I'm writing/porting a driver or touching the fast path"
1. Use the DPIO service APIs (`dpaa2_io_service_*`) for enqueue/dequeue and buffer-pool ops; never call `qbman-portal.c` directly.
2. Remember DPIO affinity: ~1 DPIO per CPU so all CPUs enqueue/dequeue simultaneously; DPCON channels distribute ingress across those CPUs; **re-arm** a channel after each notification.
3. For MAC integration, follow `dpaa2_mac_connect()`/`disconnect()` and implement the phylink `mac_ops`; gate phylink on the non-`FIXED` DPMAC check.
4. For switch offloads, respect the DPSW requirements (≥ FDBs = #ports, per-FDB flood/broadcast, control interface enabled) and the one-action-per-ACL-entry rule.

### Navigating multiple reference topics
`references/dpaa2.md` bundles four sub-topics under one file — search within it by heading
(Overview / DPIO / MAC / Switch). Ethernet-driver specifics are split out into
`references/ethernet.md`. When a summary here cites a topic, open the matching section of the
reference for the full text.

## Source Map

| Reference | Source type | Confidence | Size | Pages |
|-----------|-------------|-----------|------|-------|
| `dpaa2.md` | Official kernel docs | Medium | ~20 KB | 4 |
| `ethernet.md` | Official kernel docs | Medium | ~4.6 KB | 1 |
| `index.md` | Category index | Medium | ~152 B | — |

**Source agreement:** All references come from one consistent origin — the official Linux
kernel networking documentation (docs.kernel.org). They describe complementary, non-overlapping
topics and are internally consistent, so **no discrepancies were detected**. If you later add
codebase-analysis or GitHub-issue sources, apply this priority when they disagree:
1. Code patterns (what the driver actually does) — ground truth
2. Official documentation (intended API/usage) — the current basis of this skill
3. GitHub issues (real-world usage / known problems)
4. PDF/other documentation (additional context)

## Notes

- Content is synthesized from official NXP/Freescale DPAA2 Linux kernel documentation.
- DPAA2 targets QorIQ SoCs; switch (DPSW) support is specifically documented for **LS2088A** and **LX2160A**, and the Ethernet driver for **LS2080A/LS2088A/LS1088A**.
- Reference files preserve the structure and wording of the source docs.

## Updating

To refresh this skill with updated documentation:
1. Re-run the scraper against the DPAA2 pages under `docs.kernel.org/networking/device_drivers/ethernet/freescale/dpaa2/`.
2. The skill will be rebuilt; re-verify the API lists and command examples against the driver source if a codebase source is added.
