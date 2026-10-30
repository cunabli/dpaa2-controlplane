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

### D2 — Two-ledger capacity face: census and MC-global

Admission reads two independently-moving ledgers, not one. The capacity
face's pool rows read the per-container free-object census; a create
verb is refused by the MC-global hardware pools (`dprc show mc.global
--resources`: bp, channels, portals), and the two move apart — a create
in another container draws the global pool without moving the root
census, and a create in root raises the census while lowering the global
pool (ADR-0011; V-ROLLBACK-1 rev 3). The face therefore carries the
MC-global pools as a second ledger of budget rows observed each loop,
with parsing in the southbound adapter so the face stays pure data
(sans-io, ADR-0018). Pending creates are judged against the global row
of their family where one is proven; the census rows keep judging draws
and claims. The monotonic-growth rule (ADR-0023 Consequences) holds:
only bp (the dpbp family) joins now, proven by V-CEIL-1 and V-DPRC-7
rev 2 (a dpbp create refuses exactly at the bp free count; bp moves
symmetrically to the object). dpcon and dpmcp global rows join only with
their own proving observations — the dpcon channel pools (cqch/swpch)
are a scoped disambiguation sitting, and an mcp row stays behind the
ADR-0011 listing-lag question, not scoped here.

### D3 — Recording layer: decorator today, layer idiom by analogy

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

### D4 — Journal and record shape

`ActuationRecord` is an enum over the mutating actuation kinds,
carrying **observed** values only (assigned name/id, home container,
bind state). The journal is an append-only run-scoped sequence created
at execution start and harvested by the run's outcome; it dies with
the run. The record vocabulary lives in `dpaa2-api` (pure, sans-io);
the wrappers live with the trait seams they wrap.

### D5 — Reversal table: the D10 total-dispatch idiom

A compile-checked total `match` over `ActuationRecord` returning
`Reversal::Reversible { undo, preconditions }` or
`Reversal::Irreversible { residue }`. A new record variant without an
arm fails the build. Preconditions encode the forward ordering guards
reversed (unbind before destroy, child before container); residue is a
typed vocabulary naming the reboot remedy (ADR-0020 root growth,
ADR-0003 §7 dpio seats). A runtime map is refused for the same reason
as the dispatch table: a missing insertion compiles.

### D6 — Policy seam

`#[non_exhaustive]` enum at the `dpaa2-api` execution entry:
`Rollback` (the `Default`) | `Abort`. `Continue` (skip the failed verb
and its guard-dependents, converge the rest, next loop finishes) is
the named deferred variant — semantics agreed, implementation not
licensed here. The CLI renders the enum as one flag
(`--on-failure=rollback|abort`); dry-run prints the selected policy.
The policy is uniform over failure classes: no per-class branching, no
in-run retry (a failed undo verb is reported unreversed and the unwind
continues — the loop re-judges). [Scoped: a dispatch-time `McStatus` on
the child-DPRC create resolves as `ContainerOutcome::Refused` and does
not reach the one failure seam; the carve-out is tracked by bead
dpaa2-controlplane-985.]

### D7 — CLI render: the global budget rows beside the census rows

The admission table renders the MC-global rows beside the census rows in
the same draw/headroom/verdict shape, carried in a distinguishable
family-row voice the suite can grep. When a global row refuses, the
ensure refusal names the global budget, so an operator tells a
global-ledger refusal from a census one at planning time.

### D8 — Model: rollback action, ORCH-I3 restated

The model gains a run-journal projection (executed actuations of the
current run) and a rollback action enabled on a mid-run guard refusal
under the rollback policy; its effect folds the reverse journal onto
the census state, irreversible rows contributing residue. ORCH-I3 is
restated as net-identity-up-to-residue — residue that can strand its
container: a standing resident blocks the container's teardown, so
that create is reported unreversed and the dprc stays (ORCH-I7
containment through the unwind). The engine never reaches that exact
pairing — root-homed residue never homes in a destroyable container
and child residents are reclaimable (ADR-0020 §3); the model's
single-container abstraction conflates the two homes, and the guard
conservatively encodes the MC's real refusal of a non-empty
container destroy — plus planning-time refusal
while the drain holds — the no-oscillation claim. The unwind order is
the reverse journal, so ORCH-I6 gains no scheduler choice. The trace
set is re-frozen with the overcommit-rollback trace, and the replay
re-derives both verdicts through the Rust admission (process rule 6).

