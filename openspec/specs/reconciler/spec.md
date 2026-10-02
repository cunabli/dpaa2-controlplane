# reconciler Specification

## Purpose
Define the backend-neutral domain model of MC objects and the pure
`reconcile(desired, observed) -> Plan` engine that computes the transitions to
converge observed MC state toward operator intent.
## Requirements
### Requirement: Topology is modeled as an object graph with lifecycle state
The `dpaa2-api` crate SHALL model the topology as a graph of typed MC
objects, each carrying a provisioning lifecycle state (at minimum:
Absent, Created, Connected, Bound), plus connection edges and container
memberships. The desired side of the graph SHALL be the compiled plan:
every derived object with its provenance — produced by `compile` from
an `Intent` and an `Inventory`, or built programmatically through the
plan's public witness-taking constructors; `reconcile` SHALL NOT depend
on `Intent`. The model SHALL
be backend- and frontend-neutral (no ioctl, no serde) and SHALL admit
every object family the plan derives (dprc, dpni, dpmac, dpio, dpbp,
dpcon, dpmcp, dpseci, dpsw) without redefinition.

#### Scenario: Object carries lifecycle state
- **WHEN** observed state is constructed for a managed DPNI connected
  to its DPMAC but not yet bound
- **THEN** the object's lifecycle state is Connected

#### Scenario: A plan built by hand reconciles
- **WHEN** a library user builds a `DesiredTopology` through the plan
  constructors without an `Intent`
- **THEN** `reconcile` accepts it and the relationship locks still hold

#### Scenario: Desired is a compiled plan
- **WHEN** a `DesiredTopology` value exists
- **THEN** every object in it carries the rule, construct, and evidence
  anchor that derived it

### Requirement: Reconciliation is a pure function
The core SHALL expose `reconcile(desired, observed) -> Plan` as a pure function that
performs no I/O and is deterministic for a given input. The plan SHALL be an ordered
list of transitions (Create, Connect, Bind, Reconfigure, Disconnect, Unbind,
Destroy) sufficient to move observed toward desired.

#### Scenario: Absent port yields create-then-connect
- **WHEN** desired declares a port whose DPMAC has no connected DPNI in observed
- **THEN** the plan contains a Create followed by a Connect for that port

#### Scenario: Converged state yields empty plan
- **WHEN** observed already satisfies desired
- **THEN** the plan is empty (idempotence)

### Requirement: Level-triggered, hardware-authoritative matching
Reconciliation SHALL treat observed MC state as authoritative and SHALL NOT rely on
any persisted association, marker file, or in-memory memory of prior runs. Managed
DPNIs SHALL be matched to desired ports by their connection edge to the configured
DPMAC, not by index.

#### Scenario: Renumbered DPNI still matches
- **WHEN** a managed DPNI appears at a different index after reboot but is connected
  to the same configured DPMAC
- **THEN** reconcile matches it to the same desired port and plans no change

### Requirement: Ownership is limited to the configured subgraph
Reconciliation SHALL only plan changes to objects reachable from a DPMAC named in
the desired topology. It SHALL NOT enumerate all MC objects and delete those absent
from desired, EXCEPT for child DPRC containers, which this change makes prunable by
their label-anchored ownership fingerprint (review PASS4-F1): an undeclared child
container carrying a non-empty MC label plus a matching derived option mask and root
placement is a prune candidate under the double gate (`--prune` AND
`--allow disruptive`), per the ADDED "Undeclared consumer containers are pruned
under the double gate" requirement. The fence survives unchanged for every
non-container object, for containers with an empty label (bare restool creates), and
for zero fingerprint overlap: objects outside the configured subgraph (e.g.
DPL-provisioned or foreign objects) that are not label-anchored prunable containers
SHALL be left untouched.

#### Scenario: Foreign object preserved
- **WHEN** the MC contains a DPNI connected to a DPMAC not present in desired
- **THEN** the plan contains no operation affecting that object

