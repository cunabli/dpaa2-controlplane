# Spec Delta

## ADDED Requirements

### Requirement: The wire plan judges both observed ends
The wire planner SHALL consume the observed connection state of **both**
endpoints, not only the a-side, and the engine's held pre-pass SHALL read
both ends before planning. An endpoint held by a foreign peer — on either
side — SHALL yield the typed held refusal; a Connect SHALL never be
planned over an end whose observed peer is not the declared one.

#### Scenario: A foreign-held b end refuses, offline and in the faces
- **WHEN** the declared link's b endpoint is observed connected to a
  foreign peer
- **THEN** the plan carries the typed held refusal (rewire-refused/held),
  no Connect is emitted, and the same refusal surfaces through the
  engine's link faces

#### Scenario: The a-side hold keeps refusing
- **WHEN** the declared link's a endpoint is observed held by a foreign
  peer
- **THEN** the typed held refusal is planned exactly as before the
  widening

### Requirement: The contract fake obeys the board's symmetric-edge law
The contract fake SHALL reproduce the V-LINK-6 rev 1 board transcript
verbatim: disconnect SHALL drop the connection mirror of **both** ends,
and destroy of a still-connected end SHALL remove the edge atomically
with the endpoint — the survivor's connection read answers none (the
board's "endpoint: No object associated", state −1) — and SHALL remove
the destroyed object from pool observation. A fake that is merely
symmetric but disagrees with the transcript is wrong.

#### Scenario: The survivor of a destroy reads no connection
- **WHEN** a connected dpni↔dpni pair exists in the fake and one end is
  destroyed without a prior disconnect
- **THEN** the peer's connection read returns none and the destroyed id
  is absent from the dpni pool observation

#### Scenario: Disconnect severs both mirrors
- **WHEN** a connected pair is disconnected through either end
- **THEN** both ends' subsequent connection reads return none

## MODIFIED Requirements

### Requirement: Post-bind create carries an eager DeferredVisibility obligation and post-bind destroy leaves lazy residue
A create into an already-bound container SHALL be representable **only**
with an eager `DeferredVisibility` obligation attached (the object is
MC-accepted but kernel-invisible, so convergence is judged by
re-observation after a scan, never by the create's acceptance alone;
ADR-0017 healing policy, design D5). The destroy side SHALL be the lazy
mirror: a post-bind destroy may leave a stale node that blocks nothing
and may stand indefinitely as typed residue. The stale-node residue
SHALL be minted only on proof of bus visibility — the destroy face
demands the same visible-endpoint witness its sibling faces demand, so
destroying a populated-but-invisible end mints no residue, matching the
model.

#### Scenario: Post-bind create without the obligation does not construct
- **WHEN** a library consumer attempts to express a create into a bound
  container without attaching the `DeferredVisibility` obligation
- **THEN** the construction is unrepresentable — no post-bind create
  exists on the surface absent its eager obligation

#### Scenario: Post-bind destroy leaves a stale node as residue
- **WHEN** a plan destroys an object in a bound container
- **THEN** the predicted post-state carries the stale node as typed
  residue that blocks no convergence verdict and demands no discharge

#### Scenario: An invisible end's destroy mints no residue
- **WHEN** a destroy is expressed for an end without the bus-visibility
  witness
- **THEN** the stale-mint face is not reachable — the surface demands
  the visible-endpoint witness before the residue can be minted

### Requirement: The DeferredVisibility obligation discharges only through a consented Disruptive rebind cycle
A standing `DeferredVisibility` obligation SHALL discharge only through a
**consented rebind cycle** (unbind → bind → re-observe), represented as a
first-class `Disruptive`-class plan transition riding the ADR-0015
consent machinery (design D5). A declined consent SHALL
report a typed standing residue (the pool-objects honest-residue idiom)
and rebind SHALL never fire silently. No reboot SHALL exist on this
surface — reboot residue stays exclusive to ADR-0020 pool shrink. The
no-silent-rebind law SHALL be type law on every twin: the obligation
bundle exposes no public constructor or field through which a rebind
cycle can be minted outside the consent gate, and the unforgeability is
proven by a compile-time witness, not a runtime test.

#### Scenario: Consent discharges the obligation through a rebind
- **WHEN** a standing `DeferredVisibility` obligation is discharged under
  explicit `Disruptive` consent
- **THEN** the plan carries the unbind → bind → re-observe cycle and
  re-observation after the scan judges the object visible

#### Scenario: Declined consent reports typed residue, never a silent rebind
- **WHEN** the rebind cycle's consent is declined
- **THEN** the obligation is reported as typed standing residue, no
  unbind/bind step is emitted, and nothing rebinds silently

#### Scenario: A forged discharge fails to compile
- **WHEN** a library consumer attempts to construct the rebind cycle
  directly from the obligation bundle's parts, bypassing the consent
  gate
- **THEN** the construction fails to compile — the compile-fail witness
  in the crate's doctests pins the law
