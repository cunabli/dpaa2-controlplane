# Spec Delta

## Purpose

Unwind a mid-run-failed run from its own observed actuations: a
run-scoped journal, a total reversal table, a reverse-order rollback
executor with typed residue, and the per-run mid-run failure policy
callers choose at planning time.

## ADDED Requirements

### Requirement: Every actuation is journaled as observed

A transparent recording layer over the backend control traits SHALL
forward every verb unchanged and append each successful actuation's
observed result (assigned name/id, home container, bind state) to a
run-scoped, in-memory journal. The layer SHALL NOT alter, reorder, or
refuse a verb. The journal SHALL die with the run: nothing persists
and nothing is reserved (ADR-0008 unchanged). Recording SHALL sit
inside retry, so the journal holds the actuation that finally
succeeded.

#### Scenario: The journal records observations, not intent

- **WHEN** a create verb succeeds and the MC assigns an object id
  other than the intended one
- **THEN** the journal row carries the observed id, and a later
  rollback addresses the object that exists, not the one intended

#### Scenario: A failed verb leaves no journal row

- **WHEN** a verb refuses or errors
- **THEN** the journal gains no row for it, and rollback unwinds only
  what the board actually received

#### Scenario: The run ends and the journal is gone

- **WHEN** a run completes, rolls back, or aborts
- **THEN** no journal state survives into the next run, which begins
  from re-observation alone

### Requirement: The reversal table is total over the actuation vocabulary

A compile-checked total reversal table SHALL map every actuation kind
to exactly one row: `Reversible` (an undo verb plus preconditions that
honor the ordering guards in reverse — unbind precedes destroy, a
child unwinds before its container) or `Irreversible` (a typed
reboot-named residue: grow-only root population per ADR-0020, dpio
seats per ADR-0003 §7). Adding an actuation kind without a reversal
row SHALL fail the build.

#### Scenario: A new actuation kind without a row cannot ship

- **WHEN** the actuation vocabulary gains a kind and no reversal row
  is written
- **THEN** the build fails — omission is a compile error, never a
  runtime discovery

#### Scenario: Reversal honors ordering in reverse

- **WHEN** rollback reaches a journaled create whose object was
  subsequently bound
- **THEN** the unbind reversal fires before the destroy reversal, the
  same guard the forward run honored

### Requirement: Rollback unwinds the journal in reverse order

On a mid-run verb failure under the `rollback` policy, the executor
SHALL replay the journal in reverse order through the reversal table,
dispatching undo verbs from observed values. Irreversible rows SHALL
report their typed reboot-named residue and never block the remainder
of the unwind. The run SHALL end as a typed refusal naming the failure
class, the unwound actuations, and any residue.

#### Scenario: An overcommit unwinds the executed prefix

- **WHEN** admission passed, the environment drained a budget, and a
  create verb refuses mid-run under the rollback policy
- **THEN** every journaled actuation is reversed in reverse order and
  the board returns to its pre-run state up to typed residue

#### Scenario: Irreversible work becomes residue, not pretense

- **WHEN** the journal holds an actuation with an `Irreversible` row
- **THEN** rollback reports its reboot-named residue in the run's
  refusal and continues unwinding the remaining rows

#### Scenario: A failed undo verb does not wedge the unwind

- **WHEN** an undo verb itself fails (the environment interferes
  during the unwind)
- **THEN** the actuation is reported as unreversed in the run's
  refusal, the unwind continues with the remaining rows, and no in-run
  retry fires — the next loop re-observes and re-judges

#### Scenario: Rollback of an empty journal is the identity

- **WHEN** a run fails before any verb succeeded
- **THEN** rollback dispatches no undo verb and the refusal reports an
  empty unwind — the judgment applied to its own null output

#### Scenario: Post-unwind truth comes from re-observation

- **WHEN** a rollback completes
- **THEN** the next loop re-observes before judging; the journal's
  values are never carried forward as board state

### Requirement: The mid-run failure policy is a planning-time choice

The execution seam SHALL accept a per-run mid-run failure policy,
chosen by the caller (CLI or library) at planning time and never
prompted mid-run: `rollback` (default) or `abort` (stop at the
failure, leave the executed prefix standing as a typed refusal for the
next loop to re-judge). The policy SHALL apply uniformly to every
mid-run verb failure class, with no per-class branching and no in-run
retry.

#### Scenario: The default rolls back

- **WHEN** a caller requests execution without naming a policy and a
  mid-run verb fails
- **THEN** the run rolls back

#### Scenario: Abort leaves the prefix for the loop

- **WHEN** a caller selects `abort` and a mid-run verb fails
- **THEN** no undo verb fires, the run ends as a typed refusal naming
  the executed prefix, and the next level-triggered loop re-judges

#### Scenario: The policy never changes mid-run

- **WHEN** a run is executing
- **THEN** no interface exists to alter its failure policy until the
  next planning

### Requirement: The policy vocabulary is open for growth

The failure-policy type SHALL be declared open so new policies join
without breaking callers; `continue` (skip the failed verb and its
guard-dependents, converge the rest, the next loop finishes) is the
named deferred variant, recorded but not implemented here.

#### Scenario: A future policy is additive

- **WHEN** a later change introduces a new policy variant
- **THEN** existing callers compile unchanged and the CLI flag gains a
  value without renaming

### Requirement: Overcommit is named and bounded

A mid-run budget failure after a passed admission SHALL be reported as
**overcommit**, distinct from the planning-time **headroom refusal**.
After an overcommit rollback, the next admission SHALL refuse at
planning time while the drain holds — one rollback per drain event, no
apply/rollback oscillation — for both ledgers the face reads: the
per-container census and the MC-global pools create verbs draw against
(ADR-0011).

#### Scenario: Overcommit and headroom refusal are distinct verdicts

- **WHEN** an operator reads a run refusal
- **THEN** a planning-time capacity refusal names headroom and a
  mid-run one names overcommit, and the two are never collapsed

#### Scenario: Re-admission bounds the churn

- **WHEN** a run rolled back on overcommit and the environment still
  holds the drained budget
- **THEN** the next run refuses at admission and dispatches no verb

#### Scenario: A hardware-side drain is bounded too

- **WHEN** an overcommit rollback followed a drain of the MC-global bp
  pool (`dprc show mc.global --resources`) by concurrent creates in
  another container, and the drain still holds
- **THEN** the next admission re-judges the MC-global bp row (bp now,
  proven by V-CEIL-1 and V-DPRC-7 rev 2; dpcon and dpmcp only with their
  own proving observations), refuses at planning time, and dispatches no
  verb — the drain is no longer invisible to re-admission
