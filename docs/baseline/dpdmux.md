# dpdmux baseline

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

The family's model parameters live in [models/families/dpdmux.qnt](../../models/families/dpdmux.qnt); every invariant candidate below has a disposition row in [models/COVERAGE.md](../../models/COVERAGE.md), and the board suites that settled them are indexed in [models/board/README.md](../../models/board/README.md).

The DPDMUX is an L2 demultiplexer: one uplink shared across N downlink
interfaces, each connectable to a dpni (or another object) — the object
family ROADMAP phase 12 designates for **kernel/VPP port sharing** (one
dpmac feeding a kernel dpni and a VPP dpni). No such topology exists in
the corpus yet: our dprc-script creates no dpdmux, and VPP passes no
dpaa2 devargs, so the DPDK mux path is currently not VPP-visible
(`vpp-nxp-dpaa2-port-analysis.md:311`) [read]. This baseline is
forward-looking: it maps what the three consumers (restool/ls, kernel
`dpaa2-evb`, DPDK `dpaa2_mux`) can each actually drive — and they turn
out to want **different, mutually exclusive create methods**.

## Command surface

restool v2.4: 4 verbs (`dpdmux_commands.c:948-966`) — no
enable/disable/reset or any `if-*`/`set-*` verb; restool's vendored
dpdmux flib is a drastic subset (10 functions of the ~35 in the full
flib) [read]. `info` prints id, version, plugged state, a per-interface
block (connection, link state, max frame length, 12 counters), raw +
decoded options, method, manip, num_ifs, default_if, mem_size, label;
`max_dmat_entries`/`max_mc_groups`/`max_vlan_ids` only when the
negotiated cmd version ≥ V3, `custom_key_size` only ≥ V4 and
method==CUSTOM [read]. Version negotiation picks CREATE_V5 at dpdmux
API 6.11, CREATE (V4) at 6.9 — with silent fallback to the last table
entry, not an error [read].

Two `assert(false)` abort paths ship in release builds (no `-DNDEBUG`):
`info` on a dpdmux with method `S_VLAN` (0x4) or manip
`ADD_REMOVE_S_VLAN` — both creatable via DPL — kills restool with
SIGABRT (`dpdmux_commands.c:353-355,366-368`) [read].

## Option inventory: used vs available

