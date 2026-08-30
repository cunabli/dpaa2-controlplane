# ADR-0002: Formal-methods process — Quint primary, dual-mode model-based testing

- **Status:** Accepted — scoping session 2026-08-22
- **Date:** 2026-08-22
- **Supersedes / relates to:** OpenSpec change `restool-baseline` (design D2–D3);
  ADR-0003 (board protocol the online mode runs under)

## Context

The port series replaces `ls-main` and eventually `restool` with typestate
Rust across all 16 MC object families. The MC and the kernel driver are two
opaque state machines: every hard lesson of the predecessor work (ADR-0001
C1/C2) came from behavior no document stated and code review could not
reveal. Testing the Rust against our *beliefs* about the MC would encode the
same blind spots the beliefs contain. We therefore make formal models the
authority in the middle: the model is checked against the board (is the model
right about the hardware?), and the Rust is checked against the model (is the
code right about the model?).

## Decision

### 1. Quint is the modeling language; TLA+ is a per-model escalation

Models are written in [Quint](https://quint-lang.org/) — TLA+ semantics with
a typed, executable surface: Apalache-checkable, runnable in a simulator that
emits ITF traces, and pnpm-installable, which fits this repo's toolchain and
a workflow of frequent rebaselining. A single model is hand-translated to
TLA+ only when Apalache cannot check a temporal property that model needs;
each escalation is recorded (in the model's header and the owning change)
when taken, never presumed in advance.

Alloy is the second escalation (amended 2026-08-30, change `intent-layer`
design D7), for a different shape of property: a *relational* one — a
plan's containment, connect-edge and companion-coupling structure, the
kind the intent compiler must hold over every derivable plan — that
proves awkward to state or check in Quint's state-machine surface. It is
taken per model on the same terms as TLA+: only when Quint has been tried
on that property and found awkward, recorded in the model's header and
the owning change when taken, never up front.

### 2. Models are living artifacts

A model is amended and rebaselined as understanding grows — a board
divergence amends the model in the same change that finds it, exactly as
baseline documents are amended in place. A model is never a frozen appendix;
its invariants are named, and the named set only grows or is explicitly
renegotiated in an ADR.

### 3. Model-before-code ordering

A change's Quint model lands green (typecheck, simulate, marked invariants
Apalache-checked) *before* its Rust lands. The model is the design artifact;
the typestate encoding follows it.

What a typestate can prove (amended 2026-08-23): the encoding captures the
transition *sequences* the code itself enforces, making undesired
hardware/kernel/userspace configuration states unrepresentable. A transition
may be gated by a fallible runtime check on observed data —
parse-don't-validate: success promotes the value into the next state's type,
and that type is a witness that the check ran and passed *at transition
time*. What no type can prove is that the hardware *remains* in that state:
drift is re-discovered as plain observed data and handled level-triggered
through `reconcile` (ADR-0001 §2). The reconciler's observed snapshot is
always plain data, never a type parameter, and a typestate never substitutes
for re-observation.

### 4. Model↔board conformance is dual-mode MBT over one shared adapter

One adapter binds the three faces together: model action ↔ restool command ↔
observed board state. Two modes run over it:

- **Batch (the default).** Traces are generated offline from the Quint
  simulator, emitted as reviewable scripts (ADR-0003), replayed linearly on
  the board by the operator, and diffed offline against the model's expected
  states. Cheap, repeatable, and auditable — the standing regression form.
- **Online (discovery).** A driver couples the model and the board
  step-by-step, choosing the next action from the model's enabled set and
  steering into unexplored states. Used where MC semantics are opaque (dprc
  containment/pools, dpsw, dpdmux), for exhaustion and error paths, and
  whenever a batch divergence shows the model's *branching structure* — not
  just a value — is wrong. Online runs only under the human-in-the-loop
  protocol of ADR-0003.

### 5. Every online finding is frozen; frozen traces outlive the board

Each online discovery is frozen into a committed batch trace. Frozen traces
replay two ways: on the board (regression against the hardware) and against
the Rust core as ITF replays in `cargo test` (regression against the code,
no board needed). This is the mechanism by which invariants hold *before,
during, and after* the port: the same trace that validated the model against
the MC later validates the Rust against the model.

### 6. CI ladder

Model checks run as a ladder, cheapest first: typecheck → simulate →
ITF replay against the Rust core → Apalache on invariants explicitly marked
for symbolic checking. Board replay is not CI; it is an operator-run
milestone (ADR-0003).

### 7. Deferred: volume replay against the Rust core (added 2026-08-23)

The ITF replay rung runs only *frozen* traces — few, curated, each one a
past divergence or a hand-directed scenario. Cedar's differential random
testing shows what the same rung looks like at volume: two computable
artifacts diffed on millions of generated inputs nightly. Our board rungs
can never scale that way (steps mutate hardware under the ADR-0003
envelope), but the replay rung is board-free on both sides and could:
generate fresh traces with `quint run --mbt` over the same
scenario-constrained modules the board suites use — constrained, not raw
`main.qnt`, so generation stays inside meaningful state space — and replay
them against the Rust core in bulk.

Deferred, not adopted: today every discovered divergence is covered by a
frozen trace, so volume would answer no open question. Revisit if either
occurs: (a) a board divergence that simulation over the frozen scenarios
*would* have reached but the frozen set missed, or (b) a reconciler bug
that escapes the replay rung. Either is evidence that curation is
under-covering and volume has a question to answer.

## Consequences

**Positive**

- Divergences are caught at the model, in review, before Rust is written.
- Board evidence compounds: every discovery becomes a permanent, board-free
  regression test.
- The engineering habit the series is meant to build — formal specs as
  working artifacts — is exercised on every change, not on a side quest.

**Negative / to watch**

- Two artifacts (model + code) must be kept honest; the amend-in-place rule
  and ITF replay are the guards, and they only work if divergences are fed
  back promptly.
- Quint/Apalache are younger tools than TLC; the TLA+ and Alloy escalation
  hatches are the mitigation, and each use of either is a data point on
  whether Quint was the right primary.

## References

- OpenSpec change `restool-baseline`, `design.md` D2–D3.
- ADR-0003 — board interaction protocol the batch/online modes run under.
- `docs/ROADMAP.md` — `verify-foundation` (change #2) builds the adapter,
  the CI ladder, and retro-models of the already-validated reconciler.
- [How we built Cedar with automated reasoning and differential testing](https://www.amazon.science/blog/how-we-built-cedar-with-automated-reasoning-and-differential-testing)
  — the volume-scale differential testing §7 defers to.
- [Alloy](https://alloytools.org/) — the relational escalation of §1;
  OpenSpec change `intent-layer`, `design.md` D7.
