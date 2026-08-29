# dpni baseline

<!-- Instantiated from _template.md; every section mandatory, empty sections
     state so explicitly (spec: object-baseline). -->

Claim markers, used per claim throughout: **[read]** = derived from source or
manual, not yet observed; **[verified]** = observed on the board against the
pinned reference environment (see `reference-environment.md`).

Findings are written to be provable: behavioral claims name their
observables and are distilled into the Invariant candidates section as
propositions a Quint model can carry — invariant-bearing or
invariant-breaking.

Cross-family relationships are mapped in [object-model.md](object-model.md);
the board scenarios that will settle this document's open items are
classified in [traffic-inventory.md](traffic-inventory.md).

The family's model parameters live in [models/families/dpni.qnt](../../models/families/dpni.qnt); every invariant candidate below has a disposition row in [models/COVERAGE.md](../../models/COVERAGE.md), and the board suites that settled them are indexed in [models/board/README.md](../../models/board/README.md).

The DPNI (Data Path Network Interface) is the network-interface object: a
consumer (kernel `fsl_dpaa2_eth`, DPDK dpaa2 PMD) binds it to get an
ingress/egress datapath, and it connects to a DPMAC (physical port), another
DPNI (point-to-point pair), a dpsw/dpdmux port, or itself (loopback). It is
the largest object family and the anchor of ADR-0001's two hardware
corrections: C1 (a bare DPNI fails at kernel probe — the driver *allocates*
dpbp/dpmcp/dpcon it does not *create*) and C2 (the netdev MAC is inherited
DPMAC → DPNI at connect) [verified, ADR-0001].

## Command surface

restool v2.4 exposes 5 subcommands (`dpni_commands.c:1345-1367`); this is a
deliberately thin slice of the ~90-function MC flib surface [read]:

| Command | MC interaction | Notes |
|---|---|---|
| `--help` | none | the literal token is `--help`; `restool dpni help` does **not** exist despite the top-level usage advertising it (exact-match dispatch, `restool.c:1088-1093`) |
| `info <dpni.N> [--verbose]` | `dpni_open`, `dpni_get_attributes`, `dpni_get_api_version`, `dpni_get_link_state`, `dpni_get_primary_mac_addr` (skipped under `DPNI_OPT_NO_MAC_FILTER`), `dpni_get_max_frame_length`, `dpni_get_statistics` ×7 pages, `dprc_get_connection` | `--verbose` adds irq mask/status |
| `create [options] [--container=<dprc.N>]` | `dpni_get_api_version` then `dpni_create` with a **runtime-selected command version**: `CREATE_V8` if DPNI API ≥ 8.6 else `CREATE_V7` if ≥ 8.3 (`dpni_commands.c:863-873`, `common/utils.h:95-110`) | board DPNI API is 8.5 → V7 is what our restool emits [read] |
| `destroy <dpni.N>` | `dpni_destroy` | refuses if a Linux driver is bound (`in_use()` reads `/sys/bus/fsl-mc/devices/<obj>/driver`, `restool.c:573-601`) |
| `update <dpni.N> --mac-addr=<a>` | `dpni_set_primary_mac_addr` | the **only** attribute restool can mutate post-create |

Absent from restool (MC has the command, restool has no verb) [read]: link
config, max frame length, pools, tx shaping, MAC/VLAN filter tables,
offloads, statistics reset, object reset. Consumers reach those through
their own flib (kernel driver, DPDK PMD) — the Rust port must not assume
restool parity with the MC surface.

## Option inventory: used vs available

Create defaults are uniform: `dpni_cfg` is memset to zero and an omitted
flag sends literal 0 to the MC, which applies its own default
(`dpni_commands.c:888`; defaults documented at
`mc_release_10.39.0/include/dplib/fsl_dpni.h:172-235`) [read]. restool never
substitutes a non-zero default.

Live create options (`dpni_commands.c:109-293`), with users. "dprc-script"
is our `vpp-dpaa2-support/scripts/vpp-dpaa2-dprc.sh`, the board-validated
VPP provisioning path [verified]:

