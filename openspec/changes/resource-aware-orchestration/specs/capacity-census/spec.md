# capacity-census — resource-aware-orchestration delta

## ADDED Requirements

### Requirement: The census carries a typed capacity face

The census SHALL expose a typed projection of remaining budgets,
re-derived from the same observations the pool passes already parse on
every loop and never ledgered between loops (ADR-0008). The initial
rows are the two board-proven budgets only: per-container free pools
(dpmcp/dpbp/dpio/dpcon) and the dpsw FDB budget (the V-DPSW-2 bracket).
A budget row joins only with the change that proves its observation on
the board. Known non-reclaimable budgets (dpio seats, ADR-0003 §7)
render as headroom already spent, never as reclaimable.

#### Scenario: Budgets are re-minted from observation

- **WHEN** two consecutive loops observe the board
- **THEN** each loop's capacity face derives from that loop's
  observation alone, with no carried allocation state

### Requirement: Admission judges the gross draw before dispatch

The derived draw of an intent SHALL be computed as a pure function of
the compiled plan — creates counted, planned frees not counted (design
D8) — and compared against the capacity face before any verb is
dispatched. An intent whose gross draw exceeds observed headroom in any
budget is refused whole, before dispatch, with the refusal naming the
budget, the draw, and the headroom as data (the established refusal
voice; ADR-0013 vocabulary). Admission holds no reservation; dispatch
truth stays re-observation (design D7).

#### Scenario: An over-budget intent is refused naming the shortfall

- **WHEN** an intent's derived draw on a container pool exceeds that
  pool's observed free count
- **THEN** the run refuses before any verb, naming the budget, the
  draw, and the headroom

#### Scenario: A rebuild against an exhausted budget takes two runs

- **WHEN** an intent plans destroy-and-recreate for an immutable-drift
  object and the object's budget has no free headroom
- **THEN** the run is refused (gross draw does not count the planned
  free), the teardown run frees, and the next run admits

#### Scenario: Refusals are stable

- **WHEN** an intent is refused for capacity and neither the census nor
  the intent changes
- **THEN** every subsequent run refuses identically — admission never
  oscillates