#### Scenario: Teardown is opt-in
- **WHEN** a previously-configured port is removed from desired and prune is not
  enabled
- **THEN** the plan does not destroy the corresponding DPNI

#### Scenario: Empty-label container survives the ownership fence
- **WHEN** the root holds an unlabeled child container (bare restool create) absent from intent
- **THEN** the ownership fence holds and no plan step targets it, regardless of flags

### Requirement: The dpni cfg-drift decision is consumed by the planner
The planner SHALL judge dpni cfg drift through the typed
`drift_disposition` surface (`families::dpni`): `plan_present` SHALL
consume `ObservedDpni.cfg_observation` and plan destroy + create on any
cfg divergence, never repair (ADR-0001 §4). The unsized-create sizing
seam SHALL be resolved so a port-only dpni created without an explicit
`num_queues` does not read back as drifted: either the effective queue
count derives core-side from `Inventory.cpus` (retiring the shim
fallback and amending its recorded contract in the same commit) or
unsized cfgs are fenced out of drift comparison by construct — the
landed choice is recorded in the task. The fake backend SHALL project
`Some(DpniObservation::project(cfg))` at create so fake-vs-reconcile
tests cover cfg drift. (Review synthesis rows 1-3.)

#### Scenario: Cfg drift plans destroy-and-create in production
- **WHEN** an observed dpni's read-back projection differs from the
  desired cfg's projection on any compared field
- **THEN** `plan_present` emits the destroy + create disposition through
  `drift_disposition`, not the legacy attribute map

#### Scenario: An unsized port-only dpni does not false-drift
- **WHEN** a dpni created through the port-only path (no explicit
  `num_queues`) is re-observed
- **THEN** the plan reports convergence, not a permanent
  destroy-then-create loop

### Requirement: Assert-only intent is verified, not actuated
Fields declared assert-only (e.g. link speed, board-burned MAC) SHALL be compared
against observed reality and reported on mismatch, and SHALL never produce an
actuating transition.

#### Scenario: Asserted MAC mismatch
- **WHEN** a port's MAC is assert-mode and the live DPNI MAC differs
- **THEN** reconcile reports a mismatch and plans no MAC write

### Requirement: Plan-only objects are reported, not reconciled as drift
`reconcile(desired, observed)` SHALL execute transitions only for the
object families it has executors for, and SHALL report every other
derived object as plan-only — present in the desired plan, awaiting
the change that adds its executor — never as drift, never as an error,
and never as a reason to refuse the executable subset.

#### Scenario: A userspace-poll plan against today's executors
- **WHEN** a compiled plan holds a child DPRC, dpios, dpbps, dpmcps,
  and two dpni↔dpmac ports, and only the dpni↔dpmac executor exists
- **THEN** the plan contains transitions for the two ports and a
  plan-only report listing the remaining objects by family and count

#### Scenario: Plan-only objects do not block convergence
- **WHEN** the executable subset is converged and plan-only objects
  remain
- **THEN** `is_converged` is true and the plan-only report is still
  emitted

### Requirement: Edited intent re-associates to standing objects by the identity ladder
The `dpaa2-api` crate SHALL re-associate an edited intent's compiled
constructs to the objects a prior converge left on the board through a
pure, stateless matching relation (ADR-0015 decisions 9–11), the Rust
twin of `models/intent/match.qnt` run on real construct names and dpmac
anchor sets. The relation SHALL run four stages in order, each binding
only objects the earlier stages left free: (1) an **anchor rung**
binding an anchored construct to the same-family board object with an
equal dpmac anchor set, ignoring label and rename (decision 9), so a
dpmac move binds nothing and becomes create+remove — a rewire, not a
rename; (2) a **config-gated label pass** binding an unanchored
construct to the board object whose label equals its name, EXCEPT a
contested rename source — a board object labelled by some declared
`renamed` `from` whose config disagrees with that exact claimant
(decision 10 rule ii); (3) a **rename pass** binding a construct
carrying `renamed` to the board object labelled by its `from`, recording
the binding as a rename; and (4) a **leftover rung** that, when two or
more same-family unanchored board objects remain with a construct
wanting one and their configs are not all interchangeable, SHALL refuse
by name (`Ambiguous`) rather than guess (decision 11), and otherwise
bind any sound bijection. The verdict SHALL be a plan of pairs,
renames, creates, and removes, or a non-empty refusal set; ties SHALL
break on the least board handle. Config equality SHALL compare only the
attributes that distinguish two same-family constructs (never the name,
an ordinal, or the `renamed` clause), so indiscernible candidates match
soundly and a rename self-neutralizes after one converge.

