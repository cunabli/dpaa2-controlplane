# dpni-hardening

## Why

The dpni-typestate epic review (synthesis at
`openspec/changes/dpni-typestate/review/synthesis.md`) confirmed the change's
core promise and the ADR-0002 isomorphism on the create/refusal/projection
surface, but found the epic's drift promise unwired: `drift_disposition` has
zero planner callers, the planner still judges the legacy string map the shim
fills empty, and wiring it naively would mark every port-only dpni permanently
drifted. Three replay faces are blind to the frozen-trace rung, and three
one-screen shim/engine defects sit on the create hot path. This change carries
those review findings to closure and gates the dpni-typestate archive.

## What Changes

- **Wire cfg drift end-to-end (review finding 1-3, bead A)**: `plan_present`
  consumes `ObservedDpni.cfg_observation` via `drift_disposition` and plans
  destroy+create per the recorded D1 law; the unsized-create sizing seam is
  resolved core-side from `Inventory.cpus` (retiring the shim fallback) OR
  unsized cfgs are fenced out of drift comparison by construct — the task
  records which, amending the `contract/mc.rs` contract in the same commit;
  the fake backend projects `Some(DpniObservation::project(cfg))` at create so
  fake-vs-reconcile tests cover cfg drift.
- **Close the three replay blind faces (finding 6, bead B)**: `raw_escape()`
  decodes `HasReplication` (parser arm lands first); a McClearedFlags
  create/read-back scenario is frozen and replayed; a queue-envelope
  RefusedTrace twin freezes the guu.4a intent arm; `scenarioUnpricedRefusedTest`
  freezes the Unpriced arm.
- **Shim/engine hardening (findings 7-9, bead C)**: `stamp_label` and
  `read_inventory` route through `run_verb`'s typed classification; the
  engine's convergence zip gains a length check so an undispatched declared
  container is a loud error; `RawDpniAttr` captures `wriop_version`, making
  the mc-backend spec line true.
- **Review gate and archive**: the amend-at-archive doc pass from the
  synthesis ledger lands (deferral rows, `NUM_QUEUES_HI` model constant,
  guu.4a revisit trigger, wording amends), then dpni-typestate archives with
  its review directory.

## Capabilities

### New Capabilities

None — the change lands entirely as deltas to existing capabilities.

### Modified Capabilities

- `reconciler`: cfg drift on a dpni is judged through the typed
  `drift_disposition` surface (destroy+create per D1), with unsized-create
  sizing resolved so a port-only dpni does not false-drift.
- `mc-backend`: `stamp_label`/`read_inventory` join the typed-classification
  funnel; the observation mapping carries `wriop_version`; the unsized-queues
  contract moves or is fenced per the reconciler decision.
- `mbt-harness`: the frozen-trace corpus covers the McClearedFlags,
  queue-envelope-intent, and Unpriced refusal faces; the ITF parser decodes
  the full `RawEscape` vocabulary.

## Impact

- Crates: `dpaa2-api` (plan/reconcile, contract/fake, families::dpni docs),
  `dpaa2-mc` (restool verbs, parse), `dpaa2-verify` (dpni_itf, replay,
  frozen traces), `dpaa2-tools` (engine length check).
- Models: `models/families/dpni.qnt` + `models/intent/replay.qnt` scenario
  runs; new frozen traces under `models/traces/`.
- Docs: the amend-at-archive pass touches `docs/baseline/dpni.md`,
  `models/COVERAGE.md`, ADR-0013/0018, `models/README.md`, verify README,
  and dpni-typestate change artifacts before archival.
- Beads: epic + task beads at propose time; the gate task carries the
  dpni-typestate archive.
