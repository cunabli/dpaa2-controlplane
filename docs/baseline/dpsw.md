# dpsw baseline

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

The DPSW is the multi-port L2 switch. Ownership is asymmetric to
dpdmux's: the kernel has a real, maintained upstream driver
(`dpaa2-switch`), while **DPDK has no claim path at all** — dpsw is not
in the fslmc scan allowlist, so a dpsw in a DPDK-scanned container is
silently discarded (debug-level log) and `dpaa2:dpsw.N` devargs are
`-EINVAL`; the qoriq fork only *labels* dpsw peers of its dpnis, it
never binds one [read]. NXP's own documented sharing model
(`README_DPSW` in the qoriq DPDK) is therefore **kernel-owned dpsw
with dpmac and DPDK-owned dpni endpoints** — the mirror image of
dpdmux's DPDK-steered story, and the reason ROADMAP sequences dpsw
(phase 11) before dpdmux (phase 12) [read]. No dpsw exists in our
corpus topologies today; vpp-dpaa2-support has zero mentions [read].

## Command surface

restool v2.4: **5 verbs** — `info`, `create`, `destroy`, plus the
family-unusual `update` (taildrop only) (`dpsw_commands.c:1152-1174`).
The vendored flib is a 12-function stub of the full ~91-function
surface [read]. Version negotiation carries only object version 8; on
an MC 9 device every dpsw verb dies in command-version lookup [read].
`info` prints attributes, decoded options, and a per-interface block
(endpoint, link state, taildrop, max frame length, 13 counters);
`max_meters_per_if` is fetched and never printed. An endpoint of
unexpected type with `if_id != 0` prints **no `connection:` line at
all** — and ls-main's `has_endpoint` parses exactly that line, so a
linked port can read as free [read]. One `assert` abort path
(`dpsw_id == attr.id`, no `-DNDEBUG`) [read].

## Option inventory: used vs available

| Option | Values | restool sends | ls-addsw passes |
|---|---|---|---|
| `--num-ifs` | 1–65535 | **default 4** (the only real client-side default) | endpoint count, or 4 |
| `--options` | 7 flags: FLOODING_DIS, MULTICAST_DIS, CTRL_IF_DIS, FLOODING_METERING_DIS, METERING_EN, BP_PER_IF, LAG_DIS | 0 | "" |
| `--max-vlans` | 1–65535 | 0 (MC default) | 16 |
| `--max-fdbs` | 1–255 | 0 | num_ifs |
| `--max-fdb-entries` | 1–65535 | 0 | 1024 |
| `--fdb-aging-time` | 1–65535 | 0 | 300 |
| `--max-fdb-mc-groups` | 1–65535 | 0 | 32 |
| `--mem-size` | 0–65535 | untouched | 0 |
| `--component-type` | C_VLAN \| S_VLAN | 0 (C_VLAN) | C_VLAN |
| `--flooding-cfg` | PER_VLAN \| PER_FDB | 0 (PER_VLAN) | **PER_VLAN** |
| `--broadcast-cfg` | PER_OBJECT \| PER_FDB | 0 (PER_OBJECT) | **PER_OBJECT** |

restool's help text lies about defaults (advertises 16/16/1024/300/32;
the code sends 0 and MC applies its own defaults) [read]. Dead flib
field: `adv.max_meters_per_if` — never settable, never printed [read].

**ls-addsw produces a kernel-unbindable switch by default**: the
kernel driver hard-requires `flooding_cfg == PER_FDB` and
`broadcast_cfg == PER_FDB` (plus ctrl-if enabled and
`max_fdbs >= num_ifs`), refusing probe with a loud `-EOPNOTSUPP`
otherwise — and ls-addsw hardwires PER_VLAN/PER_OBJECT. NXP's own
README_DPSW recipe overrides both to PER_FDB; the script's defaults
contradict the only in-corpus consumer [read]. Its second defect is
the **inverted dpbp condition** (ls-main:556-559): the intended
semantics (per its own comment) is "create the control-interface dpbp
unless CTRL_IF_DIS"; the test uses `!=` where `=` was meant, so the
dpbp is created exactly when ctrl-if is *disabled* (wasted object) and
skipped when any other option is passed with ctrl-if on — where the
kernel then defers forever on the missing dpbp [read]. The companion
dpmcp *is* placed correctly (`$container`, ls-main:554 — addmux is
the outlier, see `dpmcp.md`). Every return code in the recipe is
discarded (`create_dpmcp`/`create_dpbp`/`connect`/`assign`/
`set-label`/`sync`), endpoint validation `exit 1`s inside a pipeline
subshell that cannot stop the script, the port-connect loop runs in a
subshell that swallows partial-connect failures, and `object_exists`
is the family-wide unanchored grep [read]. `ls-append-dpl` has no
dpsw-specific handling; its generic DTB path applies with the usual
drops-"0"-values caveat [read].

