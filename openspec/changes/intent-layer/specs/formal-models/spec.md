## ADDED Requirements

### Requirement: The intent model corpus runs under the ladder with paired configs
The corpus SHALL contain `models/intent/` — the vocabulary as types,
the derivation as pure definitions, every rule as a named invariant
citing its evidence anchor, and a `scenarios/` directory in which every
`<name>.qnt` sits beside a `<name>.toml` expressing the same intent as
an operator would type it. The intent corpus SHALL run under the same
CI ladder as the core corpus, and `dpaa2-verify` SHALL hold each pair
equivalent: the TOML parses and compiles to the plan the scenario's
frozen ITF trace carries.

#### Scenario: A scenario pair is equivalent
- **WHEN** `cargo test` runs the pairing test for `router.toml`
- **THEN** the compiled plan equals the plan in `router.itf.json`
  object-for-object, with no board attached

#### Scenario: An unpaired scenario fails the ladder
- **WHEN** a `<name>.qnt` exists under `scenarios/` with no
  `<name>.toml` beside it
- **THEN** the ladder's typecheck rung fails naming the missing file

#### Scenario: Derivation rules are marked for Apalache
- **WHEN** the Apalache rung runs on the intent corpus
- **THEN** it checks the feasibility, companion-count, and
  isolated-container invariants marked in `models/intent/` headers over
  the finite intent alphabet, the alphabet drawing tenant isolation and
  pool so the private-VLAN shape and every pool refusal are exercised
