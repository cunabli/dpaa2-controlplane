# formal-models — delta

## ADDED Requirements

### Requirement: Asserting surfaces bind the production guard's operand
Every operand a suite, replay, or COVERAGE mark asserts SHALL be the
same named accessor the production guard consumes, cited by name; and
an `itf-replay` COVERAGE mark SHALL be earned by a per-state assert of
the named law — transition-level inference does not earn the mark.
Concretely for the pool corpus: the below-draw refusal assert binds
`drawn_managed()` (the netted base the guard consumes), and the pool
replay's `check_state` asserts POOL_CUSTODY (drawn disjoint unplugged)
and POOL_DPL_SURVIVES (born-present) in every state of every trace
(review synthesis L7/L8: PASS2-F5/F6; fences the V-POOL-6 rev 1–3 and
3.14-audit defect class).

#### Scenario: A trace with a born draw still encodes the post-D9 predicate
- **WHEN** a frozen pool trace carries `born_drawn > 0` and the below-draw refusal replays
- **THEN** the assert consumes `drawn_managed()` and agrees with the production guard's judgment

#### Scenario: The custody and survival marks are executed per state
- **WHEN** any frozen pool trace replays
- **THEN** every state is checked for drawn disjoint unplugged and for the presence of DPL-born members, and a violating state fails the suite

### Requirement: The frozen dpio corpus covers the DpdkSeat regime
The frozen dpio trace corpus SHALL carry at least one directed
DpdkSeat-regime run, replayed green, so the regime's guard arms are
witnessed beyond unit tests (review synthesis L25: PASS2-F7). The
freeze remains idempotent: re-running the pool and dpio freeze scripts
against the committed corpus SHALL produce no diff.

#### Scenario: A DpdkSeat run freezes and replays
- **WHEN** the dpio replay suite runs over the committed corpus
- **THEN** at least one trace exercises the DpdkSeat regime and replays green

#### Scenario: Re-freeze is a no-op
- **WHEN** `pnpm model:freeze-pool && pnpm model:freeze-dpio` runs at the sealed commit
- **THEN** `git diff --exit-code models/traces` reports no change
