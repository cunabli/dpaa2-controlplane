# Design

## Context

See proposal.md for motivation and the invariant ledger. Constraints
that shape the approach: admission is check-without-reservation with
no lock on the budgets (ADR-0023); nothing persists past a run and no
reservation state exists in any host (ADR-0008); all mutations already
flow through the `McControl`/`KernelControl` traits (sans-io seams,
ADR-0018); dispatch is serialized; the engine is level-triggered and
re-judges from observation every loop. The actuation vocabulary is the
set of mutating verbs those traits carry today.

## Goals / Non-Goals

**Goals:**

- A journal that is a faithful witness: recording is transparent,
  observation-only, and sits where it sees final outcomes.
- Reversal correctness by construct: totality and ordering are
  compile-checked, not reviewed.
- One policy seam shared by CLI and library callers.

**Non-Goals:**

- See proposal.md Non-goals. Design-level additions: no generic
  middleware/layer trait (promotion trigger: a second cross-cutting
  layer); no journal access for any consumer other than the rollback
  executor and the run report.

## Decisions

### D1 — Vocabulary: apply/rollback, overcommit (grilled, settled)

The forward-verb lineage is declarative apply (kubectl/terraform), not
commit: there is no candidate datastore or config history. **Borrowed-
word caveat (stated wherever rollback is documented): rollback unwinds
this run's observed actuations; it never restores a prior
configuration generation — the JunOS `rollback <n>` reading does not
apply.** The mid-run concept is **overcommit**: the lockless admission
plus concurrent writers jointly exceed a budget, discovered at
dispatch. Admission-time stays **headroom refusal**. "shortfall" exits
the repo vocabulary; the ADR-0023 amendment carries the rename and
closes resource-aware-orchestration review carry 5. Alternatives
considered: shortfall (meaningless to operators), overdraft (monetary),
exhaustion/depletion (name the board state, not the systemic cause).

### D2 — Recording layer: decorator today, layer idiom by analogy

One generic wrapper per backend trait (`Recording<B>` implementing the
trait by forwarding) with a shared handle to the run's journal;
dispatch is serialized, so single-threaded interior mutability
suffices. The tower `Layer`/`Service` analogy is the architectural
hint: nesting is the composition mechanism (`Recording<Retrying<...>>`)
and the ordering law is explicit — **recording sits inside retry**, so
the journal holds the actuation that finally succeeded, never failed
attempts. A true tower-style chain is refused now: it needs one
uniform call shape, i.e. a verb-enum funnel that would erase the
typestate at the exact seam the project exists to type; blanket
per-method forwarding is a decorator regardless of badge. Promotion
trigger: a second cross-cutting layer (tracing, or retry migrating out
of the shim).

### D3 — Journal and record shape

`ActuationRecord` is an enum over the mutating actuation kinds,
carrying **observed** values only (assigned name/id, home container,
bind state). The journal is an append-only run-scoped sequence created
at execution start and harvested by the run's outcome; it dies with
the run. The record vocabulary lives in `dpaa2-api` (pure, sans-io);
the wrappers live with the trait seams they wrap.

### D4 — Reversal table: the D10 total-dispatch idiom

A compile-checked total `match` over `ActuationRecord` returning
`Reversal::Reversible { undo, preconditions }` or
`Reversal::Irreversible { residue }`. A new record variant without an
arm fails the build. Preconditions encode the forward ordering guards
reversed (unbind before destroy, child before container); residue is a
typed vocabulary naming the reboot remedy (ADR-0020 root growth,
ADR-0003 §7 dpio seats). A runtime map is refused for the same reason
as the dispatch table: a missing insertion compiles.

### D5 — Policy seam

`#[non_exhaustive]` enum at the `dpaa2-api` execution entry:
`Rollback` (the `Default`) | `Abort`. `Continue` (skip the failed verb
and its guard-dependents, converge the rest, next loop finishes) is
the named deferred variant — semantics agreed, implementation not
licensed here. The CLI renders the enum as one flag
(`--on-failure=rollback|abort`); dry-run prints the selected policy.
The policy is uniform over failure classes: no per-class branching, no
in-run retry (a failed undo verb is reported unreversed and the unwind
continues — the loop re-judges).

### D6 — Model: rollback action, ORCH-I3 restated

The model gains a run-journal projection (executed actuations of the
current run) and a rollback action enabled on a mid-run guard refusal
under the rollback policy; its effect folds the reverse journal onto
the census state, irreversible rows contributing residue. ORCH-I3 is
restated as net-identity-up-to-residue plus planning-time refusal
while the drain holds — the no-oscillation claim. The unwind order is
the reverse journal, so ORCH-I6 gains no scheduler choice. The trace
set is re-frozen with the overcommit-rollback trace, and the replay
re-derives both verdicts through the Rust admission (process rule 6).

### D7 — Evidence order

Offline first: fixpoint/identity tests, fake-MC budget simulation,
twin replay of the re-frozen traces — all green before the board row
is written. The board row (rollback under live kernel probing, the
DPSW-I9 race class) follows the standing suite conventions and the
commit-before-run rule.

## Risks / Trade-offs

- [Journal infidelity — a recorder bug makes rollback unwind fiction]
  → the layer is observation-only by contract, the twin re-derives
  verdicts, and the empty-journal identity fixpoint is a required
  test.
- [Undo verbs race the same environment that caused the overcommit] →
  failed undos are reported unreversed, never retried; the next loop
  re-observes. The board row witnesses exactly this race class.
- [Vocabulary churn — overcommit replacing shortfall touches ADR
  prose] → one amendment clause, atemporal style, grep-verified
  repo-wide ("shortfall" count goes to zero outside archives).
- [Residue accumulation across repeated rollbacks] → residue kinds are
  grow-only populations by hardware law; reporting names the reboot
  remedy, and re-admission refusal bounds repetition to one per drain.