#### Scenario: Anchor binds across a rename, a dpmac move rewires
- **WHEN** a board dpni anchored on `dpmac.7` is labelled `wan0` and the
  edited intent names the `dpmac.7` port `e0`
- **THEN** the anchor rung binds them with no rename consumed; and when
  the intent instead moves the port to `dpmac.8`, nothing binds — the
  old object is removed and a fresh one created

#### Scenario: A name swap emits two relabels, not a disruptive repair
- **WHEN** the board carries two unanchored dpnis labelled `wan0` and
  `e0` and the edited intent swaps their names, each config travelling
  with its object
- **THEN** the config-gated pass defers both contested sources, the
  rename pass cross-binds them, the match plan holds exactly two
  renames with nothing created or removed, and a second match run once
  those relabels are carried consumes no rename

#### Scenario: Indistinguishable leftovers refuse by name
- **WHEN** two unanchored board objects of one family have drifted labels
  and different configs and the intent still names two constructs
- **THEN** the relation refuses `Ambiguous` for that family and binds
  nothing — no guess reaches a match plan

### Requirement: Every plan transition and match plan carries a disruption class
The `dpaa2-api` crate SHALL class every plan transition into one of three
ordered disruption classes — Hitless, Boundary, Disruptive (ADR-0015
decision 12) — and SHALL expose the plan's headline as the maximum class
over its parts. A `set-label` relabel with no name change and an
attribute assert SHALL be Hitless; a relabel that changes an
externally-held name (the netdev name a consumer holds) SHALL be
Boundary; a create, destroy, disconnect, unbind, or rewire SHALL be
Disruptive. The match plan's headline SHALL judge each rename relabel
from the object's prior label — a name reclaimed elsewhere in the plan
(a swap or name-cycle) staying Hitless because the externally-held name
set is unchanged.

#### Scenario: The headline is the maximum class
- **WHEN** a plan holds a create and an attribute assert
- **THEN** its headline is Disruptive, the create dominating the assert

#### Scenario: A drift repair is hitless, a lone rename is boundary
- **WHEN** the match plan relabels an object whose prior label was unset
  (a drift repair)
- **THEN** that pair is Hitless; and when it relabels an object away from
  a name held nowhere else in the plan, that pair is Boundary

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

### Requirement: Consumer convergence is container-only in this change
Converging a declared consumer SHALL produce the container itself —
existence, options, label, placement, lock state, VFIO bindability — and its
population SHALL converge through the pool passes this change adds: root pool
convergence runs before the port loop and `converge_population` runs after
container convergence (this delta's own ADDED requirements). Companion-set
sizing is therefore emitted by those pool/population passes, not forbidden;
the dpni option surface is tile #5's, delivered. The container-only fence is
retired (review PASS4-F2).

#### Scenario: Consumer declared on an empty board
- **WHEN** intent declares one consumer and the board lacks its container
- **THEN** the plan creates the child DPRC with derived options/label/placement and its companion-population steps follow via `converge_population`


### Requirement: The resident census is family-qualified
Observed residents SHALL be keyed by family-qualified object reference,
never by bare id — `dpbp.0` and `dpmcp.0` are distinct residents — so the
predicted post-state and the unplug plan count every resident (review
M1: PASS3-F14). The lifecycle typestate's `ResidentId` (the ADR-0014
model twin) is unchanged.