| Option | cfg field | restool range | MC default at 0 | Used by |
|---|---|---|---|---|
| `--options=<list>` | `options` | comma-separated names or raw mask | 0 = no options | ls-addni (always passes, default **empty**); dprc-script (two profiles, below) |
| `--num-queues` | `num_queues` | 1–32 | 1 | ls-addni (auto-injects `nproc` if absent, `ls-main:999-1004`); dprc-script (16) |
| `--num-tcs` | `num_tcs` | 1–16 | 1 | ls-addni (pass-through); dprc-script (16 VPP-side, 1 kernel-side) |
| `--mac-entries` / `--mac-filter-entries` | `mac_filter_entries` | 1–80 | **16** [board-observed 2026-08-25, V-READBACK-1] — 80 is restool's maximum, not the MC default | ls-addni (old spelling only) — the pair is mutually exclusive; new spelling exists "to align with the options passed through DPL" |
| `--vlan-entries` / `--vlan-filter-entries` | `vlan_filter_entries` | 1–16 | 0 = VLAN filtering disabled | ls-addni (old spelling); dprc-script (16) |
| `--qos-entries` | `qos_entries` | 1–64 | **0** with one TC [board-observed 2026-08-25, V-READBACK-1] — the QoS table exists only for a multi-TC DPNI (manual §7.3.63); 64 is restool's maximum | ls-addni; dprc-script (64) |
| `--fs-entries` | `fs_entries` | 1–1024 | 64 | ls-addni; dprc-script (1) |
| `--num-cgs` | `num_cgs` | 1–128 | one CG per TC | ls-addni (but shell caps at 8 — see silent-failure notes); dprc-script (queues+8) |
| `--dist-key-size` | `dist_key_size` | 1–56 | treated as 24 | ls-addni |
| `--num-channels` | `num_ceetm_ch` | 1–32 | single CEETM channel | ls-addni; dprc-script (1, gated on restool ≥ 2.4 — older restool rejects the flag) |
| `--num-opr` | `num_opr` | 1–128 | `num_tcs × num_queues` | ls-addni |
| `--container` | (dprc token) | — | root dprc | ls-addni (explicit only); dprc-script (child dprc) |

**Dead options** [read, `dpni_commands.c:125-199`]: 11 options sit in the
getopt table but are never consumed — create-time `--mac-addr` and the ten
v9-era `--max-*` flags (`--max-senders`, `--max-tcs`, `--max-dist-per-tc`,
`--max-fs-entries-per-tc`, `--max-unicast-filters`,
`--max-multicast-filters`, `--max-vlan-filters`, `--max-qos-entries`,
`--max-qos-key-size`, `--max-dist-key-size`). They are not marked
deprecated anywhere. Passing one is worse than an error: see silent-failure
notes. No script passes any of them [read].

**Never settable**: `dpni_cfg.num_rx_tcs` is wired into the MC command
(`mc_v10/dpni.c:136`) but no restool option writes it — always 0, so Rx TCs
= `num_tcs` (capped at 8 by MC). Only the DPL boot path can declare a
distinct value, and restool's own `generate-dpl` emits it, which is what
breaks the round-trip (silent-failure notes) [read].

