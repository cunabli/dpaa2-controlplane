# reconciler delta — dpni-hardening

## MODIFIED Requirements

### Requirement: The dpni cfg-drift decision is consumed by the planner
The planner SHALL judge dpni cfg drift through the typed
`drift_disposition` surface (`families::dpni`): `plan_present` SHALL
consume `ObservedDpni.cfg_observation` and plan destroy + create on any
cfg divergence, never repair (ADR-0001 §4). The unsized-create sizing
seam SHALL be resolved so a port-only dpni created without an explicit
`num_queues` does not read back as drifted: either the effective queue
count derives core-side from `Inventory.cpus` (retiring the shim
fallback and amending its recorded contract in the same commit) or
unsized cfgs are fenced out of drift comparison by construct — the
landed choice is recorded in the task. The fake backend SHALL project
`Some(DpniObservation::project(cfg))` at create so fake-vs-reconcile
tests cover cfg drift. (Review synthesis rows 1-3.)

#### Scenario: Cfg drift plans destroy-and-create in production
- **WHEN** an observed dpni's read-back projection differs from the
  desired cfg's projection on any compared field
- **THEN** `plan_present` emits the destroy + create disposition through
  `drift_disposition`, not the legacy attribute map

#### Scenario: An unsized port-only dpni does not false-drift
- **WHEN** a dpni created through the port-only path (no explicit
  `num_queues`) is re-observed
- **THEN** the plan reports convergence, not a permanent
  destroy-then-create loop
