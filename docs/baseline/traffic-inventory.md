# Traffic inventory: validation scenarios vs the port safety matrix

Populated by task 6.2 (spec: object-baseline, "Validation scenarios are
traffic-classified against the port matrix"). The planned validation
scenarios are the board-pending invariants and unknown/unverified register
items of the 16 family baselines; each scenario cites the ids it settles.
Every scenario is classified as exactly one of:

- **object-lifecycle-only** — MC-bus mutations and queries only, no link
  semantics;
- **link-signaling** — asserts or observes link state, no frames;
- **traffic-bearing** — frames emitted; explicitly flagged, allowed ports
  only.

Port safety matrix (ADR-0003 §4, normative copy there):

| Ports | Class ceiling |
|---|---|
| dpmac.3 | total-deny — appears in **no scenario of any class** in this inventory |
| dpmac.17 / dpni.0 | total-deny — management plane, never enumerated or touched |
| dpmac.4–6, dpmac.8, dpmac.10 | lifecycle-only — unwired; absorb all object-lifecycle and connect-edge churn |
| dpmac.7, dpmac.9 | link-signaling / traffic-bearing, each run explicitly flagged |

Reading rules:

- **Class describes semantics; the Ports column binds the matrix.** A
  scenario that names no dpmac (scratch-container churn, virtual dpci
  links, internal dpni↔dpni pairs) trivially satisfies the matrix — the
  ceiling applies only where a dpmac is named. The harness still rejects
  any trace whose declared class exceeds what its named ports allow
  (ADR-0003 §5).
- All mutating scenarios run in **scratch child DPRCs** with unconditional
  teardown (ADR-0003 §6); the two families that structurally cannot
  (dpdbg, dprtc — root-container residents) are called out in §4 below.
- Nothing here runs in this change: `restool-baseline` is docs-only; the
  first mutating suites arrive with `verify-foundation` (change #2), and
  each later change picks its scenarios from this inventory.

## 1. Object-lifecycle-only scenarios

No link semantics; any dpmac named is from the unwired set.

| Id | Scenario | Settles | Ports |
|---|---|---|---|
| V-DPRC-1 | Scratch-container lifecycle sweep: create → populate → assign/unassign → destroy; non-empty destroy behavior; teardown reachability | DPRC-I3, I9; dprc.md unknowns 1, 2 | none |
| V-DPRC-2 | Permission-matrix mapping: per-option-bit denial probes (SPAWN vs OBJ_CREATE vs TOPOLOGY_CHANGES vs ALLOC) in scratch children | dprc.md unknown 3 | none |
| V-DPRC-3 | `set-locked` semantics: what a locked hierarchy still allows, who unlocks | DPRC-I11; unknown 4 | none |
| V-DPRC-4 | `mc.global` observation + `dump-mem` semantics (read-only) | dprc.md unknowns 5, 10 | none |
| V-DPRC-5 | Rescan visibility: mutate a child container, prove `sync` refreshes nothing there; autorescan state | DPRC-I6; unknown 12 | none |
| V-DPNI-1 | Bare-create default probe: `dpni create` with no options, read back attributes | DPNI-I7 | none |
| V-DPNI-2 | num_queues ceiling walk (16 → 32) and dead-option create-then-fail exit shape | DPNI-I6; dpni.md unknown 2 | none |
| V-DPNI-3 | Bind/unbind reset coverage: plug a scratch dpni (unconnected), set runtime state, unbind, read back | DPNI-I2, I8; unknown 4 | none |
| V-DPNI-4 | TX_CONFIRMATION_MODE v1-vs-v2 handler probe (raw command via `/dev/dprc.N`) | DPNI-I11; unknown 1 | none |
| V-DPMAC-1 | `dpmac info` on an unwired port: API version, counter refusal behavior at ids ≥ 28 | DPMAC-I7; dpmac.md unknowns 5, 7 | dpmac.4–6/8/10 (read-only) |
| V-DPMAC-2 | `dpmac create --mac-id=<unused>` against a DPC with no such port entry; destroy after | dpmac.md unknown 1 | none (phantom id) |
| V-POOL-1 | Pool mechanics sweep in a scratch container: plugged-vs-allocator visibility, exhaustion defer, top-up unblock | DPBP-I2, I4; DPMCP-I4 | none |
| V-POOL-2 | Reset coverage probes: `dpbp_reset` drain semantics, dirty-object circulation, dpmcp statefulness across owners | DPBP-I3; DPMCP-I3; dpbp.md unknown 1, dpmcp.md unknown 2 | none |
| V-POOL-3 | uapi portal concurrency: N openers vs N−1 free dpmcps, exhaustion errno | DPMCP-I2 | none |
| V-DPIO-1 | NO_CHANNEL dpio: what MC reports as num_priorities; kernel notification behavior | DPIO-I3; dpio.md unknown 1 | none |
| V-DPSECI-1 | Create-validation probes: priority 0, priorities ≠ queue count, asymmetric counts via raw command | DPSECI-I2; dpseci.md unknown 1 | none |
| V-DPSECI-2 | Options read-back via raw GET_ATTR (kernel dpseci HAS_CG; restool blindness) | DPSECI-I3; unknowns 2, 3 | none |
| V-DPDMUX-1 | V4-create acceptance on 10.39 (restool's V5 struct layout), method S_VLAN / manip probes | dpdmux.md unknowns 1, 3 | none |
| V-DPDMUX-2 | Uplink-restriction probe: connect dpdmux.N.0 to a dpni, expect the 10.37 refusal; then to an unwired dpmac | DPDMUX-I8; dpdmux.md [read, MC changelog] | dpmac.4–6/8/10 (connect only) |
| V-DPDMUX-3 | Cross-regime reset interference: DPDK `set_resetable` skip flags vs evb probe reset | DPDMUX-I3; unknown 2 | none |
| V-DPSW-1 | ls-addsw-shaped create (PER_VLAN/PER_OBJECT): MC acceptance, then loud kernel probe refusal | DPSW-I1, I2; dpsw.md unknown 1 | none |
| V-DPSW-2 | `dpsw_reset` coverage (no set_resetable analogue: total?) | dpsw.md unknown 2 | none |
| V-DPSW-3 | Port connects dpsw.N.M ↔ unwired dpmac; per-port FDB budget observation | dpsw.md unknown 6 | dpmac.4–6/8/10 (connect only) |
| V-DPAIOP-1 | `dpaiop create` refusal status on this platform; AIOP-flagged dprc create as a separate step | DPAIOP-I1, I2; dpaiop.md unknowns 1, 2 | none |
| V-DPAIOP-2 | `/dev/dpaa2_aiop_console` open outcome with no AIOP | dpaiop.md unknown 3 | none |
| V-DPCI-1 | GPP↔GPP pair: create ×2, connect, destroy-while-connected, asymmetric-priority connect, rescan visibility | DPCI-I3, I4; dpci.md unknowns 2, 3, 5 | none |
| V-DPCI-2 | Options-discard hardware probe: OPR config on an object created "with" the flag | DPCI-I2; unknown 6 | none |
| V-DPDCEI-1 | `dpdcei create` probe (success now expected — hardware present per Table 2-1), GET_API_VERSION, dce_version read | DPDCEI-I1; dpdcei.md unknowns 1–3 | none |
| V-DPDMAI-1 | Bare-create default (`num_queues=0` on the wire), queue ceiling, 1-queue/2-priority kernel probe outcome | DPDMAI-I3, I5; dpdmai.md unknowns 1–3 | none |
| V-DPDMAI-2 | Shutdown wrong-token destroy: does a created dpdmai survive a kernel shutdown cycle? | dpdmai.md unknown 4 | none |
| V-DPRTC-1 | Second `dprtc create` refusal: status code, and `dprc show` unchanged after (10.37 blast-radius fix observable) | DPRTC-I1; dprtc.md unknowns 1, 2 | none |
| V-DPRTC-2 | Read-only observations: reported API version, paddr/little_endian vs DT, two-step-only manual claim vs one-step kernel path | dprtc.md unknowns 4, 5, 6 | none |
| V-DPRTC-3 | Destroy of the DPL-born dprtc.0: bound attempt, unbind, unbound destroy, reboot-restore check — gated on the recovery guarantee (see §4), sequenced last in its sitting | dprtc.md unknown 3 | none |
| V-DPDBG-1 | Singleton probes: create in root, second create, create in non-root; set/dump with out-of-range values; unhandled dump type | DPDBG-I1, I4; dpdbg.md unknowns 1, 2, 5 | none |
| V-READBACK-1 | Read-back sweep: bare `dpni`, bare `dpdmai`, NO_CHANNEL `dpio`, `dpbp` and `dpdcei` created unplugged in one scratch container and each read back with `info` before the trap reclaims them — the defaults the lifecycle suites never read | DPNI-I7; DPDMAI-I5; DPIO-I3 (reported priorities); DPBP-I5 (bpid vs id); DPDCEI-I1 (API version); dpio.md unknown 1, dpbp.md unknown 2, dpdcei.md unknown 2, dpdmai.md unknown 1 | none |
| V-GENDPL-1 | generate-dpl round-trip audit against created objects (lossy/wrong emitters across dpni/dpdcei/dpaiop/dpdmai) | dpni.md silent-failure notes; DPDCEI-I2; DPAIOP-I3; DPDMAI-I4 | none |

## 2. Link-signaling scenarios

Assert or observe link state, no frames. Physical-port instances run on
dpmac.7/9 only, explicitly flagged per run; virtual-link instances name no
port.

| Id | Scenario | Settles | Ports |
|---|---|---|---|
| V-LINK-1 | dpci pair liveness: does link go up on connect alone or only after both ends enable? | DPCI-I5; dpci.md unknown 1 | none (virtual link) |
| V-LINK-2 | dpmac connection-state vs MAC link-state split: `dpmac info` "link is up" against peer `dpni_get_link_state` under a real link transition | DPMAC-I5 | dpmac.7 or dpmac.9, flagged |
| V-LINK-3 | `set_link_state` with `state_valid=0`: does the `up` bit take effect? (every kernel push depends on it) | DPMAC-I9; dpmac.md unknown 2 | dpmac.7 or dpmac.9, flagged |
| V-LINK-4 | Directional link pair: peer request (`dpni_set_link_cfg`) surfacing in `dpmac_get_link_cfg`, PHY reality surfacing in `dpni_get_link_state` | DPMAC-I4 | dpmac.7 or dpmac.9, flagged |
| V-LINK-5 | `assign --plugged=0/1` race against a bound, link-up netdev-backed dpni | DPRC-I2; dprc.md unknown 9 | dpmac.7 or dpmac.9, flagged |

## 3. Traffic-bearing scenarios

Frames emitted. V-TRAF-0 is the pattern — the smallest configured object
group that can carry a frame, run by `verify-foundation` (design D7 step
5) to prove the class end to end: setup by a driven trace, frames judged
on the dpni's own counters, the peer read-only — **passed 2026-08-24**
(`models/board/README.md`). V-TRAF-1..4 arrive with
the later changes and are listed as placeholders so the classification
exists before their first frame does. All physical-port instances:
dpmac.7/9 only, explicitly flagged; the Mellanox device-tree decision
(ADR-0003 §8) fired at V-TRAF-0 (option a, reachability only) and is
re-decided at #9.

| Id | Scenario | Owning change | Ports |
|---|---|---|---|
| V-TRAF-0 | Reachability pattern: one kernel-bound dpni with its census companions on a flagged port; a limited broadcast burst in from the peer and a minimal broadcast burst out of the netdev, each asserted as a delta on the dpni's `ingress_all_frames` / `egress_all_frames`, the peer's rx counter read only | verify-foundation (#2) | dpmac.7, flagged |
| V-TRAF-1 | Cross-dprc dpni↔dpni pseudo-wire carrying frames (kernel↔VPP) | cross-dprc-links (#9) | none (internal pair, no wire) |
| V-TRAF-2 | Kernel-owned dpsw switching between a dpmac uplink and member dpnis | dpsw sharing (#11) | dpmac.7/9, flagged |
| V-TRAF-3 | dpdmux shared uplink: one dpmac feeding kernel + VPP dpnis | dpdmux sharing (#12) | dpmac.7/9, flagged |
| V-TRAF-4 | dpseci datapath scenarios (wedge reproduction, queue-status observables) | crypto follow-ons | none (SEC-internal, no port) |

## 4. Out-of-envelope: excluded or gated scenarios

Named so their absence from §§1–3 is a decision, not an oversight:

- **Anything naming dpmac.3 or dpmac.17/dpni.0** — total-deny; no scenario
  class exists for them, including read-only info (dpmac.17 is a foreign
  management object, ADR-0001 §4). dpmac.3's status is revisited only at
  the Mellanox decision point (ADR-0003 §8).
- **`dprtc destroy` of the DPL-born dprtc.0** (dprtc.md unknown 3) — a
  root-container mutation of a kernel-owned singleton that kills kernel
  PTP; violates the scratch-container rule (ADR-0003 §6). Was deferred
  until a re-DPL-able recovery path existed; V-RECOVERY-1's green marker
  (ADR-0003 §7) unlocked it, and it now runs as §1's V-DPRTC-3 — last in
  its sitting, ending in the reboot that restores the DPL baseline.
- **dpdbg `set --uart` reroute** (dpdbg.md unknown 3) — potentially
  unrecoverable console loss; explicitly an ADR-0003 safety question
  before any ls-debug parity testing.
- **dpdbg/dprtc scenarios in general run against root-container
  residents** (both families are structurally root-only) — they cannot be
  scratch-contained, so V-DPRTC-*/V-DPDBG-* carry a mandatory per-step
  operator confirmation regardless of promotion state (ADR-0003 §3
  promotion never applies outside scratch containers).
- **Wedge-inducing dpseci load** (DPSECI-I6) — verified already in
  vpp-dpaa2-support; re-running it is consumer-side stress, not object
  validation, and stays out of this series' suites.
