# Tasks

## 1. Vocabulary and ADR amendment

- [x] 1.1 Amend ADR-0023 in place (atemporal style): the overcommit
  rename clause — mid-run concept is overcommit, admission-time stays
  headroom refusal, the borrowed-word rollback caveat (this run's
  observed actuations, never a prior config generation), and the
  policy enum direction (rollback default | abort, open enum,
  `continue` named deferred). Verify: `grep -rn shortfall docs/ crates/
  models/ openspec/specs/` returns nothing (archives exempt), ADR
  reads prescriptive with no travel narrative.

## 2. Orchestration model: rollback action and re-proof

- [x] 2.1 Add the run-journal projection and the rollback action to
  `models/orchestration/` (enabled on mid-run guard refusal under the
  rollback policy; effect folds the reverse journal onto census state,
  irreversible rows contributing residue). Verify: quint typecheck and
  the existing invariant suite stays green.
- [x] 2.2 Restate ORCH-I3 (net identity up to typed residue;
  planning-time refusal while the drain holds; no apply/rollback
  oscillation reachable) and check it Apalache-marked within ladder
  bounds. Verify: the restated invariant is red when the rollback
  action is allowed to re-apply without re-admission (retro-oracle),
  green as modeled.
- [x] 2.3 Re-freeze the trace set including the overcommit-rollback
  trace (admission passes, environment drains, run unwinds, next
  admission refuses). Verify: frozen ITF files updated and the model's
  trace-generation run documented in the change.

## 3. Core vocabulary in dpaa2-api

- [x] 3.1 Add `ActuationRecord` (observed values only), the run-scoped
  append-only journal, and the `#[non_exhaustive]` failure-policy enum
  (`Rollback` default | `Abort`) at the execution seam. Verify: unit
  tests cover observed-not-intended values and default policy;
  quality-floor green.
- [x] 3.2 Add the reversal table as a compile-checked total match
  returning `Reversible { undo, preconditions }` |
  `Irreversible { residue }`, preconditions encoding the reversed
  ordering guards, residue typed reboot-named (ADR-0020, ADR-0003 §7).
  Verify: unit tests for ordering (unbind-before-destroy row) and
  residue typing; a doc-comment states totality is the D10 idiom;
  quality-floor green.

## 4. Recording layer

- [x] 4.1 Implement `Recording<B>` wrappers over `McControl` and
  `KernelControl`: transparent forwarding, append-on-success, shared
  single-threaded journal handle, placed inside retry. Verify: unit
  tests with the fake backend — observed ids recorded, failed verb
  leaves no row, wrapper never alters or refuses a verb (passthrough
  equivalence test); quality-floor green.

## 5. Rollback executor and policy wiring

- [x] 5.1 Implement the rollback executor (reverse-order replay
  through the reversal table; irreversible rows report residue and
  continue; a failed undo verb reports unreversed and continues, no
  in-run retry) and wire the policy at the engine's execution entry
  (abort = typed refusal naming the executed prefix). Verify: unit
  tests for reverse order, residue continuation, failed-undo
  continuation, empty-journal identity (process rule 6 fixpoint);
  quality-floor green.
- [x] 5.2 Fake-MC budget simulation in `dpaa2-verify` intent/: drain a
  budget between admission and a create; assert full unwind to the
  pre-run snapshot up to residue under rollback, and no undo verbs
  plus prefix-naming refusal under abort. Verify: `cargo test` suite
  names the overcommit scenario.
- [x] 5.3 Twin replay of the re-frozen traces: the overcommit-rollback
  trace replays against the engine with both verdicts re-derived
  through the Rust admission (never trusted from structure). Verify:
  orchestration replay suite green, and red when the re-derivation is
  short-circuited (trip-wire check).

## 6. CLI surface

- [x] 6.1 Add the policy flag (one flag, values from the enum, default
  rollback), dry-run policy preview, and the rollback report rendering
  (overcommit distinct from headroom refusal, unwound actuations in
  observed terms, residue with reboot remedy). Verify: CLI integration
  tests for flag default, dry-run preview line, and report wording in
  operator data terms; quality-floor green.

## 7. Board witness and docs

- [x] 7.1 Write the board row — rollback under live kernel probing
  (DPSW-I9 race class) inside the safety envelope, suite conventions,
  expectations inline; offline gates all green first and the pre-run
  record committed per the commit-before-run rule. Verify: the row
  renders and dry-runs offline; sitting execution is operator-launched.
- [x] 7.2 Update docs/ROADMAP.md (the axq row) and the COVERAGE
  anchors for the new suites. Verify: quality-floor green; ROADMAP row
  states delivery in the standing style.

## Workflow follow-up

- Board sitting execution and verdict recording (operator-gated), then
  `/opsx:archive plan-rollback` after review.
