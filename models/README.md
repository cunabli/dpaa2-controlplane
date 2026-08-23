# models/ — the executable object model

Quint models that make the DPAA2 baseline executable. The prose ground
truth lives in `docs/baseline/` (16 family documents distilled into
`object-model.md`); this tree renders it as one state machine the
simulator can drive, so a wrong belief about the Management Complex
fails a check here before it becomes Rust. Process rules: ADR-0002
(formal methods, CI ladder), design decisions D2–D5 and D9 of
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

Cheapest rung first (ADR-0002 §6); a failure stops the ladder there.

1. **Typecheck** — every module parses and typechecks.
2. **Simulate** — the machine's guards admit the baseline's canonical
   lifecycle traces and refuse the transitions the baseline forbids.
   Directed runs in `main.qnt` pin both directions; random simulation
   sweeps for runtime errors and deadlocks. Named invariants (phase 2)
   will run here too, under their baseline ids (`DPRC-I6`, `DPNI-I2`, …).
3. **ITF replay** — frozen traces from `traces/` replay against the
   Rust core in `cargo test` (task 3.2), keeping model and code honest
   against each other with no board attached.
4. **Apalache** — symbolic checking of the invariants each model header
   explicitly marks; unmarked invariants are simulator-only. Version
   pinned in `package.json` `config.apalache_version`.

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
│   └── machine.qnt       the composed machine: consts, init, 20 actions, EVIDENCE
├── families/             16 per-family FamilyParams records; laws as data,
│                         no family forks the lifecycle logic
├── retro/                reconciler dpni↔dpmac retro-model (task 3.1)
├── traces/               committed frozen ITF traces (tasks 3.x, phase 5)
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

Quint is repo-local; always go through `pnpm exec` (until task 3.3
wires pnpm scripts and the CI job):

```sh
# rung 1: typecheck (silent on success, exit 0)
pnpm exec quint typecheck models/main.qnt

# rung 2a: directed runs — canonical lifecycle + refusal tests
pnpm exec quint test models/main.qnt --main=main

# rung 2b: random exploration
pnpm exec quint run models/main.qnt --main=main --max-steps=40 --max-samples=200

# poke at the machine interactively
pnpm exec quint repl
>>> .load models/main.qnt
```

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

**An invariant (phase 2 convention)** — encode it under its baseline id
so ledger, docs, and model never drift. Breaking candidates encode the
*absence* of the convenient assumption plus a property that would catch
code relying on it. List it in the owning model's header; mark it there
if Apalache should check it symbolically (and record any TLA+
escalation in the same header). Add its row to `COVERAGE.md`.

**A discovered divergence (board evidence)** — amend the model and the
owning baseline document in the same change (ADR-0002 §2), freeze a
reproducing ITF trace into `traces/`, and settle the ledger row.
