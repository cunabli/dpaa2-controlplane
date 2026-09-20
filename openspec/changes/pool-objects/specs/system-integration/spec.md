# system-integration delta — pool-objects

## ADDED Requirements

### Requirement: One intent file converges the MVP setup end to end
From a single declarative intent file, the tool SHALL converge the
board to the first MVP workable setup: a kernel tenant's dpni in
dprc.1 with its regime-derived pool companions, connected to a
permitted dpmac and kernel-attached as a live network interface; and a
userspace tenant's child dprc.N populated with a dpni and its derived
companions and VFIO-bound, consumable by any userspace dataplane. The
convergence SHALL be idempotent and level-triggered with no persisted
state, and a re-run over the converged board SHALL be a no-op.
Sustained traffic is out of scope (roadmap #9); liveness means
kernel-attached and link-connected within the dpmac safety
constraints.

#### Scenario: Fresh board converges to both tenants
- **WHEN** the intent declaring one kernel tenant and one userspace
  tenant is applied to the reference baseline
- **THEN** dprc.1 gains the live kernel interface, dprc.N exists
  populated and VFIO-bound, and a second run emits an empty plan

#### Scenario: Drift heals to the intent
- **WHEN** pool objects are added or removed out of band (within the
  free set) and the tool re-runs
- **THEN** the board returns to the derived counts — surplus pruned,
  deficit recreated — without touching drawn objects or DPL-born
  objects

#### Scenario: Teardown returns the baseline
- **WHEN** the tenants are removed from the intent and the tool
  re-runs
- **THEN** every runtime-created object of the four families and their
  consumers is destroyed and the board census matches the DPL baseline
