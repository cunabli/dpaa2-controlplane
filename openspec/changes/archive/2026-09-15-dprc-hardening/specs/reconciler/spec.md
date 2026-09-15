# reconciler delta: dprc-hardening

## MODIFIED Requirements

### Requirement: Destroy planning encodes the eviction law
Teardown plans SHALL encode ADR-0007 §3: destroying a container destroys the
residents it created and evicts assigned-in residents unplugged into the
parent; a non-empty destroy is therefore plannable and its post-state is
predicted, not discovered. Prune classification and teardown planning SHALL
consult the observed container state before emitting any step: a Locked
candidate yields a typed permission gap (`Attribution::LockGate`) and an empty
step list — a step the model refuses is never emitted (the module's own
doomed-step doctrine) — and move-out planning refuses inactive faces exactly
as its sibling planners do (review M1: PASS2-F3/F4).

#### Scenario: Non-empty scratch container teardown
- **WHEN** a plan destroys a container holding one created and one assigned-in resident
- **THEN** the predicted post-state has the created resident absent and the assigned-in resident present, unplugged, in the parent — and re-observation confirms it

#### Scenario: Locked orphan plans a gap, not a doomed destroy
- **WHEN** `--prune` classifies a foreign-labelled container whose observed state is Locked
- **THEN** the candidate carries a LockGate gap and no unplug/destroy steps; the render names the gap

### Requirement: Containment refusals are discriminated, not collapsed
Planning and drift reporting SHALL distinguish the three board-verified
option-bit refusal shapes — Configuration error (0x6, SPAWN absent), No
resources (0x8, ALLOC absent), No privilege (0x4, topology/lock faces) — and
SHALL NOT treat "No privilege" as the only shape of a permission refusal.
The 0x4 face SHALL be discriminated further by the refused verb: `attribute_mc`
SHALL take the refused verb, so a 0x4 on a create/destroy/assign verb under a
lock is attributed to the lock (`Attribution::LockGate`), not to a topology
permission gap, matching the model's per-action guards (review M7: PASS2-F2).
Attribution lives in `dpaa2-api`, including the error-to-attribution map the
shell previously carried (PASS3-F6).

#### Scenario: ALLOC-less child create failure is reported as a permission gap, not exhaustion
- **WHEN** a create inside a child fails with No resources and the child's options lack ALLOC_ALLOWED
- **THEN** the report attributes the refusal to the option mask, not to pool exhaustion

#### Scenario: Lock-strip create refusal names the lock
- **WHEN** a create inside a locked child is refused No privilege under the child DEFAULT mask
- **THEN** the report attributes the lock, not `PermissionGap{TopologyChanges}`

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
and prune success is judged by re-observation only (DPRC-I6). Prune dispatch
SHALL record a typed outcome per candidate (`PruneOutcome::Refused{id,
attribution}` on refusal, with MC status 0x10 mapped to the typed
plugged-resident teardown refusal), SHALL NOT abort the pass on one
candidate's refusal, and SHALL always run the re-observation naming survivors
(review M1: PASS3-F4/F5).

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

#### Scenario: One refusing candidate does not hide the others
- **WHEN** candidate one destroys and candidate two is refused 0x10
- **THEN** both outcomes are reported per id, the refusal is discriminated (not a fatal error), and the re-observation names the survivor

## ADDED Requirements

### Requirement: The resident census is family-qualified
Observed residents SHALL be keyed by family-qualified object reference,
never by bare id — `dpbp.0` and `dpmcp.0` are distinct residents — so the
predicted post-state and the unplug plan count every resident (review
M1: PASS3-F14). The lifecycle typestate's `ResidentId` (the ADR-0014
model twin) is unchanged.

#### Scenario: Same-id residents of two families both survive the census
- **WHEN** a child holds `dpbp.0` plugged and `dpmcp.0` unplugged
- **THEN** the observation carries two residents and teardown plans an `UnplugResident` for the plugged one
