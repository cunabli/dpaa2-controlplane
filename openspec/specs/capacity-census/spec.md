# capacity-census Specification

## Purpose
Give the census a typed capacity face — remaining budgets re-derived
from each loop's observation, never ledgered — and judge every intent's
gross draw against that face before any verb is dispatched, refusing
over-budget intents whole with the budget, draw, and headroom named as
data.
## Requirements
### Requirement: The census carries a typed capacity face

The census SHALL expose a typed projection of remaining budgets,
re-derived from the same observations the pool passes already parse on
every loop and never ledgered between loops (ADR-0008). The initial
rows are the two board-proven budgets only: dpmcp/dpbp/dpcon free pools
and the dpsw FDB budget (the V-DPSW-2 bracket), with dpio seats as spent
headroom.
A budget row joins only with the change that proves its observation on
the board. Known non-reclaimable budgets (dpio seats, ADR-0003 §7)
render as headroom already spent, never as reclaimable.

#### Scenario: Budgets are re-minted from observation

- **WHEN** two consecutive loops observe the board
- **THEN** each loop's capacity face derives from that loop's
  observation alone, with no carried allocation state

### Requirement: Admission judges the gross draw before dispatch

The derived draw of an intent SHALL be computed as a pure function of
the compiled plan and the observed census — the pending creates the
run would dispatch counted, already-converged objects and planned
frees not counted (design D8) — and compared against the capacity face
before any verb is dispatched. Admission never refuses the fixpoint
its own convergence produced. One admission judgment SHALL derive its
capacity face, its pending counts, and its switch work from one
observation snapshot: two reads of one truth never split a verdict.
An intent whose gross draw exceeds observed headroom in any
budget is refused whole, before dispatch, with the refusal naming the
budget, the draw, and the headroom as data (the established refusal
voice; ADR-0013 vocabulary). Admission holds no reservation; dispatch
truth stays re-observation (design D7).

#### Scenario: An over-budget intent is refused naming the headroom

- **WHEN** an intent's derived draw on a container pool exceeds that
  pool's observed free count
- **THEN** the run refuses before any verb, naming the budget, the
  draw, and the headroom

#### Scenario: A rebuild against an exhausted budget takes two runs

- **WHEN** an intent plans destroy-and-recreate for an immutable-drift
  object and the object's budget has no free headroom
- **THEN** the run is refused (gross draw does not count the planned
  free), the teardown run frees, and the next run admits

#### Scenario: A converged intent re-admits

- **WHEN** every object the intent compiles to is observed converged
  and the drawn budgets show no remaining free headroom
- **THEN** the run admits — the pending draw is zero, and the headroom
  its own convergence consumed is never demanded twice

#### Scenario: Refusals are stable

- **WHEN** an intent is refused for capacity and neither the census nor
  the intent changes
- **THEN** every subsequent run refuses identically — admission never
  oscillates

#### Scenario: One snapshot feeds one judgment

- **WHEN** the board changes between the moments a split judgment
  would have read it twice
- **THEN** the verdict is internally consistent — observed draw and
  pending draw come from the same snapshot, so nothing is
  double-charged against a departed object or admitted against an
  arrival the face never saw

### Requirement: A live free count is headroom, never a population bracket

A family whose ceiling observation is a live free count (dpbp's `bp`
pool row, ADR-0011 decision 1) SHALL be judged by that count alone: the
free units are the remaining headroom, a create plan admits while the
count is above zero, and a growth plan draws at most the observed free
count. Population SHALL be compared only against static brackets
(`Observed` ceilings: the per-boot dpmcp budget, the measured dpni
ceiling).

#### Scenario: Zero free refuses at any population

- **WHEN** a family's observed live free count is zero and the intent
  requires another object of that family
- **THEN** admission refuses the create naming the budget, the draw,
  and the zero headroom, whatever the family's population (the
  V-CEIL-1 law — the board refuses a dpbp create exactly at zero free)

### Requirement: An unsatisfiable draw refuses, never a silent zero-create plan

No plan SHALL silently drop a required draw because population outgrew
the remaining free units: an unsatisfiable draw surfaces as a refusal
naming the budget, the draw, and the headroom (the established refusal
voice), never as a zero-create plan.

#### Scenario: An aged board still plans the draw

- **WHEN** a family's population meets or exceeds its observed live
  free count while the intent requires more objects and free units
  remain
- **THEN** the plan creates up to the observed free count and the
  admission face renders the family's budget rows — the draw never
  vanishes from the dry-run
