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
- Quint/Apalache are younger tools than TLC; the TLA+ escalation hatch is
  the mitigation, and each use of it is a data point on whether Quint was
  the right primary.

## References

- OpenSpec change `restool-baseline`, `design.md` D2–D3.
- ADR-0003 — board interaction protocol the batch/online modes run under.
- `docs/ROADMAP.md` — `verify-foundation` (change #2) builds the adapter,
  the CI ladder, and retro-models of the already-validated reconciler.