`--options` parsing (`restool.c:1561-1611`) [read]: comma-separated only
(the help text's "or space separated" is not implemented); tokens matched
case-sensitively against a 15-entry map (`dpni_commands.c:349-365`); an
unrecognized token falls back to `strtoull(base 0)`, so a raw numeric mask
is silently accepted — this is how dprc-script passes `0x80000000`
(PFDR_IN_PEB, a value outside the named map) [verified in use].

Option profiles in production on this board [verified, dprc-script:94-117]:

- **PMD-facing dpni**: `DPNI_OPT_SINGLE_SENDER, DPNI_OPT_CUSTOM_CG,
  DPNI_OPT_HAS_KEY_MASKING, DPNI_OPT_HAS_OPR, DPNI_OPT_OPR_PER_TC,
  0x80000000` with 16 queues / 16 TCs.
- **kernel-facing dpni** (kdpni injection pairs): `DPNI_OPT_HAS_KEY_MASKING`
  only, 1 queue / 1 TC — the PMD profile's PFDR_IN_PEB and SINGLE_SENDER
  are deliberately absent because the kernel side must tx freely.

The restool map also knows `DPNI_OPT_HAS_REPLICATION` (0x4000), which does
**not** exist in the MC 10.39.0 flib header (14 flags, ending
`STASHING_DIS` 0x2000, `fsl_dpni.h:84-149`) — restool is ahead of the
pinned firmware's documented vocabulary here (unknown register) [read].

ls-debug touches dpni only as a glob passed to the dpdbg dumper
(`ls-debug:266,278`); ls-append-dpl is fully generic (DPL property names
mechanically become `--long-options`) and has no dpni-specific code [read].

**ls-listni** (`ls-main:1049-1096`) is the family's read-only consumer:
`dprc list --full-path`, then per container an unanchored
`dprc show | grep dpni` (labels containing "dpni" match too), then per
dpni a scrape of rendered `dpni info` text for label and endpoint — two
full info calls per dpni, each pulling all 7 statistics pages. The netdev
name is resolved from sysfs under the **root-container path only**
(`/sys/bus/fsl-mc/drivers/fsl_mc_dprc/<root>/<dpni>/net/`, retried up to
`PROBE_MAX_TRIES=10000`): a child-container or VFIO-bound dpni — every
VPP dpni on this board — spins the retry loop and then lists with no
interface name, indistinguishable from a probe failure [read].

## Attribute mutability

The create/runtime split is absolute and asymmetric [read,
`mc_release_10.39.0/include/dplib/fsl_dpni.h`]:

- **Every `dpni_cfg` field is create-time-immutable.** No `dpni_set_*`
  resizes `options`, `num_queues`, `num_tcs`, `num_rx_tcs`, table sizes,
  `num_cgs`, `num_opr`, `dist_key_size`, or `num_channels`. There is no
  `dpni_set_options`. Changing any of these means destroy + create.
- Runtime setters mutate only the **contents** of tables whose **sizes**
  were fixed at create (MAC/VLAN filters, QoS table, FS entries) plus
  behavior knobs with no cfg counterpart: buffer layout, offloads, link
  cfg, tx shaping, max frame length, promiscuity, tx priorities, rx
  distribution/policing, early drop, congestion notification, taildrop,
  OPR, tx confirmation mode, custom TPID, 1588, SP profile, MACsec (36
  `dpni_set_*` total in the 10.39 flib).
- restool exposes exactly one mutation: the primary MAC. The kernel driver
  exercises ~20 of the setters via netdev/ethtool ops (kernel-side
  section); the rest have no consumer in this corpus.

For the typestate design: the `dpni_cfg` block is the immutable type
parameter of a DPNI; the runtime surface is state within that type. Drift
on any cfg field is refuse-and-report, never repair (ADR-0001 §4).

Attribute read-back asymmetry [read]: `dpni_attr` returns `num_rx_tcs` +
`num_tx_tcs` (split), adds `qos_key_size`, `fs_key_size`, `wriop_version`
(0xC00 = WRIOP 3.0.0 = LX2160), but **omits `dist_key_size`** — that
create-time value can never be read back, so the reconciler cannot detect
drift on it and must treat it as write-only.

## MC API notes

Pinned-pair skew (restool built against MC 10.32.0 headers, board firmware
10.39.0): the dpni flib delta across the span is **additive except for one
wire-format change** [read, diff of
`mc-utils/api/mc_release_10.32.0` vs `mc_release_10.39.0`]:

- DPNI object API 8.2 → 8.5 (8.3 at MC 10.33, 8.4 at 10.34, 8.5 at 10.39).
- **`DPNI_CMDID_SET/GET_TX_CONFIRMATION_MODE` bumped to command version 2
  at MC 10.34**, and byte 0 of the payload changed from reserved pad to a
  live `ceetm_ch_idx` input (`fsl_dpni_cmd.h:137-138,813-819`). A
  10.32-built restool zeroes that byte (implicitly channel 0) but emits
  version 1; whether MC 10.39 retains a v1 handler is not determinable
  from the corpus — client-side flib only, no MC dispatch table (unknown
  register). Not currently reachable through restool (no verb), but the
  Rust southbound must emit v2 with an explicit channel index.
- New at 10.39: the full MACsec block (23 commands, `dpni_secy_*` +
  `dpni_is_macsec_capable` + `dpni_get_macsec_stats`) and
  `dpni_get_mac_statistics`. New at 10.33/10.34: `dpni_sp_enable`,
  per-queue tx confirmation mode. Nothing removed anywhere in the span.
- `DPNI_OPT_*` flags: byte-identical 14-flag list across the span — no
  create-surface drift.
- A new semantic contract on `dpni_set_tx_priorities` for >8-TC DPNIs
  (`fsl_dpni.h:1019-1030`): the first 8 TCs are locked to strict priority
  and only the {8-12 WEIGHTED_A, 12-16 WEIGHTED_B} grouping is accepted —
  "any other configuration will get rejected by the MC firmware." Our
  16-TC PMD dpnis are in scope of this rule [read].

Versioning is entirely static in the flib: command versions are baked into
the CMDID macros; nothing negotiates, nothing branches on
`dpni_get_api_version` (zero version checks in `dpni.c`, 10.39) [read].
The only dynamic selection in the whole path is restool's create V7/V8
gate. Version skew is therefore a *deployment* property, invisible at
runtime until the MC rejects (or misinterprets) a command — the Quint
model should carry the emitted command version as part of the action, not
assume a negotiated common dialect.

Contradiction worth a board check [read]: the `dpni_cfg` doc says
`num_queues` max is **8** (`fsl_dpni.h:211-212`), restool caps at **32**,
NXP's own LX2160A DPLs declare **16** — and 16 is what our board runs
[verified]. The documented limit is stale for WRIOP 3.0.0; the true
ceiling is unknown.

`dpni_set_mtu`/`dpni_get_mtu` are declared but have no implementation and
no CMDID — a permanent dangling pair in the flib (both endpoints of the
span). Frame length is `dpni_set_max_frame_length` [read].

DPL surface: only 7 `dpni_cfg` fields have DPL syntax in the entire
mc-utils config corpus (`options`, `num_queues`, `num_tcs`,
`mac_filter_entries`, `vlan_filter_entries`, `fs_entries`, `qos_entries`);
`num_rx_tcs`, `num_channels`, `num_cgs`, `num_opr`, `dist_key_size` appear
in no DPL. Every LX2 ethernet DPL uses `options = ""` [read].

## Kernel-side behavior (Linux 6.6.52)

The consumer that defines most observable dpni semantics is
`fsl_dpaa2_eth` (`drivers/net/ethernet/freescale/dpaa2/`) [read,
`.build/src/linux`].

**Bind resets the object.** `dpaa2_eth_probe` →
`dpaa2_eth_setup_dpni` calls `dpni_reset()` unconditionally
(`dpaa2-eth.c:3921`) before reading attributes. Any MC-side configuration
made before bind — a restool-set primary MAC included — is discarded by
the act of binding. The only pre-bind state that survives is the immutable
`dpni_cfg` block. This is the kernel-side mirror of the sync double-blind:
provision-then-plug sequences must set runtime state *after* the driver
binds, or through the driver's own netdev interfaces.

**Probe floor and pool draw** (ADR-0001 C1, now with exact expressions):

- Rejects DPNI API < 7.0 (`dpaa2-eth.c:3908-3914`); feature-gates on ≥7.3
  (split tx TCs), ≥7.9 (multi-FQ enqueue), ≥7.13 (pause), ≥8.2 (1588
  direct), ≥8.5 (MACsec).
- Draws from the container pool: **1 DPMCP** (as MC portal,
  `fsl_mc_portal_allocate`), **1 DPBP**, and **one DPCON per channel**
  where channels = `min(#online CPUs with an affine DPIO,
  dpni.num_queues)` (`dpaa2-eth.c:3268-3325`). DPIOs are not drawn per
  dpni — one affine DPIO per CPU is shared bus-wide.
- Pool exhaustion on the *first* object of a type → `-EPROBE_DEFER`
  (retried); exhaustion after the first DPCON → **"Not enough DPCONs,
  will go on as-is"** and probe succeeds degraded (silent-failure notes).
- No minimum queue count and no required/forbidden `DPNI_OPT_*`: 1-queue
  DPNIs bind fine (our kdpni pairs rely on this [verified]);
  `TX_FRM_RELEASE`, `HAS_POLICING`, `SHARED_CONGESTION` are never read by
  the driver at all.

**MAC precedence** (ADR-0001 C2, exact order,
`dpaa2-eth.c:4573-4636`) [read; chain board-confirmed 2026-07-05
[verified]]:

1. `dpni_get_port_mac_addr` non-zero (DPMAC-side, burned-in) → it wins
   **and is written back** as the DPNI primary MAC.
2. Both port and primary zero → random MAC, written back, and
   `addr_assign_type` forced to `NET_ADDR_PERM` so it does not *read* as
   random.
3. Port zero, primary non-zero → primary used as-is.

The same derivation re-runs on every `ENDPOINT_CHANGED` interrupt (connect
or disconnect while bound), with its errors discarded
(`dpaa2-eth.c:4839`). Connect/disconnect also toggles the dpmac takeover:
the eth driver detaches the standalone dpmac driver and drives the MAC
through phylink; disconnect hands it back.

**Runtime knob → MC command map** (what the netdev surface actually
exercises): `ip link up/down` → `dpni_enable/disable`; `ip link set
address` → `dpni_set_primary_mac_addr`; promisc/filter lists →
`dpni_set_*_promisc`/`dpni_add_mac_addr`/`dpni_clear_mac_filters`;
`ethtool -A` → `dpni_set_link_cfg` (the driver *requests* pause on at
probe when API ≥ 7.13, but the first link event overwrites that request
with the PHY's negotiated reality — on the wired pair the board came up
"flow control off"; and `ethtool -a` reads the driver's cached request,
which is what the last write asked for, not the firmware's state until the
next link interrupt [V-LINK-4 rev 1, 2026-08-29]); `ethtool -N/-U` →
`dpni_set_rx_hash_dist`/`dpni_add_fs_entry`; checksum toggles →
`dpni_set_offload`; `tc tbf` → `dpni_set_tx_shaping`; VLAN filter →
`dpni_enable_vlan_filter`/`dpni_add_vlan_id`. MTU changes do **not** reach
the MC unless an XDP program is attached — max frame length stays pinned
at 10240 and only `dev->mtu` moves (`dpaa2-eth.c:2775-2791`). No
driver-private sysfs and no ethtool priv flags; debugfs offers per-dpni
stats plus `reset_mc_stats` → `dpni_reset_statistics` [read].

**Queue mapping.** The kernel's effective queue count is `num_channels`
(DPCON-bound), *not* `dpni.num_queues`; RSS distribution size follows
`num_channels` too. Taildrop defaults programmed at probe: 1 MiB per-FQ
byte taildrop + per-TC frame-unit CG taildrop, retoggled on every link
state change against tx-pause (`dpaa2-eth.c:2158-2219`) [read].

**Unbind** (`dpaa2_eth_remove`): frees DPBP/DPCON back to the container
pool, then calls `dpni_reset()` — a *clean* unbind leaves the object in
initial state. But a reset failure is only `netdev_warn`ed and the object
is closed anyway, leaving the full driver config (pause, taildrops,
filters, pools binding) live for the next opener; the kernel re-binder is
immune (it resets first), a non-kernel consumer (VPP/DPDK via VFIO) is
not [read]. Object destroy while bound is refused by restool's `in_use`
check, but nothing in the kernel handles the MC-side object vanishing
under a bound driver — that race is unguarded [read].

`/dev/dprc.N` whitelists exactly five dpni commands for the root
container's ioctl path: set/get primary MAC (set requires
`CAP_NET_ADMIN`), get statistics, get link state, get max frame length
(`fsl-mc-uapi.c:60-64,253-283`); the full whitelist, with every
verb the series drives resolved against it, is
`docs/baseline/mc-ioctl-policy.md` (task 6.5) [read].