#### Scenario: Same-id residents of two families both survive the census
- **WHEN** a child holds `dpbp.0` plugged and `dpmcp.0` unplugged
- **THEN** the observation carries two residents and teardown plans an `UnplugResident` for the plugged one

### Requirement: The dpni create surface is typed and invalid configurations are unrepresentable
The `dpaa2-api` crate SHALL model the dpni family in
`families::dpni` with the validated create block (`dpni_cfg`) as the
immutable private create block of a DPNI behind a shared-ref
accessor: the twelve live create options
(baseline `docs/baseline/dpni.md` option inventory) SHALL carry refined
range types bounded by the restool-verified envelope, and the options
mask SHALL be a typed flag set over the named ten of the 14-flag MC 10.39
vocabulary (the four unnamed flags are a recorded gap with no constructor)
plus a provenance-carrying raw-mask constructor for values outside the named
map (`HAS_REPLICATION` and `0x80000000` PFDR_IN_PEB ride the escape, not the
vocabulary). No `dpni_cfg` field SHALL be mutable
after creation; changing one SHALL plan destroy + create, never repair
(ADR-0001 §4).

#### Scenario: Out-of-range option has no constructor
- **WHEN** code attempts to build a dpni create block with a value
  outside a live option's verified range (e.g. `num_queues` 0 or 33)
- **THEN** the value does not construct — the refusal is a type error or
  a refused fallible constructor, not a board rejection

#### Scenario: Cfg drift plans destroy-and-create
- **WHEN** an observed dpni differs from the desired one on any
  `dpni_cfg` field
- **THEN** the plan refuses repair and reports the drift with the
  destroy + create disposition

### Requirement: Dead options and num_rx_tcs are unrepresentable with parity refusals
The eleven dead restool options (create-time `--mac-addr` and the ten
v9-era `--max-*` flags) and the never-settable `num_rx_tcs` SHALL have
no constructor in the typed surface, and each SHALL be named by a
programmatic refusal so the design-D11 rows stay two-sided
(vocabulary-v2 precedent).

#### Scenario: A dead option is refused by name
- **WHEN** a caller asks the programmatic surface whether a dead option
  is expressible
- **THEN** the answer is a named refusal identifying the option, not a
  silent absence

### Requirement: The runtime surface is state within the DPNI type, limited to the primary MAC
The DPNI typestate SHALL carry its runtime surface as state within the
type parameterized by the immutable create block. In this change the
runtime surface SHALL expose exactly one mutation — the primary MAC —
because it is the only setter the restool transport can drive; the
remaining `dpni_set_*` surface SHALL be a named deferral to the
mc-portal backend and SHALL slot into the same state position without
reshaping the family.

#### Scenario: Primary MAC mutation plans without touching cfg
- **WHEN** desired and observed dpnis differ only in primary MAC
- **THEN** the plan carries a MAC mutation and no destroy + create

### Requirement: dist_key_size is write-only by construct
Because `dpni_attr` omits `dist_key_size`, the reconciler SHALL treat it
as write-only: it participates in create and never in observation
comparison, so it can never produce a drift report.

#### Scenario: dist_key_size never reports drift
- **WHEN** a dpni is observed after creation with any `dist_key_size`
- **THEN** the comparison excludes the field and no drift is reported

### Requirement: Planning re-observes per candidate container
The planning surface SHALL provide an `observe_container(id)` seam
beside the enumerate verb, and per-candidate re-observation SHALL use it
instead of rescanning every root child.

#### Scenario: Re-observation targets one container
- **WHEN** the reconciler re-checks a single affected container during
  an ensure
- **THEN** exactly that container is re-observed through
  `observe_container(id)`

### Requirement: The P3 counted-companion shape is one generic implementation
`dpaa2-api` SHALL implement ADR-0019 P3 for the four pool families as
one generic implementation parameterized by `FamilyParams` for the
allocator trio (dpbp/dpmcp/dpcon) plus a seat-typed dpio variant
(DPIO-I1/I2: regime-typed, never pooled). The surface SHALL expose
census and sizing types and a pure count-drift disposition; it SHALL
NOT mint per-object identity types, phase machinery, or any trait
implemented across ADR-0019 patterns. Macros are admissible only where
they carry semantic/structural sharing a generic cannot (e.g. ADR-0014
linted-enum stamping) and every shape SHALL remain structurally
isomorphic to the Quint model (ADR-0002 §3).

