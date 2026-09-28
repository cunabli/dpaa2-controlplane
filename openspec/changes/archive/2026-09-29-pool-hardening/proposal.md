# pool-hardening

## Why

The pool-objects epic review (archive/2026-09-27-pool-objects/review/synthesis.md) merged 25 findings into 8 parcels: core promise YES with one typed-surface caveat, ADR-0002 isomorphism YES. This change lands the dispositioned parcels (epic bead dpaa2-controlplane-p6m). The caveat is the lead defect: on the real restool backend a child below-draw shrink surfaces ONLY through the probe `-EBUSY` path, and today both discovery paths collapse to untyped `Error::Config(String)` — the typed refusal the specs promise exists but is dead code.

## What Changes

- **Bead C (p6m.3, leads — model-first):** verify/model twin fidelity. `pool_replay.rs` binds the refusal assert to `drawn_managed()` (the operand the production guard consumes, L7); `check_state` gains per-state asserts for drawn-disjoint-unplugged and born-present so the `itf-replay` COVERAGE marks are earned, not inferred (L8); one qnt comment line names the labeled-plugged residue operand (ADR-0020 decision 2, L9); the reboot-reconciliation clause cites ADR-0008 §4 + ADR-0003 §7 on both dpio twins (L20); a directed DpdkSeat run is frozen and replayed — that regime has no frozen trace at all (L25); re-freeze idempotence re-verified after the new freeze.
- **Bead A (p6m.1):** the child-scope `ShrinkBelowDraw` refusal is typed at both discovery paths — `converge_population` and the probe `-EBUSY` discovered-draw path — retiring the dead `ChildPlan::shrink_refusal` (L4).
- **Bead B (p6m.2):** the pure seat gate gains its production caller and a child seat SURPLUS surfaces as the typed grow-only residue its root twin reports, instead of looping to untyped "did not converge" (L10).
- **Bead D (p6m.4):** COVERAGE + baseline ledger amends — ADR-0020 anchors and the root-surplus-residue law row, 5y7/#10 routing, w01/#9 rider, dpcon #9 carrier, DPNI-I5 status (L1, L5, L19, L21; L9 row half).
- **Bead F (p6m.6):** code-comment residue — stale tile fences, dead `PoolShortfall` clause, moved qnt line citations, grow-only qualifier code half (L6, L13–L17).
- **Bead G (p6m.7):** ADR/design/retro residue — grow-only qualifier ADR half, D3/D9 reclaim pointer, ADR-0008 open question resolved, retro-model prose reframed historical, no re-freeze (L6, L11, L12, L18).
- **Bead H (p6m.8):** plan types (`PoolFamilyDrift`/`PoolDrift`, `PlannedChildDpni`/`ChildPlan`) relocate to `dpaa2-api::plan` beside their `ContainerPlan` precedent; custody judgment delegates to `pool_lifecycle::membership`; small folds (L22–L24).
- **Rule amendment (nominated by the synthesis, adopted at this scoping):** repo rule 12 — *an asserting surface (suite, replay, or COVERAGE mark) binds the same named accessor the production guard consumes, cited by name; an `itf-replay` mark requires a per-state assert of the named law.* Fences the two recorded classes (V-POOL-6 rev 1–3 double-feed, 3.14 audit) and both in-epic recurrences (PASS2-F5/F6).
- Bead E (p6m.5) already landed directly on the mainline specs (268a841) and is outside this change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `reconciler`: child-scope pool refusals surface typed at both discovery paths (planned shrink and probe-discovered draw); a child seat surplus reports the typed grow-only residue instead of an untyped convergence error.
- `formal-models`: `itf-replay` marks require per-state asserts of the named law (drawn disjoint unplugged; born-present); the frozen dpio corpus covers the DpdkSeat regime.

## Impact

- `crates/dpaa2-verify` (pool_replay.rs, dpio replay `check_state`), `models/families/*.qnt`, `models/traces/` (new DpdkSeat freeze), `models/COVERAGE.md`
- `crates/dpaa2-tools/src/engine.rs`, `crates/dpaa2-mc/src/{populate.rs,pool.rs,restool.rs}`, `crates/dpaa2-api` (refusal + plan types, render seam)
- `docs/baseline/{dpbp,dprc,dpcon}.md`, `docs/adr/{0003,0008,0019}` citations, pool-objects archived design annotations (G's targets)
- `.claude/agents/{rust-developer,opus48-developer}.md` (rule 12)
- Ordering is model-first (user directive): C leads; B's dpio.qnt question answers model-side before Rust wiring; G's retro reframe early. Text dispositions prefer deletion over amendment.
