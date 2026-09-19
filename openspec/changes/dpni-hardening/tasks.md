# dpni-hardening tasks

Land order per design: gate first (seals the review, archives
dpni-typestate), then the beads in any order except 2.x's
parser-arm-first rule. One bead at a time through acceptance.

## 0. Review gate

- [x] 0.1 The amend-at-archive doc pass lands on the dpni-typestate
  record (synthesis §4: deferral rows in baseline + COVERAGE,
  `NUM_QUEUES_HI` model constant, guu.4a revisit trigger in ADR-0013 +
  baseline, wording amends, trait-doc + ADR-0018 chain-policy note,
  review rule amendment), then dpni-typestate archives with its review
  directory; quality floor green

## 1. Wire cfg drift end-to-end (bead A; synthesis rows 1-3)

- [ ] 1.1 `plan_present` consumes `ObservedDpni.cfg_observation` via
  `drift_disposition` and emits destroy + create per D1; unsized-create
  sizing resolved core-side from `Inventory.cpus` (shim fallback
  retired, `contract/mc.rs` contract amended in the same commit) OR
  unsized cfgs fenced out of drift by construct — choice recorded here;
  `contract/fake.rs` projects `Some(DpniObservation::project(cfg))` at
  create; a reconcile test proves an unsized-created dpni does not
  drift

## 2. Close the replay blind faces (bead B; synthesis row 6)

- [ ] 2.1 `raw_escape()` decodes `HasReplication`; then
  `scenarioMcClearedCreateReadbackTest` + the queue-envelope
  RefusedTrace twin in `models/intent/replay.qnt` + the frozen
  `scenarioUnpricedRefusedTest` land with their `package.json` match
  entries and TRACES rows; `cargo test -p dpaa2-verify` green

## 3. Shim/engine hardening (bead C; synthesis rows 7-9)

- [ ] 3.1 `stamp_label` and `read_inventory` route through `run_verb`;
  the engine convergence zip gains a length check with a
  mismatched-lengths unit test; `RawDpniAttr` captures `wriop_version`
  as `Option<u16>`