## Attribute mutability

All 12 `dpsw_cfg` fields are create-time-immutable — including the
three the kernel's bindability predicate reads (`options`,
`flooding_cfg`, `broadcast_cfg`): a wrongly-created dpsw can only be
destroyed and recreated, never repaired [read]. The runtime surface is
the largest in the portfolio (~91 functions): object lifecycle (8),
IRQ (6), per-if link/frame/MAC (9), per-if L2 behavior — STP, TCI,
accepted frames, flooding/broadcast/multicast/learning toggles (12),
QoS/policing/shaping (6), counters (2, including a *writable*
`if_set_counter`), VLAN (11), FDB (11), ACL (7), control interface
(5), mirroring (3), LAG (2), soft parser (2, 10.37+), custom TPID (2),
misc (2). restool reaches 4 groups; everything else is
kernel/DPL-only [read].

## MC API notes

10.32.0 → 10.39.0 is **additive, no wire break**: object version 8.11
→ 8.13 (8.12 at 10.35, 8.13 at 10.38; kernel headers speak 8.12), no
CMDID version-byte changes, three new commands — `SET_SP_PROFILE`
(0x0AE), `SP_ENABLE` (0x0AF), `IF_SET_LAG_STATE` (0x0B0) [read,
mc-utils diff]. `dpsw_cfg`/`dpsw_attr` unchanged. Two flib sharp
edges in the new code: `struct dpsw_lag_cfg` grew a trailing `phase`
byte that `dpsw_lag_set` now marshals — source-compatible, so a
10.32-era caller recompiled against the new header sends stack
garbage as the phase; and `dpsw_sp_enable` writes its `if_id` without
`cpu_to_le16` (benign on our LE hosts, an endian bug by inspection)
[read]. The restool subset's CMDIDs match both ends of the pin
exactly — no skew for what restool uses [read].

The vendor firmware changelog (`qoriq-mc-binary/CHANGELOG.md`) adds
the behavior-side delta the flib diff cannot see: 10.35 made LAG
runtime-reconfigurable with automatic regroup on link events; 10.36
began **validating `_set_error_behavior` parameters** (configurations
older firmware accepted are now refused) and moved timestamps for
dpnis behind a dpsw to the originating dpmac (stable 1588 path
delay); 10.38.0 dates the LAG additions the flib diff found
(`IF_SET_LAG_STATE`, master-port removal, ctrl-if traffic from LAG
ports). One known bug ships **live on the pinned 10.39**: a linkdown
event discards that port's tx shaping configuration — fixed only at
10.40 [read, MC changelog].

## Kernel-side behavior (Linux 6.6.52)

`dpaa2-switch` is upstream and maintained (`CONFIG_FSL_DPAA2_SWITCH=y`)
— by far the loudest driver in the family set: nearly every MC call
has a `dev_err` + unwind [read]. Probe: allocate an atomic dpmcp
portal (1 pool draw, `-ENXIO` → defer) → open → get_attributes → API
gate **≥ 8.9, loud refusal** ("use firmware 10.28.0 or greater",
`-EOPNOTSUPP`, no defer) → bindability preconditions (ctrl-if
enabled, PER_FDB flooding *and* broadcast, `max_fdbs >= num_ifs` —
each a loud refusal) → **`dpsw_reset` at bind** → dismantles the
MC-default state wholesale (VLAN 1 removed everywhere, FDB 0
removed — that one silently) → control interface setup: 2 frame
queues, **1 dpbp allocated from the container pool** handed to MC via
`ctrl_if_set_pools`, notifications via the **shared kernel dpio
service** (ANY_CPU registration; zero dpio/dpcon draws) → per-port
init: **one FDB per port**, each sized `max_fdb_entries / num_ifs`,
VLAN 1 re-added as PVID, egress flood domains always appending the
control interface → netdevs registered last [read].

