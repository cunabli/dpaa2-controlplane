# PASS 1 report: dpmac-typestate (residue, deferrals, verify re-run)

Main is at 81f7b86, the end of the reviewed span. The working tree is clean apart from untracked `.claude/agents/` and `review/`.

## Verify re-run (how each obligation was checked)

| Obligation | Method | Result |
|---|---|---|
| 2.2 negative face: unbind-before-sever does not typecheck | `cargo test -p dpaa2-api --doc unbind` | holds. It is a `compile_fail` doctest at `crates/dpaa2-api/src/plan/transition.rs:161` and it passed. It landed in transition.rs, not compile_props.rs, which the epic did not touch. See F3 for its weakness. |
| 2.2 positive face, plan tests | `cargo test -p dpaa2-api --lib transition` (4 passed) and `--lib dpmac` (11 passed) | holds |
| 2.3 replay suites | `cargo test -p dpaa2-verify --test dpmac_replay` | 8/8 passed. This includes `every_committed_trace_is_listed` (an orphan guard that reads the trace directory, at dpmac_replay.rs:222) and `arbitration_table_is_covered_by_the_traces` |
| Frozen traces | grep of all 8 under `models/traces/families/dpmac/` against dpmac_replay.rs | every trace is referenced and none is orphaned. The directory guard confirms this mechanically. |
| 3.1 shim against captured fixtures | `cargo test -p dpaa2-mc --test shim` | 24 passed. Both fixtures are used (`dpmac_info_phy.txt` and `dpmac_info_fixed.txt`, shim.rs:36-37), so neither is orphaned. |
| 3.2 hal carrier primitive | `cargo test -p dpaa2-hal` | 5 passed |
| 4.1 hooks-never-gate proof | `cargo test -p dpaa2-tools --test port_detail` (3 passed) plus inspection | holds. `displayed_values_never_reach_the_plan` re-scripts carrier and counters between two observe runs; both plan zero transitions and neither diverges. |
| 5.1 operand pin | `cargo test -p dpaa2-tools --test vdpmac3_intents` (1 passed) | holds. Root needs dpmcp 18, dpbp 2, dpcon 32, 16 seats; child needs 1, 2, 6, 12 seats. |

## Sweeps (inspection and grep over the 47 epic-touched files, traces excluded)

- **Scaffolding:** no `todo!`, `unimplemented!`, `dbg!`, `allow(dead_code)`, `allow(unused…)`, `#[ignore]`, TODO, FIXME or XXX in any of them. No added line in the crates diff looks like commented-out code.
- **Wording from before the board outcomes:** nothing says unknown #1 is still pending, and nothing says the phantom-create semantics are unknown outside the recorded answer (dpmac.md:36 and :288-296 and COVERAGE:100 all carry the DPC-gated answer). The driverless-interval text in dpmac.md:182-203 states the kernel mechanism and the correct order; the I6 row at :279 records the typed resolution. Nothing is stated as an open hazard.
- **ponytail markers:** all six are pre-existing. fake.rs:607 blames to 8380277a (2026-09-24) and sits outside every epic hunk (442-468 and 795-800). The other five markers are in files the epic did not touch. The epic added no `ponytail:` line. It did add one unmarked simplification (F3).
- **Deferral carriers in COVERAGE:**
  - requests-down channel → #10: DPMAC-I4, :103
  - MC-view link read → #10, additive as one `McControl` method: :103
  - bulk statistics → #10: DPMAC-I7, :106
  - DPRTC-I4 → #13: :180
  - All of these are present.
- **Deferral carriers in `docs/baseline/dpmac.md`:** none of them are there. See F1 and F2.

## Findings

