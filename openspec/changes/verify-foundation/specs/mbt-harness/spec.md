## ADDED Requirements

### Requirement: One shared adapter binds model, restool, and observed state
The `dpaa2-verify` crate SHALL provide a single adapter mapping each
model action to its restool command and each model observable to board
state obtained by read-back. Exit status SHALL NOT be used as an
observation of resulting state.

#### Scenario: Observation is read-back, not exit status
- **WHEN** a suite step's command returns exit 0 but the read-back
  differs from the model's expected state (or returns nonzero with the
  expected state present)
- **THEN** the diff reports the read-back result as the observation and
  the exit status only as auxiliary evidence

### Requirement: Batch suites are reviewable scripts with expectations inline
The batch-suite generator SHALL consume Quint simulator traces and emit
scripts in which every restool command is visible, the model's expected
outcome sits as a comment beside each step, the pinned reference pair
(MC 10.39.0 + Linux 6.6.52) is asserted before any action, and results
are captured to files for offline diffing.

#### Scenario: A generated suite is auditable before it runs
- **WHEN** the operator reviews an emitted suite
- **THEN** every board-touching command and its expected post-state are
  readable in the file, and the script refuses to proceed on a
  reference-pair mismatch

#### Scenario: Suites execute serially and return files
- **WHEN** the operator runs the board program
- **THEN** suites run one at a time in their declared order and each
  produces a result file the harness diffs offline against the model's
  expected states

### Requirement: The port safety envelope is enforced at generation and execution
The harness SHALL encode the ADR-0003 port matrix and traffic classes
as data, refuse to emit or execute any step whose declared traffic
class exceeds what its named ports allow, and make dpmac.3,
dpmac.17, and dpni.0 unreferenceable in any scenario class.

#### Scenario: A forbidden port never reaches a script
- **WHEN** a trace or scenario names dpmac.3, dpmac.17, or dpni.0
- **THEN** generation fails with the violation named, and the execution
  wrapper independently refuses such a step if one appears in a script

#### Scenario: Class must match ports
- **WHEN** a trace declared object-lifecycle-only names dpmac.7 or
  dpmac.9, or an unflagged run declares link-signaling or
  traffic-bearing class
- **THEN** the harness refuses it

### Requirement: Mutating suites are gated on the verified recovery guarantee
The harness SHALL treat the recovery guarantee — a reboot restores the
DPL baseline — as unverified until a recovery-verification run has
passed, and SHALL refuse to emit mutating suites while it is
unverified. The recovery verification itself SHALL mutate only a
scratch-DPRC set that the reboot is expected to erase.

#### Scenario: Recovery check runs first
- **WHEN** the board program begins
- **THEN** the first suite captures pre-state, applies the scratch
  mutation set, has the operator reboot, and diffs post-boot state
  against the DPL baseline; only a clean diff marks the guarantee
  verified

#### Scenario: Root-container teardown stays gated
- **WHEN** a scenario destroys a root-container resident (dprtc.0)
- **THEN** it is emitted only after the recovery guarantee is verified
  and runs under per-step operator confirmation

### Requirement: The online driver is operator-launched and supervised
The crate SHALL provide an online-MBT driver the operator starts, with
step, pause, and abort controls, a full transcript of every action and
observation, and per-step confirmation while a family is in learning
mode; promotion to per-block execution follows a family's model
surviving a complete batch suite and is recorded in the owning change.

#### Scenario: Per-step confirmation in learning mode
- **WHEN** the driver runs a family that has not survived a full batch
  suite, or any root-container-resident scenario regardless of
  promotion
- **THEN** each action requires explicit operator confirmation before
  it executes, and the transcript records the action, the observation,
  and the model's expectation

### Requirement: Frozen traces replay against the Rust core in cargo test
Every board-discovered behavior SHALL be frozen into a committed ITF
trace, and the crate SHALL replay committed traces against the Rust
core as part of `cargo test`, requiring no board.

#### Scenario: A discovery becomes a permanent regression test
- **WHEN** an online or batch run reveals behavior that amends the
  model
- **THEN** the amending change commits a frozen trace reproducing it,
  and the ITF replay rung fails thereafter if the Rust core stops
  conforming to the amended model
