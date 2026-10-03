# formal-models delta: cross-dprc-links

## ADDED Requirements

### Requirement: The link lifecycle model consumes the connection surface
`models/families/link_lifecycle.qnt` SHALL model the dpni↔dpni wire by
consuming the `models/core/connect.qnt` operators (`legalPair`,
`canConnect`, `connected`, `edgeDemandsSeveredWitness`) rather than
re-deriving them (design D7): the wire typestates (ends-exist →
connected → disconnected → end-destroy-legal), the D4 populate → connect
→ bind partial order with post-bind connect of visible endpoints legal,
and both D5 container-interplay obligations with their consented
discharge. Invariants SHALL be Apalache-marked where the state space
permits. The structural-isomorphism law (ADR-0002) SHALL bind the model
sums to the Rust connection surface.

#### Scenario: Model gate is green before Rust lands
- **WHEN** the CI ladder runs typecheck, simulate, and marked-Apalache on
  `link_lifecycle.qnt`
- **THEN** all pass with the invariants named, before dependent Rust
  merges

#### Scenario: The wire typestates reuse the connect operators
- **WHEN** `link_lifecycle.qnt` guards a connect or disconnect
- **THEN** it calls the `core/connect.qnt` operators rather than
  re-deriving pair legality or cardinality, and a dpni↔dpni edge demands
  no severed witness

### Requirement: The LINK-I* invariants hold in the model
`link_lifecycle.qnt` SHALL carry the named invariants **LINK-I\*** (design
D7): disconnect-before-destroy (a destroy of a connected endpoint is
unreachable), no post-bind create without a planned discharge,
cardinality-one / disconnect-before-reconnect (**DPRC-I5 promoted from
candidate**), and MC-refused patterns surfacing as refusal states. The
state-expressible invariants SHALL carry Apalache marks; action-guard or
Breaking-absence faces ride directed simulate-only runs, matching the
model header split.

#### Scenario: Disconnect-before-destroy is unreachable to violate
- **WHEN** Apalache checks the disconnect-before-destroy invariant
- **THEN** no reachable state destroys an endpoint whose link edge is
  still connected

#### Scenario: A double connect is refused
- **WHEN** the simulator drives a connect on an already-connected
  endpoint
- **THEN** the connect action is disabled and the DPRC-I5 cardinality
  invariant stays green

#### Scenario: A refused pattern surfaces as a refusal state
- **WHEN** a modeled connect resolves to an MC-refused pattern
- **THEN** the trace records a typed refusal state, not a silent drop

### Requirement: The container-interplay obligations are modeled with the consented discharge
`link_lifecycle.qnt` SHALL model both ADR-0017 drift obligations (design
D5/D7): the eager `DeferredVisibility` obligation attached to a post-bind
create (representable only with a planned discharge) and the lazy
stale-node residue of a post-bind destroy. The only modeled discharge
SHALL be the consented `Disruptive` rebind cycle (unbind → bind →
re-observe); a declined consent SHALL leave typed standing residue; no
reboot SHALL appear on this surface.

#### Scenario: Post-bind create attaches the obligation and discharges only on consent
- **WHEN** the simulator creates an object into a bound container
- **THEN** the state carries the eager `DeferredVisibility` obligation, a
  consented rebind cycle discharges it to visible, and a declined consent
  leaves it as standing residue

#### Scenario: Post-bind destroy leaves a stale node that blocks nothing
- **WHEN** the simulator destroys an object in a bound container
- **THEN** the next state carries a stale node as residue that gates no
  convergence verdict and demands no discharge

### Requirement: The V-TRAF-1 scenario module renders the suite
`link_lifecycle.qnt` SHALL carry a `V-TRAF-1` scenario module rendering
the board suite's faces (design D10): root↔root converge with the frame
witness and saturation smoke, cross-container populate→connect→bind with
the post-bind connect dmesg law, child↔child generality, the create-side
heal (obligation → consented rebind → resident), the destroy-mirror
unknown, and the teardown laws (disconnect-live, disconnect-before-
destroy, the DPRC-I5 double-connect refusal).

#### Scenario: The scenario module renders the suite faces
- **WHEN** the scenario module runs under the ladder
- **THEN** it renders the V-TRAF-1 faces as directed runs, each citing
  the law it witnesses

## MODIFIED Requirements

### Requirement: The coverage ledger accounts for every invariant candidate
The corpus SHALL include `models/COVERAGE.md` with one row per baseline
invariant candidate recording its disposition: modeled (with model
location and CI rung), deferred to a named roadmap change, or
board-pending with the traffic-inventory scenario that settles it. This
change SHALL add rows for the LINK-I\* invariants and DPCON-I3 and the
dpni `single_sender` knob (baseline unknown #7) as modeled in this
change, record DPRC-I5's promotion from candidate to modeled invariant,
and re-point the three portal-dependent rows it hands to tile #10 —
DPCON-I4 and dpni baseline unknowns #4 and #11 — to `mc-portal-backend`
(#10) with the blocking fence stated in each row (design D9). The ledger
lint SHALL stay green.

#### Scenario: No candidate is silently dropped
- **WHEN** the ledger is checked against the family documents'
  invariant-candidate sections
- **THEN** every candidate identifier appears exactly once with a
  disposition, and every board-pending row names its settling scenario

#### Scenario: Board results fold back into the ledger
- **WHEN** an operator-run suite settles a board-pending candidate
- **THEN** the same change updates the ledger row and, on divergence,
  amends the model and the owning baseline document together

#### Scenario: The link rows and the #10 re-pointings are accounted
- **WHEN** the COVERAGE rows are read after this change
- **THEN** the LINK-I\* rows, DPCON-I3, and the `single_sender` knob read
  modeled in this change, DPRC-I5 reads modeled (promoted from
  candidate), and DPCON-I4 and dpni unknowns #4 and #11 re-anchor to
  `mc-portal-backend` (#10) with each blocking fence stated
