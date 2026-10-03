# system-integration — dpseci-typestate delta

## ADDED Requirements

### Requirement: One crypto intent converges end to end on the board

The system SHALL witness the full path in one sitting: a `[[crypto]]`
declaration compiles, `dpaa2ctl` converges it into a child-container
dpseci with the derived cfg (queues = flows, uniform priority 2,
`HAS_CG`), the typed surface is read back through both transports
(restool `info` and the raw GET_ATTR read), the VFIO RemoteOwned
arrangement is read back across the container boundary, and the typed
teardown leaves a clean census. The sitting SHALL also bank the
read-only boot-object answers: the reported dpseci API version
(baseline unknown #2) and whether the kernel dpseci carries `HAS_CG`
(unknown #3) — each amending `docs/baseline/dpseci.md` in-change.

#### Scenario: The safety bit is witnessed, not trusted

- **WHEN** convergence reports the dpseci created
- **THEN** the suite's GET_ATTR hook independently shows `HAS_CG` in
  the live object's options mask, and the verdict row records the
  observable, not the writer's intent

#### Scenario: Divergence triage is implementation-first

- **WHEN** any sitting observation contradicts the model or baseline
- **THEN** triage suspects the implementation, then the board's base
  state, and amends the characterization only after both are ruled out
  with evidence, in-change
