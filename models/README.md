# models/ — the executable object model

Quint models that make the DPAA2 baseline executable. The prose ground
truth lives in `docs/baseline/` (16 family documents distilled into
`object-model.md`); this tree renders it as one state machine the
simulator can drive, so a wrong belief about the Management Complex
fails a check here before it becomes Rust. Process rules: ADR-0002
(formal methods, CI validation), design decisions D2–D5 and D9 of
`openspec/changes/verify-foundation`.

## Quint in one minute

[Quint](https://quint-lang.org/) is a specification language from the
TLA+ family: a model is a **state machine** — typed state variables, an
`init` state, and **actions** whose guards say when a transition is
allowed and whose effect says what the next state is. Nondeterminism
(`nondet x = S.oneOf()`) stands in for everything the model does not
control: which object an operator touches next, which pool object a
probe draws. A **trace** is one concrete sequence of states produced by
running actions from `init` — the machine's behaviors are the set of
all such traces.

Three tools consume the same model, with different strength:

- The **simulator** (`quint run`/`test`) executes traces concretely —
  directed ones we script, or thousands of random ones. It *finds*
  violations; not finding one is evidence, not proof.
- **Apalache** is a bounded symbolic model checker: it covers *all*
  traces up to a depth, so within that bound it proves an invariant
  rather than sampling it. That strength costs encoding restrictions —
  hence only explicitly marked invariants run there.
- **ITF** (Informal Trace Format) is the JSON serialization of a
  concrete trace — every state, every variable. A trace frozen as ITF
  is replayable forever: the replayer feeds its steps to the Rust core
  and diffs the code's behavior against the model's states, which is
  what lets one board observation keep guarding the code in CI with no
  board attached.

The precondition to all of it: the model is only as true as the
baseline documents it encodes. Quint proves the *documented laws are
mutually consistent and imply what we think they imply* — it cannot
prove the documents describe the real Management Complex. That gap is
exactly what the evidence tags mark and the phase-5 board suites close.

## What is proven, and by which rung

Cheapest rung first (ADR-0002 §6); a failure stops the validation there.

1. **Typecheck** — every module parses and typechecks.
2. **Simulate** — the machine's guards admit the baseline's canonical
   lifecycle traces and refuse the transitions the baseline forbids.
   Directed runs in `main.qnt` pin both directions; random simulation
   sweeps for runtime errors, deadlocks, and the named state invariants
   of `core/invariants.qnt`, under their baseline ids (`DPRC_I1`,
   `DPNI_I9`, …). Candidates that are action-guard properties or
   Breaking absence-of-assumption witnesses are directed `<ID>Test`
   runs in `main.qnt` instead; candidates not expressible over the
   model state are harness-owned or deferred — `COVERAGE.md` records
   every disposition.
3. **ITF replay** — frozen traces from `traces/` replay against the
   Rust core in `cargo test` (`crates/dpaa2-verify`), keeping model and
   code honest against each other with no board attached. The replayer
   projects each observation state of a trace to the reconciler's
   `ObservedTopology`, runs the pure `reconcile()`, and diffs the plan
   against the steps the model actually took.
4. **Apalache** — symbolic checking of the invariants each model header
   explicitly marks; unmarked invariants are simulator-only. Version
   pinned in `package.json` `config.apalache_version` (0.56.1 — the
   newest version the quint 0.32.0 client can drive; 0.62.1 starts a
   server the client never completes a check against). The marked set
   is all nine state invariants at bounded depth 1: init plus every
   single-step successor class, symbolically — deeper bounds cost tens
   of minutes per step on this machine size, and the simulator already
   covers depth 40 randomly. Empirical constraints the corpus honors:
   `ALL_PARAMS` lives in `families/params.qnt` because the verify-path
   flattener loses sum-type constructors whose only value use is in the
   main module (QNT404), and integer ranges must be constant
   (`machine.MAX_ENDPOINT_PORTS`).

The board itself is never a CI rung (ADR-0003): board suites are
generated from these models by the `dpaa2-verify` harness (phase 4) and
run by the operator.

What the model does **not** claim: every transition carries an evidence
tag in `core/machine.qnt` `EVIDENCE` — `BoardExercised` (prior work
drove it on the board) or `Read` (stands on source/restool/manual
reading, awaits phase 5). A board failure against a `Read` transition
means "claim was never verified", not "model wrong".
`COVERAGE.md` is the ledger closing the loop: every invariant candidate
from the family docs is modeled, deferred to a named change, or
board-pending — never silently dropped.

## Layout

```
models/
├── core/                 one generic object machine (design D2/D4)
│   ├── types.qnt         shared state: CoreState, ObjState, FamilyParams, Evidence
│   ├── containment.qnt   view 1 — DPRC tree, lock stripping, move gates
│   ├── connect.qnt       view 2 — edge legality table, cardinality one
│   ├── create_allocate.qnt view 3 — create gates, pool membership
│   ├── pools.qnt         view 4 — draw census predicates
│   ├── lifecycle.qnt     view 5 — pure CoreState -> CoreState transforms
│   ├── machine.qnt       the composed machine: consts, init, 20 actions, EVIDENCE
│   └── invariants.qnt    phase-2 state invariants under baseline ids; §6 law map
├── families/             16 per-family FamilyParams records; laws as data,
│                         no family forks the lifecycle logic; params.qnt
│                         is the corpus-wide ALL_PARAMS table
├── retro/                reconciler dpni↔dpmac retro-model: directed
│                         runs mirroring the RestoolMc recipe, epoched
│                         by the reconciler's observation points
├── traces/               committed frozen ITF traces (retro now,
│                         board divergences in phase 5)
├── main.qnt              instantiation of machine with all 16 families;
│                         directed *Test runs
├── COVERAGE.md           the invariant coverage ledger (design D9)
└── README.md             this file
```

View modules are pure (functions over `CoreState`); state and actions
live only in `machine.qnt`. Every action has two forms sharing one set
of guards: `<name>At(args)` for directed runs and the ITF replayer, and
a nondet wrapper for random simulation — refusal semantics can never
drift between the two.

## Running

The validation is wired as pnpm scripts (task 3.3; the CI job `model-validation`
runs the same rungs in the same order):

```sh
pnpm model:typecheck   # rung 1: main + retro (silent on success)
pnpm model:test        # rung 2a: directed runs — lifecycle, refusals, retro
pnpm model:simulate    # rung 2b: random exploration under stateInvariants
pnpm model:replay      # rung 3: frozen ITF traces vs the reconciler (cargo)
pnpm model:verify      # rung 4: Apalache on the marked subset (~3 min; JVM)
pnpm model:validation  # all of the above, cheapest first, stop on failure

pnpm model:freeze      # regenerate models/traces/ from the retro runs

# poke at the machine interactively
pnpm exec quint repl
>>> .load models/main.qnt
```

`model:freeze` rewrites the committed traces; the replayer's epoch
tables (`crates/dpaa2-verify/tests/retro_replay.rs`) transcribe the
observation points documented in `retro/reconciler.qnt`, so a trace
whose shape changed fails `model:replay` loudly — update both together.

`quint test` only picks up `run` definitions whose names match `Test` —
name directed runs `<what>Test`.

## Adding to the model

**A family behavior (parameter-expressible)** — edit the family's record
in `families/<fam>.qnt`. Values must carry their anchor as a comment
(`DPIO-I1`, `§3 census`, …); if the claim is new, it goes into the
family baseline doc first — the model never introduces ground truth.

**A transition** — add the pure transform to the owning view module in
`core/`, then the guarded `<name>At` action plus nondet wrapper in
`machine.qnt`, add it to `step`, and give it an `EVIDENCE` entry. Extend
a `main.qnt` directed run (or add a `…Test`) so the new guard is pinned
in both the admitted and the refused direction.

**An invariant** — encode it under its baseline id so ledger, docs, and
model never drift. A machine-enforced state predicate goes into
`core/invariants.qnt` and the `stateInvariants` conjunction in
`main.qnt`; an action-guard property or Breaking absence-of-assumption
witness goes in as a directed `<ID>Test` run (Breaking = the *absence*
of the convenient assumption plus a property that would catch code
relying on it). List it in the owning model's header; mark it there if
Apalache should check it symbolically (and record any TLA+ escalation
in the same header). Add its row to `COVERAGE.md`. NB: a failed action
leaves the state unset in the simulator, so a `.fail()` step must be
the last step of its run — split refusal and continuation into sibling
runs off a shared prefix run.

**A discovered divergence (board evidence)** — amend the model and the
owning baseline document in the same change (ADR-0002 §2), freeze a
reproducing ITF trace into `traces/`, and settle the ledger row.