## Lifecycle ordering and dependencies

The board-validated creation order (ls-addni `ls-main:808-847` and
dprc-script agree; C1 [verified]):

1. Top up the container's allocatable pool **first**: DPIOs to core count
   (each with a companion DPMCP in ls-addni), 1 DPBP, 1 DPMCP,
   `min(num_queues, cores)` DPCONs — each created *and* plugged
   (`assign --plugged=1`).
2. `dpni create` (in the target container).
3. Optional `dpni update --mac-addr` (the only pre-plug mutation restool
   can make — and the kernel will overwrite it from the DPMAC at
   connect+bind anyway if the port MAC is non-zero).
4. `dprc assign --plugged=1` — this is what triggers the kernel probe and
   netdev creation.
5. `dprc connect` dpni ↔ endpoint, issued from a common ancestor. Works
   across containers: our kdpni pairs connect a child-container dpni to a
   parent-container dpni from the parent [verified]. Loopback = connect
   the dpni to itself (ls-addni `--loopback`) [read].
6. `dprc sync` (kernel bus rescan; see dprc.md for its limits).

Plug-then-connect and connect-then-plug both occur in the wild — ls-addni
plugs before connecting; the ENDPOINT_CHANGED interrupt exists precisely
so a bound driver can absorb a late connect [read].

