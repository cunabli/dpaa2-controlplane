## ADDED Requirements

### Requirement: CLI exposes reconcile, status, and dry-run
The `dpaa2-tools` binary SHALL provide subcommands to reconcile the system toward
the desired topology, to report status, and to preview actions without applying
them. The dry-run output SHALL be the exact plan that reconcile would execute.

#### Scenario: Dry-run applies nothing
- **WHEN** the operator runs the dry-run subcommand
- **THEN** the planned transitions are printed and no MC or kernel state changes

#### Scenario: Status exposes observed state and delta
- **WHEN** the operator runs the status subcommand
- **THEN** it prints each managed object's lifecycle state and the delta from
  desired, and exits non-zero if the system has diverged from desired

### Requirement: The imperative shell converges asynchronously
The shell SHALL drive an observe → reconcile → act → wait → re-observe loop until
the system converges or a deadline is reached, accounting for the asynchronous
appearance of netdevs after connection/binding. A single invocation SHALL run to
completion.

#### Scenario: Waits for netdev after connect
- **WHEN** a connect transition is applied and the netdev has not yet appeared
- **THEN** the shell re-observes until the netdev appears or the deadline elapses,
  rather than reporting premature success

#### Scenario: Deadline reached without convergence
- **WHEN** the deadline elapses before convergence
- **THEN** the shell exits non-zero and reports which objects did not converge

### Requirement: Runs are idempotent and retry-safe
Re-running the CLI against an already-converged system SHALL make no changes and
SHALL succeed. A run interrupted after partial progress SHALL, on re-run, complete
correctly by re-observing actual state.

#### Scenario: Second run is a no-op
- **WHEN** reconcile is run twice against an unchanged system
- **THEN** the second run applies no transitions and exits zero

### Requirement: Structured, debuggable logging
The CLI SHALL emit structured logs describing observed state, planned transitions,
and applied actions, sufficient to diagnose divergence between desired and actual
state.

#### Scenario: Applied actions are logged
- **WHEN** reconcile applies transitions
- **THEN** each applied transition is logged with its target object and outcome