### D9 — Model's second ledger: the global pool counter and the hardware vector

The orchestration model carries a global pool counter beside the census
budget. The environment-adversary and any container's creates draw it,
and a create's mid-run guard refuses on the global row of its family
even when the census row shows headroom — the hardware-side drain the
census cannot see. The overcommit action and its frozen trace are
re-framed so the drain hits the global ledger, and ORCH-I3's re-proof
shows re-admission refuses on the global row while the drain holds: no
apply/rollback oscillation, now including the hardware vector. The
retro-oracle masks the global row (admission reads only the census) and
the oscillation re-opens — the rev-3 blindness as the red witness. The
unwind order stays the reverse journal, so ORCH-I6 gains no scheduler
choice from the second ledger.

### D10 — Evidence order

Offline first: fixpoint/identity tests, fake-MC budget simulation,
twin replay of the re-frozen traces — all green before the board row
is written. The board row (rollback under live kernel probing, the
DPSW-I9 race class) follows the standing suite conventions and the
commit-before-run rule.

The overcommit-rollback ITF trace (`overcommitRollbackTest` in
`models/orchestration/pipeline.qnt`) is generated-deferred: it lands in
`models/traces/orchestration/` with group 5, NOT the model half (group
2.3). The exact generation command, mirroring the `model:freeze-orch`
rung, is:

```
pnpm exec quint test models/orchestration/pipeline.qnt --main=orchestration \
  --match 'overcommitRollbackTest' \
  --out-itf='models/traces/orchestration/{test}.itf.json'
```

It is held back now because the replay harness
(`crates/dpaa2-verify/tests/orchestration_replay.rs`) pins the trace dir
to its `TRACES` list exactly; the new ITF plus its `TRACES` row land
together in group 5. The model half verified it generates by emitting to
a scratch path.

The nine-trace set is re-frozen from the updated model via the standing
rung `pnpm model:freeze-orch` (9/9 passing). Regeneration is
content-stable: two consecutive runs agree on every trace up to the ITF
`#meta` generation timestamp. The twin replay suite
(`orchestration_replay`) is green over the set once the engine's
capacity face carries the MC-global rows (groups 9–10); while the face
is census-only, the re-framed overcommit trace's global-row verdicts
are not derivable from the census alone and the replay names that step
red — the same blindness the retro-oracle masks.

### D11 — Offline twins: the fake's second counter

The fake backend carries the second counter: a create refusal driven by
the global counter, with the census counter independent, so the offline
sim stages exactly the rev-3 blindness — admission passes on the census
while the global counter is drained — and asserts it CLOSED once the
global row joins the face. The existing single-counter sim stays as the
census-visible lane.

### D12 — Suite: V-ROLLBACK-1 rev 4

The board row is re-staged to rev 4: the step-1 precondition gate reads
the global bp row as well, with counts derived at runtime from the
dry-run global row and never hardcoded, so the writer needs only a
handful of drains inside the ~5s kernel-bind-to-child-create window;
step 4's grep moves to the global-budget planning-time refusal voice.
The sitting stays operator-launched and the commit-before-run rule
holds. [SUPERSEDED in part by ADR-0024: the row's machinery is
board-proven (rev 3); the live mid-run witness is deferred to ADR-0024's
revisit triggers and the re-sit requirement is retired (bead
dpaa2-controlplane-275). The aged-board ceiling conflation (bead
dpaa2-controlplane-b4c) is resolved by the capacity-census delta (task
12.2).]

## Risks / Trade-offs

- [Journal infidelity — a recorder bug makes rollback unwind fiction]
  → the layer is observation-only by contract, the twin re-derives
  verdicts, and the empty-journal identity fixpoint is a required
  test.
- [Undo verbs race the same environment that caused the overcommit] →
  failed undos are reported unreversed, never retried; the next loop
  re-observes. The board row witnesses exactly this race class.
  [SUPERSEDED in part by ADR-0024: the row's machinery is board-proven
  (rev 3); the live mid-run witness of this race is deferred per
  ADR-0024.]
- [Vocabulary churn — overcommit replacing shortfall touches ADR
  prose] → one amendment clause, atemporal style, grep-verified
  repo-wide ("shortfall" count goes to zero outside archives).
- [Residue accumulation across repeated rollbacks] → residue kinds are
  grow-only populations by hardware law; reporting names the reboot
  remedy, and re-admission refusal bounds repetition to one per drain.
