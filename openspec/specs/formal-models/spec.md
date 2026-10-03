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

### Requirement: P3 mechanisms are pattern-owned, machines family-owned per the re-amended ADR-0019 Quint module architecture
The model SHALL follow ADR-0019's re-amended Quint module architecture
(2026-09-20, pool-objects phase 1): `models/families/pool_lifecycle.qnt`
(`module pool_lifecycle`) SHALL carry ONLY the mechanisms genuinely
common to P3 — the parameterized custody substrate: the
allocator-custody cycle (create → free-pool membership → draw →
return, with DPBP-I3's dirty return on free) and ceiling-bound refusal
at the census (ADR-0011, DPBP-I7's board-settled floor) — and SHALL
contextualize each member family rather than host its particulars.
Each member file `models/families/{dpmcp,dpbp,dpcon,dpio}.qnt` SHALL
own its stateful modules in the `dprc.qnt`/`dpni.qnt` shape: `module
<f>_lifecycle` (and `module <f>_scenario` when scenario content
exists) — thin for the allocator trio, each instantiating the
pattern substrate contextualized to its family; dpio's standalone
(unpooled, it reuses the shared core transforms rather than the
pooled substrate) and carrying its family-particular depth (seat
record and arithmetic, the probe-time dpmcp draw of
DPIO-I1/DPMCP-I1, the DPIO-I3 surface) — alongside the
family-specific types (dpio's regime-typed seat vocabulary, the
mutable dpcon→dpio notification edge type of DPCON-I4) and the
named-invariant index. Family-particular state records, actions, or
directed runs in the pattern file are a defect in review, as is a
member module that re-derives the shared mechanisms instead of
instantiating them. `models/core/` SHALL gain only corpus-wide census
extensions; `main.qnt` keeps the baseline-id runs. Invariants SHALL be
Apalache-marked per the DoD model gate and the structural-isomorphism
law (ADR-0002 §3) SHALL hold between the Quint sums and the Rust P3
shapes that phase 2 builds against them.

#### Scenario: Model gate runs green before Rust lands
- **WHEN** the CI ladder runs typecheck, simulate, and Apalache on the
  marked pool invariants
- **THEN** all pass before the corresponding Rust P3 surface merges

#### Scenario: Exhaustion refuses at the census, not the board
- **WHEN** the simulator drives creates past a container's family
  ceiling
- **THEN** the create action is disabled by the census predicate and no
  accepted state exceeds the ceiling

### Requirement: Count convergence and prune are modeled as laws
The model SHALL encode the convergence discipline for anonymous
capacity as named invariants: converged means the per-(container,
family) census equals the derived requirement; in a child container a
shrink step destroys only free individuals; an intent below the
current draw disables the shrink action and surfaces a refusal; prune
removes exactly the objects that are undeclared, non-DPL-born, and
free. Victim selection among free individuals SHALL be
nondeterministic in the model — no law distinguishes free individuals
of one family. The model SHALL carry a root-surplus-is-residue law: no
destroy of plugged root capacity is reachable and the surplus renders
as the typed disposition (ADR-0020); prune's reach at root is the
never-plugged gate.

#### Scenario: Convergence is idempotent and level-triggered
- **WHEN** the simulator replays the same derived counts against a
  converged state
- **THEN** no create, destroy, or prune action is enabled

#### Scenario: Shrink below draw is a refusal, not a teardown
- **WHEN** in a child container the derived requirement drops below
  the currently drawn count for a family
- **THEN** no destroy of a drawn individual is reachable and the
  refusal state is the only successor

#### Scenario: DPL-born objects survive prune
- **WHEN** prune runs over a state containing boot-baseline objects
  absent from intent
- **THEN** every DPL-born object remains and every undeclared free
  runtime object is removed

### Requirement: Custody carries the plug facet distinct from draw
The model SHALL split the two board custody observables the census
collapsed (pool-objects design D10; 2026-09-24 audit): *plugged* —
allocatable, in the kernel's pool (DPBP-I2: allocatable ⟺ plugged ∧
allocator-bound) — and *drawn* — held by a consumer. `pool_lifecycle`
SHALL model plug/unplug as transitions distinct from draw/return;
reclaim of a managed individual in a child container SHALL be the
unplug-probe law (ADR-0020 decision 3): an unplug of a drawn
individual is refused and the refusal is the drawn signal; a
plugged-free individual unplugs, then destroys. At root, surplus
renders as the typed residue, never a reclaim; the grow-only dpio
residue SHALL be a typed reboot-required disposition — one instance
of that residue, not a carve-out from it. The ITF twins' observation
mapping SHALL carry the plugged facet explicitly so a Rust census
that infers drawn from plugged fails a frozen twin offline, before
any board sitting.

#### Scenario: Managed surplus reclaims through the probe
- **WHEN** in a child container the derived requirement drops below
  the managed count and the surplus individuals are plugged but not
  drawn
- **THEN** each unplugs and is destroyed, and the census converges to
  the requirement

#### Scenario: The probe refuses on a drawn individual
- **WHEN** a shrink in a child container selects a managed individual
  a consumer holds
- **THEN** the unplug is refused, no destroy of that individual is
  reachable, and the refusal surfaces as the ShrinkBelowDraw face

#### Scenario: Teardown orders consumers before pools
- **WHEN** an empty intent replays over a converged two-tenant state
- **THEN** consumer and container teardown steps precede every pool
  step, the child's census returns to its pre-population state;
  runtime-created plugged root capacity renders as the reported
  reboot-required residue (ADR-0020) and the dpio residue is one
  instance of it, not a failure

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

### Requirement: The dpmac family model carries the P4 offer reference shape

`models/families/dpmac.qnt` SHALL model the boot-born offer (ADR-0019
P4) as the reference implementation: the driver-arbitration phase sum
`Offered | KernelOwned | RemoteOwned` (DPMAC-I6) with typed transitions
judged from observation (endpoint + driver-face read-backs), never
commanded; the two directional MC link channels as distinct named types
(DPMAC-I4) with the requests-down channel typed `Unreadable` on the
restool transport; a firmware-version-indexed counter vocabulary
(DPMAC-I7: a representative counter slice readable at MC 10.39, the 10.40
extension named but unread — the full 28-row board vocabulary and its
verbatim names live in the adapter, not the model); and the MAC address
as an immutable value (DPMAC-I2). The
model SHALL NOT add a create action for the family
(`creatable: false`, DPMAC-I1) and the core machine SHALL be unchanged.

#### Scenario: The ladder is green on the grown model

- **WHEN** the model CI ladder runs on `families/dpmac.qnt` and its
  instantiations
- **THEN** typecheck and simulate pass, and every invariant marked for
  Apalache (DPMAC-I2, I3, I6, I7 encodings) checks within the ladder's
  bounds

#### Scenario: The two link channels cannot be conflated

- **WHEN** any action writes or reads link information in the model
- **THEN** it names exactly one of the two channel types, and no
  expression exists that reads the requests-down channel's value on the
  current transport

### Requirement: The connection surface types per-edge-kind teardown laws

The connection surface SHALL carry a teardown-order law per edge kind
(the ADR-0019 edge facet, beside the ADR-0009 `legalPorts` guard). For
the dpni–dpmac edge kind: `sever` is enabled only from `KernelOwned`,
consumes it, and yields `Offered` plus a severed witness; the dpni
kernel-face unbind of a dpmac-connected dpni requires that witness. No
other edge kind SHALL gain a law in this change.

#### Scenario: The driverless-port sequence is unreachable

- **WHEN** Apalache checks the sever-order invariant on the dpni–dpmac
  edge
- **THEN** no reachable state holds an unbound dpni whose dpmac edge
  was severed after the unbind — the ADR-0008 §8 hazard sequence does
  not exist in the state space

#### Scenario: Other edge kinds are untouched

- **WHEN** the existing connection-surface tests and frozen traces for
  dpdmux, dpci, and dpsw edges replay
- **THEN** they pass unchanged — no witness demand and no new guard
  applies to any non-dpmac edge

### Requirement: The dpmac coverage rows move from deferred to modeled

`models/COVERAGE.md` SHALL account for DPMAC-I2, I3, I4 (reachable
half), I6, and I7 as modeled with their checking rung named, carry the
phantom-create face's disposition from the V-DPMAC-2 verdict, and
re-anchor the out-of-scope rows loudly: the requests-down channel and
MC-view link read to `mc-portal-backend` (#10), DPRTC-I4 off this
change. Frozen traces for the new behaviors SHALL be committed beside
the model.

#### Scenario: The ledger lint holds after the sync

- **WHEN** the ledger lint cross-checks COVERAGE rows against model
  annotations and recorded verdicts
- **THEN** every DPMAC row names an existing encoding or an explicit
  anchor, and no row claims a verdict VERDICTS.json does not hold
