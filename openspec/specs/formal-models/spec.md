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

### Requirement: The dprc model carries the container lifecycle and its laws
`models/families/dprc.qnt` SHALL model the child-DPRC lifecycle as a sum type
(the Rust typestates' isomorphic twin), with the board-settled containment laws
as guarded transitions: the per-option-bit permission matrix with its three
distinct refusal statuses, the eviction law (ADR-0007 §3), the visibility law
(DPRC-I6), and the plugged-move precondition (DPRC-I3). Invariants DPRC-I1,
I5, I7, I9, I10 and the remaining face of I11 SHALL have named, simulate-green
properties, and COVERAGE.md dispositions updated. Apalache marks cover only the
state-expressible subset — DPRC-I10 and DPRC-I12, carried in the `stateInvariants`
conjunction; DPRC-I1, I5, I7, I9 and the I11 remainder are action-guard, liveness
or Breaking-absence properties carried as directed simulate-only runs (review
PASS4-F3, matching the `dprc.qnt` header split and COVERAGE.md dispositions).

#### Scenario: Model gate green before Rust
- **WHEN** the model changes land
- **THEN** typecheck, simulate, and marked-Apalache runs are green with the invariants named, before dependent Rust merges

#### Scenario: Refusal statuses are distinguishable in traces
- **WHEN** a simulated action violates SPAWN, ALLOC, or a topology/lock gate
- **THEN** the trace records the matching distinct refusal (0x6, 0x8, 0x4 respectively), not a single generic denial

#### Scenario: Eviction law is a transition, not an error
- **WHEN** a modeled destroy hits a non-empty container
- **THEN** the next state removes created residents and re-parents assigned-in residents unplugged, and DPRC-I9 (teardown reachability) still holds

### Requirement: Every reachable guard arm has a witness
The frozen-trace corpus SHALL witness the accepted `moveResidentOutAt`
and `unplugResidentAt` transitions, the lock-strip refusal sweep (the
five `Container<Locked>` refusing faces without a trace today), the
`Unlocked::Empty` arm, and the three DPRC-I12 directed runs (states
only; ghost fields stay model-side) — existing runs frozen, no new model
surface, the D8 fingerprint untouched (review M9: PASS2-F7/F8/F9).

#### Scenario: The replay corpus is self-accounting
- **WHEN** the trace inventory test runs
- **THEN** every committed trace is listed, every listed trace replays green, and the corpus holds 17+ traces

### Requirement: The replay asserts the refusal vocabulary, not itself
The ITF replay SHALL map each `Attribution` arm to its expected MC
status and assert the observed status against that map — a mutated
attribution function fails the suite — and a phase/variant mismatch in
the refusal check is a loud finding, never a silent skip (review M6:
PASS2-F1).

#### Scenario: A wrong attribution fails replay
- **WHEN** `attribute_mc` is mutated to return one fixed arm
- **THEN** `cargo test -p dpaa2-verify --test dprc_replay` fails

### Requirement: The dpni family model carries the create-option surface with named invariants
`models/families/dpni.qnt` SHALL grow the create-option surface — the
twelve live options with their ranges, the typed flag vocabulary with the
raw-mask escape, and the two consumer profiles — as model state and
actions, with named invariants covering at minimum: create-range refusal
(no accepted create outside the verified envelope), profile totality
(every `Dataplane` + interface construct maps to exactly one option set),
parity of unrepresentable options (dead options and `num_rx_tcs` never
appear in an accepted create), and the write-only field law
(`dist_key_size` never participates in observation). Invariants SHALL be
Apalache-marked per the DoD model gate, keeping the structural-isomorphism
law (ADR-0002) between the Quint sums and the Rust typestates.

#### Scenario: Model gate runs green before Rust lands
- **WHEN** the CI ladder runs typecheck, simulate, and Apalache on the
  marked dpni invariants
- **THEN** all pass before the corresponding Rust typestates merge

#### Scenario: Profile derivation is total in the model
- **WHEN** the simulator explores intents over every dataplane and
  interface construct combination
- **THEN** every reachable create action carries exactly one derived
  option set and no action carries an operator-supplied option

