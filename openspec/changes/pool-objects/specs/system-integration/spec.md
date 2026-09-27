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
constraints. At root scope drift heals upward only — root pool
capacity is grow-only at runtime — and teardown returns the DPL
baseline modulo the typed reboot-required residue (ADR-0020).

#### Scenario: Fresh board converges to both tenants
- **WHEN** the intent declaring one kernel tenant and one userspace
  tenant is applied to the reference baseline
- **THEN** dprc.1 gains the live kernel interface, dprc.N exists
  populated and VFIO-bound, and a second run emits an empty plan

#### Scenario: Drift heals to the intent
- **WHEN** pool objects are added or removed out of band (within the
  free set) and the tool re-runs
- **THEN** deficit recreated; undeclared never-plugged surplus
  pruned; plugged surplus reported as the typed reboot-required
  residue, never destroyed (ADR-0020); drawn and DPL-born objects
  untouched

#### Scenario: Teardown returns the baseline
- **WHEN** the tenants are removed from the intent and the tool
  re-runs
- **THEN** every consumer (the kernel dpni, the populated child
  dprc) and the child's residents are destroyed; runtime-created
  root pool capacity that was plugged — every family, not only the
  grow-only dpio seats — is reported as the typed reboot-required
  residue, observed vs. required stated and the reconciliation path
  named (ADR-0020, widening ADR-0003 §7), and that residue is the
  only census delta against the DPL baseline, which the closing
  reboot restores
