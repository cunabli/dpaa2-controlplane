# orchestration-hardening — the model earns the budget law the board proved

## Why

The resource-aware-orchestration four-pass review (archived with the
change, `review/synthesis.md`) returned a split verdict: the change's
core promise holds offline and live, but ADR-0002 isomorphism is not
yet earned at the budget level. At every point where the twins
disagree, the Rust side is correct and the model owes the amendment:
the model prices a port wire one FDB unit where the board proved a
connect allocates none (V-DPSW-2), and prices dpio seats against the
refusing pool where the design ships them as spent headroom. Both
divergences shipped invisible because the replay harness checks
structure only — it never re-derives the admission verdict it replays.
Two smaller gaps compound it: `Status::Orphan` is unreachable, so the
prune lane of the confluence evidence and ORCH-I7's teardown half are
vacuous; and one engine judgment (`plan_admission`) reads the board
three times, so its FDB verdict can mix two snapshots of one truth.

This change closes the review's three code-level beads and adopts the
rule the review nominated: every judgment the engine renders ships
with a self-application check, so the admission-basis class — a
judgment never applied to its own output — cannot ship sealed again.

## What Changes

- **Group 1 — the model's budget law aligns to the board.** The
  wire-FDB pricing leaves `grossFdbDraw`/`wireEnabled`/`applyWire`;
  dpio leaves the refusing pool fold; the retro-oracle's hand-copied
  wire effect folds onto `applyWire`; the replay gains the verdict
  re-derivation (rebuild the capacity face and gross draw from each
  prior state, assert the Rust `admit` agrees); one new frozen
  `envEatFdb → admit` trace witnesses the FDB refusal lane; all traces
  re-freeze against the amended law.
- **Group 2 — the model reaches what the engine does.** An orphaning
  environment action makes `Status::Orphan` reachable so `doPrune`
  fires and ORCH-I7's teardown half stops being vacuous; one frozen
  teardown trace replays; the Unimplemented refusal becomes
  whole-intent in the model (creates guarded on no unimplemented
  family pending), matching the engine and the model's own row doc.
- **Group 3 — one snapshot per admission judgment.** `plan_admission`
  derives face, pending, and switch work from ONE inventory read and
  returns the carrier (`Admission`, pool drift, switch work) so the
  dry-run sections render from the same snapshot; the offline pin
  gains the `render_admission` table asserts the board suite greps.
- **Process — judgment self-application becomes acceptance law.** A
  rendered judgment ships with (a) a fixpoint test (applied to its own
  converged output it yields the identity verdict) and (b) a twin-side
  verdict re-derivation in replay. Group 1's replay assertion is (b)'s
  first instance; the shipped converged-intent-re-admits tests are
  (a)'s template.

## Capabilities

### Modified Capabilities

- `orchestration-model`: the model's budget arithmetic states the
  board-proven law; every status in the vocabulary is reachable;
  refusal scope is whole-intent; replay re-derives verdicts and the
  trace set covers the FDB refusal and teardown lanes.
- `capacity-census`: one admission judgment derives from one
  observation snapshot.

## Validation

1. The F2-class oracle: with the verdict re-derivation in place and
   the budget law amended, replay stays green — and goes red if either
   side's budget arithmetic drifts (the review's trip trace
   `envEatFdb → admit` is frozen as the standing witness).
2. The reachability oracle: the Orphan-unreachability probe flips from
   `[ok]` to `[violation]`; the teardown trace replays.
3. The snapshot oracle: exactly one `read_inventory()` is reachable
   from `plan_admission`; `vadmit1_intents.rs` pins the dry-run table
   row the board suite greps.
4. Gates: quality floor, `model:validation` ladder (typecheck, test,
   simulate, retro-orch, confluence, replay, verify), spec-align per
   group.

## Impact

`models/orchestration/pipeline.qnt`, `retro_oracle.qnt`, the frozen
orchestration traces (re-frozen), `crates/dpaa2-verify/tests/
orchestration_replay.rs`, `crates/dpaa2-tools/src/{engine,main}.rs`,
`crates/dpaa2-tools/tests/vadmit1_intents.rs`, `models/COVERAGE.md`.
No northbound schema change, no new budgets, no board sitting
required (all witnesses are offline; the V-ADMIT-1 verdict stands).
The axq rollback change builds on the seams this change stabilizes
and follows it; #12 dpdmux consumes the de-vacuized confluence
evidence.
