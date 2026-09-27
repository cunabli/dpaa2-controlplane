# Pass 1: residue, deferrals, and verify re-run (pool-objects, df950e0..9abb7fa, HEAD = 9abb7fa)

Nothing was modified. I did not run `cargo test` or `quint`, because they write to `target/` and the read-only mandate forbids that. The verify obligations below were checked by grep and structure only. The exact commands to re-run are in the footer.

## Verify re-run (tasks.md, offline-checkable)

Every grep-checkable obligation still holds:
- **3.6:** the per-port provisioning chain is gone from the shim. `provision_chain`, `dpni_dep_steps` and `ensure_dpio` appear only in `models/retro/reconciler.qnt` (see F3).
- **2.1:** ADR-0019 carries the P3 reference-implementation line (0019:119, :334).
- **2.3:** ADR-0019 records the no-cfg-facet dpio row (0019:180).
- **3.16:** `KernelControl::unbind` exists (`contract/kernel.rs:22`), backed by `hal sysfs.rs:83` and `:164`.
- **3.15:** the teardown order is disconnect → unbind → destroy (`reconcile.rs:606`, `:827`).
- **4.1 / 3.7:** the named suites exist: `pool_replay.rs`, `dpio_replay.rs`, `vpool6_intents.rs`, `vmvp_intents.rs`.

**Trace coverage:** all 15 dpbp and 4 dpio frozen traces are listed in the replay tables. Both replay files have an `every_committed_trace_is_listed` orphan guard (`pool_replay.rs:427`, `dpio_replay.rs:212`). There are no dpcon or dpmcp trace directories. `package.json` `model:freeze-pool` includes `rootSurplusResidueTest`. `dpio.qnt:277` `seatResidueReportedTest` is deliberately not frozen: COVERAGE marks it `simulate` only, and there is a unit twin at `dpio.rs:640`.

**Scaffolding sweep** over the epic-added lines in crates/ and models/: no `todo!`, `unimplemented!`, `dbg!`, `#[allow(dead_code)]`, `#[ignore]`, TODO/FIXME or commented-out code. The only allows are `clippy::implicit_hasher` at `engine.rs:1252` and `clippy::too_many_lines`, both justified inline. There are no orphaned insta snapshots and no stray `.snap.new` files.

**Unpaced-grow caveats:** none are stale. ADR-0008 lines 341–349 and 405–409 record the no-pace decision itself.

## Findings

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F1 | models/COVERAGE.md:196-197, 202-207, 222 (table 213-232) | stale / doc-drift | 73f8df9 (task 3.18, ADR-0020) | breaks-a-claim | amend | `grep -c 'ADR-0020\|rootSurplusResidueTest' models/COVERAGE.md` should be ≥ 2 (today it is 0) and `grep -n 'fourteen runs' models/COVERAGE.md` should return nothing |

F1 detail: the Pool convergence laws section never mentions ADR-0020. It says "The fourteen runs are frozen", but 15 are frozen. It states "reclaim is the unplug-probe law" without saying the law is child-scoped. It has no law row for root-surplus-is-residue (`rootSurplusResidueTest`, destroy=0). 5.1 (9abb7fa) sealed this section without the ADR-0020 amendment.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F2 | crates/dpaa2-api/src/contract/mc.rs:17-20 | stale | f06fbb3 (task 3.6, D9) | misleads-a-reader | amend: `create_dpni` is create+stamp, so the "one exception" clause is gone; keep the ADR-0018 pointer only for child population | `grep -n 'companion-provisioning chain' crates/dpaa2-api/src/contract/mc.rs` should return nothing |

F2 detail: the trait doc still says `create_dpni` "is a transactional companion-provisioning chain (shim policy today)".

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F3 | models/retro/reconciler.qnt:8-13, 59-61 + crates/dpaa2-verify/README.md:221-222 | stale / doc-drift | f06fbb3 (task 3.6, D9) | misleads-a-reader | amend: add a note that the retro trace encodes the pre-D9 expansion and stays valid because the classifier hides sub-steps. Do not re-freeze unless Pass 2 says so | `grep -n 'mirrors RestoolMc\|RestoolMc::create_dpni order' models/retro/reconciler.qnt` should return nothing |

