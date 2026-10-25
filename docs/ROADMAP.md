# Roadmap: the DPAA2 configuration-management port series

This is the master contract for the series scoped on 2026-08-22 (openspec
change `restool-baseline`, design D1–D11). Each row becomes a real openspec
change only when its turn arrives — proposed just-in-time with everything
learned up to that moment. The roadmap is a living document: re-ordering or
splitting tiles as understanding grows is a documented amendment, not a
broken promise.

Two operating modes frame the whole series (design D8; restool-baseline):

- **FPGA mode** — level-triggered runtime reconciliation, topology re-derived
  from intent every boot. The spine; always primary.
- **ASIC mode (tape-out)** — the same compiled object model solidified into a
  DPL blob consumed by MC firmware at boot. Late, optional, self-contained.

## The series

| # | Change | Delivers | Depends on | Status |
|---|--------|----------|------------|--------|
| 1 | `restool-baseline` | `docs/baseline/` for all 16 object families; `object-model.md` relationship map; traffic inventory; pinned reference environment (MC 10.39.0 + Linux 6.6.52 + restool v2.4); this roadmap; ADRs 0002–0006 | — | archived 2026-08-22 |
| 2 | `verify-foundation` | `models/` + Quint CI ladder (typecheck → simulate → ITF replay → Apalache on marked invariants); `dpaa2-verify` crate (MBT adapter, batch-suite generator, online driver, ITF replayer); retro-models of the already-validated reconciler and dpni/dpmac flow, validated by the first board batch suite — proving the method on known ground; **verifies the recovery guarantee** (reboot restores the DPL baseline) before any mutating suite runs | 1 | archived 2026-08-30 |
| 3 | `intent-layer` | Network-construct intent schema in `dpaa2-config`; name-keyed identity across the config surface (ADR-0015); pure derivation compiler in `dpaa2-api` codifying the sizing rules (DPIO ≥ 2·(1+workers), two-DPBP, DPCON per queue, DPMCP companions); Quint derivation invariants INTENT_I1–I10; raw-surface MBT conformance (`intent_raw.qnt` replaying through `dpaa2-config`); dry-run with per-object rule provenance; additive per-(tenant, family) extra channel; phase-1 gate artefact ADR-0013 (the accepted intent vocabulary); ADR-0014 (linted enumerations, the standing copy-drift law this change minted); identity across time — the anchor-first/label two-pass matcher with disruption-classed plans (`dpaa2-api`), the four identity laws (`match.qnt`/`edits.qnt`) with property + frozen-trace MBT twins in `dpaa2-verify`, gate artefact ADR-0015; **vocabulary-v2** (post-delivery B1 hardening, 2026-09-07) made the illegal pool shapes unrepresentable (`Isolation::Restricted { pool }`, ADR-0002 structural-isomorphism law amended), gave `TenantAbsent` a typed `Referrer`, unified a port owner and each link end under the shared `TenantRef` sum, and added the three programmatic-parity refusals (`LinkSelfLoop`/`RenameDoubleClaim`/`KernelDeclared`) closing the design-D11 one-sided rows | 1, 2 | delivered 2026-09-06 (vocabulary-v2 hardening 2026-09-07) |
| 4 | `dprc-encapsulation` | Child-DPRC lifecycle + VFIO binding typestates; consumer containers; **first online-MBT discovery target** (containment/pool semantics are the most opaque MC behavior) | 2, 3 | delivered 2026-09-14 (ADR-0017; ADR-0007/0011 amendments); hardened 2026-09-15 by `dprc-hardening` (epic-review findings: prune sound on Locked/plugged, shim judgment-free, verb-aware attribution, 19 frozen witnesses) |
| 5 | `dpni-typestate` | Extend the existing dpni support to the full baseline option surface as typestates; model + batch suite | 2, 4 | delivered 2026-09-19 (epic dpaa2-controlplane-guu; ADR-0013 §5 QueueEnvelopeExceeded fence, ADR-0011 corroboration) |
| 6 | `pool-objects` | dpbp + dpio + dpcon + dpmcp as one change (they live and die together as the driver's allocation pool); models + suites | 2, 4 | delivered 2026-09-27: the P3 pool trio (dpbp/dpcon/dpio) + dpmcp modeled as one lifecycle family on the shared `pool_lifecycle` substrate (ADR-0019 pattern), a single provider owning each pool count (design D9), and grow-only root reclaim with typed reboot-required residue (ADR-0020); the kernel-face laws witnessed at ROOT scope (V-POOL-7 rev 1 + V-DPIO-1 rev 1, 2026-09-22 — DPBP-I2/I3, DPCON-I5 and DPIO-I3's reported half board-verified) and the MVP witnessed end to end (V-POOL-6 rev 7 + V-MVP-1 rev 5, 2026-09-27); the DPL-defined-child escape judged NOT FIRED (bead dpaa2-controlplane-960.13, the 4.4 gate), so the DPRC-I8 plug→probe and DPBP-I4 top-up child faces ride bead dpaa2-controlplane-5y7 / `mc-portal-backend` (#10); hardened 2026-09-29 by `pool-hardening` (epic-review findings: child-scope shrink refusal typed at both discovery paths, seat gate wired pure with typed child surplus residue, plan types moved home to `dpaa2-api::plan`, the drawn-foreign census facet netted out of `managed()` so it no longer masks a grow deficit, review rule amendment adopted — asserting surfaces bind the production guard's named accessor) |
| 7 | `dpmac-typestate` | Full dpmac surface (link types, counters, MAC inheritance semantics); model + suite | 2, 4 | delivered 2026-10-03 (epic dpaa2-controlplane-0xu): the P4 boot-born-offer reference (`dpmac_lifecycle` model + isomorphic `families/dpmac.rs` — driver arbitration, MAC inheritance, firmware-versioned counter vocabulary, split link channels); the dpni–dpmac edge teardown law typed as a severed witness on the connection surface (ADR-0008 §8; ADR-0019 edge facet); the design-D6 sysfs carrier route as a policy-free hal primitive; read-only port detail in `dpaa2ctl status --detail`; board-witnessed end to end (V-DPMAC-3 rev 1 — both regimes hitless, typed teardown with no driverless interval) and the phantom create settled DPC-gated (V-DPMAC-2 rev 1: baseline unknown #1 answered, `Invalid state (status 0xc)` refusal); kernel finding 52 recorded (teardown-window detached-timer WARN, milder sibling of finding 49) |
| 8 | `dpseci-typestate` | SEC queue pairs, priorities, congestion; anchored to the vpp-dpaa2-support crypto ADRs; model + suite | 2, 4 | delivered 2026-10-03 (epic dpaa2-controlplane-lbk): the P2 crypto surface (`dpseci_lifecycle` cfg-only model + isomorphic `families/dpseci.rs` typestates; refusal surface replaying banked V-DPSECI-1); convergence as a multiset census on the observable cfg signature (design D9, Quint twins the model gate); the MC-portal read-slice DEBUT behind the kernel whitelist (ADR-0021, the workspace's first confined-unsafe ioctl primitive) with the root-portal routing law; `status --detail` honest-unknown dpseci row; board-witnessed V-DPSECI-3 rev 2 (8/8, hook 2/2 — the V-DPSECI-2 dual-transport HAS_CG witness), baseline unknowns #2 (API 5.4 confirmed) and #3 (kernel dpseci carries no HAS_CG) answered; hardened 2026-10-03 by `dpseci-hardening` (review findings: an unknown firmware bit now rides the decode as a per-bit raw escape rendered by value on the display face while the census projection stays conservative against the V-LIFE-DPSECI-1 destroy loop; the whole-census-poisoning law stated in Quint as `judgeCensus` with the world-mutating destroy surface trace-frozen and replayed, the pure census operators recorded hand-twin-sufficient in COVERAGE; `complete_kernel` folded to one lib definition ending the four hand-mirrored operand pins; the five stale pre-D9 doc comments on the `mc-portal-backend` (#10) reading path synced) |
| 9 | `cross-dprc-links` | dpni↔dpni pseudo-wires (kernel↔VPP) as a first-class link construct; netlink side. Owns the ADR-0017 VFIO rebind-drift policy (deferred from `pool-objects` design D11, bead dpaa2-controlplane-w01): healing a bound child's drift needs an unbind/rebind cycle only a live dataplane can schedule | 4, 5 | delivered 2026-10-07 (epic dpaa2-controlplane-kux): the edge-kind table with per-kind reification (ADR-0022), dpni↔dpni actuation at the root ancestor, the ADR-0017 healing obligations with consented discharge; V-TRAF-1 rev 4 standing PASS 35/35 with the frame witness exact +8, LINK-I1 hardware-anchored by V-LINK-6 (the edge dies with its endpoint); review follow-ups sealed 2026-10-09 via `link-hardening` (epic dpaa2-controlplane-7on): both-ends wire judgment, the consent-gate and stale-mint type laws, the LINK_I1 state-face sweep |
| 10 | `mc-portal-backend` | Rust ioctl MC-portal transport — the workspace's single unsafe module; MC v10 single-version with startup firmware assertion; per-family migration off restool behind the unchanged `McControl` trait, each gated by differential testing (same plan through both backends → identical observed state); scope fenced by ADR-0004 §8: whitelist-parity on the stock reference pair, no kernel change — beyond-whitelist rows ride the VFIO consumer-rig route; encode fixtures pre-banked by the #11/#12 capture tap; the family×verb×backend migration ledger (generated, `cargo test`-checked, the `mc-ioctl-policy.md` pattern) is its consistency backbone | 5–8, 11–12 (`McControl` surface freeze — ADR-0004 §8) | carries the three portal-dependent coverage rows — DPCON-I4, dpni unknowns #4/#11 — via dossier bead dpaa2-controlplane-94k (cross-dprc-links design D9: no restool verb for set_notification, no table read-back, no multi-priority observation) |
| 11 | `dpsw-typestate` | Switch object; online-discovery-heavy; switching topologies beyond point-to-point; board sittings carry the passive ioctl-payload capture tap (#10 encode fixtures, ADR-0004 §8) | 9 | delivered 2026-10-22, archived 2026-10-23 with its review on the record (epic dpaa2-controlplane-dbf; review break-fixes sealed via `dpsw-hardening`, epic dpaa2-controlplane-8z8): model/intent/plan/shim/CLI/engine landed — the kernel-bindability predicate as construction law (DPSW-I1), the switched-domain compiled with rule provenance, the two edge kinds in the ADR-0022 table with planning actuation (DPSW-I9/I10 minted), the D7 capture tap standing in the suite driver; board-witnessed V-DPSW-2 rev 2 (discovery, 11/11 — reset scope, bpid law, FDB budget) and V-DPSW-5 rev 6 (witness, structural 8/8 — converge, sever-first interposition, kernel bind, idempotence, typed teardown); the reachability-traffic leg deferred to #15's driver-occupancy ordering guard plus the kernel refresh (the named revisit trigger, ADR-0023; finding-49 handover UAF) |
| 12 | `dpdmux-typestate` | Demux object; kernel/VPP port-sharing topologies; board sittings carry the capture tap (ADR-0004 §8) | 9 | — |
| 13 | `tier-c-families` | dpaiop, dpci, dpdcei, dpdmai, dprtc, dpdbg — split into per-family changes as reached; each first answers board-exercisability from its baseline doc | 10 (transport + consumer-rig machinery, not a kernel change — ADR-0004 §8) | — |
| 14 | `dpl-tape-out` | Intent → DPL compilation via the build DTI (ASIC mode). Spec must solve the ownership inversion (DPL objects are foreign under current rules) and the return of persisted state | 3, 10 | — |
| 15 | `resource-aware-orchestration` | The ensure pipeline formalized as a guarded-action convergence model in Quint (ADR-0023): completeness (no dead family — the dpsw-typestate executor gap class made unrepresentable), order-as-guards, bounded convergence, confluence follow-on; plus the capacity face of the census (per-container pools + dpsw FDB budget) with typed admission refusals at dry-run. #12 incorporates by adding only its rows and pass — that is this change's DoD | 11 | delivered 2026-10-25 (epic dpaa2-controlplane-m7a, 7 beads): the orchestration model as a guarded-action convergence system in Quint (ORCH-I1–I7 named invariants) with the mask-a-family retro-oracle and the handover live-read pair; the compile-checked total dispatch (Execute/Unimplemented rows, dpseci the first Unimplemented); the census capacity face (dpmcp/dpbp/dpcon pools + dpsw FDB budget, dpio seats spent) with typed admission refusals at dry-run; MBT twins + frozen traces replaying green; bounded pairwise confluence (ORCH-I6 simulator-first); board-witnessed V-ADMIT-1 rev 1 (6/6, 2026-10-25 — the live admission refusal, dpcon over an Unknown ceiling, dpbp the design-D7 pool-tight anchor); the mid-run shortfall rollback deferred to its own change (bead dpaa2-controlplane-axq); #12 (dpdmux) incorporates by adding only its rows and pass — the standing forward-proof DoD; hardened 2026-10-25 by `orchestration-hardening` (epic dpaa2-controlplane-07q, the review's code beads: the model's budget law aligned to the board-proven draws — a switch-port wire draws no FDB unit, dpio exempt from every refusing fold; the replay re-derives each verdict through the Rust `admit` with `fdbRefusesTest` the standing trip wire; `Status::Orphan` reachable so the prune confluence lane and ORCH-I7's teardown half carry weight; whole-intent refusal in the model; `plan_admission` folded onto one inventory snapshot returning the wide carrier, with the dry-run table pinned to the V-ADMIT-1 suite greps; judgment self-application adopted as process rule 6) — `axq` (mid-run shortfall rollback) is the next consumer of the stabilized admission seams |

Tiers (design D1; restool-baseline): A = #4–8 datapath core; B = #11–12 switching; C = #13.
All 16 families are ported; tiers order the work, they do not cut it.

## Decision points

- **Mellanox DT revert (fired at #2).** Choose between (a) careful, flagged
  use of dpmac.7/9 against the cn10k production peer, or (b) reverting the
  device tree so dpmac.3 lands on the on-board Mellanox — which removes the
  forbidden external wire and gains a local, non-production, link-up-capable
  peer that suites can hammer freely. `verify-foundation` (#2) became the
  first traffic-bearing phase, so the decision fired early: (a) is exercised
  there at reachability level only. Option (b) is no longer a scheduled
  decision point and carries no tile; its revisit trigger is a phase
  requiring sustained external traffic (ADR-0003 §8). dpmac.3 remains
  total-deny in the harness envelope; the control-plane library carries no
  dpmac.3 reservation (ADR-0003 §4 scope).
- **Portal route and Tier-B ordering (fired 2026-10-12).** ADR-0004 §8
  resolves the §7 fork by verb scope: control-plane verbs (create / wire /
  label / observe) are whitelist-covered by construction, beyond-whitelist
  rows ride the VFIO consumer-rig route, and a kernel patch is a
  named-trigger escape hatch requiring its own ADR. The series order is
  #11 → #12 → #10 so the `McControl` surface freezes before the portal
  implements it — one differential campaign over a closed verb set —
  and #11/#12 witnesses on the stock reference pair are final. #11/#12
  sittings carry the capture tap that banks #10's encode fixtures.
- **DPL tape-out (#14).** Stays on the table, deprioritized; nothing earlier
  depends on it, so it can never hold the series hostage.
- **TLA+ or Alloy escalation (any model).** Taken per-model only when
  Apalache cannot check a needed temporal property (TLA+) or a relational
  property proves awkward in Quint (Alloy); recorded when taken (ADR-0002).
- **YANG (or similar) northbound.** Revisit trigger: intent expression
  outgrows the tool (ADR-0005).
- **Multi-MC-version support.** Non-goal; revisit trigger: a second board on
  different firmware (ADR-0004).

## Standing rules for every change (the DoD, design D11; restool-baseline)

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
7. **Witness class** — each knob declares its witness class
   (board-witnessable vs offline-pin-only) at design time, so the
   validation path is chosen before implementation and checked after it.

Board sessions are the only step with the operator in the critical path;
changes are sequenced so several suites batch into one sitting where
possible — those sync points are marked in each change's beads.

Task tracking: beads epic per change; `tasks.md` is the entry point only.
One bead at a time through acceptance.
