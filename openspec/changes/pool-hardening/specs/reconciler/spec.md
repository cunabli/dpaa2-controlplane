# reconciler — delta

## ADDED Requirements

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
