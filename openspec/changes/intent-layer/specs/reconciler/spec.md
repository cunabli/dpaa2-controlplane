## MODIFIED Requirements

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

## ADDED Requirements

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
  rename pass cross-binds them, the plan holds exactly two renames with
  nothing created or removed, and a second match of the same intent
  consumes no rename

#### Scenario: Indistinguishable leftovers refuse by name
- **WHEN** two unanchored board objects of one family have drifted labels
  and different configs and the intent still names two constructs
- **THEN** the relation refuses `Ambiguous` for that family and the
  converge changes nothing

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
