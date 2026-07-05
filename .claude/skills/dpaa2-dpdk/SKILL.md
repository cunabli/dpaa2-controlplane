---
name: dpaa2-dpdk
description: Use when working with DPDK (userspace Data Plane Development Kit) drivers for NXP/Freescale DPAA2 (Data Path Acceleration Architecture Gen2) hardware — the net/dpaa2 Ethernet PMD, the dpaa2_sec crypto PMD (DPSECI/CAAM), the event_dpaa2 eventdev driver, the dpaa2_dpci CMDIF rawdev driver, or the dpaa2_dpdmai QDMA dmadev driver on NXP QorIQ SoCs. Covers the fslmc bus driver, the DPAA2 object model (DPRC, DPNI, DPMAC, DPIO, DPBP, DPMCP, DPCONI, DPCI, DPDMAI, DPSECI), EAL vdev/device args (-a fslmc:..., dev-arg tuning), Traffic Management/QoS, and per-driver --log-level options. Not for the Linux kernel dpaa2-eth/fsl-mc drivers — use dpaa2-linux-kernel for that.
---

# Dpaa2-Dpdk Skill

Guide to using DPDK's NXP DPAA2 (Data Path Acceleration Architecture Gen2) drivers: the
`net/dpaa2` NIC PMD, the `dpaa2_sec` crypto PMD, the DPAA2 eventdev driver, the DPAA2 CMDIF
rawdev driver, and the DPAA2 QDMA dmadev driver. All five drivers share the same underlying
object model — the fslmc bus, the Management Complex (MC), and DPAA2 hardware objects
(DPRC/DPNI/DPMAC/DPIO/DPBP/DPMCP/DPSECI/DPCI/DPCONI/DPDMAI) — so understanding one makes the
others easier to pick up.

## Source Overview

This skill was built from 5 official DPDK documentation pages (doc.dpdk.org, DPDK
26.07.0-rc2), ~23K chars total. The build pipeline classified the same scraped content under
two labels — `documentation` and `unknown` — but a byte-for-byte diff of the underlying files
(`references/guides.md` vs `references/documentation/dpaa2-dpdk_docs/guides.md`) confirms they
are identical copies of one source, not independent sources. **There is only one real source
here**, so "multi-source synthesis" reduces to: no discrepancies are possible by construction,
and everything below can be treated as directly-attributable official documentation rather than
a reconciliation across viewpoints. If this skill is regenerated later with real second sources
(e.g. `drivers/net/dpaa2/` codebase scans or DPDK GitHub issues), prefer those over this doc
snapshot per the priority order in **Conflict Resolution** below — dev-arg names, defaults, and
limitations drift between DPDK releases faster than the prose describing them.

## When to Use This Skill

Use this skill when you need to:
- Understand the DPAA2 object model (DPRC, DPNI, DPMAC, DPIO, DPBP, DPMCP, DPCONI, DPCI,
  DPDMAI, DPSECI) and how the MC mediates control-plane operations while DPIO handles
  fast-path enqueue/dequeue directly through memory-mapped regions, bypassing the MC.
- Bring up or debug the `net/dpaa2` Ethernet PMD (`librte_net_dpaa2`) — device args, the fixed
  10240-byte jumbo frame ceiling, RSS constraints (immutable hash key, no RETA config), or
  Traffic Management (TM) hierarchical scheduling / shaping.
- Configure the `dpaa2_sec` crypto PMD (DPAA2_SEC/DPSECI, backed by the SEC/CAAM hardware
  accelerator) — which cipher/auth/AEAD algorithms are supported, and why hash-then-cipher
  chaining and session-less APIs aren't available.
- Work with the DPAA2 eventdev (`event_dpaa2`, DPCONI/DPCI-based hardware event scheduler),
  CMDIF rawdev (`dpaa2_dpci`, GPP↔AIOP firmware communication), or QDMA dmadev
  (`dpaa2_dpdmai`, DPDMAI-based CPU-offloaded DMA) vdev/dma drivers.
- Look up dev-arg options (e.g. `drv_loopback`, `drv_tx_conf`, `drv_dump_mode`,
  `fle_pre_populate`) or per-driver `--log-level` matching criteria before debugging.
- Set up allowlist/blocklist rules to keep DPAA2's limited HW portals and Management Control
  Ports available across primary/secondary multi-process applications.