Teardown (ls-delete): unbind driver → `dpni destroy` → recurse into
supplier dpbp/dpcon/dpmcp/dpio that have no other consumer → rescan. The
supplier set is discovered from the kernel device-link graph, which
exists only for kernel-bound consumers — a VFIO-owned dpni has no such
links, so the Rust port must carry the dependency graph itself
(dprc.md finding, reconfirmed here) [read].

## Intent mapping

The dpni is the object behind every interface construct of the intent
layer (ADR-0005): physical port (dpni↔dpmac), host-injection pair
(dpni↔dpni across containers), loopback test port (dpni↔self), and — per
this doc — a future dpsw/dpdmux port endpoint. Derivation rules the
compiler must carry, all evidence-anchored:

- `num_queues` ≥ the maximum rx/tx queues any consumer will ask for;
  VPP's tx floor is `main + workers` threads and cannot be rationed down
  (ADR-0012 [verified] — a shared tx ring silently drops enqueues).
- One DPCON per polled queue, DPIO = 2 per thread (bus-wide), DPBP per
  pool — the companion-object math lives with the pool families
  (`dpbp.md`/`dpio.md`/`dpcon.md`/`dpmcp.md`) but is *triggered* by dpni
  sizing.
- Option profiles are consumer-typed, not global: the PMD profile and
  kernel profile differ (option inventory above); an intent compiler
  choosing options must know which driver will bind [verified].
