# formal-models Specification

## Purpose
Make the baseline's object model executable: a Quint corpus under
`models/` whose core runs the five relationship views of
`docs/baseline/object-model.md` as one generic object machine, with the
reconciler's dpni↔dpmac flow as its first instantiation, the baseline's
invariant candidates encoded as named Quint invariants, and a CI ladder
(typecheck → simulate → ITF replay → Apalache on marked invariants) so a
wrong belief about MC behavior fails a check before it becomes Rust.

## Requirements
### Requirement: The core object-lifecycle model executes the baseline's relationship views
The repository SHALL contain a Quint model corpus under `models/` whose
core makes the five views of `docs/baseline/object-model.md` executable
as one generic object machine with per-family parameters: containment
(the DPRC tree), connect edges, create-vs-allocate, allocation pools,
and lifecycle ordering.

#### Scenario: A lifecycle trace runs in the simulator
- **WHEN** the Quint simulator runs the core model
- **THEN** it produces traces that create pool companions, create a
  consumer object, plug, connect, and tear down in the baseline's
  canonical order, and refuses transitions the baseline forbids (e.g.
  assigning a plugged object across containers)

#### Scenario: Per-family parameters instantiate the core
- **WHEN** a family module (e.g. dprtc, dpdbg) instantiates the core
- **THEN** the family's placement gates, cardinality limits, pool
  membership, and reset-on-bind class constrain the shared machine
  without a family-specific fork of the lifecycle logic

### Requirement: Invariant candidates are encoded under their baseline identifiers
The model corpus SHALL encode the baseline's invariant candidates as
named Quint invariants or temporal properties whose names are the
baseline identifiers (e.g. `DPRC-I6`, `DPNI-I2`), with the
**Breaking:** candidates preserved faithfully — the model MUST NOT
contain the assumption each Breaking candidate prohibits.

#### Scenario: A Breaking candidate rejects the convenient assumption
- **WHEN** the model state includes an object whose debug configuration
  was written (DPDBG-I2) or whose destroy returned exit 0 in a child
  container (DPMAC-I8)
- **THEN** no model observable exposes the written debug state as
  readable, and no invariant derives destruction from the exit status

#### Scenario: Encoding order follows touched families
- **WHEN** invariant encoding is sequenced
- **THEN** families already exercised by prior work (dprc, dpni, dpmac,
  dpbp, dpio, dpcon, dpmcp, dpseci) are encoded before Tier B and
  Tier C, so the recorded fallback truncates rather than reworks

### Requirement: A retro-model of the reconciler's dpni-dpmac flow instantiates the core
The corpus SHALL contain a retro-model of the already-validated
reconciler behavior — the dpni↔dpmac association flow — built as an
instantiation of the core model, and its traces SHALL replay against
the existing Rust reconciler.

#### Scenario: Known ground replays against the code
- **WHEN** a frozen trace of the retro-model runs through the ITF
  replayer in `cargo test`
- **THEN** the reconciler's observed decisions match the model's
  expected states, with no board attached

### Requirement: The coverage ledger accounts for every invariant candidate
The corpus SHALL include `models/COVERAGE.md` with one row per baseline
invariant candidate recording its disposition: modeled (with model
location and CI rung), deferred to a named roadmap change, or
board-pending with the traffic-inventory scenario that settles it.

#### Scenario: No candidate is silently dropped
- **WHEN** the ledger is checked against the family documents'
  invariant-candidate sections
- **THEN** every candidate identifier appears exactly once with a
  disposition, and every board-pending row names its settling scenario

#### Scenario: Board results fold back into the ledger
- **WHEN** an operator-run suite settles a board-pending candidate
- **THEN** the same change updates the ledger row and, on divergence,
  amends the model and the owning baseline document together

### Requirement: The model CI ladder runs cheapest-first without board access
Model checks SHALL run as a ladder — Quint typecheck, then simulator
runs over all named invariants, then ITF replay of frozen traces
against the Rust core, then Apalache on invariants explicitly marked
for symbolic checking — wired as pnpm scripts and a CI job. Board
replay SHALL NOT be part of CI.

#### Scenario: The ladder gates on the cheapest failure
- **WHEN** a model fails typecheck or a simulator invariant
- **THEN** the ladder stops there and later rungs do not run

#### Scenario: Only marked invariants reach Apalache
- **WHEN** the Apalache rung runs
- **THEN** it checks exactly the invariants marked in each model's
  header, and any escalation to TLA+ is recorded in that header and
  the owning change