| Option | Values | restool default | ls-addmux passes |
|---|---|---|---|
| `--num-ifs` | 1–65535, required | — | user's `-i` |
| `--method` | NONE, C_VLAN_MAC, MAC, C_VLAN, CUSTOM | C_VLAN_MAC | **MAC** (differs from restool's own default) |
| `--manip` | only `DPDMUX_MANIP_NONE` parseable (TODO in source) | NONE | NONE |
| `--options` | BRIDGE_EN, CLS_MASK_SUPPORT, AUTO_MAX_FRAME_LEN | none | BRIDGE_EN (VEB); `-v/--vepa` clears it |
| `--default-if` | 0–num_ifs | 0 | 0 |
| `--max-dmat-entries` | 1–65535 | 0 sent (MC default 64) | 64 |
| `--max-mc-groups` | 1–65535 | 0 sent (MC default 32) | 32 |
| `--mem-size` | 0–65535 (256 B units) | 0 | 0 |
| `--custom-key-size` | 0–56; **rejected `-EOPNOTSUPP` unless create cmd ≥ V5 (API 6.11 = MC 10.40+)** | absent | **0, unconditionally** |

Dead capabilities: `adv.max_vlan_ids` is in `dpdmux_cfg` and marshalled
but has no CLI option (always 0 → MC default 16); method `S_VLAN`
exists in the enum but is neither parseable nor printable [read].

**ls-addmux is dead on arrival against the pinned MC**: it always
passes `--custom-key-size=0` (ls-main:363,407), restool sets the option
mask on presence not value, and the V5 gate fires — so `ls-addmux`
fails with `-EOPNOTSUPP` on every MC from 10.32 through 10.39. The
vendor recipe for this family only works on MC ≥ 10.40 [read]. Its
other defects, for completeness: the companion dpmcp is created in the
**root** container even when `-c` targets a child (ls-main:345 vs :361
— the sole outlier; addsw/addni use `$container`, see `dpmcp.md`);
`object_exists` greps unanchored so `dpdmux.1` matches `dpdmux.10` and
a *successful* create prints "EVB creation failed!"; the recipe order
is create → connect uplink endpoint → `dprc assign --plugged=1` →
set-label, with connect/assign/set-label unchecked beyond `set -e`
[read]. `ls-append-dpl` has no dpdmux-specific handling: its generic
DTB→CLI mapping silently drops any property whose value is "0" and
fails the create on any property with no restool option (e.g.
`max_vlan_ids`) [read].

## Attribute mutability

Everything in `dpdmux_cfg` is create-time-immutable — method, manip,
num_ifs, options, max_dmat_entries, max_mc_groups, max_vlan_ids,
mem_size — with exactly one exception: **`default_if` has a runtime
setter pair** (`dpdmux_if_set_default`/`get_default`) [read]. The full
flib's runtime surface (all unreachable from restool):
enable/disable/is_enabled, reset, `set_resetable`/`get_resetable`,
set/get_max_frame_length, per-interface enable/disable, accepted
frames, L2 rules add/remove, link cfg/state, taildrop, errors
behavior, custom key + classification entries, counters,
ul_reset_counters, dump_table, IRQs [read].

`set_resetable` makes **reset persistence itself a stored policy**:
skip flags (MODIFY_DEFAULT_INTERFACE 0x01, UNICAST_RULES 0x02,
MULTICAST_RULES 0x04, RESET_DEFAULT_INTERFACE 0x08 from 10.37) decide
what a later `dpdmux_reset` preserves [read].

## MC API notes

10.32.0 → 10.39.0 is **additive, no wire break**: no CMDID version
byte changes in the range; object version 6.9 → 6.10 (bump at 10.37);
two new soft-parser commands (`SET_SP_PROFILE` 0x0c0, `SP_ENABLE`
0x0c1); skip-flag rename `SKIP_DEFAULT_INTERFACE` →
`SKIP_MODIFY_DEFAULT_INTERFACE` (same value) plus the new 0x08 flag
widening the resetable field 3 → 4 bits [read, mc-utils diff]. The
wire break sits **just past the pin at 10.40**: CREATE → V5, GET_ATTR
→ V4, version 6.11, `custom_key_size` added to the create payload.
restool v2.4 already marshals the 10.40 struct layouts (custom_key_size
byte where 10.32–10.39 declare pad) while emitting V4/V3 cmdids —
benign today because it sends 0 and gates reads, but the pairing the
Makefile advertises (`MC_VERSION_COMPATIBLE='10.32.0'`) is untested by
construction [read].

The vendor firmware changelog (`qoriq-mc-binary/CHANGELOG.md`) is a
second oracle the flib diff cannot see: firmware **behavior** moved
inside the "additive" window. 10.36 added `default_if` support for
methods MAC/C_VLAN/C_VLAN_MAC and deferred replicator membership to
linkup; 10.37 **imposed connection restrictions** (uplink connectable
only to a dpmac) that earlier firmware never enforced — a topology
10.32-era firmware accepted can be refused by the pinned 10.39;
10.38.0 fixed an MC **hang** on unsupported `dpdmux_create`
configurations; 10.38.1 fixed `SKIP_RESET_DEFAULT_INTERFACE` to
actually spare the uplink — the exact DPDK close-without-link-flap
case the skip flags exist for. The 10.40 entry independently
corroborates the create gate ("Add a new dpdmux_create() parameter —
custom_key_size") and reveals that ≤ 10.39 firmware caps
classification at a **64-entry software limit** regardless of
`max_dmat_entries` — the hardware limits (4096 exact-match / 1024
TCAM) apply only from 10.40 [read, MC changelog].

## Kernel-side behavior (Linux 6.6.52)

The binding driver is `dpaa2-evb` — an NXP **staging prototype**
(`drivers/staging/fsl-dpaa2/evb/`, "Prototype driver" in Kconfig),
enabled in the built config (`CONFIG_FSL_DPAA2_EVB=y`, via Kconfig
`default y`, not our config fragments) [read]. Probe: allocate an
**atomic dpmcp portal** (1 pool draw; `-ENXIO` → silent
`-EPROBE_DEFER`, the family pattern) → open → get_attributes → API
gate ≥ 6.0 → **`dpdmux_reset()`** — the dpni-class bind-resets-object
behavior: any pre-bind MC-side rules are wiped unless skip-reset flags
survive (unknown register) → register `num_ifs + 1` netdevs: `evb%d`
uplink (index 0, IFF_MASTER|IFF_PROMISC) + `evb%dp%d` ports
(IFF_SLAVE), all dev_open()ed, link-state IRQs wired [read].

The driver is **method-gated**: FDB ndo's require MAC or C_VLAN_MAC;
VLAN ndo's require C_VLAN or S_VLAN; `DPDMUX_METHOD_CUSTOM` is not in
the kernel's enum at all — a CUSTOM dpdmux binds evb and then every
FDB/VLAN operation is rejected [read]. Probe error paths carry real
bugs: a double `fsl_mc_portal_free` + use-after-free
(`evb.c:1328-1333` falls through after `evb_remove`), an
`alloc_etherdev` failure that leaves `err` unset so **probe can return
0 after tearing down**, and a leaked netdev on portal-allocation
failure [read].

Bus-level, independent of evb: dpdmux is a first-class fsl-mc bus type;
the uapi allowlist admits only `GET_COUNTER` and
`IF_GET_MAX_FRAME_LENGTH` through `/dev/dprc.N` [read].

DPDK side (the VPP-relevant claim path): dpdmux is a **bus object, not
an ethdev** — claimed via VFIO in whatever DPRC EAL scans, configured
through the shared EAL MC portal with **zero pool draws** [read]. At
probe it sets `default_if` and, on API ≥ 6.6, `set_resetable` skip
flags so later resets don't wipe its rules; on API ≥ 6.9 it sets
errors-behavior CONTINUE on **hardcoded interface 0** (the uplink is
assumed to be if 0) [read]. `rte_pmd_dpaa2_mux_flow_create` requires
**method CUSTOM** (drives `set_custom_key` + `add_custom_cls_entry`)
and holds a process-global static key layout: one extract set per
process, across all dpdmuxes, and the TCAM entry index never resets;
`flow_destroy` is declared in the header but **not implemented** (link
error); an out-of-range `dest_if` returns success without programming
anything [read]. A dpni whose endpoint is a dpdmux is recognized
(`ep_dev_type = DPAA2_MUX`), so DPDK dpnis know they hang off a mux
[read].

## Lifecycle ordering and dependencies

dpdmux is created (never pool-allocated). Order (ls recipe): create →
`dprc connect` uplink ↔ endpoint (dpmac) → assign `--plugged=1` →
label [read]. A kernel-bound dpdmux draws **one dpmcp** from its
container's pool at probe (already in `dpmcp.md`'s census); a
DPDK-claimed one draws nothing [read]. Downlink dpnis connect to
`dpdmux.N.M` interface endpoints; the kernel/VPP sharing topology
additionally splits containers — dpdmux in one container, consumer
dpnis in each regime's container — which is exactly the cross-dprc
link machinery of ROADMAP phase 9, hence the phase ordering (9 → 12)
[read]. Teardown: restool `destroy` checks only root-sysfs driver
binding (`in_use`), so child-container or VFIO-bound dpdmuxes pass the
precondition; no endpoint/connection check exists [read].

## Intent mapping

The target construct is **shared-uplink**: one dpmac, N consumers. The
derivation is not just sizing (`num_ifs` = consumer count, uplink = if
0) — the create-time `method` is a **regime-compatibility law**:

- kernel evb can operate methods NONE/MAC/C_VLAN/C_VLAN_MAC (+S_VLAN
  nominally), not CUSTOM;
- DPDK's flow API can operate only CUSTOM (its L2-rule helper covers
  MAC-family methods, but rule *removal* doesn't exist in this DPDK);
- restool can create any method but operate none.

No single method gives both regimes full function [read]. The intent
compiler must therefore pick the method from *who steers the demux at
runtime*, and the model must reject topologies whose steering owner
can't drive the chosen method. dpsw (phase 11) precedes dpdmux in the
roadmap; the corpus contains no argued comparison between them for
mac-sharing — only the implied asymmetry that dpsw has a maintained
upstream kernel driver while dpdmux has a staging prototype [read].

## Silent-failure notes

- `dpdmux destroy` in a child container **overwrites the destroy error
  with the dprc_close result**: prints an MC error but exits 0 while
  the object survives (`dpdmux_commands.c:897-901`; the guarded
  pattern exists 400 lines up) [read].
- ls-addmux prints "EVB creation failed!" on success (unanchored grep)
  and fails outright on the pinned MC (custom-key-size gate) — the
  recipe's exit status is wrong in both directions [read].
- `info` refuses to decode the whole options mask when any unknown bit
  is set; and it aborts (SIGABRT) on S_VLAN method / unknown manip
  [read].
- Per-interface counters: MC read errors are discarded and the last
  (or zero) value is printed with exit 0; interface 0's stats are
  suppressed entirely when the uplink is unconnected [read].
- evb probe can return 0 after tearing itself down (unset `err`), and
  a portal-exhaustion defer is silent — a dpdmux that looks scanned
  may have no working driver behind it [read].
- DPDK `flow_create` with an out-of-range `dest_if` returns success
  with nothing programmed; a second key layout in the same process
  fails `-ENOTSUP` ("Single flow support only") even on a different
  dpdmux [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPDMUX-I1 | Method is a regime selector: operability(method, consumer) is a fixed partial matrix — kernel ∈ {NONE,MAC,C_VLAN,C_VLAN_MAC,S_VLAN}, DPDK-flow ∈ {CUSTOM}, restool ∈ ∅; the model must reject a topology whose runtime steering owner cannot operate the created method | evb ndo return codes; DPDK flow_create errno; method from `info` | candidate |
| DPDMUX-I2 | **Breaking:** the model must NOT assume create/destroy capability implies manageability — restool creates all methods but can operate none of the runtime surface; convergence on rules/link state is unreachable through restool alone | restool verb list vs flib surface | candidate |
| DPDMUX-I3 | Bind-resets-object (dpni-class): kernel probe issues `dpdmux_reset`; pre-bind MC-side rules survive only per the stored resetable mask — reset persistence is itself mutable state the model must carry | rule presence across evb bind; `get_resetable` | candidate |
| DPDMUX-I4 | `default_if` is the sole mutable create attribute; all other `dpdmux_cfg` fields are immutable (drift = refuse, never repair) | set_default acceptance vs absence of any other setter | candidate |
| DPDMUX-I5 | Resource law: kernel-bound dpdmux draws exactly 1 dpmcp (defer on exhaustion); DPDK-claimed draws zero pool objects (shared EAL portal) | dpmcp free-count delta at bind; DPDK probe with empty pools | verified 2026-08-23 (V-DPDMUX-1 rev 2) for the kernel face: `dpaa2_evb` bound with a single created dpmcp as its only companion; the DPDK-claimed face is unprobed |
| DPDMUX-I6 | **Breaking:** the model must NOT treat the vendor recipe as an oracle for this family — ls-addmux cannot succeed on MC 10.32–10.39 (unconditional `--custom-key-size` vs the API-6.11 gate), and its dpmcp lands in the wrong container when targeting a child | ls-addmux exit on pinned MC; dpmcp parent container | candidate |
| DPDMUX-I7 | Exit-code truth gap (family instance): `destroy` exits 0 on failure in child containers; ls-addmux's message inverts the outcome — convergence only by read-back | object existence after "successful" destroy | candidate |
| DPDMUX-I8 | Uplink identity: interface 0 is the dpmac-facing side by convention hardcoded in DPDK (errors-behavior, counter dump); the model should pin uplink=0 until MC evidence shows otherwise | `dprc show` endpoint of dpdmux.N.0 | candidate |

## Unknown / unverified register

1. Whether the board's DPC/DPL contains any dpdmux at all, and whether
   MC 10.39 accepts a V4 CREATE whose (restool-V5-layout) pad bytes it
   reads as pad — the advertised-compatible pairing is untested.
2. Whether evb's probe-time `dpdmux_reset` honors skip-reset flags set
   earlier by another owner (cross-regime interference for the sharing
   topology — decides how DPDMUX-I3 composes with DPDK's
   `set_resetable`).
3. MC behavior for method `S_VLAN` (creatable via DPL, unparseable and
   SIGABRT-printing in restool) and for manip `ADD_REMOVE_S_VLAN`
   (parser TODO says no MC support — refused or accepted?).
4. `mem_size` semantics (256-byte units of what memory?) and the
   observable effect of undersizing it.
5. Soft-parser profile commands (10.37+): no in-corpus caller; are
   they usable on the pinned MC and do they interact with method
   CUSTOM's key?
6. ~~Whether uplink = interface 0 is an MC contract or only
   convention (DPDMUX-I8's footing).~~ **Answered, final** — board suite
   V-DPDMUX-2 rev 1–5, 2026-08-29: interface 0 is how the MC reads a bare
   object name in `dprc connect`; the dpmac-only uplink rule the 10.37
   changelog describes is **not enforced at connect** on the pinned
   10.39. The MC accepts a dpni on the uplink (interface 0, rev 2) and,
   from a fresh boot, on the downlink (interface 1, rev 5, with the
   read-back agreeing — `dpdmux info` interface 1 and `dpni info`
   endpoint both name the pairing), and then refuses the *disconnect*
   from every end: Configuration error (0x6) from the dpni end and from
   the demux downlink end, No resources (0x8) from the bare-name uplink
   end. So a dpni on either interface is un-disconnectable, and the
   pairing cannot be undone short of destroying an object. No pairing
   survives a reboot or a destroy-and-recreate (rev 5). The model keeps
   its own `legalPorts` guard ahead of the firmware for exactly this
   reason (ADR-0009), and treats any dpdmux↔dpni edge as destroy-only.
   Two controls stay unissued and deferred to `dpdmux-typestate` (#12):
   whether a dpmac uplink peer disconnects cleanly, and whether the
   downlink accepts once the uplink is already populated.
7. Real rule capacity: `max_dmat_entries` default 64/if vs the DPDK
   8-entry `rule[]` (declared, unused) vs TCAM depth for CUSTOM
   entries — no corpus source states the CUSTOM-entry limit.