- `num_cgs = num_queues + 8` under `CUSTOM_CG` is the deployed heuristic
  [verified in use]; its rationale is unrecorded (unknown register).

## Silent-failure notes

restool and scripts:

- **Dead-option create leaks.** `restool dpni create --max-senders=8`
  creates the dpni, prints its id, *then* exits non-zero on the unconsumed
  option (`restool.c:1174-1178`). Under `set -e`, a wrapper aborts after
  the object exists — a leaked dpni whose id was on stdout [read].
- **`dpni update` returns 0 on three failure paths**
  (`dpni_commands.c:1294-1325`): malformed MAC (which then still calls
  `dpni_set_primary_mac_addr` with a partially-filled stack buffer —
  silent MAC corruption), `DPNI_OPT_NO_MAC_FILTER` refusal, and
  no-options-given. ls-addni's update call cannot detect any of them
  [read].
- **`ls-append-dpl` cannot round-trip restool's own `generate-dpl`
  output for dpni**: `num_rx_tcs` becomes `--num-rx-tcs`, which no create
  option accepts; the `options` string list word-splits instead of
  becoming comma-separated; properties valued `0` are silently dropped;
  no exit status is checked anywhere in that script [read].
- ls-addni shell-side ranges disagree with restool in both directions
  (`--num-cgs` capped at 8 vs restool's 128; `--dist-key-size` allowed to
  64 vs restool's 56; out-of-policy `--num-queues` silently rewritten to
  core count) [read].
- `restool dpni info` prints unsupported statistics pages as all-zero
  counters (error assigned, never checked, `dpni_commands.c:731-735`)
  [read]. The supported pages are exact: `ingress_all_frames` and
  `egress_all_frames` moved by precisely the frames sent through a
  kernel-bound dpni in both directions, so they are the board-side
  reachability oracle and no capture is needed [verified 2026-08-24,
  V-TRAF-0].

kernel driver — the ones that matter for a reconciler's observation
model:

- **Quiet channel shortfall.** A DPCON shortage after the first lets
  probe succeed with `num_channels < num_queues`; rx/tx flowids ≥
  `num_channels` are then **never bound to any DPCON** — MC queues with
  no destination, invisible from Linux (ethtool reports the reduced
  count; nothing reports the shortfall as an error)
  (`dpaa2-eth.c:3158-3352`) [read].
- **Half-committed MAC.** `ndo_set_mac_address` mutates the netdev
  address before the MC call; on MC failure the two permanently diverge
  — `ip link` shows a MAC the DPNI does not filter on
  (`dpaa2-eth.c:2428-2448`). Same class: every error in the
  ENDPOINT_CHANGED MAC re-derivation is discarded [read].
- **Unbind reset is best-effort.** `dpni_reset` failure at remove is a
  warning; MC state survives while Linux looks torn down [read].
