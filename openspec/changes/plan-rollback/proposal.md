# Proposal

## Why

An admitted plan can still fail mid-run. Admission is deliberately
check-without-reservation (ADR-0023), so the run plus concurrent
writers (kernel, firmware) can jointly **overcommit** a budget between
admission and a verb; other verb failures (MC error, bind failure)
share the same shape: an executed prefix stands on the board and the
engine today leaves it there as a typed refusal for operator
hand-cleanup. ADR-0023 already decides the direction and names this
change (bead `dpaa2-controlplane-axq`): unwind from a run-scoped
journal by default, under a caller-chosen policy.

## What Changes

- **Vocabulary**: the mid-run concept is named **overcommit**
  (lockless admission plus concurrent writers jointly exceed a budget,
  discovered at dispatch); the admission-time concept stays **headroom
  refusal**; "shortfall" exits the vocabulary. The deferred ADR-0023
  clarifying clause (resource-aware-orchestration review carry 5)
  lands as this rename. The forward-verb lineage is apply/rollback
  (declarative, kubectl/terraform-shaped); renaming the CLI's `ensure`
  is out of scope. Rollback unwinds **this run's observed
  actuations**, never a prior config generation — the JunOS-trained
  reading is explicitly disclaimed in design.
- A run-scoped, in-memory **run journal**: a transparent **recording
  layer** over the backend control traits forwards every verb
  unchanged and appends each successful actuation's **observed**
  result (assigned name/id, container, bind state). It never alters,
  reorders, or refuses a verb. Nothing persists past the run and
  nothing is reserved — ADR-0008 is untouched.
- A compile-checked **total reversal table** over the actuation
  vocabulary (the D10 total-dispatch idiom): every row is
  `Reversible` (undo plus preconditions — the ordering guards honored
  in reverse, unbind precedes destroy) or `Irreversible` (typed
  reboot-named residue: grow-only root population ADR-0020, dpio seats
  ADR-0003 §7). An actuation kind without a row fails the build.
- A **rollback executor**: reverse-order replay of the journal through
  the reversal table; irreversible rows report residue instead of
  pretending reversal.
- A **mid-run failure policy** at the `dpaa2-api` call seam, so CLI
  and library callers choose per run at planning time:
  `rollback` (default) | `abort` (stop, leave the executed prefix, the
  next loop re-judges). The enum is declared open for growth;
  `continue` (skip the failed verb and its dependents, converge the
  rest, eventual consistency through the loop) is the named deferred
  variant. The policy governs every mid-run verb failure class with no
  per-class branching; overcommit is the evidence-anchored class.
- The orchestration model grows a **rollback action**, ORCH-I3 is
  restated and re-proved under rollback side effects, and the frozen
  traces are re-frozen including an overcommit-rollback trace.
- A **fake-MC budget simulation** drives the overcommit path offline,
  and a new **board row** witnesses rollback under live kernel probing
  (the DPSW-I9 race-class lesson).

## Invariant ledger

What rollback preserves, restates, and cannot promise:

- **Preserved — ORCH-I1 (completeness), mirrored**: every actuation
  kind has a reversal row or a typed residue; omission fails the
  build, the total-dispatch discipline pointing backwards.
- **Preserved — ORCH-I2/I7 (order, containment)**: the unwind honors
  the same guards reversed; the reversal table preconditions carry
  them.
- **Preserved — ORCH-I6 (confluence)**: reverse journal order is
  deterministic; the unwind has no scheduler choice.
- **Preserved — ADR-0008**: in-memory journal, dies with the run, no
  reservation.
- **Preserved — identity fixpoint**: rollback of an empty journal is
  the identity (process rule 6 self-application).
- **Restated — ORCH-I3 (stable refusal)**: a mid-run-refused run has
  side effects — the executed prefix plus its unwind; the restated
  claim is net identity **up to typed residue**, and the next
  admission re-judges the drained budget and refuses at planning time,
  bounding churn to one rollback per drain event.
- **Limited**: restoration holds only up to irreversible residue
  (reboot-named, never silent), and post-unwind visibility comes only
  from re-observation — the journal's observed values are undo-verb
  inputs, not post-state truth.

## Non-goals

- The same-run rebuild refusal (pool-objects design D12) is untouched.
- No persisted journal, no reservation, no change to admission
  semantics.
- No in-run retry or backoff smuggled in as a policy; `abort` and
  `rollback` both defer re-judgment to the next loop.
- No rollback to a prior configuration generation.
- No CLI verb renaming (`ensure` et al. are a later, separate
  refresh).
- No formal middleware/layer trait while the recording layer is the
  only layer; the promotion trigger is a second cross-cutting layer.

## Capabilities

### New Capabilities

- `rollback`: the run journal recorded through the transparent
  recording layer, the total reversal table over the actuation
  vocabulary, the reverse-order rollback executor, typed reboot-named
  residue, and the per-run mid-run failure policy.

### Modified Capabilities

- `orchestration-model`: the model gains the rollback action over the
  journal projection; ORCH-I3 stable refusal is restated under
  rollback side effects; the frozen trace set grows an
  overcommit-rollback trace.
- `mbt-harness`: the fake-MC budget simulation drives the overcommit
  path offline, and the board program gains the
  rollback-under-kernel-probe row inside the safety envelope.
- `provisioning-cli`: the planning surface carries the per-run
  mid-run failure policy (rollback default, abort opt-out) and renders
  residue in operator data terms.

## Impact

- `dpaa2-api`: actuation-record vocabulary, journal seam, reversal
  table shape, failure-policy enum (pure; sans-io).
- `dpaa2-mc`/`dpaa2-hal` consumers: recording layer over
  `McControl`/`KernelControl` — zero-pass refactor, all mutations
  already flow through the traits; recording sits inside retry so it
  journals the actuation that finally succeeded. The traits are
  untouched.
- `dpaa2-tools`: policy flag, residue reporting.
- `models/orchestration/` + `dpaa2-verify`: rollback action, ORCH-I3
  re-proof, trace re-freeze, fake-MC simulation, board row.
- `docs/adr/0023`: overcommit rename clause (amend in place,
  atemporal style).