- Find the right SoC support / prerequisite notes before targeting a board (all DPAA2 drivers
  only run on NXP DPAA2 SoCs; check the "Supported DPAA2 SoCs" subsection per driver page in
  `references/guides.md` — the exact SoC list isn't enumerated in this scrape).

## Key Concepts

- **Management Complex (MC)**: hardware block that owns DPAA2 resources (queues, buffer
  pools, ports) and exposes them as objects via memory-mapped MC portals. The MC mediates
  slow-path operations (create/discover/connect/configure/destroy); it is *not* involved in
  packet TX/RX, which goes directly through DPIO-mapped memory regions.
- **DPRC (Datapath Resource Container)**: a container object holding all other DPAA2 objects,
  functionally similar to a plug-and-play bus (like PCI) from the OS's point of view. Objects
  can be hot-plugged in/out; a Linux userspace tool called `restool` (or a static config file
  passed to the MC at firmware start) creates/destroys them. All objects in one DPRC share the
  same hardware isolation context — for ARM-based SoCs this "device-id" is the IOMMU stream ID,
  so isolation granularity is per-container, not per-object.
- **DPNI (Datapath Network Interface)**: TX/RX queues + network interface configuration; one
  DPNI maps to one DPDK Ethernet device. Must be connected to a DPMAC, another DPNI, or an
  L2-switch port via a DPRC command, and needs an associated DPBP for RX buffers.
- **DPMAC (Datapath Ethernet MAC)**: represents the physical Ethernet MAC/PHY connection;
  supports link up/down, link config, and stats commands, and raises a DPNI link-change IRQ.
- **DPIO (Datapath I/O)**: the mechanism for enqueue/dequeue and buffer-pool management,
  decoupled from the queues themselves. Typically one DPIO per physical CPU so all cores can
  enqueue/dequeue concurrently; shared across all DPAA2 drivers (Ethernet, crypto, eventdev...).
  IRQs cover data availability, congestion notification, and buffer-pool depletion.
- **DPBP (Datapath Buffer Pool)**: a hardware-backed buffer pool; the Ethernet driver
  configures the DPBP(s) that back RX buffers for a DPNI.
- **DPMCP (Datapath MC Portal)**: the command portal drivers use to talk to the MC; raises a
  command-completion IRQ.
- **DPCONI / DPCI**: the concurrency/notification (DPCON) and communication-interface (DPCI)
  objects backing `event_dpaa2` — the eventdev vdev is built by probing a set of DPCON and
  DPCI devices at EAL init.
- **DPSECI / DPCI / DPDMAI**: the crypto (SEC), CMDIF (GPP↔AIOP), and QDMA (DMA) counterparts
  of DPNI — each represents the MC-managed object backing that driver's function.
- **fslmc bus driver**: a `rte_bus` driver that scans the fsl-mc bus, sets up the VFIO group,
  and parses/enumerates MC objects into per-type device lists; also provides the generic MC
  object driver. Part of its flib (MC object library) code is dual-licensed BSD/GPLv2 but used
  as BSD within DPDK userspace.

```
DPRC.1 (bus)
  |
  +--+--------+-------+-------+-------+---------+
     |        |       |       |       |         |
   DPMCP.1  DPIO.1  DPBP.1  DPNI.1  DPMAC.1  DPSECI.1
   DPMCP.2  DPIO.2          DPNI.2  DPMAC.2  DPSECI.2
   DPMCP.3
```
*From official docs — a container with 8 objects across 5+ types, illustrating a NIC + crypto
setup on a 2-CPU system (2 DPIOs, one per core).*

### Object resource summary

| Object | MMIO regions | IRQs | Key commands |
|---|---|---|---|
| DPMAC | — | link change (raised on the DPNI) | link up/down, link config, get stats, IRQ config, enable, reset |
| DPNI  | TX/RX queues in memory | — | port config, offload config, queue config, parse/classify config, IRQ config, enable, reset |
| DPIO  | queue ops, buffer mgmt | data availability, congestion, buffer-pool depletion | IRQ config, enable, reset |
| DPBP  | — | — | enable, reset |
| DPMCP | MC command portal | command completion | IRQ config, enable, reset |

## Driver-by-Driver Quick Reference

All examples below are extracted verbatim from the official DPDK docs (`references/guides.md`).

### 1. `net/dpaa2` NIC PMD

Device args (EAL `-a`/vdev syntax, format `fslmc:dpni.<id>,<opt>=<val>`):
```
fslmc:dpni.1,drv_loopback=1       # loop packets back at driver level
fslmc:dpni.1,drv_no_prefetch=1    # disable RX pull-command prefetch
fslmc:dpni.1,drv_tx_conf=1        # enable TX confirmation (poll tx-conf queues to free bufs)
fslmc:dpni.1,drv_rx_parse_drop=1  # let hardware drop parse-error packets
fslmc:dpni.1,drv_error_queue=1    # don't drop error packets; deliver them to an error queue for inspection
```

Logging:
```
--log-level=bus.fslmc,<level>       # FSLMC bus logs
--log-level=pmd.net.dpaa2,<level>   # net/dpaa2 PMD logs
```

Limitations:
- Max jumbo frame is a **fixed 10240 bytes** — setting `rxmode.mtu` lower does not stop
  frames up to 10240 bytes from reaching the host interface.
- RSS hash key **cannot be modified**; RSS RETA **cannot be configured**.

Traffic Management (TM) — hierarchical scheduling via the generic DPDK TM API:
- TM is a tree: one root (non-leaf) node, then leaf nodes — leaf count can't exceed the
  number of configured TX queues. Build top-down (root first, then leaves), then commit.
- Node settings: **weight** (egress scheduler) and a **private shaper** (egress rate limiter).
- Supported capabilities: Level0 (root)/Level1/Level2 hierarchy, 1 private shaper at the root
  (port level), 8 TX queues per port (1 channel per port), both Strict Priority (SP) and WFQ
  scheduling on all 8 queues. Query node/level capabilities via testpmd commands.
- No taildrop/WRED — once configured, the driver won't enqueue past what the shaper/scheduler
  allows.
- *Note*: the source doc's full testpmd walkthrough (per-queue shaper/WFQ setup, flow creation
  by source IP, traffic injection) was truncated in this scrape after the capability list —
  see "Traffic Management" in DPDK Testpmd Runtime Functions docs, or re-scrape
  `guides/nics/dpaa2.html`, for the exact command sequence.

