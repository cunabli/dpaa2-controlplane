# Synthesis: pool-objects epic review (df950e0..9abb7fa)

All paths relative to .

## 0. Filter log (dropped / no-action, before the ledger)

- **Nothing filed against a protected decision.** All four passes stayed inside the fence; no finding proposes the DPL edit, reverses D9/D10/D12/ADR-0020, reopens the no-cfg-facet judgment, or touches the CHANGELOG. PASS2-F4 and PASS2-F3/PASS4-F6 explicitly preserve the recorded decisions and fix only missing qualifiers.
- **No unanchored staleness claims** — every stale row carries a superseding commit/task; none dropped on that ground.
- **Retired by settled facts:** Pass 1's "not executed" test caveats (pool_replay 7 + dpio_replay 9 green, full workspace 41 targets 0 failures, quality-floor PASS, openspec validate --strict green). Pass 4 OQ2's rescope caveat (bead 5y7 unchanged → DPMCP-I3 stays on #10 alone; docs already consistent, no finding).
- **Accepted, no action (recorded-deliberate or below the fold threshold):** PASS2-F8 (qnt `container()` helper copy — fold only when a third machine appears), PASS3-F11 (local ScriptedRunner — recorded "stands alone" choice), Pass 1 category-(b) items (fake.rs:535 ponytail with honest ceiling, `implicit_hasher` allow, `seatResidueReportedTest` simulate-only), Pass 1 category-(c) historical prose, Pass 2 OQ3 ruling (NO retro re-freeze — classifier granularity hides the pre-D9 expansion; the 3.15 precedent confirms re-freeze is reserved for classifier-visible changes).
- **Still not run:** the trace re-freeze idempotence check (`pnpm model:freeze-pool && model:freeze-dpio && git diff --exit-code models/traces`). Carried into Bead C acceptance.

## 1. Merged findings ledger (most severe first)

### Tier 1 — breaks-a-claim

| # | Merged from | What is wrong | Disposition → bead |
|---|---|---|---|
| L1 | PASS1-F1 + PASS4-F1-row + PASS2-F7(iii) | `models/COVERAGE.md:186-232` — the pool-law ledger has zero ADR-0020 anchors, no root-surplus-residue law row (`rootSurplusResidueTest`, destroy=0), the reclaim sentence unscoped to child custody, "fourteen runs" vs 15 frozen. The single break in an otherwise complete ADR-0020 trail; the 5.1 seal missed its own law table. | amend → **Bead D** |
| L2 | PASS4-F2 | `openspec/specs/reconciler/spec.md:321-328` — mainline requirement "consumer convergence is container-only … SHALL NOT emit companion-set sizing (tile #6)" contradicts the shipped engine; the pool-objects delta only ADDs, so archiving merges a self-contradictory spec. | MODIFIED entry → **Bead E** |
| L3 | PASS4-F3 + PASS1-F14(spec) | `openspec/specs/intent-compiler/spec.md:470-472` "sizing rules dormant until tiles #5/#6" is false (both shipped); `:196` reserves `PoolShortfall` which nothing implements (`ShrinkBelowDraw` + ADR-0020 residue took the role). Same file, one edit. | MODIFIED entry → **Bead E** |
| L4 | PASS3-F1 + PASS3-F2 (merged per Pass 3 OQ1) | Child-scope refusal typing hole: `converge_population` (crates/dpaa2-tools/src/engine.rs:1091-1137) collapses the child `ShrinkBelowDraw` to `Error::Config(String)` while `ChildPlan::shrink_refusal` (crates/dpaa2-mc/src/populate.rs:141-146) sits dead; and the probe `-EBUSY` discovered-draw path (crates/dpaa2-mc/src/pool.rs:240-246) never becomes the typed face its own docs promise — on the real restool backend this is the ONLY way a child below-draw surfaces, so fixing one without the other types only the fake path. | one code bead, both scopes → **Bead A** |

### Tier 2 — misleads-a-reader

| # | Merged from | What is wrong | Disposition → bead |
|---|---|---|---|
| L5 | PASS4-F1 + PASS1-F6 + PASS1-F7 | 4.4 NOT-FIRED survivors: `openspec/specs/mc-backend/spec.md:100-101`, `mbt-harness/spec.md:135` (spec side → **Bead E**); `docs/baseline/dpbp.md:144` (DPBP-I4), `docs/baseline/dprc.md:349` (DPRC-I8) missing the 5y7/#10 routing and the closed window (→ **Bead D**). | split E/D |
| L6 | PASS2-F3 = PASS4-F6 = P1-OQ2 | `crates/dpaa2-api/src/families/dpio.rs:176` and `docs/adr/0019-…:180` — "wrong-cfg repair is destroy + recreate / count-level destroy+create" lacks the grow-only qualifier ("never live; across the reboot boundary = RebootRequired residue"). Both decisions reconcilable; the missing qualifier is the defect. Two-sided fix, one wording. | code half → **Bead F**, ADR half → **Bead G** |
| L7 | PASS2-F5 | `crates/dpaa2-verify/tests/pool_replay.rs:320` uses `census.drawn()` where the refusal field is the NETTED base `drawn_managed()`; equal today only because the trace has born_drawn == 0 — a re-frozen trace with a born draw would encode the pre-D9 predicate. One-token amend. | amend → **Bead C** |
| L8 | PASS2-F6 | `models/COVERAGE.md:220-221` `itf-replay` marks overstate: POOL_CUSTODY's drawn ⊆ plugged and POOL_DPL_SURVIVES's born-present are never asserted per state. Cheapest fix ~3 lines in `check_state`; else soften the marks. | strengthen check_state → **Bead C** |
| L9 | PASS2-F4 | `models/families/pool_lifecycle.qnt:115-119,148-152` — the labeled-plugged residue operand (ADR-0020 decision 2, engine.rs:706) is documented only Rust-side; one qnt comment line prevents the next reader calling the engine operand a drift. Pairs with L1: the COVERAGE row and the qnt comment must both carry it (Pass 2 OQ2). | amend → **Bead C** (qnt) + **Bead D** (row) |
| L10 | PASS3-F3 + PASS3-F10 | Seat typing at child scope: pure `admit_seat`/-ERANGE gate has no production caller (raw grow loops at engine.rs:974-989, populate.rs:332-339 surface raw McStatus); and an unbound child with a seat SURPLUS loops to untyped `Error::Backend "did not converge"` (populate.rs:109-118 demands exact equality while dispatch can only grow) instead of the typed grow-only residue its root twin reports. | code bead → **Bead B** |
| L11 | PASS4-F4 | `openspec/changes/pool-objects/design.md:88, 203-205` — D3/D9 still promise runtime root reclaim; add the one-line ADR-0020 pointer D10 already carries. | amend → **Bead G** |
| L12 | PASS4-F5 | `docs/adr/0008-…:414-417` — the "which read-back field diverges is unpinned" open question fired and was answered (V-MVP-1 rev 2, bee68a1/1ebd61a); mark resolved naming the default-fill fields. | amend → **Bead G** |
| L13 | PASS1-F2 (+Pass 3 extension) | `crates/dpaa2-api/src/contract/mc.rs:17-20` still describes create_dpni as a "companion-provisioning chain"; `mc.rs:63` cites the nonexistent `populate::populate_child` (real: `plan_child_population`/`dispatch_child_population`). | amend → **Bead F** |
| L14 | PASS1-F4, F13, F15 (all Pass-3 confirmed) | Stale code prose: restool.rs:653-654 "private DPCON" clause (contradicts :662 in the same fn); engine.rs:600-601,625 "tiles #5/#6" rustdoc AND operator-visible error string; dpni.rs:838 "assign/move machinery (companion tile #6)". | amend → **Bead F** |
| L15 | PASS1-F14(code, Pass-3 confirmed) | `crates/dpaa2-api/src/intent/refuse/mod.rs:70-71` orphaned `PoolShortfall` reservation; delete the clause, `#[non_exhaustive]` stays justified by the change-#4 passthrough. | amend → **Bead F** |
| L16 | PASS1-F11 = PASS2-F1 | `pool_lifecycle.rs:442,450,458,555-557` cite qnt line numbers that all moved; cite predicates by name only. | amend → **Bead F** |
| L17 | PASS1-F12 = PASS2-F2 | `dpio.rs:45-47` "the main loop's call" — the judgment is recorded (ADR-0019 dpio row); cite it. | amend → **Bead F** |
| L18 | PASS1-F3 (Pass-4 upheld, Pass-2 ruled no re-freeze) | `models/retro/reconciler.qnt:8-13,59-61` + `crates/dpaa2-verify/README.md:221-222` claim the Create expansion "mirrors RestoolMc" — reframe historical ("pre-D9 chain, retired at f06fbb3"), no re-freeze. | amend → **Bead G** |
| L19 | PASS1-F5, F8, F9, F10 (all Pass-4 upheld) | Ledger routing residue: dpcon.qnt:9,:21 says "later #6 phase" vs COVERAGE's #9; dpcon.md:113-114 missing the #9 carrier; w01/#9 VFIO rebind-drift rider absent from COVERAGE AND baselines; COVERAGE:89 DPNI-I5 status "deferred" while its text says both halves landed. | amend → **Bead D** |
| L20 | PASS2-F9 | Same reboot-reconciliation clause cites ADR-0008 §4 in dpio.qnt:56-58 and ADR-0003 §7 in dpio.rs:314,330 — cite both (§4 race, §7 recovery). | amend → **Bead C** |
| L21 | PASS4-F7 | `docs/baseline/dpbp.md:153-157` unknown #2 cites only V-READBACK-1; add V-POOL-5 rev 2 (rev 1 read was void). Low. | amend → **Bead D** |

### Tier 3 — carries-cost (structure/duplication)

| # | Merged from | What | Disposition → bead |
|---|---|---|---|
| L22 | PASS3-F4 | Pure, renderable plan types (`PoolFamilyDrift`/`PoolDrift` in engine.rs:680-780, `PlannedChildDpni`/`ChildPlan` in populate.rs:56-146) live shell/adapter-side while their precedent (`ContainerPlan`) lives in dpaa2-api/src/plan/dprc.rs; render.rs imports dpaa2_mc+engine for pure data; #7/#8/TUI inherit it. Move types+judgments to `dpaa2-api::plan`, keep observing fns in place; absorbs the seat-deficit arithmetic duplicated engine.rs:975/populate.rs:335. The biggest seam-quality item for #7/#8. | → **Bead H** (lead item) |
| L23 | PASS3-F5 | engine.rs:465-478 hand-rolls the one-label custody judgment instead of delegating to `pool_lifecycle.rs` `membership` (:794-803) — a third expression of the law D10 fought to unify; not a reversal, a unification. | → **Bead H** |
| L24 | PASS3-F6, F7, F8, F9, F16 | Small folds: shim row-level `prunable_at_scope` twin (pool.rs:229-232); the ~6× copied gate message in main.rs; the verbatim render family block (render.rs:251-294 ↔ 355-389); `bind_eth` ignoring `drivers_root` while the epic's `unbind_eth` respects it (sysfs.rs); the duplicated `num_queues == 0` fallback (restool.rs:657-661 ↔ 680-684 → `effective_queues`). | → **Bead H** |
| L25 | PASS2-F7 | Unreplayed guard arms: DpdkSeat appears in NO frozen dpio trace (highest value — the `2·threads` ceiling arm is unit-only); root-suppression of shrinkBelowDraw, probe-skipped refusal, probeDpio refusal arms, D12 simulate-only (rides convergence.rs + V-MVP-1). Freeze the cheap ones, mark the rest in COVERAGE. Per Pass 2 OQ1: offline-cheap → new freeze work, not 5y7. | → **Bead C** |

## 2. Verdict — the core promise: **YES, with one typed-surface caveat**

The pool converges as anonymous P3 capacity, laws held on both sides of the sans-io seam.

| Law | Model side | Rust side | Witness |
|---|---|---|---|
| Count-declared, never identity-matched | `censusAdmitsCreate`, count-only guards | `admits_create`, count→individual crossing confined to pool.rs per D2 | pool_replay green (7), Pass 3 mandate (a) HOLDS |
| Level-triggered, idempotent | `isConverged` | `converged`; probe.rs delegates the whole bind judgment | replay per-state law `managed() == managedCount + foreign_drawn` every trace; V-POOL-6 rev 7 all-PASS |
| Free-only shrink, refusal not teardown | `shrinkEnabled` free-managed victim gate; `shrinkBelowDrawAt` | `shrink_enabled`; `ShrinkBelowDraw` typed core→PoolOutcome→main at root | `shrinkBelowDrawRefusedTest` three-legged (qnt:519, unit :1221, replay :301) |
| DPL-born survival + born-drawn netting (D9) | `prunable` root gate; `drawnManaged` | `foreign_free_unplugged`; `drawn_managed = drawn − born_drawn` | `envBornDrawnNetsTest` frozen + replayed with dedicated netting assert |
| Plug ≠ draw custody (D10) | plugged facet, two-step unplug→destroy | `ObservedPoolObject::{plugged,drawn}` split; reclaim unplugs first | replay unplug-of-drawn/still-plugged-destroy findings; one-label both arms at V-POOL-6 rev 7 |
| Typed root residue, destroy=0 (ADR-0020) | RootScope `(destroy, prune) = (0, foreign_free_unplugged)` | `drift_disposition` RootScope; rendered on ensure/status/dry-run; teardown reports what it cannot return | `rootSurplusResidueTest` frozen+replayed; V-POOL-6 rev 7 residue lines; V-MVP-1 rev 5 dpcon 34>32 |
| D12 same-run-rebuild refusal | `dpni_rebuild::driftDisposition` | `plan_present` run_created refusal, standing Disconnect→Unbind→Destroy | field-name parity; convergence.rs; V-MVP-1 rev 5 |

**Caveat (does not overturn the yes):** the laws hold both sides, but the *typed refusal surface* has two child-scope holes — L4 (child ShrinkBelowDraw stringly, probe -EBUSY untyped) and L10 (child seat surplus → untyped Backend error). Root-scope typing is complete end-to-end. These are Beads A and B.

## 3. Verdict — ADR-0002 isomorphism: **YES**

The shipped P3 substrate is structurally isomorphic to `pool_lifecycle.qnt` including all four amendments.

| Row | Evidence |
|---|---|
| Guard-for-guard map | censusAdmitsCreate↔admits_create, growEnabled↔grow_enabled, shrinkEnabled↔shrink_enabled, isConverged↔converged, shrinkBelowDrawAt↔shrinks_below_draw/ShrinkBelowDraw, drawnManaged↔drawn_managed, prunable↔foreign_free_unplugged, POOL_ROOT_GROW_ONLY↔drift_disposition destroy=0 (Pass 2 (a) HOLDS) |
| Shape divergence declared | machine-actions vs pure-predicates stated in the module doc per ADR-0002 §3 |
| Thin instantiations | dpbp/dpcon/dpmcp.qnt are param-only imports; dpcon's `NotificationEdge` is family-local; no law leaks either direction; dpio's separate machine is the sanctioned ADR-0019 shape |
| D9 two-sided | drawnManaged ↔ drawn − born_drawn; envBornDrawnNetsTest frozen, netted-base-0/raw-on-books asserted |
| D10 two-sided | plugged facet ↔ census straight-through split; replay identity sets |
| D12 two-sided | RebuildRefusal(Set[str]) ↔ refusals push, diff field names match by string; refusal leaves the object standing both sides |
| ADR-0020 two-sided | CustodyScope as data both sides; child-only shrink guards ↔ ChildScope-gated refusal; destroy=0 both sides |
| dpio seat variant | seatCeiling/seatDisposition/-ERANGE ↔ seat_ceiling/seat_disposition/admit_seat; DpioCfg Copy/setterless, no cfg facet through any side door — structural judgment stands |

Nits L7/L9/L20 are twin-*fidelity* (comments/test operands), not structural drift. L25 is coverage breadth, not isomorphism.

## 4. Follow-up work (bead-shaped, for one just-in-time openspec change)

**Bead A — Type the child-scope pool refusal at both discovery paths** (L4). Why: mandate (b) holds at root and breaks at child; on the real backend the probe -EBUSY is the only child below-draw surface. Acceptance: `PopulationOutcome::ShrinkRefused{label, refusal}` consulted from `ChildPlan::shrink_refusal()` before dispatch (mirror of engine.rs:899-909); pool-reclaim-path `McStatus{0x10}` mapped to the ShrinkBelowDraw face or both doc claims (pool.rs:240-246, pool_lifecycle.rs:739-743) amended to "propagates raw"; FakeBackend tests: child drawn>requirement and `with_in_use_pool_object` through converge_pools each assert the typed outcome; quality floor green.

**Bead B — Wire the seat gate and type the child seat surplus** (L10). Why: the pure -ERANGE gate exists unconsumed and an unbound child seat surplus loops to an untyped Backend error where its root twin reports RebootRequired. Acceptance: `admit_seat` pre-gates both grow loops (engine.rs:974-989, populate.rs:332-339) or the raw-status surface is recorded as the decision; child seat-surplus convergence yields the typed grow-only residue, not exact-equality failure; FakeBackend test seeds child seats > derived and asserts typed outcome; Pass-2 check whether dpio.qnt admits the child-surplus state (Pass 3 OQ3) closed either way in a model comment.

**Bead C — Verify/model twin-fidelity parcel** (L7, L8, L9-qnt, L20, L25 + the unrun idempotence check). Why: three small fidelity gaps are the same defect class the board caught twice at rev cost (see §5). Acceptance: pool_replay.rs:320 uses `drawn_managed()`; check_state asserts `drawn ⊥ unplugged` and born-present (or the COVERAGE marks are softened — pick strengthen); qnt comment names the labeled-plugged residue operand; ADR-0008 §4 + ADR-0003 §7 cited on both dpio sides; a directed DpdkSeat run frozen and replayed (the only regime never trace-driven); `pnpm model:freeze-pool && model:freeze-dpio && git diff --exit-code models/traces` clean; both replay suites green.

**Bead D — COVERAGE + baseline ledger amend** (L1 lead, L5-baseline half, L9-row, L19, L21). Why: the ledger is the 5.1 deliverable and it omits the ADR-0020 law it sealed. Acceptance: pool-law section carries the ADR-0020 anchor + a root-surplus-residue row (rootSurplusResidueTest, destroy=0), child-scoped reclaim sentence, 15-run count; dpbp.md:144/dprc.md:349 name 5y7 + closed window; dpcon.md:113-114 + dpcon.qnt:9,:21 carry #9; a w01→#9 locality row (OI-3 shape); COVERAGE:89 status fixed; dpbp.md unknown #2 cites V-POOL-5 rev 2; Pass-1 grep verifications all pass.

**Bead E — Mainline spec touch** (L2, L3, L5-spec half, L15's spec sibling). Why: archiving pool-objects as-is merges a self-contradictory reconciler spec and two false scenario claims; deltas only ADD today. Acceptance: MODIFIED entries (in the pool-objects deltas if convention allows pre-archive, else this new change) retiring reconciler's container-only fence, intent-compiler's "dormant until #5/#6" + the PoolShortfall reservation, and routing mc-backend:100-101/mbt-harness:135 DPRC-I8 to 5y7/#10; `openspec validate --strict` green; the Pass-4 greps return clean.

**Bead F — Code-comment residue parcel** (L6-code, L13, L14, L15-code, L16, L17). Why: eight stale prose sites, one operator-visible, all one-line amends with grep verifications already written. Acceptance: every Pass-1/Pass-3 grep in the ledger rows returns the stated result; the dpio.rs:176 sentence carries the never-live/reboot-boundary qualifier; quality floor green.

**Bead G — ADR/design/retro residue parcel** (L6-ADR, L11, L12, L18). Why: two ADRs and the epic design carry pre-amendment promises; the retro model claims present-tense shim mirroring. Acceptance: ADR-0019:180 qualified (judgment untouched); design.md D3/D9 carry the ADR-0020 pointer; ADR-0008 open question marked resolved with the fields and witness; retro/reconciler.qnt + verify README reframed historical, explicitly NO re-freeze.

**Bead H — Plan-type relocation and small folds** (L22 lead, L23, L24). Why: pure plan types on the io side is the divergence pressure #7/#8 hit first; the rest are copy-cost. Acceptance: `PoolDrift`/`ChildPlan` (+judgments) live in `dpaa2-api::plan`, render.rs imports only dpaa2_api types; one-label candidacy delegates to a core predicate with both arms unit-tested; `effective_queues`, gate-message, render-helper, `prunable_at`, `bind_eth` drivers_root folds land with their Pass-3 grep counts; quality floor green. Suggest sequencing L22/L23 first (seam-shaping), the folds behind them.

Suggested order: A → B (typed surfaces, share test scaffolding) ∥ C; then D ∥ F ∥ G; E when the openspec mechanism is settled (Pass 4 OQ1); H last.

## 5. Rule-amendment nomination (one, per mandate)

Both candidate classes are the same defect: **an asserting surface bound to a different operand than the production guard**. The V-POOL-6 rev 1–3 double-feed was the census consuming an operand the model didn't (companion counted twice); the 3.14 audit was suite legs asserting laws the shipped census could not execute; and the class recurred post-rules inside this very epic's verify layer — PASS2-F5 (replay asserts `drawn()` where the guard consumes `drawn_managed()`) and PASS2-F6 (COVERAGE `itf-replay` marks claiming per-state laws the replay never executes). Nominated amendment: *"Every operand a suite, replay, or COVERAGE mark asserts must be the same named accessor the production guard consumes, cited by name; an `itf-replay` mark requires a per-state assert of the named law — transition-level inference does not earn the mark."* One rule fences both recorded classes and the two in-epic recurrences.
