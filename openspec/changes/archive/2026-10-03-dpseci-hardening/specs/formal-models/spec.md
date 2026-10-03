# formal-models — dpseci-hardening delta

## ADDED Requirements

### Requirement: The whole-census-poisoning law is stated in the model

The dpseci model SHALL state the census-poisoning law — any unobservable
custody row makes the dpseci face judge nothing — as a pure predicate
with a directed run, twinning the Rust guard in `populate.rs` so the
convergence-affecting law no longer lives Rust-only (ADR-0002,
quint-is-the-spec). The change SHALL record whether the dpni and pool
families share the same idiom gap.

#### Scenario: One unobservable member silences the family judgment

- **WHEN** a census carries one unobservable custody row among observable
  dpseci rows
- **THEN** the model's dpseci face judges nothing for the family and the
  directed run witnesses it

### Requirement: The census and destroy surface is replay-anchored or its sufficiency is recorded

The dpseci freeze tooling SHALL either freeze census/destroy ITF traces
with a dpaa2-verify replay arm matching the create surface's oracle, or
record in COVERAGE why hand-mirrored twins suffice for the pure-operator
surface. One of the two outcomes MUST land; silence is not an option.

#### Scenario: The census surface gains an oracle or a reason

- **WHEN** the freeze run over the census/destroy surface completes
- **THEN** either the frozen trace count grows and the replay arm is
  green, or the recorded sufficiency note is in COVERAGE

## MODIFIED Requirements

None.
