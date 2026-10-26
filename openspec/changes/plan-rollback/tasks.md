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

- [x] 7.3 Record the rev-3 finding — admission's census budget is blind
  to the MC-global hardware ledger, so the one-rollback-per-drain bound
  is unenforced for a hardware-side drain — in ADR-0023 Consequences,
  ADR-0011 open questions, and the V-ROLLBACK-1 suite-ledger row
  (revs 1–3 narrated). Recording only; the engine-side global budget
  face lands in-change (the amended groups below).

## 8. Orchestration model: the second ledger and re-proof

- [x] 8.1 Add the MC-global pool ledger to `models/orchestration/` as a
  second budget counter beside the per-container census budget: the
  environment-adversary action and any container's creates draw it, and
  a create's mid-run guard refuses on the global row of its family even
  when the census row shows headroom (the hardware-side drain the census
  cannot see; ADR-0011, ADR-0023 Consequences). Verify: quint typecheck
  and the existing invariant suite stay green; a witness reaches a state
  where the global row is drained while the census row is not.
- [x] 8.2 Re-frame the overcommit action and re-prove ORCH-I3 under the
  second ledger: the drain hits the global ledger (a concurrent create
  in another container draws the global pool without moving the root
  census), and after the rollback the next admission refuses on the
  global row while the drain holds — no apply/rollback oscillation, now
  including the hardware vector. Verify: the restated ORCH-I3 is
  Apalache-marked within ladder bounds and green as modeled; the
  retro-oracle masks the global row (admission reads only the census)
  and the restated ORCH-I3 goes red — the rev-3 blindness as the red
  witness, green with the row present.
- [x] 8.3 Re-freeze the trace set with the re-framed overcommit-rollback
  trace (admission passes, the environment drains the global ledger, the
  run unwinds, the next admission refuses on the global row). Verify:
  `pnpm model:freeze-orch` regenerates byte-stably (9/9) and the
  model's trace-generation run is documented in the change.

## 9. Observation, capacity face, and admission

- [x] 9.1 Parse `dprc show mc.global --resources` in the southbound
  adapter (dpaa2-mc) into backend-neutral per-family global pool counts;
  the face stays pure data (sans-io, ADR-0018). Verify: adapter unit
  tests parse the bp global row from a captured fixture; quality-floor
  green.
- [x] 9.2 Add the MC-global budget rows to the capacity face
  (`crates/dpaa2-api/src/plan/capacity.rs`) as a second ledger beside
  the census pool rows — bp only now, under the monotonic-growth rule
  (proven by V-CEIL-1 + V-DPRC-7 rev 2: a dpbp create refuses exactly at
  the bp free count, bp moves symmetrically to the object). dpcon and
  dpmcp global rows join only with their own proving observations, and
  no mcp row is scoped (ADR-0011 listing-lag). Verify: unit tests show
  the global bp row present and the census rows unchanged; the
  monotonic-growth doc names bp as the only family added now;
  quality-floor green.
- [x] 9.3 Judge pending CREATES against the global row of their family
  where one is proven (bp); census rows keep judging draws and claims.
  Verify: admission refuses a pending bp create when the global bp row
  is exhausted though the census row shows headroom; the converged-intent
  re-admits fixpoint test (`crates/dpaa2-verify/tests/admission_oracle.rs`)
  extends to the global rows — the judgment on its own converged output
  yields the identity verdict (process rule 6 fixpoint); quality-floor
  green.

## 10. CLI render, report, and twins

- [x] 10.1 Render the MC-global budget rows beside the census rows in
  the admission table (same draw/headroom/verdict shape, a
  distinguishable family-row voice the suite can grep), and name the
  global budget in the ensure refusal voice when that row refuses.
  Verify: CLI integration tests assert the global bp row renders
  distinguishably and the refusal voice names the global budget;
  quality-floor green.
- [x] 10.2 Give the fake backend a second counter in `dpaa2-verify`
  intent/: create-refusal driven by the global counter, the census
  counter independent, so the offline sim stages exactly the rev-3
  blindness (admission passes on the census while the global counter is
  drained) and asserts it CLOSED once the global row joins the face; the
  existing single-counter sim stays as the census-visible lane. Verify:
  `cargo test` names the rev-3-blindness-closed scenario and the
  census-visible lane both; quality-floor green.
- [x] 10.3 Replay the re-framed overcommit-rollback trace: the mid-run
  overcommit verdict (now on the global bp row) and the follow-on
  planning-time refusal are each re-derived through the Rust admission
  from the prior state's census and global counts
  (`crates/dpaa2-verify/tests/orchestration_replay.rs` check_pair —
  process rule 6 verdict re-derivation, never trusted from trace
  structure). Verify: orchestration replay green over the re-frozen set;
  red when the global-row re-derivation is short-circuited (trip-wire).

## 11. Suite re-stage and sittings

- [ ] 11.1 Re-stage V-ROLLBACK-1 to rev 4: the step-1 precondition gate
  reads the global bp row too (counts derived at runtime from the
  dry-run global row, never hardcoded, so the writer needs only a
  handful of drains inside the ~5s kernel-bind-to-child-create window),
  and step 4's grep moves to the global-budget planning-time refusal
  voice. Offline gates all green first and the pre-run record committed
  per the commit-before-run rule. Verify: the row renders and dry-runs
  offline; `sh -n` and shellcheck clean; the operand pins stay offline
  (`crates/dpaa2-tools/tests/vrollback1_intents.rs`); sitting is
  operator-launched.
- [ ] 11.2 Scope and stage the dpcon channel-pool disambiguation sitting
  (cqch/swpch semantics unproven): a reviewable board row that reads the
  dpcon global rows under create-until-refused, so a dpcon global row can
  join the face with its own proving observation. Operator-gated:
  authoring and offline gates only; the sitting is operator-launched and
  no dpcon global row ships until it proves out. Verify: the row renders
  and dry-runs offline; `sh -n` and shellcheck clean.

## Workflow follow-up

- Board sitting execution and verdict recording (operator-gated,
  V-ROLLBACK-1 rev 4 and the dpcon disambiguation sitting), then
  `/opsx:archive plan-rollback` after review.
