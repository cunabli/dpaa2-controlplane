# reconciler delta: dprc-hardening

## MODIFIED Requirements

### Requirement: Prune planning honors the observed container state
Prune classification and teardown planning SHALL consult the observed
container state: a Locked candidate yields a typed permission gap
(`Attribution::LockGate`) and an empty step list — a step the model
refuses is never emitted (the module's own doomed-step doctrine) — and
move-out planning refuses inactive faces exactly as its sibling planners
do (review M1: PASS2-F3/F4).

#### Scenario: Locked orphan plans a gap, not a doomed destroy
- **WHEN** `--prune` classifies a foreign-labelled container whose observed state is Locked
- **THEN** the candidate carries a LockGate gap and no unplug/destroy steps; the render names the gap

### Requirement: Prune outcomes are typed per candidate and re-observed
Prune dispatch SHALL record a typed outcome per candidate
(`PruneOutcome::Refused{id, attribution}` on refusal, with MC status 0x10
mapped to the typed plugged-resident teardown refusal), SHALL NOT abort
the pass on one candidate's refusal, and SHALL always run the
re-observation naming survivors (review M1: PASS3-F4/F5).

#### Scenario: One refusing candidate does not hide the others
- **WHEN** candidate one destroys and candidate two is refused 0x10
- **THEN** both outcomes are reported per id, the refusal is discriminated (not a fatal error), and the re-observation names the survivor

### Requirement: The resident census is family-qualified
Observed residents SHALL be keyed by family-qualified object reference,
never by bare id — `dpbp.0` and `dpmcp.0` are distinct residents — so the
predicted post-state and the unplug plan count every resident (review
M1: PASS3-F14). The lifecycle typestate's `ResidentId` (the ADR-0014
model twin) is unchanged.

#### Scenario: Same-id residents of two families both survive the census
- **WHEN** a child holds `dpbp.0` plugged and `dpmcp.0` unplugged
- **THEN** the observation carries two residents and teardown plans an `UnplugResident` for the plugged one

### Requirement: Refusal attribution is verb-aware
`attribute_mc` SHALL take the refused verb: a 0x4 on a create/destroy/
assign verb under a lock is attributed to the lock, not to a topology
permission gap, matching the model's per-action guards (review M7:
PASS2-F2). Attribution lives in `dpaa2-api`, including the
error-to-attribution map the shell previously carried (PASS3-F6).

#### Scenario: Lock-strip create refusal names the lock
- **WHEN** a create inside a locked child is refused No privilege under the child DEFAULT mask
- **THEN** the report attributes the lock, not `PermissionGap{TopologyChanges}`