#### Scenario: The trio instantiates one shape
- **WHEN** dpbp, dpmcp, and dpcon are reviewed against the P3 module
- **THEN** each is an instantiation of the same generic implementation
  differing only in `FamilyParams` data, and no family carries a
  bespoke lifecycle

#### Scenario: No cross-pattern framework exists
- **WHEN** the public API of `dpaa2-api` is inspected
- **THEN** no trait or macro couples a P3 family's lifecycle to dprc
  (P1) or dpni (P2) surfaces

### Requirement: The disposition converges counts fully and prunes the undeclared
The pure disposition SHALL judge each (container, family) pair from
observed census versus intent-derived requirement (ADR-0012 counts,
consumed unchanged from the compiled plan) and emit: creates for a
deficit; destroys of free individuals only for a surplus in a child
container, selected arbitrarily; a typed refusal when the requirement
falls below the currently drawn count; and prune destroys for objects
that are undeclared in intent, not DPL-born, and free. At root scope a
surplus emits no destroy and renders as the typed reboot-required
residue disposition (ADR-0020). The count→individual boundary SHALL
sit at the dispatch edge: the disposition speaks deltas, the adapter
resolves deltas to concrete object ids. Prune destroys keep reaching
objects that are undeclared, not DPL-born, and — at root — never
plugged; an undeclared plugged root object is residue (ADR-0020
decision 4).

#### Scenario: Surplus shrinks through free individuals only
- **WHEN** a child container's census shows 3 dpbp against a derived
  requirement of 2 and one dpbp is drawn
- **THEN** the disposition emits one destroy resolvable only to a free
  dpbp and the drawn individual is never a candidate

#### Scenario: Requirement below draw refuses
- **WHEN** the derived dpcon requirement is 4 and 5 dpcons are
  currently drawn in a child container (root drawn-ness is
  unobservable, ADR-0020)
- **THEN** the disposition returns a typed refusal naming the family
  and counts, and emits no destroy

#### Scenario: Root surplus is typed residue
- **WHEN** the root census shows plugged capacity above the derived
  requirement
- **THEN** the disposition emits no destroy and reports the
  reboot-required residue naming observed vs. required and the
  reconciliation path (ADR-0020)

#### Scenario: Convergence is idempotent
- **WHEN** the disposition runs twice over an unchanged converged
  observation
- **THEN** the second run emits an empty plan

### Requirement: Child-scope pool refusals surface typed at both discovery paths
The typed below-draw refusal the pure disposition returns SHALL survive
to the operator-facing surface at child scope on both discovery paths:
the planned path (`converge_population` consuming the disposition's
`ShrinkBelowDraw`) and the probe-discovered path (a destroy dispatch
refused `-EBUSY` revealing a draw the census could not see). Neither
path SHALL collapse the refusal to an untyped configuration or backend
error string — on the restool backend the probe path is the only way a
child below-draw surfaces, so both paths type or neither does (review
synthesis L4: PASS3-F1/F2).

#### Scenario: Planned child shrink below draw reports the typed refusal
- **WHEN** a child's derived requirement falls below its drawn count and convergence runs
- **THEN** the operator-facing report carries the typed refusal naming the family and counts, not a stringly configuration error

#### Scenario: Probe-discovered draw reports the same typed face
- **WHEN** a planned child destroy is refused `-EBUSY` because the individual is drawn
- **THEN** the dispatch surfaces the same typed refusal face as the planned path, and no further destroy of that family is attempted in the pass

