# Tasks: cross-dprc-links

Sequential by design (grilling Q8): the model fixes the vocabulary the
Rust implements, the api surface feeds the adapters, the suite renders
from the scenario module. One bead at a time through acceptance once
the epic is cut; parcels go to `opus48-developer` (no model override);
the main loop gathers context, writes parcel specs, reviews, gates,
and commits. Every group ends at the quality floor
(`scripts/checks/quality-floor.sh`).

## 1. Model gate (parcel 1 — Quint)

- [x] 1.1 `models/families/link_lifecycle.qnt`: wire typestates
      (ends-exist → connected → disconnected → end-destroy-legal)
      consuming `core/connect.qnt` operators (design D3/D7)
- [x] 1.2 Container interplay in the model: populate→connect→bind
      partial order; eager DeferredVisibility obligation on post-bind
      create; lazy stale-node residue on post-bind destroy; consented
      rebind as the only discharge (D4/D5)
- [x] 1.3 LINK-I* invariants named and marked: disconnect-before-
      destroy, obligation-attached create, cardinality-one /
      disconnect-before-reconnect (DPRC-I5 promoted), refusal
      surfacing; Apalache marks where the state space permits
- [x] 1.4 V-TRAF-1 scenario module (one action enabled per state)
      covering the D10 faces
- [x] 1.5 COVERAGE.md: LINK-I* rows added; DPCON-I3 and single_sender
      rows modeled-this-change; DPCON-I4 + dpni unknowns #4/#11
      re-pointed to tile #10; typecheck + simulate + marked-Apalache
      green

## 2. Docs (parcel 2 — ADR drafted by main loop, mechanical edits parceled)

- [x] 2.1 New connection-surface ADR: edge-kind table, per-kind
      reification rows including the healing policy (D3/D5/D8)
- [x] 2.2 Amendments: ADR-0017 (PASS4-F8 discharged, pointer to new
      ADR), ADR-0019 (edge facet second inhabitant + kind table),
      ADR-0003 §8 (Mellanox decision point dropped; trigger reworded)
- [x] 2.3 Roadmap: row #9 cleaned of Mellanox; row #10 Delivers gains
      the three portal-dependent rows via one line naming the dossier
      bead; create that single dossier bead carrying the D9 analysis
      (trigger = roadmap, reparent at #10 scoping)

## 3. Connection surface in dpaa2-api (parcel 3a — Rust)

- [x] 3.1 Edge-kind table with per-kind reification policy in
      `dpaa2-api::plan`, mirroring `core/connect.qnt`; dpmac machinery
      claimed, not retyped (D3 bounded lift)
- [ ] 3.2 Link transitions keyed by ends; container-agnostic
      representability; common-ancestor resolution; typed refusal
      surfacing for MC-refused patterns (D2)
- [ ] 3.3 Container typestate: populate→connect→bind partial order;
      post-bind connect/disconnect faces for visible endpoints (D4)
- [ ] 3.4 Obligation types: eager DeferredVisibility on post-bind
      create (unconstructible without planned discharge), lazy
      stale-node residue, consented Disruptive rebind transition,
      declined-consent typed residue (D5)
- [ ] 3.5 Plan laws: disconnect-before-destroy, disconnect-before-
      reconnect; intent-side types for the dpcon priority and
      single_sender knobs with provenance (D9)

## 4. Conformance twins (parcel 3b — Rust tests)

- [ ] 4.1 ITF replay twins for `link_lifecycle` (property + frozen
      traces) green against the new surface
- [ ] 4.2 The a-bounded proof: dpmac family's existing frozen traces
      replay green, unchanged — any diff fails this parcel (D3)
- [ ] 4.3 Unit tests for obligations, partial order, and refusal
      surfaces per rust idiom

## 5. Adapters and frontend (parcel 4 — dpaa2-mc, dpaa2-config, dpaa2-tools)

- [ ] 5.1 McControl connection verbs named 1:1 with whitelisted MC
      commands, ancestor-explicit, typed returns; restool text parsing
      stays behind the trait; no portal read-slice additions (D6)
- [ ] 5.2 Child populate resolves dpni↔dpni peers and issues the
      connect; root reconcile actuates link edges
- [ ] 5.3 `DriftRefused` → typed DeferredVisibility obligation
      plumbing; consented rebind execution in the engine
- [ ] 5.4 dpaa2-config: parse + convert the two additive knobs with
      refusal idiom; links gain no attributes (rates refused by
      omission)
- [ ] 5.5 dpaa2-tools: dry-run renders link transitions with
      provenance; consent flow for the rebind; link + obligation rows
      in `status --detail` with honest-unknown idiom

## 6. Board suite (parcel 5 — dpaa2-verify + hooks)

- [ ] 6.1 Generate V-TRAF-1 from the scenario module; faces per D10
      (root↔root, cross-container, child↔child, heal, destroy-mirror,
      teardown laws); V-DPCI-1 refusal replayed from the bank, not
      re-run; no PHY dpmac anywhere
- [ ] 6.2 Hook scripts: netns rig + ping frame witness (exact-count
      oracle) + saturation smoke (monotone counters, zero discards,
      no rate target)
- [ ] 6.3 Offline gates green: rendered suite reviewed, safety
      envelope + RECOVERY-VERIFIED asserted, argv conforms to
      baseline verb tables

## 7. Board milestone and close-out (operator + main loop)

- [ ] 7.1 Pre-run record commit (standing rule: commit before the run)
- [ ] 7.2 Operator sitting: V-TRAF-1 executed; results diffed;
      divergences fed back to the model; full teardown verified
- [ ] 7.3 Baseline deltas from board answers (post-bind connect dmesg
      law, destroy-mirror behavior) into dprc.md/dpni.md; close bead
      dpaa2-controlplane-w01 (healing policy delivered)
- [ ] 7.4 Spec deltas promoted; CHANGELOG via cliff; quality floor
      green; seal