**PASS1-F1** · docs/baseline/dpmac.md:277 · stale · superseded-by: task 1.3 (409537f, COVERAGE re-anchored DPMAC-I4 to #10 under design D6), not carried through at 6.1 (81f7b86) · misleads-a-reader · amend: change the row's tail "→ `dpmac-typestate` (#7)" to route the requests-down read and the MC-view `dpni_get_link_state` read to `mc-portal-backend` (#10) as restool-absence rows, matching COVERAGE:103 · verification: `grep -n '(#7)' docs/baseline/dpmac.md` returns nothing, and `grep -n 'mc-portal-backend' docs/baseline/dpmac.md` hits the I4 row.

**PASS1-F2** · docs/baseline/dpmac.md:96 and :280 · doc-drift · bulk `dpmac_get_statistics` appears in the MC API notes (:96), and the DPMAC-I7 row (:280) is stamped verified. Neither says the bulk read is a deferral routed to #10, which COVERAGE:106 does. The brief requires the routing in both places. · misleads-a-reader · amend: add a clause to the I7 row (or :96) saying it is not whitelisted, never called by restool 2.4, and has its differential gate at `mc-portal-backend` (#10) · verification: `grep -n 'get_statistics.*#10\|#10.*statistics' docs/baseline/dpmac.md`.

**PASS1-F3** · crates/dpaa2-api/src/plan/transition.rs:56-57 (and the doctest at :161) · simplify (an unmarked ceiling; Pass 2 should judge it under mandate b) · superseded-by: n/a · misleads-a-reader · `SeveredProof(())` derives `Clone, Copy` and does not record which dpni it came from. `sever(dpni A)` therefore produces a proof that unbinds dpni B, and one proof can be reused for any number of unbinds. "Held by type" really means "some sever was minted", not "this edge was severed". The planner calls (reconcile.rs:120/123 and 209/212) pair them correctly, so today this is planner discipline, not a type guarantee. That contradicts the doc's claim that "the ordering is owned by the types, not by planner discipline" (:146-149). The model is a single-edge world (dpmac.qnt:256-358), so it cannot show the gap. Separately, the doctest is a plain `compile_fail` with no error code, so it would also pass if it failed to compile for an unrelated reason, such as a renamed import. · disposition: amend. Either store `DpniId` privately in the proof, make `unbind` check or take the id from the proof, and drop `Copy`; or add a `ponytail:` marker naming the ceiling. Pin the doctest as `compile_fail,E0423`. · verification: `cargo test -p dpaa2-api --doc unbind`, plus a second compile_fail case for cross-dpni or reused proofs if you choose binding.

**PASS1-F4** · models/COVERAGE.md:87 · stale · superseded-by: task 2.1 (f4782bf, the typed `MacRelation` judgment from D5) and the V-DPMAC-3 rev 1 inheritance witness that COVERAGE:101 already cites · misleads-a-reader · amend: the DPNI-I3 row still says "MAC value semantics → `dpmac-typestate` (#7)", which forward-routes to the change that delivered it. Cite `families/dpmac.rs` MacRelation and the V-DPMAC-3 rev 1 inheritance face instead. The epic diff never touched this row, even though the brief's grounding lists DPNI-I3 among the synced rows; Pass 4 should reconcile that. · verification: `grep -n 'DPNI-I3' models/COVERAGE.md` shows no `(#7)`.

**PASS1-F5** · models/board/README.md:85, :605, :881 · stale · superseded-by: task 1.3 (409537f) and design D6, which moved the raw link reads to #10 · misleads-a-reader (low) · amend with a forward pointer only: "re-anchored to `mc-portal-backend` (#10) by dpmac-typestate 1.3". These are dated V-LINK-4 records, so do not rewrite the verdict text. · verification: `grep -n "stay.* with .*dpmac-typestate. (#7)" models/board/README.md` hits only lines that carry the pointer.

## Footer

**What was read:** brief.md and tasks.md in full. The epic diff stat and hunk headers. transition.rs:45-245. The port_detail.rs proof test. vdpmac3_intents.rs assertions. dpmac_replay.rs function index. COVERAGE DPMAC, DPNI-I3 and DPRTC-I4 rows plus the COVERAGE diff. dpmac.md:10-20, 86-206, 275-312. Grep sweeps across all epic-touched files. Targeted cargo tests as listed above.

**What was deliberately not read:** the bodies of dpmac.qnt, dpmac.rs and dpmac_itf.rs (Pass 2's job); mc/hal/tools sources beyond test runs (Pass 3); ADR-0019, the proposal, design, specs and findings docs (Pass 4); board scripts (operator-sealed); compile_props.rs (untouched by the epic).

**Open questions:**
1. DPRTC-I4's baseline home is `docs/baseline/dprtc.md:145`, not dpmac.md, and neither baseline mentions #13. Is COVERAGE:180 enough, or should dprtc.md carry the re-anchor? Not raised as a finding.
2. The port_detail proof re-scripts carrier and counters, but not the MAC relation or the arbitration fields. Pass 3 should sweep those for side doors.
3. The brief says COVERAGE "+ DPNI-I3" was synced, but the diff shows it untouched (F4). Which is right?

Approximate tokens consumed: ~40k.