F3 detail: both files claim the `Create{port}` expansion "mirrors RestoolMc" (ensure_dpio followed by the dpbp/dpmcp/dpcon dependency chain). The shim no longer does this. The file was epic-touched at a6c1dfb, but only the teardown order was fixed.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F4 | crates/dpaa2-mc/src/restool.rs:653-654 | stale | f06fbb3 (task 3.6, D9) | misleads-a-reader | amend: drop the private-DPCON clause | `grep -n 'private DPCON' crates/dpaa2-mc/src/restool.rs` should return nothing |

F4 detail: the comment says "the host-derived fallback … which the private DPCON count follows". No private per-port DPCON is created any more; the pool construct provides them.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F5 | models/families/dpcon.qnt:9, :21 | stale / doc-drift | 9abb7fa (5.1 re-routed DPCON-I4 to #9 in COVERAGE:124) | misleads-a-reader | amend: say "cross-dprc-links (#9)" | `grep -n 'later #6' models/families/dpcon.qnt` should return nothing |

F5 detail: the file says "retarget dynamics land in a later #6 phase". #6 is closed, and this contradicts COVERAGE. This is the carrier twin of the #9 routing.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F6 | docs/baseline/dpbp.md:144 | stale / doc-drift | dff678b (task 4.4 NOT FIRED, bead 960.13) | misleads-a-reader | amend to match COVERAGE:112 (route to 5y7 / #10, DPL window closed) | `grep -n 'DPBP-I4' docs/baseline/dpbp.md \| grep -c 5y7` should be 1 |

F6 detail: the DPBP-I4 row still reads "root-only until a DPL-defined child or the raw command path (#10)". It does not name 5y7 or the closed window. 9abb7fa edited dpbp.md (DPBP-I2 and I3) but left I4.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F7 | docs/baseline/dprc.md:349 | doc-drift | dff678b (task 4.4 NOT FIRED) | misleads-a-reader | amend: route to bead 5y7 / #10 | `grep -n 'DPRC-I8' docs/baseline/dprc.md \| grep -c 5y7` should be 1 |

F7 detail: the DPRC-I8 row reads "needs a DPL-defined child or the raw command path (#10)" with no 5y7. COVERAGE:79 routes it to 5y7 → #10. dprc.md itself was not touched by the epic.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F8 | docs/baseline/dpcon.md:113-114 | doc-drift | 9abb7fa (5.1 routing) | misleads-a-reader | amend: add the #9 carrier to both rows | `grep -n 'DPCON-I[34]' docs/baseline/dpcon.md \| grep -c '#9'` should be 2 |

F8 detail: DPCON-I3 and DPCON-I4 read only "candidate", with no #9 routing. COVERAGE routes both to cross-dprc-links (#9).

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F9 | models/COVERAGE.md (no row) + docs/baseline/ (no row) | doc-drift | 7e865ec / task 3.13 (drift inside a bound child made a typed refusal, deferred to w01) | misleads-a-reader | amend: add a disposition line (bead w01 → #9, ADR-0017) under the pool/population section | `grep -c 'w01' models/COVERAGE.md` should be ≥ 1 |

F9 detail: the VFIO rebind-drift rider is absent from both COVERAGE and every baseline doc: `grep w01` finds nothing in either. The carriers do exist in ROADMAP:29, design.md:313, `engine.rs:1015` and `main.rs:432`.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F10 | models/COVERAGE.md:89 | doc-drift | 9abb7fa (edited the row text, left the status) | misleads-a-reader | amend the status, or state what remains deferred | `sed -n 89p models/COVERAGE.md` |

F10 detail: DPNI-I5's status column says `deferred`, but the row text now says both halves landed (#5, and #6 as the companion-draw census). The witness column is `—`.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F11 | crates/dpaa2-api/src/families/pool_lifecycle.rs:442, 450, 458, 555-557 | stale / twin-drift (anchor rot) | model growth at a2492bf / f3c2838 / 73f8df9 (tasks 3.8 / 3.9 / 3.18) | misleads-a-reader | amend: re-anchor, or cite by name only (names are stable, line numbers are not) | `grep -n '(:[0-9]\+)\|pool_lifecycle\` :[0-9]' crates/dpaa2-api/src/families/pool_lifecycle.rs` should return nothing |

F11 detail: the docs cite `pool_lifecycle.qnt` line numbers (:106, :108, :116, :197, :208, :219). The targets have moved: `growEnabled` is ~169, `isConverged` 179, `growCreateAt` 269, `shrinkDestroyAt` 299, `pruneAt` 313.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F12 | crates/dpaa2-api/src/families/dpio.rs:45-47 | stale | 4f99e93 (task 2.3, which recorded the "no cfg facet" row in ADR-0019 in the same commit) | misleads-a-reader | amend: cite ADR-0019's dpio row as the recorded judgment | `grep -n "main loop's" crates/dpaa2-api/src/families/dpio.rs` should return nothing |

F12 detail: the module doc says whether this earns a cfg facet "is the main loop's and the user's call … this tile reports the evidence, it does not legislate it". The judgment has since been recorded.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F13 | crates/dpaa2-tools/src/engine.rs:600-601, 625 | stale | 9abb7fa (ROADMAP #6 delivered) / task 3.4 | misleads-a-reader | amend: say "emitted by the root pool / population passes, never this path" | `grep -n 'tiles #5/#6' crates/dpaa2-tools/src/engine.rs` should return nothing |

F13 detail: the doc comment and an operator-visible error string both say "companion/dpni are tiles #5/#6". Both tiles have now landed, and root pool convergence and population own companions.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F14 | crates/dpaa2-api/src/intent/refuse/mod.rs:70-71 + openspec/specs/intent-compiler/spec.md:196 | doc-drift (orphaned deferral) | 9abb7fa (#6 closed without it; ShrinkBelowDraw took the refusal role) | misleads-a-reader | Pass 4: re-route or delete the reservation (follow-up bead if the spec edit is out of this delta) | `grep -rn PoolShortfall crates openspec/specs` |

F14 detail: "a `PoolShortfall` variant is reserved for `reconcile` (change #6 …)". #6 is closed and no `PoolShortfall` exists anywhere.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F15 | crates/dpaa2-api/src/families/dpni.rs:838 | doc-drift | 9abb7fa (#6 delivered without assign/move) | misleads-a-reader | amend: name the real carrier | `grep -n 'companion tile #6' crates/dpaa2-api/src/families/dpni.rs` should return nothing |

F15 detail: "a dpni's container is the assign/move machinery's concern (companion tile #6)". #6 shipped no assign/move, and `restool.rs:655` calls it "the assign/move tile's concern".

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F16 | crates/dpaa2-mc/src/restool.rs:657-661 ↔ crates/dpaa2-mc/src/restool.rs:680-684 | duplicate / simplify | — | carries-cost | fold into a single `fn effective_queues(&self, cfg) -> usize` (readability cost: none, it is a 5-line block copied into `create_dpni_in` at 7e865ec) | `grep -c 'cfg.num_queues.get() == 0' crates/dpaa2-mc/src/restool.rs` should be 1 |

F16 detail: the `num_queues == 0` fallback is duplicated. This is not a protected lockstep twin. Handed to Pass 3.

## Ponytail markers

| marker | epic-touched file? | origin commit | classification |
|---|---|---|---|
| crates/dpaa2-api/src/core/model.rs:552 | file yes, lines no | 620feec / 7904ff7 (both before df950e0) | pre-existing, out of scope |
| crates/dpaa2-api/src/contract/fake.rs:535 | yes | 8380277 (in-epic) | (b) deliberate |
| crates/dpaa2-api/tests/compile_props.rs:168 | no | 549928c (pre) | pre-existing |
| crates/dpaa2-verify/src/board/fitcheck.rs:112 | no | 5c5f6d8 (pre) | pre-existing |
| models/intent/observed.qnt:30 | no | 2104d6c (pre) | pre-existing |
| models/intent/invariants.qnt:105 | no | 83f5579 (pre) | pre-existing |

- **fake.rs:535 ceiling is honest.** Clearing the whole set on destroy is correct because draws are only seeded by tests (`with_in_use_pool_object`, fake.rs:187). The upgrade path is named: per-consumer tracking when a test needs it.

## Deferral routing (the promised carriers)

| deferral | COVERAGE | baseline docs |
|---|---|---|
| DPRC-I8 | 5y7 → #10 (:79), holds | dprc.md:349 missing 5y7 (F7) |
| DPBP-I4 | 5y7 / #10 (:112), holds | dpbp.md:144 stale (F6) |
| DPIO-I3 kernel half | 5y7 / #10 (:118), holds | dpio.md:142 holds |
| DPMCP portal state (DPMCP-I3) | #10, no bead (:128) | dpmcp.md:151 row says only "candidate"; the #10 routing sits in open question 2 (:163-168). Neither names 5y7 (see open questions) |
| DPCON-I3/I4 | #9 (:123-124), holds | dpcon.md:113-114 missing #9 (F8); carrier twin dpcon.qnt says #6 (F5) |
| w01 / #9 VFIO rebind drift | absent (F9) | absent (F9) |

## Classification for downstream passes

**(a) Stale code hits, for Passes 2 and 3:**
- F2 `contract/mc.rs:17-20`
- F4 `restool.rs:653-654`
- F11 `pool_lifecycle.rs` qnt line anchors (Pass 2)
- F12 `dpio.rs:45-47` (Pass 2)
- F13 `engine.rs:600-601`, `625`
- F14 `refuse/mod.rs:70-71`
- F15 `dpni.rs:838`
- F16 `restool.rs` duplicate fallback (Pass 3)

**(a) Stale doc and model hits, for Pass 4:**
- F1 COVERAGE pool-law section (the most severe: the ADR-0020 law is missing from the ledger)
- F3 `retro/reconciler.qnt` and the dpaa2-verify README
- F5 `dpcon.qnt:9`, `:21`
- F6 `dpbp.md:144`
- F7 `dprc.md:349`
- F8 `dpcon.md:113-114`
- F9 w01 absent from COVERAGE and baselines
- F10 COVERAGE:89 DPNI-I5 status
- F14 the spec half (`openspec/specs/intent-compiler/spec.md:196`)

**(b) Deliberate:** `fake.rs:535` ponytail; `engine.rs:1252` `implicit_hasher` allow; `seatResidueReportedTest` simulate-only (not frozen, and COVERAGE marks it that way).

**(c) Historical prose, fine as is:** these name the retired `plugged ⇒ drawn` proxy in the past tense:
- `pool_lifecycle.rs:203`, `735-736`, `809`
- `pool.rs:212`
- `pool_lifecycle.qnt:30`, `283`
- `pool.rs:85` "counts are the only plan input in 3.1" (a task-stamped scope note)

## Footer

**Read:**
- the brief in full; tasks.md
- the epic file list (git diff --name-only)
- the ponytail sites and their blame
- COVERAGE rows 72-131 and 186-232
- baselines dpbp, dpcon, dpio, dpmcp and dprc (the deferral rows)
- the replay tables and orphan guards in `pool_replay.rs` and `dpio_replay.rs`
- the run lists in `pool_lifecycle.qnt` and `dpio.qnt`; `package.json` freeze scripts
- targeted excerpts of `restool.rs`, `pool.rs`, `contract/mc.rs`, `fake.rs`, `engine.rs`, `dpio.rs`, `dpni.rs`, `refuse/mod.rs`, `retro/reconciler.qnt`, ADR-0008 §9, ADR-0018:144-152, ADR-0019 rows, design.md:112-124

**Deliberately not read:**
- `models/board/**` (operator-sealed)
- the openspec spec deltas and proposal (Pass 4)
- the full bodies of `pool_lifecycle.rs`, `pool_lifecycle.qnt`, `pool_itf.rs` and `dpio_itf.rs` (Pass 2)
- the full `engine.rs` and `populate.rs` (Pass 3)
- out-of-scope surfaces

**Not executed:** `cargo test -p dpaa2-verify --test pool_replay --test dpio_replay`, `cargo test -p dpaa2-tools --test vpool6_intents --test vmvp_intents --test render`, `pnpm model:freeze-pool && pnpm model:freeze-dpio && git diff --exit-code models/traces`, and the quint typecheck and model gate. These all write to `target/` or the repo, so they are blocked in read-only mode. The synthesis step or a writable pass should run them.

**Open questions:**
1. Should DPMCP-I3 portal state name bead 5y7, or is #10 alone the intended carrier? The brief pairs them; the docs name only #10.
2. `dpio.rs:181` says "a wrong-cfg repair is destroy + recreate", and the ADR-0019 dpio row says "count-level destroy+create". Both sit beside the grow-only-seat law with no seat-teardown action. Is that consistent? This is for Pass 2 to judge.
3. `retroAssociationTest` still encodes the pre-D9 Create expansion. Is a re-freeze warranted, or is the note in F3 enough? This is for Pass 2.
