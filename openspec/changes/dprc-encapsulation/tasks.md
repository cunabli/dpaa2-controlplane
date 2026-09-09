## 1. Model gate (Quint first)

- [x] 1.1 Extend `models/families/dprc.qnt` with the container lifecycle sum (Declared → Created/unplugged → Populated → Plugged|Locked → Emptied → Destroyed, VFIO bind state on the plugged face) and the guarded transitions: permission matrix with distinct refusal statuses (0x6/0x8/0x4), eviction law (ADR-0007 §3), visibility law (DPRC-I6), plugged-move precondition (DPRC-I3); typecheck + simulate green
- [x] 1.2 Name and mark the invariants (DPRC-I1, I5, I7, I9, I10, restool-reachable remainder of I11) with Apalache marks; update `models/COVERAGE.md` dispositions; record deferral rows for the portal faces (I8, I11 unlock face, OBJ_CREATE gate → tile #10); resolve design open question on label-under-lock repairability in the model

## 2. Pure core (dpaa2-api, sans-io)

- [x] 2.1 Child-DPRC lifecycle typestates isomorphic to the 1.1 sum; invalid transitions unrepresentable; parity test binding Rust to the Quint sum per the ADR-0002 law
- [x] 2.2 Plan semantics: containment guards (assign-only-while-unplugged ordering, plugged-move refusal), eviction-law teardown planning with predicted post-state, re-observation-based convergence verdicts (no sync), typed refusal discrimination in drift/permission-gap reporting
- [x] 2.3 ITF conformance: frozen traces from the 1.x model replay green through the pure core in cargo test

## 3. Southbound (dpaa2-mc)

- [x] 3.1 Restool shim dprc verbs — create, destroy, assign (child + plugged), unassign, set-label, set-locked — at MC-command granularity; MC statuses and restool client-guard refusals surfaced as distinct typed errors; unit tests against recorded restool transcripts
- [x] 3.2 KernelControl VFIO face: driver_override write, bind, unbind, bound-state + IOMMU-group observation for child DPRCs; sysfs plumbing testable via the existing Runner seam

## 4. Northbound and end-to-end (dpaa2-config, dpaa2-tools)

- [ ] 4.1 Consumer→container derivation: default options mask, root placement, name-keyed label, provenance citing the baseline anchor; kernel tenant derives no container; container-only (no residents) asserted by test
- [ ] 4.2 End-to-end convergence through dpaa2-tools: declared consumer converges to the container on first run, zero actions on re-run; dry-run shows per-object provenance

## 5. Board milestone (operator-launched)

- [ ] 5.1 Container lifecycle suite on scratch children (create, populate, plug, lock, unlock, evict, destroy; refusal-shape assertions); self-cleaning, reference-pair asserted
- [ ] 5.2 VFIO suite: driver_override + bind on a scratch child, override propagation to a subsequently-added child (settle the design open question on post-bind resident creation), unbind, teardown, census clean
- [ ] 5.3 End-to-end convergence suite: intent-declared consumer converged and re-converged clean; read-back diffed against the derived model
- [ ] 5.4 Online-MBT discovery sessions for the candidate containment faces (DPRC-I1 pool boundary, I9 under reconciler plans, I11 restool-reachable remainder); divergences amend model + `docs/baseline/dprc.md` in-change

## 6. Docs and close-out

- [ ] 6.1 Baseline amendments from 5.x outcomes; ADR for any decision that solidified or died on the board; roadmap row #4 status; deferral rows verified present and pointing at #10; CHANGELOG via cliff; full quality floor (`cargo build | fmt | clippy | clippy --tests | doc | test`) green