### 2. `dpaa2_sec` Crypto PMD (DPAA2_SEC / DPSECI)

Backed by the SEC/CAAM hardware accelerator; session-oriented only (see limitations).

Supported algorithms:
```
Cipher:  NULL, 3DES_CBC, AES128_CBC, AES192_CBC, AES256_CBC,
         AES128_CTR, AES192_CTR, AES256_CTR
Auth:    SHA1_HMAC, SHA224_HMAC, SHA256_HMAC, SHA384_HMAC, SHA512_HMAC,
         MD5_HMAC, AES_XCBC_MAC, AES_CMAC
AEAD:    AES_GCM
```

Device args and logging:
```
fslmc:dpseci.1,drv_dump_mode=1     # dump HW error info on SEC error (0=off,1=code,2=+session/queue/descriptor debug info)
fslmc:dpseci.1,drv_strict_order=1  # strict ordering for ordered-schedule event type (default is loose ordering)
--log-level=crypto.dpaa2,<level>   # Crypto PMD logs
```

Limitations:
- **Hash-then-cipher chaining is not supported.**
- **Only the session-oriented API is supported** — session-less crypto ops don't work.
- Default byte ordering is little-endian (configurable via software).

### 3. Eventdev driver (`event_dpaa2`)

A vdev built from DPCON + DPCI devices (hardware-based event scheduler), probed at EAL init:
```
./your_eventdev_application --vdev="event_dpaa2"
```
Or programmatically: `rte_vdev_init("event_dpaa2")`.

Logging: `--log-level=eventdev.dpaa2,<level>`

Limitation: **only one eventport per core** (platform requirement: NXP DPAA2 SoC).

### 4. CMDIF rawdev driver (`dpaa2_dpci`)

A vdev for GPP↔AIOP (firmware) communication over DPCI devices — get the DPCI object ID via
attributes, then do I/O to/from the AIOP device:
```
./your_cmdif_application <EAL args> --log-level=pmd.raw.dpaa2.cmdif,<level>
```
Or programmatically: `rte_vdev_init("dpaa2_dpci")`.

### 5. QDMA dmadev driver (DPDMAI-backed)

CPU-offloaded DMA — the CPU initiates the transfer but isn't involved while it runs; status
can optionally be polled per-operation.
```
./your_qdma_application <EAL args> --log-level=pmd.dma.dpaa2.qdma,<level>
```
Device args:
```
fslmc:dpdmai.1,fle_pre_populate=1  # pre-populate DMA descriptors with pre-initialized values
fslmc:dpdmai.1,desc_debug=1        # enable descriptor debug prints
fslmc:dpdmai.1,short_fd=1          # enable short frame descriptors
```
Lookup by name: `rte_dma_get_dev_id_by_name("dpdmai.x")` where `x` is the DPDMAI object ID
created by the MC.

## Common Cross-Driver Patterns

- **Blocking a device from a driver**: any DPAA2 object (DPNI, DPSECI, etc.) can be blocked
  using the resource-container object ID `x` — see `references/guides.md` for the exact
  allow/block flag syntax used per driver page.
- **Multi-process debugging**: DPAA2 hardware limits shared access to Management Control
  Ports and HW portals, which breaks naively in multi-process setups. The driver reserves an
  *extra* Management Control Port and HW portal specifically so debug tools like
  `dpdk-procinfo` can inspect a running primary process without needing to block its devices.
  Use allowlist/blocklist on primary vs. secondary processes to manage the rest.
