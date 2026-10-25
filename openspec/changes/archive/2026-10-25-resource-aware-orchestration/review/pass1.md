# Pass 1: Residue, deferrals, and verify re-run (`resource-aware-orchestration`)

Every offline gate passed. The sweep found no leftover scaffolding in the epic-touched files. All six pre-logged carries are confirmed and located. On top of those there are four new stale or doc-drift hits in docs and comments; none is in code logic.

HEAD = 90d1f4a (same as the brief's grounding). Repo root is `<repo-root>` (shortened to `R` below).

## Gates re-run

| Gate | Result |
|---|---|
| `cargo test -p dpaa2-verify` | **PASS**. All suites green, 0 failed, 0 ignored. Lib: 158/158 (includes 8 `orchestration_itf` twin tests). `orchestration_replay`: 3/3 (`orchestration_traces_replay_green`, `row_set_twin_agrees_with_the_model`, `every_committed_trace_is_listed`). The `TRACES` list holds exactly the 6 frozen traces (converge, handoverSafe, admissionRefuses, stableRefusal, unimplementedRefuses, envInterference), and the orphan check is enforced at `orchestration_replay.rs:196-218`. `admission_oracle`: **5/5**, not 4 — the 4 scenarios plus `refusals_are_idempotent`. `converged_intent_re_admits_against_zero_free` passes. `ledger_lint`: 2/2. |
| `cargo test -p dpaa2-tools` | **PASS**. Lib 33/33, including the `admission_admits_a_converged_board_with_zero_free`, `admission_refuses_an_over_budget_intent_before_any_verb` and 4 render admission tests. `vadmit1_intents`: 1/1 (`intent_over_is_refused_naming_the_dpcon_budget`). All other integration suites green. |
| `pnpm model:retro-orch` | **PASS**, 13/13. Both baselines green, the mask of each of the 9 families is red, ORCH-I2 live-read removed is red. |
| `pnpm model:typecheck` | **PASS**, exit 0. |
| `pnpm model:confluence` (20x200) | **PASS**. `[ok] No violation found` (442ms, traces of length 21). |
| `scripts/checks/quality-floor.sh` | **PASS**. fmt, clippy `-D warnings` and doc all clean. |

## Scaffolding sweep (epic-touched files, from the trailer-commit enumeration)

- **None found:** `todo!`, `unimplemented!`, `dbg!`, `#[allow(dead_code)]`, `#[ignore]`, TODO/FIXME/XXX/HACK, or commented-out code. The one candidate, `pipeline.qnt:546` (`// let the kernel complete…`), is prose.
- **`Unimplemented`:** every hit (dispatch.rs 8, orchestration_itf.rs 17, orchestration_replay.rs 10, pipeline.qnt 15, engine.rs 2, admission.rs 1, retro_oracle.qnt 2) is the designed `Disposition::Unimplemented` row, its model twin, or its trigger. None is the macro.
- **`ponytail:` markers:** the 13 trailer commits add or remove no `ponytail` line (`git show` grep is empty). No existing marker sits in an epic-touched file, so none went stale. No unmarked ceiling-class simplification was found; carry 2 is the closest, and it is a carry, not a hidden ceiling.

## Deferral carriers (all confirmed)

| Deferral | Carrier |
|---|---|
| Rollback | `R/docs/adr/0023-orchestration-is-a-guarded-action-convergence-model.md:141` and `:209-214` name `dpaa2-controlplane-axq`. `bd show` reports the bead as OPEN, P2, "new openspec change after #15 archives". |
| Net admission | design.md:128-129 (D8 "named revisit trigger"), ADR-0023:117, COVERAGE.md:544. |
| Full confluence | ADR-0023:192 and :207-208 (revisit trigger), COVERAGE.md:534 (ORCH-I6 row, simulate, cost 666 pairs/state) and :546. |
| Budgets beyond the two | design.md:39 (Non-Goals), proposal.md:65, ADR-0023:199-201, COVERAGE.md:545. |

## Findings

**PASS1-F1 (carry 1, WANT_PORT gross-FDB asymmetry)**
- Location: `R/models/orchestration/pipeline.qnt:176` (`+ (if (WANT_PORT) 1 else 0)` in `grossFdbDraw`; the constant is at :99) vs `R/crates/dpaa2-api/src/plan/admission.rs:30-31`, where Rust says the port wire "stays a dispatch-time re-observation".
- Category: twin-drift. Superseded-by: n/a. Severity: carries-cost — all 6 traces replay green today, but an FDB-drawing edge trace would trip.
- Disposition: follow-up bead. Pass 2 decides which side wins.
- Verification: `grep -n WANT_PORT models/orchestration/pipeline.qnt crates/dpaa2-api/src/plan/admission.rs`, then `cargo test -p dpaa2-verify --test orchestration_replay` after an FDB-edge trace is frozen.

**PASS1-F2 (carry 2, double inventory read)**
- Location: `R/crates/dpaa2-tools/src/engine.rs:1294-1295`. `plan_admission` calls `plan_pools` (which reads inventory at :1235), then calls `mc.read_inventory()` again at :1295 for the FDB bracket and label count.
- A third snapshot comes from `plan_switches` (`observe_pool(None, Dpsw)`, around engine.rs:911). The FDB draw counts dpsw labels from the second inventory read, while `fdb_pending` comes from that third observation.
- The struct doc at :1269 ("one read serves both") and the commit e7954b1 description ("ONE read_inventory") both overclaim.
- Category: duplicate. Superseded-by: n/a. Severity: carries-cost, and misleads-a-reader for the :1269 doc.
- Disposition: fold. Have `plan_pools` return or accept the inventory, or pass it into `plan_admission`.
- Verification: `grep -n "read_inventory()" crates/dpaa2-tools/src/engine.rs` should show one call reachable from `plan_admission`.

**PASS1-F3 (carry 3, "banked fixtures" wording)**
- Location: `R/openspec/changes/resource-aware-orchestration/proposal.md:112`, `tasks.md:62` and `tasks.md:70` say "banked (census) fixtures".
- The fixtures are actually hand-built in banked shape. The code says so honestly at `R/crates/dpaa2-api/src/plan/capacity.rs:129` ("the banked-fixture shape") and `R/crates/dpaa2-verify/tests/admission_oracle.rs:27`.
- Category: doc-drift. Superseded-by: n/a. Severity: misleads-a-reader.
- Disposition: amend to "hand-built in banked shape".
- Verification: `grep -n "banked.*fixture" openspec/changes/resource-aware-orchestration/{proposal,tasks}.md`.

**PASS1-F4 (carry 4, dpio four-family wording)**
- Location: `R/openspec/changes/resource-aware-orchestration/specs/capacity-census/spec.md:11` lists the "per-container free pools (dpmcp/dpbp/dpio/dpcon)" as four free-pool families.
- The shipped face is the POOL_TRIO plus dpio seats as `Spent` (engine.rs `plan_pools`; admission.rs:237). The same spec's line 13 says dpio is non-reclaimable, so lines 11 and 13 conflict.
- Category: doc-drift. Superseded-by: n/a. Severity: misleads-a-reader.
- Disposition: amend to "dpmcp/dpbp/dpcon free pools; dpio seats as spent headroom".
- Verification: `grep -n "dpmcp/dpbp/dpio/dpcon" openspec/changes/resource-aware-orchestration/specs/capacity-census/spec.md` should return nothing.

**PASS1-F5 (carry 5, "shortfall" vs "headroom")**
- The spec says the user-facing refusal names a "shortfall":
  - `R/openspec/changes/resource-aware-orchestration/specs/provisioning-cli/spec.md:24` ("budget, draw, and shortfall named")
  - `specs/capacity-census/spec.md:35` (scenario title)
  - `proposal.md:69`
- The shipped render uses `draw=… headroom=…` (`R/crates/dpaa2-tools/src/render.rs:375`, `:393`; test `the_ensure_refusal_voice_names_budget_draw_and_headroom`).
- ADR-0023:121-137 and :209-213 use "shortfall" for a different concept: a mid-run shortfall after admission, which is the rollback trigger. That is legitimate and should stay.
- Category: doc-drift. Superseded-by: n/a. Severity: misleads-a-reader.
- Disposition: amend the two spec lines and the proposal line to "headroom". Leave the ADR.
- Verification: `grep -n shortfall openspec/changes/resource-aware-orchestration/specs -r` should return nothing.

**PASS1-F6 (carry 6, dry-run does not preview the Unimplemented refusal)**
- `R/openspec/changes/resource-aware-orchestration/specs/provisioning-cli/spec.md:11-12` sits under "Dry-run renders admission verdicts" and says "An intent naming an `Unimplemented` family renders that refusal with its retirement pointer".
- `reify_guard` runs only on the ensure path (`R/crates/dpaa2-tools/src/main.rs:293`). The dry-run path (`main.rs:225`) calls only `plan_admission`, and `admission.rs:64` says Unimplemented draws nothing. So dry-run never renders that refusal.
- This is the one user-facing doc that contradicts the adopted dispatch-path reading.
- Category: doc-drift. Superseded-by: n/a. Severity: breaks-a-claim, because it is SHALL-level requirement text under a dry-run heading.
- Disposition: amend so the sentence binds to `ensure`, or note that dry-run shows the admission face only.
- Verification: `grep -n "reify_guard(" crates/dpaa2-tools/src/main.rs` (ensure only), plus a re-read of spec.md:11.

**PASS1-F7 (FDB open question still open)**
- Location: `R/openspec/changes/resource-aware-orchestration/design.md:241-243`. The Open Question still reads "characterize vs conservative constant, resolved when the admission oracle lands".
- It was resolved as characterize (`Unknown` admits) at the m7a.4 seal (4df6102). The code records this at `capacity.rs:44` ("nothing here invents a number") and :230, but nothing a reader would look at does (brief grounding item 9).
- Category: stale. Superseded-by: 4df6102 (task 4.1/4.2, m7a.4 seal). Severity: misleads-a-reader.
- Disposition: amend — mark resolved as characterize, with a pointer to `capacity.rs` and ADR-0011 admit-and-warn.
- Verification: `grep -n "conservative constant" openspec/changes/resource-aware-orchestration/design.md` should show a "resolved" annotation.

**PASS1-F8 (registry open question already decided)**
- Location: `R/openspec/changes/resource-aware-orchestration/design.md:238-240`. The Open Question "hand-written exhaustive match or macro-derived" is still listed as open.
- It was decided by a81ee32 (task 3.1): `plan/dispatch.rs` is a hand-written exhaustive match.
- Category: stale. Superseded-by: a81ee32 (task 3.1). Severity: misleads-a-reader.
- Disposition: amend (mark decided).
- Verification: re-read design.md:238 and confirm `plan/dispatch.rs` has no macro derivation.

**PASS1-F9 (model comment says "full derived draw")**
- Location: `R/models/orchestration/pipeline.qnt:170-171`. The comment reads "Gross draw (ADR-0023 design D8): the plan's full derived draw, planned frees uncounted".
- The fold is over `pend` (pending creates). Under the amended D8 the draw basis is pending creates, so "full derived draw" reads as the pre-fix full-desired-plan claim.
- The model semantics are correct; only the wording is off, which fits the brief's note that the model needed no change.
- Category: stale. Superseded-by: the pending-creates fix squashed into 4df6102 (D8 amendment; ADR amendment 537ed4b). Severity: misleads-a-reader.
- Disposition: amend to "the pending creates' draw".
- Verification: `grep -n "full derived draw" models/orchestration/pipeline.qnt` should return nothing.

**PASS1-F10 (task 4.2 text describes the pre-fix basis)**
- Location: `R/openspec/changes/resource-aware-orchestration/tasks.md:64-65`. Task 4.2 says "gross draw derivation (pure, over the compiled plan; creates counted…)".
- The shipped basis is pending creates from the passes' convergence reads (`GrossDraw::from_pending`, admission.rs:67, with the counts composed in engine.rs `plan_admission`). It is not a pure fold over the compiled plan.
- Category: stale. Superseded-by: the pending-creates fix squashed into 4df6102/e7954b1. Severity: misleads-a-reader.
- Disposition: amend (annotate the amended basis).
- Verification: `grep -n "over the compiled plan" openspec/changes/resource-aware-orchestration/tasks.md`.

**PASS1-F11 (proposal still says the sitting is in the future)**
- Location: `R/openspec/changes/resource-aware-orchestration/proposal.md:112-114`. It reads "one board suite row confirms it live on the next sitting that runs anyway".
- The sitting already happened: V-ADMIT-1 rev 1 passed 6/6 on 2026-10-25.
- Category: stale. Superseded-by: 90d1f4a (task 6.1). Severity: misleads-a-reader, low — the proposal is a historical artifact, so Pass 4 judges whether to amend.
- Disposition: amend (add "witnessed: V-ADMIT-1 rev 1") or accept as historical.
- Verification: `grep -n "next sitting" openspec/changes/resource-aware-orchestration/proposal.md`.

No "pending sitting" claim survives: `models/board/README.md:112`, VERDICTS.json and COVERAGE all say passed. Task 6.1's "riding the next sitting" at `tasks.md:86` is ticked task text and acceptable.

## Footer

**What was read:**
- `review/brief.md` in full.
- The enumeration of epic-touched files.
- Targeted reads:
  - `engine.rs:1225-1340`
  - `main.rs:215-235`
  - `pipeline.qnt:160-200`
  - `design.md:225-250`
  - `proposal.md:105-116`
  - `tasks.md:58-72`
  - `capacity-census/spec.md:8-16`
  - `provisioning-cli/spec.md:1-30`
  - `ADR-0023:200-220`
  - `COVERAGE.md:544-548`
  - the `orchestration_replay.rs` TRACES list
- Grep sweeps across the epic-touched crates, models, docs and openspec files.
- The bead record for `dpaa2-controlplane-axq` (via `bd show`).

**Deliberately not read:**
- `dispatch.rs`, `orchestration_itf.rs` and the full ADR-0023 text (Pass 2/4 scope).
- The frozen trace contents.
- The `V-ADMIT-1.sh` protocol (Pass 3).
- The VERDICTS.json `source_hash`.
- Out-of-scope dpsw-hardening surfaces.
- `pnpm model:test`/`simulate`/`verify` were not re-run. They are not in the Pass 1 mandate, and typecheck plus confluence covered the Quint gate.

**Open questions:**
1. The brief says "admission_oracle 4 scenarios", but 5 tests exist; the fifth is `refusals_are_idempotent`, the task 4.2 idempotence obligation. Should records say 4 scenarios plus 1 idempotence test?
2. F2's third snapshot (`plan_switches` observe vs inventory labels for the FDB draw) widens carry 2 beyond what was pre-logged. Pass 3 should cost whether the two views can disagree within one judgment.
3. F6: should the spec move to ensure-only, or should dry-run call `reify_guard` read-only? The adopted reading favours the former.
