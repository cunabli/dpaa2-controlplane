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

- [x] 1.1 `plan_present` consumes `ObservedDpni.cfg_observation` via
  `drift_disposition` and emits destroy + create per D1; unsized-create
  sizing resolved core-side from `Inventory.cpus` (shim fallback
  retired, `contract/mc.rs` contract amended in the same commit) OR
  unsized cfgs fenced out of drift by construct — choice recorded here;
  `contract/fake.rs` projects `Some(DpniObservation::project(cfg))` at
  create; a reconcile test proves an unsized-created dpni does not
  drift

  Landed: choice (b) — unsized cfgs fenced out of drift by construct.
  `plan_present` skips the typed `drift_disposition` comparison when the
  desired block is unsized (`num_queues == 0`, the `ls-addni`/port-only
  marker), because `project()` echoes that `0` sentinel while the board
  reads back a host-derived count — so an unsized create would false-drift
  on `num_queues` (and every other sentinel-`0` field) forever. Chosen over
  (a) core-side sizing from `Inventory.cpus` because (a) has the larger,
  less type-honest diff: it forces `Inventory` into the pure `reconcile`
  signature and, sizing only `num_queues`, would still leave the other
  sentinel-`0` fields false-drifting. The recorded shim contract
  (`contract/mc.rs:42-48`, `self.queues` fallback in `restool.rs`) therefore
  stands unchanged (synthesis row 2 "the recorded shim contract stands");
  the legacy attribute-string-map drift loop was removed from `plan_present`.
  The optional `NumQueues`-in-signature hardening (design Open Question /
  synthesis row 11) is skipped: it is gated to "if the sizing seam lands
  core-side", i.e. choice (a).

## 2. Close the replay blind faces (bead B; synthesis row 6)

- [x] 2.1 `raw_escape()` decodes `HasReplication`; then
  `scenarioMcClearedCreateReadbackTest` + the queue-envelope
  RefusedTrace twin in `models/intent/replay.qnt` + the frozen
  `scenarioUnpricedRefusedTest` land with their `package.json` match
  entries and TRACES rows; `cargo test -p dpaa2-verify` green
  Landed: only `dpni_itf.rs` gains the arm (row 6 names it); the
  `queueEnvelopeRefusedTrace` reuses `intent_main`'s `queueEnvelopeIntent`
  /`invSeven25G` rather than restating the literal.

## 3. Shim/engine hardening (bead C; synthesis rows 7-9)

- [x] 3.1 `stamp_label` and `read_inventory` route through `run_verb`;
  the engine convergence zip gains a length check with a
  mismatched-lengths unit test; `RawDpniAttr` captures `wriop_version`
  as `Option<u16>`
  Landed: both shim verbs now route through `run_verb`, whose sole raw
  read is `run_capture` (restool.rs:501), so `rg 'self.runner.run\b'`
  returns zero matches — the `\b` excludes `run_capture`; the invariant
  "only `run_verb` touches the raw runner" holds. The zip guard is
  extracted to `pair_containers` (engine.rs) for the unit test. Skipped
  the optional `TenantName` `TryFrom` carve-out: the wording amend covers
  it and no concrete failure surfaced.
