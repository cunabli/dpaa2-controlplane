# Spec Delta

## ADDED Requirements

### Requirement: The fake-MC budget simulation drives the overcommit path offline

The offline harness SHALL simulate a mid-run budget drain against the
fake MC: admission passes, the fake backend's budget is drained before
a create verb, and the run's policy outcome (rollback unwind or abort
refusal) is asserted against the journal's contents — observed ids,
reverse order, residue typing — with no board required.

#### Scenario: A simulated drain triggers a full unwind

- **WHEN** the fake MC drains a budget between admission and a create
  under the rollback policy
- **THEN** the harness asserts every journaled actuation is undone in
  reverse order and the fake board state equals the pre-run snapshot
  up to typed residue

#### Scenario: The abort policy is asserted side-effect-free of undo

- **WHEN** the same drain fires under the abort policy
- **THEN** the harness asserts no undo verb reached the fake MC and
  the refusal names the executed prefix

### Requirement: The board program witnesses rollback under kernel probing

The board program SHALL carry a rollback row inside the safety
envelope: a run is driven into mid-run overcommit on a live board
while the kernel side is actively probing (the DPSW-I9 race class),
and the witness asserts the unwind completes, residue is reported
typed, and the follow-on admission refuses. The row SHALL follow the
established suite conventions (reviewable script, expectations inline,
envelope-gated).

#### Scenario: Rollback converges under a live kernel race

- **WHEN** the board row drains a budget mid-run while kernel probing
  holds the race window open
- **THEN** the unwind completes without wedging an object mid-reversal
  and the final observed census matches the pre-run census up to the
  row's declared residue

#### Scenario: The refusal is the board's own verdict

- **WHEN** the rollback row completes
- **THEN** the recorded verdict re-derives from observed board state,
  not from the driver's expectations alone
