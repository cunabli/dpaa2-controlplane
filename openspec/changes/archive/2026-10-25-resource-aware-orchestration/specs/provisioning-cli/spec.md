# provisioning-cli — resource-aware-orchestration delta

## ADDED Requirements

### Requirement: Dry-run renders admission verdicts beside the plan

`dry-run` SHALL render the admission judgment for each budget the
intent draws on — budget, derived draw, observed headroom, verdict — in
the established data-only voice, and `ensure` SHALL refuse an
over-budget intent with the same vocabulary before any verb is
dispatched. `ensure` SHALL refuse an intent naming an `Unimplemented`
family at dispatch, naming its retirement pointer; `dry-run` renders the
admission face only.

#### Scenario: Dry-run names the draw against headroom

- **WHEN** `dry-run` compiles an intent drawing on a container pool
- **THEN** the output names the pool, the draw, the headroom, and the
  admit-or-refuse verdict alongside the plan

#### Scenario: Ensure refuses before dispatch

- **WHEN** `ensure` is invoked with an intent whose gross draw exceeds
  any observed budget
- **THEN** the run exits refused with the budget, draw, and headroom
  named, and no verb reaches the board