Resource draw per dpsw: **1 dpmcp + 1 dpbp + 1 IRQ; 0 dpcon, 0 dpio**
[read]. The full switchdev surface rides on it: STP, bridge flags,
VLAN/FDB/MDB, LAG via bonding, tc-flower with shared filter blocks,
per-port phylink/MAC [read].

**Confirmed bug** (`dpaa2-switch.c:3110`): `ethsw->bpid =
dpbp_attrs.id` — the **object id**, not the hardware `bpid`, then used
for every QBMan seed/refill/drain; meanwhile `ctrl_if_set_pools`
correctly passes the object id to MC. The two id spaces coincide on
many boards, so it works by accident; dpaa2-eth does it right
(`.bpid`) three files over [read]. Quiet paths worth carrying:
`dpaa2_io_service_register` failure is converted to `-EPROBE_DEFER`
whatever the real error; the MAC-address feature is gated on API ≥
8.6 and silently no-ops below it; ctrl-if teardown ignores all return
codes [read].

## Lifecycle ordering and dependencies

The dpmcp *and* the dpbp must exist, plugged, **in the dpsw's own
container** before probe — the dpbp is drawn at probe time from the
pool (C1/DPRC-I8 class), so ls-addsw's inverted condition starves
probe invisibly [read]. Order (working recipe, per README_DPSW):
create companions → create dpsw with PER_FDB flooding+broadcast →
connect ports (`dpsw.N.M` ↔ dpmac/dpni) → assign `--plugged=1`. The
sharing topology disconnects dpnis from their dpmacs first, interposes
the dpsw, and hands the dpnis to the other regime — port count fixed
at create (`num_ifs` immutable) [read]. Destroy: kernel unbind →
restool `destroy` (which checks only root-sysfs binding; connected
endpoints are no obstacle) [read].

## Intent mapping

The construct is **switched-domain**: N ports (dpmac uplinks + member
dpnis) in one L2 domain, kernel-steered. Derivation rules the corpus
supports: `num_ifs` = port count (+ nothing — the control interface is
implicit); `max_fdbs >= num_ifs` because the kernel runs
FDB-per-port; per-port FDB budget = `max_fdb_entries / num_ifs`, so
`max_fdb_entries` scales with ports × expected MACs; flooding and
broadcast MUST be PER_FDB and ctrl-if MUST be enabled for the object
to be kernel-bindable at all — a create-time typestate predicate, not
a tunable [read]. Regime law: dpsw's steering owner is always the
kernel (DPDK cannot bind one); a VPP/DPDK dataplane participates only
as a dpni endpoint. This is the exact complement of dpdmux
(DPDK-steerable, kernel-prototype-only) — together they close the
method/ownership matrix started in `dpdmux.md` [read].

## Silent-failure notes

- `dpsw destroy` in a child container **overwrites the destroy error
  with `dprc_close`'s result** — exits 0 while the object survives;
  the guarded pattern exists in the same file's `info` path
  (`dpsw_commands.c:965-969` vs :550-560) [read].
- `dpsw update`: open failure prints an error then **returns 0**; the
  taildrop read's return is never checked, so on failure an
  **uninitialized stack `cfg` is pushed to MC** as the new taildrop;
  seven early-return paths leak the open token [read].
- ls-addsw: inverted dpbp condition (silent probe starvation),
  all return codes discarded, subshell `exit` that stops nothing,
  false-negative `object_exists` grep [read].
- Default-config trap: create succeeds, plug succeeds, probe refuses
  (`-EOPNOTSUPP`) — loud in dmesg but invisible to the script, which
  has already printed success [read].