### Requirement: Child seat sizing converges through the seat gate and types its surplus
Child dpio convergence SHALL judge seat counts through the pure seat
gate rather than demanding exact equality around a grow-only dispatch:
a seat deficit grows toward the requirement, and a seat SURPLUS in a
bound or unbound child SHALL report the same typed grow-only residue
its root twin reports (ADR-0020), never loop to an untyped
"did not converge" backend error (review synthesis L10: PASS3-F3/F10).

#### Scenario: Child seat surplus reports residue, not divergence
- **WHEN** an unbound child holds more seats than its derived requirement and convergence runs
- **THEN** the pass completes reporting the typed grow-only residue for the seat family, and no error claims non-convergence

#### Scenario: The seat ceiling refusal has a production caller
- **WHEN** a grow would exceed the seat ceiling
- **THEN** the refusal is judged by the pure seat gate consumed by the production dispatch path, not re-derived inline

### Requirement: The dpmac arbitration surface is typestated and observation-judged

`dpaa2-api` SHALL carry `families/dpmac.rs` structurally isomorphic to
the model (ADR-0002): the phase sum `Offered | KernelOwned |
RemoteOwned` with typed transitions, judged purely from read-backs
(connection endpoint plus kernel driver-face observation). The family
SHALL expose no constructor for board objects (P4, DPMAC-I1): values
enter only through observation.

#### Scenario: Arbitration state follows the recorded equation

- **WHEN** observation reads an unconnected dpmac with the standalone
  driver bound
- **THEN** the judged state is `Offered`; a same-container kernel peer
  yields `KernelOwned`; a cross-container peer with the standalone
  driver holding the PHY yields `RemoteOwned` (DPMAC-I6)

### Requirement: The dpni–dpmac edge teardown law holds by construct

The connection surface SHALL type the sever-then-unbind law for the
dpni–dpmac edge kind: `sever` consumes `KernelOwned` and yields
`Offered` plus a severed witness, and the kernel-face unbind of a
dpni whose edge faces a dpmac SHALL require that witness. The demand
SHALL apply to no other edge kind. The planner consumes these types;
it does not own the ordering.

#### Scenario: The illegal order does not typecheck

- **WHEN** a library consumer attempts to express unbind-before-sever
  for a dpmac-connected dpni
- **THEN** the program is rejected at compile time — the sequence that
  strands a driverless port (ADR-0008 §8) is unrepresentable

#### Scenario: Non-dpmac edges are unaffected

- **WHEN** plans tear down dpdmux-, dpci-, or dpni-peered objects
- **THEN** no severed witness is demanded and existing plan behavior is
  unchanged

### Requirement: The port MAC relation is a typed judgment

Observation SHALL classify the relation between a dpmac-connected
dpni's primary MAC and the port as `Inherited` (equals the dpmac's
burned-in address), `Overridden` (equals an intent-declared value),
`Pending` (all-zeros, consumer not yet bound), or `Mismatched`.
`Pending` SHALL NOT classify as drift. `Mismatched` SHALL plan a
mutation only when intent declares a MAC; otherwise it surfaces as an
observation. Intent gains no new field: an absent MAC on a
dpmac-connected port means inherit, and the reconciler's write path is
unchanged from dpni-typestate.

#### Scenario: Bind-timing zeros do not churn

- **WHEN** a dpni is connected to a dpmac but its consumer has not yet
  bound and the primary MAC reads all-zeros
- **THEN** the judgment is `Pending`, no drift action is planned, and a
  later re-observation after bind judges `Inherited` (DPNI-I3 value
  semantics)

### Requirement: Counter reads are vocabulary-typed and never reconcile

Counter observation SHALL return `Known(value)` only for counters in
the firmware version's vocabulary and a typed `NotInVocabulary`
otherwise — a missing counter is unrepresentable as zero (DPMAC-I7).
Counters SHALL NOT participate in reconcile, drift, or convergence
judgments.

#### Scenario: Absence is not zero

- **WHEN** a counter outside the 10.39 vocabulary is requested
- **THEN** the result is `NotInVocabulary`, distinguishable by type
  from a zero reading, and no plan consumes it
