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
