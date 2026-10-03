# reconciler delta: cross-dprc-links

## ADDED Requirements

### Requirement: The connection surface is an edge-kind table with per-kind reification
The `dpaa2-api::plan` surface SHALL carry an edge-kind table mirroring
`models/core/connect.qnt` — the legal-pair inventory (`legalPair`) plus a
per-kind **reification policy** keyed on the edge's two families (the
ADR-0019 edge facet). The dpni↔dpmac (port) kind's reification SHALL be
the delivered machinery — the existing transitions, executor, and
`SeveredProof` minting with its severed-witness teardown — *claimed*
where it lives and keyed as it is, not retyped or rewritten (design D3).
The dpni↔dpni kind's reification SHALL be ancestor-issued connect with a
disconnect-only teardown: no driver handback exists for it by
construction, so `edgeDemandsSeveredWitness` stays false and no severed
witness is demanded. MC fixes edge arity at two (multi-port endpoints are
address syntax, `<object>.<id>.<port>`), so the table's shape is fixed at
two families per kind.

#### Scenario: The port-edge kind claims the delivered dpmac machinery
- **WHEN** the edge-kind table resolves the reification policy for a
  dpni↔dpmac edge
- **THEN** it yields the delivered sever-then-unbind machinery unchanged,
  and the dpmac family's frozen ITF traces replay green with no diff

#### Scenario: The dpni-dpni kind demands no severed witness
- **WHEN** a dpni↔dpni edge is torn down
- **THEN** the reification policy is disconnect-only, no `SeveredProof`
  is minted or required, and `edgeDemandsSeveredWitness` is false for the
  kind

### Requirement: dpni↔dpni links are representable in every container arrangement
The connection surface SHALL make a dpni↔dpni link representable
container-agnostically — root↔root, root↔child, and child↔child all
representable — with the connect issued at a common ancestor of both
endpoints (the root constant today, `CONNECT_ANCESTOR`; design D2). The
surface SHALL NOT pre-forbid any container arrangement in the Rust types
except the loopback intent already refuses (`LinkSelfLoop`,
programmatic-parity); a pattern the MC refuses SHALL surface as a typed
refusal, never a silent drop or an untyped backend error (ADR-0013
§link).

#### Scenario: A cross-container link plans connect at the ancestor
- **WHEN** desired declares a link whose two dpni endpoints sit in
  different containers
- **THEN** the plan carries a Connect issued at their common ancestor,
  and the same arrangement with both endpoints in the root is equally
  representable

#### Scenario: An MC-refused pattern surfaces as a typed refusal
- **WHEN** a representable link resolves to a pattern the MC refuses
- **THEN** the refusal is surfaced as a typed value naming the refused
  pattern, not swallowed and not collapsed to a backend string

### Requirement: The container connection face follows the populate→connect→bind partial order
On the child-container typestate the connect face SHALL live on the
unplugged container beside create/assign, so a fresh convergence orders
**populate → connect → bind** (ADR-0017 decision 3 extended to the
connection face, design D4). Connect and disconnect of endpoints already
visible after bind SHALL additionally be legal — a post-bind connect
manufactures no bus device — and the plan SHALL represent it, recording
the kernel-end `ENDPOINT_CHANGED` → `-EPERM`-discarded dmesg law as the
board face's expectation.

#### Scenario: Fresh convergence orders populate before connect before bind
- **WHEN** intent places two child-resident dpnis and links them
- **THEN** the plan populates and connects the endpoints while the
  container is unplugged, then binds, and a bind-then-populate ordering
  is unrepresentable

#### Scenario: Post-bind connect of visible endpoints is legal
- **WHEN** two endpoints are already visible in a bound container and a
  link between them is declared
- **THEN** the plan carries a legal post-bind Connect for them and no
  create precedes it

### Requirement: Post-bind create carries an eager DeferredVisibility obligation and post-bind destroy leaves lazy residue
A create into an already-bound container SHALL be representable **only**
with an eager `DeferredVisibility` obligation attached (the object is
MC-accepted but kernel-invisible, so convergence is judged by
re-observation after a scan, never by the create's acceptance alone;
ADR-0017 healing policy, design D5). The destroy side SHALL be the lazy
mirror: a post-bind destroy may leave a stale node that blocks nothing
and may stand indefinitely as typed residue.

#### Scenario: Post-bind create without the obligation does not construct
- **WHEN** a library consumer attempts to express a create into a bound
  container without attaching the `DeferredVisibility` obligation
- **THEN** the construction is unrepresentable — no post-bind create
  exists on the surface absent its eager obligation

#### Scenario: Post-bind destroy leaves a stale node as residue
- **WHEN** a plan destroys an object in a bound container
- **THEN** the predicted post-state carries the stale node as typed
  residue that blocks no convergence verdict and demands no discharge

### Requirement: The DeferredVisibility obligation discharges only through a consented Disruptive rebind cycle
The only modeled discharge of a standing `DeferredVisibility` obligation
SHALL be a **consented rebind cycle** (unbind → bind → re-observe),
represented as a first-class `Disruptive`-class plan transition riding
the ADR-0015 consent machinery (design D5). A declined consent SHALL
report a typed standing residue (the pool-objects honest-residue idiom)
and rebind SHALL never fire silently. No reboot SHALL exist on this
surface — reboot residue stays exclusive to ADR-0020 pool shrink.

#### Scenario: Consent discharges the obligation through a rebind
- **WHEN** a standing `DeferredVisibility` obligation is discharged under
  explicit `Disruptive` consent
- **THEN** the plan carries the unbind → bind → re-observe cycle and
  re-observation after the scan judges the object visible

#### Scenario: Declined consent reports typed residue, never a silent rebind
- **WHEN** the rebind cycle's consent is declined
- **THEN** the obligation is reported as typed standing residue, no
  unbind/bind step is emitted, and nothing rebinds silently

### Requirement: Link teardown obeys disconnect-before-destroy and disconnect-before-reconnect
The planner SHALL encode two link plan laws (DPRC-I5 promoted from
candidate, design D5/D7): destroy of a linked endpoint is reachable only
from a disconnected endpoint (disconnect-before-destroy, finding 34
generalized), and an endpoint has at most one peer so a reconnect
requires a prior disconnect (cardinality-one / disconnect-before-
reconnect). The planner consumes these as types/plan laws; it does not
re-derive the ordering inline.

#### Scenario: Destroy of a connected endpoint is unrepresentable
- **WHEN** a plan would destroy a dpni whose link edge is still connected
- **THEN** the planner refuses at type/plan level — the destroy is
  reachable only after a Disconnect — and no MC destroy is emitted

#### Scenario: Reconnect without disconnect is refused
- **WHEN** a plan would connect an already-connected endpoint to a new
  peer
- **THEN** the planner refuses (DPRC-I5), requiring a Disconnect before
  the new Connect
