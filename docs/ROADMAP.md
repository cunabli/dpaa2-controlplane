# Roadmap: the DPAA2 configuration-management port series

This is the master contract for the series scoped on 2026-08-22 (openspec
change `restool-baseline`, design D1–D11). Each row becomes a real openspec
change only when its turn arrives — proposed just-in-time with everything
learned up to that moment. The roadmap is a living document: re-ordering or
splitting tiles as understanding grows is a documented amendment, not a
broken promise.

Two operating modes frame the whole series (design D8):

- **FPGA mode** — level-triggered runtime reconciliation, topology re-derived
  from intent every boot. The spine; always primary.
- **ASIC mode (tape-out)** — the same compiled object model solidified into a
  DPL blob consumed by MC firmware at boot. Late, optional, self-contained.

## The series

| # | Change | Delivers | Depends on | Status |
|---|--------|----------|------------|--------|
| 1 | `restool-baseline` | `docs/baseline/` for all 16 object families; `object-model.md` relationship map; traffic inventory; pinned reference environment (MC 10.39.0 + Linux 6.6.52 + restool v2.4); this roadmap; ADRs 0002–0006 | — | archived 2026-08-22 |
| 2 | `verify-foundation` | `models/` + Quint CI ladder (typecheck → simulate → ITF replay → Apalache on marked invariants); `dpaa2-verify` crate (MBT adapter, batch-suite generator, online driver, ITF replayer); retro-models of the already-validated reconciler and dpni/dpmac flow, validated by the first board batch suite — proving the method on known ground; **verifies the recovery guarantee** (reboot restores the DPL baseline) before any mutating suite runs | 1 | archived 2026-08-30 |
| 3 | `intent-layer` | Network-construct intent schema in `dpaa2-config`; pure derivation compiler in `dpaa2-api` codifying the sizing rules (DPIO ≥ 2·(1+workers), two-DPBP, DPCON per queue, DPMCP companions); Quint derivation invariants; dry-run with per-object rule provenance; additive per-(tenant, family) extra channel; phase-1 gate artefact ADR-0013 (the accepted intent vocabulary) | 1, 2 | **in flight** since 2026-08-30 |
| 4 | `dprc-encapsulation` | Child-DPRC lifecycle + VFIO binding typestates; consumer containers; **first online-MBT discovery target** (containment/pool semantics are the most opaque MC behavior) | 2, 3 | — |
| 5 | `dpni-typestate` | Extend the existing dpni support to the full baseline option surface as typestates; model + batch suite | 2, 4 | — |
| 6 | `pool-objects` | dpbp + dpio + dpcon + dpmcp as one change (they live and die together as the driver's allocation pool); models + suites | 2, 4 | — |
| 7 | `dpmac-typestate` | Full dpmac surface (link types, counters, MAC inheritance semantics); model + suite | 2, 4 | — |
| 8 | `dpseci-typestate` | SEC queue pairs, priorities, congestion; anchored to the vpp-dpaa2-support crypto ADRs; model + suite | 2, 4 | — |
| 9 | `cross-dprc-links` | dpni↔dpni pseudo-wires (kernel↔VPP) as a first-class link construct; netlink side. **Decision point: Mellanox DT revert** (see below) — first traffic-bearing phase | 4, 5 | — |
| 10 | `mc-portal-backend` | Rust ioctl MC-portal transport — the workspace's single unsafe module; MC v10 single-version with startup firmware assertion; per-family migration off restool behind the unchanged `McControl` trait, each gated by differential testing (same plan through both backends → identical observed state) | 5–8 | — |
| 11 | `dpsw-typestate` | Switch object; online-discovery-heavy; switching topologies beyond point-to-point | 9 | — |
| 12 | `dpdmux-typestate` | Demux object; kernel/VPP port-sharing topologies | 9 | — |
| 13 | `tier-c-families` | dpaiop, dpci, dpdcei, dpdmai, dprtc, dpdbg — split into per-family changes as reached; each first answers board-exercisability from its baseline doc | 10 | — |
| 14 | `dpl-tape-out` | Intent → DPL compilation via the build DTI (ASIC mode). Spec must solve the ownership inversion (DPL objects are foreign under current rules) and the return of persisted state | 3, 10 | — |

Tiers (design D1): A = #4–8 datapath core; B = #11–12 switching; C = #13.
All 16 families are ported; tiers order the work, they do not cut it.

## Decision points

- **Mellanox DT revert (fired at #2; re-decided at #9 before sustained
  traffic suites).** Choose between (a) careful, flagged use of dpmac.7/9
  against the cn10k production peer, or (b) reverting the device tree so
  dpmac.3 lands on the on-board Mellanox — which removes the forbidden
  external wire and gains a local, non-production, link-up-capable peer
  that suites can hammer freely. `verify-foundation` (#2) became the first
  traffic-bearing phase, so the decision fired early: (a) is exercised
  there at reachability level only; (b) stays open and is re-decided at
  #9 when sustained traffic arrives (ADR-0003 §8, amended). Until then
  dpmac.3 remains total-deny.
- **DPL tape-out (#14).** Stays on the table, deprioritized; nothing earlier
  depends on it, so it can never hold the series hostage.
- **TLA+ or Alloy escalation (any model).** Taken per-model only when
  Apalache cannot check a needed temporal property (TLA+) or a relational
  property proves awkward in Quint (Alloy); recorded when taken (ADR-0002).
- **YANG (or similar) northbound.** Revisit trigger: intent expression
  outgrows the tool (ADR-0005).
- **Multi-MC-version support.** Non-goal; revisit trigger: a second board on
  different firmware (ADR-0004).

## Standing rules for every change (the DoD, design D11)

1. **Baseline anchor** — cite the `docs/baseline/` sections implemented;
   divergences amend the baseline in the same change.
2. **Model gate** — Quint models updated first; invariants named; typecheck +
   simulate + marked-Apalache green before Rust lands.
3. **Conformance gate** — ITF replay green against the Rust core; for
   southbound work, the restool-vs-portal differential gate.
4. **Board milestone** — a named batch suite (plus online session while the
   family is in per-step learning mode) executed by the operator; results
   diffed clean or divergences fed back to the model. Traffic class declared
   per ADR-0003; scripts assert the reference pair (MC 10.39.0 +
   Linux 6.6.52) before running.
5. **Docs** — ADR for any decision that solidified or died on the board;
   spec deltas promoted; CHANGELOG flows from conventional commits.
6. **Quality floor** — `cargo build | fmt | clippy | clippy --tests | doc |
   test` green.

Board sessions are the only step with the operator in the critical path;
changes are sequenced so several suites batch into one sitting where
possible — those sync points are marked in each change's beads.

Task tracking: beads epic per change; `tasks.md` is the entry point only.
One bead at a time through acceptance.
