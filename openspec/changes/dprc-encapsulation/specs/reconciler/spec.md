# reconciler delta: dprc-encapsulation

## ADDED Requirements

### Requirement: Child DPRC lifecycle is typestated
The object graph SHALL carry the child-DPRC family with lifecycle typestates —
create → populate-while-unplugged → plug → lock/unlock → empty → destroy — such
that an invalid container transition is unrepresentable in the Rust types, and
the state sum is structurally isomorphic to the Quint lifecycle sum (ADR-0002
structural-isomorphism law).

#### Scenario: Plugged objects cannot be planned into a move
- **WHEN** a plan would relocate an object whose observed state is plugged
- **THEN** the planner refuses at type/plan level (unplug precedes move, DPRC-I3); no MC command is emitted for the move

#### Scenario: Population is representable only while unplugged
- **WHEN** intent places a resident into a child container
- **THEN** the plan orders assign-while-unplugged before any plug transition, and a plug-then-assign ordering is unrepresentable

### Requirement: Containment refusals are discriminated, not collapsed
Planning and drift reporting SHALL distinguish the three board-verified
option-bit refusal shapes — Configuration error (0x6, SPAWN absent), No
resources (0x8, ALLOC absent), No privilege (0x4, topology/lock faces) — and
SHALL NOT treat "No privilege" as the only shape of a permission refusal.

#### Scenario: ALLOC-less child create failure is reported as a permission gap, not exhaustion
- **WHEN** a create inside a child fails with No resources and the child's options lack ALLOC_ALLOWED
- **THEN** the report attributes the refusal to the option mask, not to pool exhaustion

### Requirement: Destroy planning encodes the eviction law
Teardown plans SHALL encode ADR-0007 §3: destroying a container destroys the
residents it created and evicts assigned-in residents unplugged into the
parent; a non-empty destroy is therefore plannable and its post-state is
predicted, not discovered.

#### Scenario: Non-empty scratch container teardown
- **WHEN** a plan destroys a container holding one created and one assigned-in resident
- **THEN** the predicted post-state has the created resident absent and the assigned-in resident present, unplugged, in the parent — and re-observation confirms it

### Requirement: Mutation visibility is established only by re-observation
The reconciler SHALL NOT treat a bus rescan (`sync`) as establishing visibility
of any mutation (DPRC-I6): child-container residents are root-invisible at
runtime, and convergence verdicts come from re-querying the affected container.

#### Scenario: Converged verdict after child mutation
- **WHEN** a plan step mutates a child container's membership
- **THEN** the step's success is judged by re-observing that container via MC queries, never by issuing sync

### Requirement: Consumer convergence is container-only in this change
Converging a declared consumer SHALL produce the container itself — existence,
options, label, placement, lock state, VFIO bindability — and SHALL NOT emit
companion-set sizing (tile #6) or dpni option surface (tile #5).

#### Scenario: Consumer declared on an empty board
- **WHEN** intent declares one consumer and the board lacks its container
- **THEN** the plan creates exactly the child DPRC with derived options/label/placement and contains no companion-population steps