- `info`: unknown option bit blanks the whole decoded list;
  a mid-list counter failure truncates output; a port whose endpoint
  type is unexpected loses its `connection:` line, which upstream
  tooling (`has_endpoint`) misreads as "free" [read].
- Kernel: FDB 0 removal failure is silent; io-service registration
  failure masquerades as defer; sub-8.6 MAC feature no-ops [read].
- Pinned-firmware bug: a linkdown silently discards that port's tx
  shaping config (fixed at MC 10.40) — a shaped port comes back up
  unshaped [read, MC changelog].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPSW-I1 | Kernel-bindability is a create-time predicate: `¬CTRL_IF_DIS ∧ flooding_cfg=PER_FDB ∧ broadcast_cfg=PER_FDB ∧ max_fdbs≥num_ifs`; violation is refused loudly at probe and, all four fields being immutable, repairable only by destroy+recreate | probe dmesg `-EOPNOTSUPP` lines vs `dpsw info` attrs | candidate |
| DPSW-I2 | **Breaking:** the model must NOT treat the vendor recipe as an oracle (second family in a row): ls-addsw's default flooding/broadcast contradict the only in-corpus consumer, and its dpbp condition is inverted — a default `ls-addsw` object can never carry kernel traffic | ls-addsw output vs subsequent probe result | candidate |
| DPSW-I3 | Resource law: a kernel-bound dpsw draws exactly 1 dpmcp + 1 dpbp (+1 IRQ) from its own container, 0 dpcon, 0 dpio (shared service); the dpbp must pre-exist plugged or probe defers silently | pool free-count deltas at bind; defer loop on missing dpbp | candidate |
| DPSW-I4 | Bind-resets-object, strong form: probe resets the dpsw *and* dismantles MC-default state (VLAN 1, FDB 0) before rebuilding its own — no pre-bind MC-side switch config survives | VLAN/FDB tables before vs after bind | candidate |
| DPSW-I5 | **Breaking:** the model must NOT equate bpid and object id — the in-tree switch datapath uses the object id as bpid and works only where the two coincide (extends DPBP-I5 with a live consumer) | `dpbp info` bpid vs id on the switch's dpbp; traffic outcome where they diverge | candidate |
| DPSW-I6 | Ownership asymmetry: dpsw is kernel-ownable only — DPDK scan discards it silently and its devargs are rejected; a sharing topology types the dpsw's steering owner as kernel, with foreign regimes as dpni endpoints only | fslmc scan logs; devargs errno | candidate |
| DPSW-I7 | Exit-code truth gap (family instance): destroy-in-child exits 0 on failure, update exits 0 on open failure, ls-addsw discards every step's status — convergence only by read-back | object existence / taildrop state vs exit codes | candidate |
| DPSW-I8 | Create ≠ manage (as DPDMUX-I2): restool reaches 4 of ~17 runtime function groups; converged switch state (VLANs, FDBs, ACLs, LAG) is unreachable through restool and owned by the kernel's switchdev surface | restool verb list vs flib census | candidate |

## Unknown / unverified register

1. Whether the board's DPC/DPL contains any dpsw (expected none), and
   whether MC 10.39 even accepts the PER_VLAN/PER_OBJECT +
   ctrl-if-enabled combination ls-addsw emits (kernel refuses it; the
   MC-side semantics of per-VLAN flooding are unobserved).
2. What `dpsw_reset` preserves — dpsw has no `set_resetable`
   analogue in the flib (unlike dpdmux): is reset always total?
3. `mem_size` and `max_meters_per_if` semantics (dead/undocumented,
   same class as dpdmux's `mem_size`).
4. `IF_SET_LAG_STATE` and the new lag `phase` byte's semantics
   (APPLY/CHECK) — no in-corpus caller; plus what MC does with a
   garbage phase from a pre-10.39-compiled caller.
5. Writable counters (`if_set_counter`): who is meant to use them and
   whether MC clamps.
6. Whether the kernel's per-port FDB split (`max_fdb_entries /
   num_ifs`) is an MC-enforced budget or driver convention.
7. The control interface's frame-queue pair sizing (2 FQs, 16-slot
   stores) versus MC-side ctrl-if attributes — clamped or fixed?