- **Vendor MACsec deinit bug zeroes all netdev features**:
  `features &= !NETIF_F_HW_MACSEC` (logical not) on every
  dpmac disconnect (`dpaa2-eth-macsec.c:658`) — after a hot-unplug the
  interface silently loses csum/SG/TSO offloads [read].
- **In-tree ABI disagreement on `dpni_set_pools`**: the eth driver
  passes the DPBP *object id*, the AF_XDP path passes the *bpid*, for
  the same wire field (`dpaa2-eth.c:4473` vs `dpaa2-xsk.c:166`). Which
  is right is not determinable in-tree (unknown register) [read].
- Hash/classification programming failures at bind are logged and
  swallowed — probe succeeds with all traffic on one queue [read].

The shared pattern, feeding the loudness invariants: **exit status and
probe success are systematically weaker than converged state** for this
family. Every convergence claim must rest on read-back (`dpni info`,
attribute get) — never on the return code of the mutation.

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPNI-I1 | Every `dpni_cfg` field (options, queue/TC counts, table sizes, num_cgs, num_opr, dist_key_size, num_channels) is immutable across any post-create action sequence; no MC command mutates them | `dpni info` attribute block before/after every suite | candidate |
| DPNI-I2 | **Breaking:** the model must NOT assume MC-side runtime state set before kernel bind survives the bind — `fsl_dpaa2_eth` probe calls `dpni_reset()` unconditionally; only the `dpni_cfg` block survives | set primary MAC via restool → plug → read back after netdev appears: MAC re-derived per DPNI-I3, not preserved | falsified for the primary MAC 2026-08-29 (V-DPNI-3 rev 1): a second MAC set through restool while the dpni was unbound was carried by both the firmware and the new netdev after the rebind (read back at each step) — the probe did not reset it, because the driver keeps a non-zero firmware MAC and randomizes only a zero one (DPNI-I3). The law holds for other pre-bind state, not the primary MAC |
| DPNI-I3 | MAC inheritance: after connect+bind with a non-zero DPMAC port MAC, DPNI primary MAC = port MAC (driver writes it back); with both zero, a random MAC is written back and presented as permanent | `dpni info` primary MAC vs port MAC; netdev `addr_assign_type` | verified (ADR-0001 C2) |
| DPNI-I4 | Probe precondition (C1): kernel bind completes only if the container pool holds ≥1 dpbp, ≥1 dpmcp, ≥1 dpcon with an affine dpio; a bare dpni stays connected-but-unbound | probe outcome; `-ENXIO`/"No more resources" in dmesg; netdev absent | verified (ADR-0001 C1) |
| DPNI-I5 | **Breaking:** the model must NOT assume bind success ⇒ all provisioned queues serviced; DPCON shortage beyond the first degrades silently to `num_channels < num_queues`, leaving destination-less queues | `ethtool -l` count vs `dpni info` num_queues; dmesg "Not enough DPCONs" | candidate |
| DPNI-I6 | **Breaking:** the model must NOT assume nonzero exit ⇒ no side effect: create with a dead option creates the object and then fails; and exit 0 ⇒ success is false for `dpni update` (three 0-exit failure paths) | `dprc show` delta across a failed create; primary MAC read-back after a "successful" update | candidate |
| DPNI-I7 | Create-default determinism: an omitted create option ⇒ 0 on the wire ⇒ MC default (1 queue, 1 TC, 16 MAC entries, 0 QoS entries with a single TC, 64 FS, VLAN filtering off, one CG) | `dpni info` of a bare `dpni create` | verified 2026-08-29 (V-READBACK-1 rev 2, hook 10/10) — the corrected hook confirms the rev-1 read-back; the 80 MAC / 64 QoS this row first predicted were restool's maxima, and the DPL-born management dpni in the clean-boot reference reads the same 16/0 |
| DPNI-I8 | Clean-unbind postcondition: successful driver remove resets the object to initial state; but reset failure is non-fatal, so unbind ⇒ reset is best-effort — convergence is established only by read-back | `dpni info` after unbind: default attributes, zero filter tables | falsified for the primary MAC 2026-08-29 (V-DPNI-3 rev 1): a MAC set from the netdev survived the kernel unbind, read back present in `dpni info` while unbound — the remove-path reset does not clear the primary MAC, so the clean-unbind reset is not even best-effort on it. Max frame length read 1536 while unbound |
| DPNI-I9 | Endpoint cardinality: a dpni has at most one connection; connect requires both endpoints currently disconnected and a common-ancestor initiator (dprc.md DPRC-I5), including the cross-container dpni↔dpni case | `dprc connect` exit; `GET_CONNECTION` per endpoint | verified (kdpni pairs in production use) |
| DPNI-I10 | Consumer tx-floor: any consumer driving tx from T threads needs T independent tx rings on the dpni; a ring shared by two threads silently drops enqueues (no MC error, no counter on the dpni side) | VPP `<if>-tx` drops with `num-tx-queues < T`; clean at `= T` | verified (ADR-0012) |
| DPNI-I11 | Version-skew emission: the southbound emits statically-versioned commands (no negotiation exists); the model carries the emitted command version per action, and `SET/GET_TX_CONFIRMATION_MODE` from a 10.32-built client emits v1 against a firmware registering v2 | command version bits on the wire; MC status on mismatch | candidate |
| DPNI-I12 | Write-only attribute: `dist_key_size` has no read-back (absent from `dpni_attr`), so the reconciler must not claim drift detection on it | `dpni info` field list | candidate |

