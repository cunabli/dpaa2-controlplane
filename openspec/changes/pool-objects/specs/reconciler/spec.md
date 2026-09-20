# reconciler delta — pool-objects

## ADDED Requirements

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
deficit; destroys of free individuals only for a surplus, selected
arbitrarily; a typed refusal when the requirement falls below the
currently drawn count; and prune destroys for objects that are
undeclared in intent, not DPL-born, and free. The count→individual
boundary SHALL sit at the dispatch edge: the disposition speaks deltas,
the adapter resolves deltas to concrete object ids.

#### Scenario: Surplus shrinks through free individuals only
- **WHEN** the census shows 3 dpbp against a derived requirement of 2
  and one dpbp is drawn
- **THEN** the disposition emits one destroy resolvable only to a free
  dpbp and the drawn individual is never a candidate

#### Scenario: Requirement below draw refuses
- **WHEN** the derived dpcon requirement is 4 and 5 dpcons are
  currently drawn
- **THEN** the disposition returns a typed refusal naming the family
  and counts, and emits no destroy

#### Scenario: Convergence is idempotent
- **WHEN** the disposition runs twice over an unchanged converged
  observation
- **THEN** the second run emits an empty plan
