# formal-models delta: vocabulary-v2

## MODIFIED Requirements

### Requirement: The intent model corpus runs under the ladder with paired configs
The corpus SHALL contain `models/intent/` — the vocabulary as types,
the derivation as pure definitions, every rule as a named invariant
citing its evidence anchor, and a `scenarios/` directory in which every
`<name>.qnt` sits beside a `<name>.toml` expressing the same intent as
an operator would type it. The model is the specification (ADR-0002):
the Rust vocabulary SHALL be structurally isomorphic to the model's
sums, case-for-case and payload-for-payload — never an option, sentinel,
or flag encoding that merely produces the same traces. Concretely: the
`restricted` isolation carries its pool holder as a variant payload in
both, every tenant reference (a port's tenant, each link end) is the
same shared two-case sum (the reserved kernel, or a named tenant) in
both, and the `TenantAbsent` payload
identifies the referencing site as the same typed sum in both. A shape
the model states as a sum SHALL NOT compile to Rust as anything but the
matching enum. The isomorphism binds type structure and relationship
semantics, not spelling: case and field names on both sides SHALL
converge on the most readable English for the taxonomy, renaming an
incumbent model spelling in the same lockstep commit when it reads
poorly, never transliterating it into Rust. The intent corpus SHALL
run under the same
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
  the finite intent alphabet, the alphabet drawing tenant isolation —
  restricted shapes with their pool payloads included — so the
  private-VLAN shape and every pool-holder refusal are exercised

#### Scenario: The refusal alphabet stays bijective with the Rust surface
- **WHEN** the ledger lints (R11/R14) run after the vocabulary revision
- **THEN** the model's refusal alphabet, the Rust `REFUSAL_VARIANTS`
  list, and the regenerated witness corpus agree — the two deleted pool
  variants absent, the three parity variants present and witnessed