## Unknown / unverified register

1. Does MC 10.39 firmware retain a v1 handler for
   `SET/GET_TX_CONFIRMATION_MODE` (and if so, does it read the
   `ceetm_ch_idx` byte)? Client-side corpus cannot answer; needs firmware
   release notes or a board probe. Highest-risk item of the 10.32→10.39
   skew.
2. True `num_queues` ceiling on WRIOP 3.0.0: doc says 8, restool caps 32,
   16 is deployed and working [verified]. Where between 16 and 32 does the
   MC refuse? Partially answered [board suite V-DPNI-2 rev 1,
   2026-08-29]: a bracketing walk created dpnis at `--num-queues` 32, 28,
   24 and 20 and the MC accepted every one — so if the MC caps the count
   at all, it caps at or above restool's own limit of 32; the walk found
   no refusal below the cap, and the MC-side ceiling stays unreachable
   through restool.
3. Semantics of `num_cgs`, `num_opr`, `dist_key_size` — present in
   `dpni_cfg`, absent from its doc block; and the rationale for our
   deployed `num_cgs = num_queues + 8`.
4. What exactly `dpni_reset` clears: the flib says "returns the object to
   initial state" with no per-field enumeration (pools binding? QoS/FS
   tables?). Narrowed [V-DPNI-3 rev 1, 2026-08-29]: **`dpni_reset` does
   not clear the primary MAC** — a MAC set from the netdev survived the
   kernel unbind (which calls `dpni_reset`) and a second MAC set through
   restool while unbound survived the rebind (which calls it again), so
   whatever "initial state" the reset restores, the primary MAC is not in
   it. Max frame length read 1536 while unbound. The QoS/FS-table half is
   still unread.
5. `dpni_set_pools.dpbp_id`: DPBP object id or BPID? The two in-tree
   kernel call sites disagree; the answer decides which one is latently
   broken.
6. Observable behavior of `DPNI_OPT_TX_FRM_RELEASE`, `HAS_POLICING`,
   `SHARED_CONGESTION` — no consumer in the corpus reads them.
7. `DPNI_OPT_SINGLE_SENDER` ("ignore num_queues for tx") vs our PMD
   profile, which sets it *and* drives `main+workers` tx rings
   successfully — what the flag actually gates on LX2160 is unclear.
8. `DPNI_OPT_HAS_REPLICATION` (restool knows 0x4000; the 10.39 flib
   header does not list it) — real MC option or restool running ahead?
9. What `DPNI_CMDID_CREATE_V8` (API ≥ 8.6) adds over V7 — identical
   payload in restool; and restool's create behavior against pre-8.3
   firmware (selector fall-through unresolved in source).
10. The meaning of raw option bit `0x80000000` (PFDR_IN_PEB per NXP's
    dynamic_dpl conventions) — deployed and working [verified], never
    named in any header in the corpus.
11. Whether the >8-TC `dpni_set_tx_priorities` constraint (strict-priority
    lock on TCs 0-7, fixed weighted grouping above) affects our 16-TC
    dpnis under the PMD's default scheduling — no consumer in the corpus
    calls `dpni_set_tx_priorities`.
12. `num_rx_tcs` reachable only via DPL: can a restool-created dpni ever
    have `num_rx_tcs ≠ min(num_tcs, 8)`?
