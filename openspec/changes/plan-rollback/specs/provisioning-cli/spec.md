# Spec Delta

## ADDED Requirements

### Requirement: The execution surface carries the mid-run failure policy

The CLI's execution entry point SHALL accept a per-run mid-run failure
policy at planning time — `rollback` (default) and `abort` — rendered
as one flag whose value set grows with the policy enum. The dry-run
output SHALL state the run's selected policy so the operator previews
the failure disposition with the plan.

#### Scenario: The operator opts out of rollback

- **WHEN** the operator invokes execution with the abort policy
- **THEN** a mid-run failure stops the run and the executed prefix is
  reported, not unwound

#### Scenario: Dry-run previews the disposition

- **WHEN** the operator previews a plan
- **THEN** the output names the failure policy the run would execute
  under

### Requirement: Rollback outcomes are rendered in operator data terms

A rolled-back run's report SHALL name the failure class (overcommit
distinct from headroom refusal), the actuations unwound (observed
names/ids), and each typed residue with its reboot-named remedy — in
the operator's data vocabulary, never internal type names.

#### Scenario: An overcommit report is actionable

- **WHEN** a run rolls back on overcommit
- **THEN** the report names the drained budget in census terms, lists
  the unwound actuations, and states that re-apply refuses until
  headroom returns

#### Scenario: Residue names its remedy

- **WHEN** an unwind crosses an irreversible actuation
- **THEN** the report names the residue and that a reboot reclaims it,
  never a silent omission

### Requirement: The admission table renders the MC-global budget rows

The admission table SHALL render the MC-global pool budget rows beside
the per-container census rows in the same draw/headroom/verdict shape,
carried in a distinguishable family-row voice the suite can grep
(ADR-0011; ADR-0023 Consequences). When an MC-global row refuses a
pending create, the ensure refusal voice SHALL name the global budget,
so the operator tells a global-ledger refusal from a census one at
planning time.

#### Scenario: Global and census rows read side by side

- **WHEN** the operator previews a plan whose capacity face carries the
  MC-global bp row
- **THEN** the table renders the global bp row beside the census rows in
  the same shape, in a distinguishable voice the suite can grep

#### Scenario: The refusal names the global budget

- **WHEN** a pending create refuses because the MC-global row of its
  family is exhausted though the census row shows headroom
- **THEN** the refusal voice names the global budget, never collapsing
  it into a census headroom refusal
