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

### Requirement: The fake backend carries the MC-global counter beside the census

The fake MC SHALL carry a second counter — the MC-global pool budget —
beside the census counter, create-refusal driven by the global counter
and the census counter independent (ADR-0011). The offline sim SHALL
stage the rev-3 blindness and assert it CLOSED once the global row
joins the face. The existing single-counter sim SHALL remain as the
census-visible lane.

#### Scenario: The rev-3 blindness is staged and asserted closed

- **WHEN** the fake MC drains the global bp counter while the census
  counter still shows headroom and the global row is in the face
- **THEN** the sim asserts admission refuses on the global row — the
  hardware-side drain is no longer invisible, the rev-3 blindness closed

#### Scenario: The census-visible lane is unchanged

- **WHEN** the single-counter sim drains the census counter
- **THEN** admission refuses on the census row exactly as before — the
  second counter adds a lane, it does not alter the census one

### Requirement: The board program witnesses rollback under kernel probing

The board program SHALL carry a rollback row inside the safety
envelope: a run is driven into mid-run overcommit on a live board
while the kernel probes (the DPSW-I9 race class), and the witness
asserts the unwind completes, residue is reported typed, and the
follow-on admission refuses. The rev-4 row SHALL gate on the MC-global
bp row and grep the refusal in the global-budget voice. The row SHALL
follow the suite conventions (reviewable script, expectations inline,
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

#### Scenario: The re-run refuses in the global-budget voice

- **WHEN** the rollback row re-runs admission while the drained
  MC-global bp pool still holds (the step-1 gate having read the global
  bp row with counts derived at runtime from the dry-run global row,
  never hardcoded)
- **THEN** the run refuses at planning time in the global-budget voice
  and dispatches nothing — the step-4 grep lands on the global row, not
  the census one