- **Prerequisites (all 5 drivers)**: see the NXP QorIQ DPAA2 Board Support Package for board
  setup, and the DPDK Getting Started Guide for Linux for the base DPDK environment. All
  drivers only run on NXP DPAA2 SoCs — check each page's "Supported DPAA2 SoCs" subsection.
  DPAA2_SEC additionally requires external dependencies not shipped with DPDK (see its
  Prerequisites subsection in `references/guides.md`).

## Reference Files

This skill includes one comprehensive reference file, present at two identical paths because
the scraper filed it under both the `documentation` and `unknown` source buckets (confirmed
byte-identical — see **Source Overview**):

- **`references/guides.md`** (== `references/documentation/dpaa2-dpdk_docs/guides.md`; source:
  official docs, confidence: high, 5 pages, ~23K chars) — full text of all 5 DPAA2 driver guide
  pages from doc.dpdk.org:
  1. *DPAA2 Poll Mode Driver* (`net/dpaa2`) — architecture overview, object model, TM/QoS.
  2. *NXP DPAA2 CAAM (DPAA2_SEC)* — crypto PMD, supported algorithms, limitations.
  3. *NXP DPAA2 Eventdev Driver* — `event_dpaa2` vdev, scheduling features.
  4. *NXP DPAA2 CMDIF Driver* — `dpaa2_dpci` rawdev, GPP↔AIOP I/O.
  5. *NXP DPAA2 QDMA Driver* — DPDMAI dmadev, CPU-offloaded DMA.
- **`references/documentation/index.md`** and **`references/documentation/dpaa2-dpdk_docs/index.md`**
  — small manifest files pointing back at `guides.md` and its source URL
  (`https://doc.dpdk.org/guides/platform/dpaa2.html`); no additional content beyond what's
  synthesized above.

Use `view` to open `references/guides.md` when you need full prose (e.g. the complete object
descriptions, TM hierarchy rules, or the full crypto algorithm list) beyond what's summarized
above. The Traffic Management testpmd walkthrough at the end of the NIC PMD section was
truncated in this cache (see the TM note under **Driver-by-Driver Quick Reference § 1**) — a
fresh scrape or the live `guides/nics/dpaa2.html` page has the missing command sequence.

## Working with This Skill

### Beginners: Start Here
Read **Key Concepts** first — every DPAA2 driver (NIC, crypto, eventdev, rawdev, dmadev)
builds on the same DPRC/MC/DPIO object model, so it pays off across all five. The ASCII
diagram and resource-summary table give you the shape of a typical container before you touch
any driver-specific syntax.

### Intermediate: Working a Specific Driver
Jump to the matching subsection under **Driver-by-Driver Quick Reference** for dev-args,
vdev-init syntax, and log-level strings — they're extracted verbatim from the docs. Cross-check
**Common Cross-Driver Patterns** for allow/block syntax and multi-process portal behavior,
since those apply regardless of which driver you're debugging.

### Advanced: Full Prose and Edge Cases
Open `references/guides.md` and search for the driver's page title (e.g. "NXP DPAA2 CAAM") for
full feature lists, prerequisite details, and limitations not condensed above. If you're
chasing a TM/QoS testpmd command sequence, note the truncation caveat above before assuming the
docs are complete.

### Conflict Resolution
Not applicable today — as explained in **Source Overview**, the two "source types" the scraper
detected resolve to one identical file, so there is no real second viewpoint to reconcile. If
this skill is regenerated with a genuine second source (codebase scan of
`drivers/net/dpaa2/`, `drivers/crypto/dpaa2_sec/`, etc., or DPDK mailing-list/GitHub issues),
apply this priority order:
1. **Code patterns (codebase_analysis)** — ground truth for what the driver actually does.
2. **Official documentation** (this file's current content) — intended API and usage.
3. **GitHub issues** — real-world usage and known problems.
4. **PDF documentation** — additional context and tutorials.

## Notes

- This skill was automatically generated from official documentation (doc.dpdk.org, DPDK
  26.07.0-rc2).
- Reference files preserve the structure and examples from the source docs.
- Code examples include language detection for better syntax highlighting.
- Quick reference entries are filtered to avoid low-signal placeholders and inline tokens.
- All DPAA2 drivers described here only function on NXP DPAA2 SoCs — check the "Supported
  DPAA2 SoCs" subsection in `references/guides.md` for the current list before targeting a board.

## Updating

To refresh this skill with updated documentation:
1. Re-run the scraper with the same configuration.
2. The skill will be rebuilt with the latest information from doc.dpdk.org.
