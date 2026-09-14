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

### Requirement: Undeclared consumer containers are pruned under the double gate
The reconciler SHALL classify every child container observed under the root
against the declared consumer set by ownership fingerprint — non-empty MC
label plus derived default option mask plus root placement (`dpaa2ctl` always
labels the containers it creates; bare restool creates do not). A container
matching no declared consumer is a prune candidate when the fingerprint
matches fully OR partially (any matched subset that includes a non-empty
label); prune candidates are destroyed only when `--prune` AND
`--allow disruptive` are both given — the existing `--prune` flag widens to
containers, with no new flag surface. Containers with an empty label or zero
fingerprint overlap are report-only and SHALL never be touched (ADR-0001 §4).
Every candidate is rendered in dry-run with its matched and unmatched
fingerprint fields and the eviction-law predicted post-state (ADR-0007 §3),
and prune success is judged by re-observation only (DPRC-I6).

#### Scenario: Fully fingerprinted orphan is pruned under the double gate
- **WHEN** the root holds a labeled container matching a derived fingerprint on all fields but no declared consumer, and the run passes `--prune --allow disruptive`
- **THEN** the plan destroys it via the eviction-law teardown path and the verdict comes from re-observing the root's children

#### Scenario: Prune candidate without the disruptive gate is planned but not dispatched
- **WHEN** the same orphan is observed and the run passes `--prune` without `--allow disruptive`
- **THEN** the candidate is reported with its fingerprint fields and predicted post-state, and no destroy is dispatched

#### Scenario: Partial fingerprint match is rendered with matched and unmatched fields
- **WHEN** a non-empty-label container matches the derived mask but sits outside root placement
- **THEN** dry-run renders it as a partial prune candidate naming which fingerprint fields matched and which did not, alongside the eviction-law predicted post-state

#### Scenario: Empty-label container is report-only
- **WHEN** the root holds an unlabeled container (bare restool create) absent from intent
- **THEN** it is reported as unmanaged and no plan step targets it, regardless of flags

#### Scenario: Label-voided managed container is report-only until re-labeled
- **WHEN** a container the tool created has its label emptied out-of-band (set-label accepts the empty string even under lock, V-DPRC-3 — the accepted DPRC-I12 escape)
- **THEN** it is reported as unmanaged and never pruned; re-labeling it re-enters the fingerprint buckets, and the next prune pass under the double gate handles it

### Requirement: Consumer convergence is container-only in this change
Converging a declared consumer SHALL produce the container itself — existence,
options, label, placement, lock state, VFIO bindability — and SHALL NOT emit
companion-set sizing (tile #6) or dpni option surface (tile #5).

#### Scenario: Consumer declared on an empty board
- **WHEN** intent declares one consumer and the board lacks its container
- **THEN** the plan creates exactly the child DPRC with derived options/label/placement and contains no companion-population steps
