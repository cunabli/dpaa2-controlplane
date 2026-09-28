# pool-hardening — tasks

Ordering is the epic directive: model-first (C leads; B's model half precedes
its Rust wiring; G early), one bead at a time through acceptance. Each group
is one bead → one parcel → one commit. Bead E (p6m.5) landed at 268a841,
outside this change.

## 1. Bead C — verify/model twin fidelity (dpaa2-controlplane-p6m.3)

- [x] 1.1 `models/families/pool_lifecycle.qnt` (~:115-119,148-152): ONE comment line naming the labeled-plugged residue operand (ADR-0020 decision 2; engine twin at `engine.rs:706`) — delete stale text rather than expand (L9)
- [x] 1.2 `models/families/dpio.qnt` (~:56-58): reboot-reconciliation clause cites both ADR-0008 §4 (race) and ADR-0003 §7 (recovery) (L20, model side)
- [x] 1.3 `crates/dpaa2-verify/tests/pool_replay.rs:320`: the below-draw refusal assert binds `drawn_managed()`, the netted accessor the production guard consumes (L7)
- [x] 1.4 pool replay `check_state`: per-state asserts for POOL_CUSTODY (drawn disjoint unplugged) and POOL_DPL_SURVIVES (born-present) — strengthen, never soften the COVERAGE marks (L8)
- [x] 1.5 `crates/dpaa2-api/src/families/dpio.rs` (~:314,330): same double citation as 1.2 on the Rust twin (L20, Rust side)
- [x] 1.6 Directed DpdkSeat run added to the dpio freeze harness; trace frozen under `models/traces/`, replays green (L25)
- [x] 1.7 Gates: `pnpm model:freeze-pool && pnpm model:freeze-dpio && git diff --exit-code models/traces` clean; pool + dpio replay suites green; `scripts/checks/quality-floor.sh` PASS
- [ ] 1.8 Close bead p6m.3; commit

## 2. Rule-amendment disposition (epic acceptance)

- [x] 2.1 Append repo rule 12 (asserting surfaces bind the production guard's named accessor, cited by name; `itf-replay` marks need a per-state assert) to `.claude/agents/rust-developer.md` — the sole home of the numbered repo rule set (opus48-developer.md carries execution rules only; duplicating the list would break rule 9)
- [x] 2.2 Record the adoption on the epic bead (p6m) notes — done; the commit waits on the user's call whether the untracked `.claude/agents/` dir joins the public repo

## 3. Bead G — ADR/design/retro residue (dpaa2-controlplane-p6m.7)

- [x] 3.1 `models/retro/reconciler.qnt` + `dpaa2-verify/README.md`: reframe the "mirrors RestoolMc" claim historical (pre-D9 chain, retired at f06fbb3) — NO re-freeze (L18)
- [x] 3.2 `docs/adr/0019` (~:180): grow-only qualifier on the wrong-cfg repair sentence (never live; across reboot = RebootRequired residue) (L6, ADR half)
- [x] 3.3 Archived pool-objects `design.md` D3/D9: one-line ADR-0020 pointer D10 already carries (L11)
- [x] 3.4 `docs/adr/0008` (~:414-417): open question marked resolved naming the default-fill fields (V-MVP-1 rev 2) (L12)
- [x] 3.5 Gates + close bead p6m.7; commit

## 4. Bead B — seat gate and typed child seat surplus (dpaa2-controlplane-p6m.2)

- [x] 4.1 Model side FIRST: answer the child seat-surplus question in `dpio.qnt` (grow-only residue at child scope, twin of the root arm); freeze/replay any new arm
- [x] 4.2 Wire the pure seat gate as the production caller for the grow loops (`engine.rs` ~:974-989, `populate.rs` ~:332-339); raw McStatus no longer surfaces (L10)
- [x] 4.3 Child seat surplus reports the typed grow-only residue instead of looping to untyped `Error::Backend "did not converge"` (`populate.rs` ~:109-118)
- [x] 4.4 Gates + close bead p6m.2; commit

## 5. Bead A — typed child-scope pool refusal at both discovery paths (dpaa2-controlplane-p6m.1)

- [x] 5.1 `converge_population` (`engine.rs` ~:1091-1137) surfaces the child `ShrinkBelowDraw` typed; the dead `ChildPlan::shrink_refusal` (`populate.rs` ~:141-146) is consumed or deleted (L4)
- [x] 5.2 Probe `-EBUSY` discovered-draw path (`pool.rs` ~:240-246) becomes the same typed face its docs promise (L4)
- [x] 5.3 Gates + close bead p6m.1; commit

## 6. Bead F — code-comment residue (dpaa2-controlplane-p6m.6)

- [x] 6.1 Grow-only qualifier, code half: `dpaa2-api/src/families/dpio.rs` (~:176) (L6)
- [x] 6.2 `contract/mc.rs`: retire the "companion-provisioning chain" description and the nonexistent `populate::populate_child` citation (L13)
- [x] 6.3 Stale prose deletions: `restool.rs` private-DPCON clause; `engine.rs` tile #5/#6 rustdoc + operator-visible error string; `dpni.rs` tile-#6 clause (L14)
- [x] 6.4 `intent/refuse/mod.rs`: delete the orphaned `PoolShortfall` clause (L15)
- [x] 6.5 `pool_lifecycle.rs` + `dpio.rs` cite qnt predicates by name, not line number; dpio "main loop's call" cites the ADR-0019 dpio row (L16, L17)
- [x] 6.6 Gates + close bead p6m.6; commit

## 7. Bead D — COVERAGE + baseline ledger amend (dpaa2-controlplane-p6m.4)

- [x] 7.1 `models/COVERAGE.md` pool-law ledger: ADR-0020 anchors, root-surplus-residue law row, reclaim sentence scoped to child custody, run count corrected (L1); labeled-plugged residue operand row (L9 half)
- [x] 7.2 Honest marks for still-unreplayed arms (root shrink suppression, probe-skipped refusal, D12 simulate-only) after C's freezes (L25 residue)
- [x] 7.3 Baseline routing: `dpbp.md` DPBP-I4 + `dprc.md` DPRC-I8 5y7/#10 routing and closed window (L5); w01/#9 rider; dpcon #9 carrier; DPNI-I5 status; `dpbp.md` unknown #2 adds V-POOL-5 rev 2 (L19, L21)
- [x] 7.4 Gates + close bead p6m.4; commit

## 8. Bead H — plan-type relocation and small folds (dpaa2-controlplane-p6m.8)

- [ ] 8.1 Move `PoolFamilyDrift`/`PoolDrift` and `PlannedChildDpni`/`ChildPlan` to `dpaa2-api::plan` beside `ContainerPlan`; observing fns stay adapter-side; render drops its dpaa2_mc+engine imports; seat-deficit arithmetic folds into the moved types (L22)
- [ ] 8.2 `engine.rs` custody judgment delegates to `pool_lifecycle::membership` (L23)
- [ ] 8.3 Small folds: shim `prunable_at_scope` twin; gate-message copies in main.rs; render family block; `bind_eth` honors `drivers_root`; `effective_queues` fallback fold (L24)
- [ ] 8.4 Gates + close bead p6m.8; commit

## 9. Close-out

- [ ] 9.1 Epic bead p6m acceptance check (all children closed, rule amendment dispositioned); docs/ROADMAP.md touch if #6-adjacent state changed
- [ ] 9.2 `openspec validate --strict` green; ready for review + archive
