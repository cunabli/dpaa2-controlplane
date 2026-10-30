# Pass 1: residue, deferrals, and verify re-run (plan-rollback)

All offline gates are green. No leftover scaffolding in the epic's 68 non-trace files, and the epic added no unmarked `ponytail:`-class corner. Three doc-side stale claims still describe V-ROLLBACK-1 as the witness that ADR-0024 says cannot land, and ADR-0024 itself is stale on b4c's status. All five pre-logged carries are located and classified below.

HEAD is a4a3741. The working tree is clean apart from the out-of-scope untracked `.claude/agents/` and `.beads.gate.lock`, plus this review directory.

## Gate results

| Command | Headline |
|---|---|
| `cargo test --workspace` | 61 suites, 1059 passed / 0 failed / 0 ignored. `ledger_lint` 2/2 (the COVERAGE ledger lint). |
| `pnpm model:typecheck` | rc=0 |
| `pnpm model:test` | Green. The `&&` chain ran to its last link (orchestration: 11 passing, including `globalDrainBlindTest`), so every earlier link passed. The rc echo printed empty because of zsh `PIPESTATUS`; the chain did complete. |
| `sh -n` V-ROLLBACK-1.sh / V-DPCON-2.sh | rc=0 / rc=0 |
| shellcheck `-S warning` on both suites | Clean, rc=0. Ran via `uvx-pinned shellcheck-py` because there is no shellcheck on PATH. |
| `scripts/checks/quality-floor.sh` | PASS (clippy `-D warnings`, clippy `--tests`, `cargo doc`) |
| `openspec validate plan-rollback` | Valid, with the one recorded >500-char warning (carry 1) |

## Findings

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F1 | docs/adr/0024-the-mid-run-overcommit-witness-is-deferred-to-a-second-portal-transport.md:94-96 | stale ("b4c … is adjacent but separate from this deferral and stays tracked on its own"; b4c is closed) | a4a3741 (12.2 b4c fix, bead closed) | misleads-a-reader | new ADR note (one dated status line: b4c resolved by plan-rollback 12.2) | `grep -n 'stays tracked' docs/adr/0024-*.md` returns nothing, or the line also cites 12.2 |
| PASS1-F2 | openspec/changes/plan-rollback/proposal.md:54-55 and :67 | stale ("a new board row witnesses rollback under live kernel probing"; "the V-ROLLBACK-1 rev-4 witness closing the blindness end to end") | c02aeb9 (ADR-0024) | misleads-a-reader | amend: state that machinery is board-proven and the witness is deferred per ADR-0024. Pass 4 owns this. | `grep -n 'rev-4 witness\|witnesses rollback' openspec/changes/plan-rollback/proposal.md` |
| PASS1-F3 | openspec/changes/plan-rollback/design.md:222-223 ("The board row witnesses exactly this race class") and :212-213 (D12 "The sitting stays operator-launched") | stale: design.md has no ADR-0024 reference at all | c02aeb9 (ADR-0024) | misleads-a-reader | amend: add one ADR-0024 line to D12 and the risk row. Pass 4 owns this. | `grep -c 'ADR-0024' openspec/changes/plan-rollback/design.md` ≥ 1 |
| PASS1-F4 | models/board/V-ROLLBACK-1/V-ROLLBACK-1.sh:38-39 ("This suite takes (a) for the DRAIN because it is decisive") | stale: contradicts ADR-0024's "does not make the drain orderable". The :37 SETTLED bracket sits right above but does not retract it. | c02aeb9 (ADR-0024) | misleads-a-reader | amend (fold into carry 4 below) | `grep -n 'because it is decisive' models/board/V-ROLLBACK-1/V-ROLLBACK-1.sh` |
| PASS1-F5 (carry 1) | openspec/changes/plan-rollback/specs/capacity-census/spec.md:5-19 (body ≈768 chars) | simplify (cosmetic) | n/a | carries-cost | amend: split off the last sentence ("No plan SHALL silently drop a required draw…") as its own requirement, since it is a separate behavior. Otherwise record it as acceptable. | `openspec validate plan-rollback` shows no warning |
| PASS1-F6 (carry 2) | Recorded only in code docs: crates/dpaa2-verify/tests/pool_replay.rs:31-38 and crates/dpaa2-verify/src/intent/pool_itf.rs:41. Absent from models/COVERAGE.md, ADR-0023, ADR-0024, models/families/dpbp.qnt:40 and pool_lifecycle.qnt. | doc-drift (the model's static-bracket blindness to the aged board is not reader-visible where a model reader looks) | n/a | misleads-a-reader | amend: add a COVERAGE.md pool-row note (and/or a dpbp.qnt comment) that the live-free `Counted` semantic has no model twin and is covered by the `pool_lifecycle` units and V-CEIL-1. No re-modeling (protected item 2). | `grep -n -i 'live.free\|static bracket' models/COVERAGE.md` |
| PASS1-F7 (carry 3) | crates/dpaa2-api/src/plan/capacity.rs:84 (doc :67-69) | dead (defensive) | n/a | carries-cost (low) | Recorded-acceptable per protected item 6, or simplify. Reachability: `Counted` is minted only at crates/dpaa2-mc/src/restool.rs:97 (`Family::Dpbp` → `bp`), and all three production `FdbBudget{}` sites (capacity.rs:141, admission.rs:205, render.rs:1451) pass an Observed/Unknown bracket. | `grep -rn 'Counted(' crates/*/src` shows a single minting site |
| PASS1-F8 (carry 4) | models/board/V-ROLLBACK-1/V-ROLLBACK-1.sh:36-37 and models/board/README.md:114 (row tail, ~4.1 kB row) | doc-drift (layered annotations) | c02aeb9 supersedes the f59 bracket's "remains open on bead 3qv" / "witness reachability stays open on 3qv" clause | misleads-a-reader (mild) | amend: drop or strike the "open on 3qv" clause in the f59 bracket so the two brackets read as one sequence (f59 makes the collision benign, then ADR-0024 settles it); fold F4 in as well. They read chronologically but carry a now-false "open" claim. | `grep -n 'open on bead dpaa2-controlplane-3qv\|stays open on bead' models/board/V-ROLLBACK-1/V-ROLLBACK-1.sh models/board/README.md` |
| PASS1-F9 (carry 5) | crates/dpaa2-tools/src/render.rs:1257-1273 | none (confirmed clean) | n/a | none | Recorded-acceptable. Each of the three voices has a test: overcommit (rollback_cli.rs ~:51, snapshot), portal-busy (rollback_cli.rs:79-86, asserts `!contains("refused")`), generic (rollback_cli.rs:98-112, Aborted snapshot). No other surface stamps "refused" on a PortalBusy: the only `Error::PortalBusy` match outside dpaa2-mc is render.rs:1260, and the main.rs:159 fatal path logs via `tracing::error!("fatal")`. The cross-cases (portal-busy×Aborted, generic×RolledBack) are not pinned, which is minor. | `cargo test -p dpaa2-tools --test rollback_cli` |

## Residue and marker sweep (no finding)

- **Scaffolding.** Searched the lines the epic added for `todo!`, `unimplemented!`, `dbg!`, `allow(dead_code)`, `#[ignore]`, TODO/FIXME/XXX and commented-out code: none. The only added `#[allow]`s are justified clippy ones (`too_many_lines` / `too_many_arguments` in dpaa2-tools/src/main.rs).
- **`ponytail:` markers.** The epic diff adds exactly one: crates/dpaa2-mc/src/restool.rs:63 (PORTAL_OPEN_BACKOFF, :64; ATTEMPTS :59; discriminant :55; retry at :1178-1184).
- **Other caps and waits.** V-ROLLBACK-1.sh:237-243 (a 120s poll) and :252 (`cap=63`) are documented bounds, so they are not unmarked corners.
- **Wording swept with no hit:**
  - No population-bracket ceiling claim for dpbp survives. pool_lifecycle.rs:467-479 states the Counted/Observed split correctly.
  - No "refused" wording on the portal-busy path.
  - ROADMAP.md:35 tail and tasks.md:222-226 correctly cite ADR-0024 / bead 275.
  - "operator-launched" in V-DPCON-2.sh:12 and tasks.md:92/193/198 is protocol wording, not a pending-sitting claim.

## Deferral carriers (all confirmed)

- **Mid-run witness → ADR-0024 revisit triggers 1-3** (ADR-0024:73-87). Confirmed.
- **GlobalBudget split → bead b4c close reason**, which records the split as the alternative not taken. Confirmed.
- **dpcon's unproven rows → README.md:115**, which says `swpch.8wq` and `cqch.*` stay unproven. No `swpch.8wq`/`cqch` budget row exists in crates/*/src; the only hits are the snapshot parser fixtures. Confirmed.
- **Retired re-sit → bead 275** (closed "retired by ADR-0024"), backed by the annotations at V-ROLLBACK-1.sh:37 and README.md:114. Confirmed.

## Footer

- **Read:** the review brief; ADR-0024 in full; render.rs failure-disposition, main.rs failure seam and fatal path; capacity.rs FdbBudget, pool_lifecycle.rs admits_create; restool.rs ceiling_of and the retry constants; rollback_cli.rs voice tests; V-ROLLBACK-1.sh header and drain loop, V-DPCON-2.sh header; README rows 114/115, capacity-census spec delta, proposal.md:50-70, design.md D12 and risks, ROADMAP row 15; bead texts for b4c, 275, 3qv and f59; grep sweeps over all 68 non-trace epic-touched files.
- **Deliberately not read:** the .itf.json traces, the Quint model bodies beyond grep, the journal/reversal/executor internals (Pass 2/3), the remaining spec deltas and ADR-0023 (Pass 4), and the insta snapshot files.
- **Open questions:**
  1. render.rs:1269 says "mid-run failure" even when a PortalBusy fires before any actuation (for example in ensure's first observe), and the 0x8 overcommit voice fires for any McStatus 0x8 reaching the seam, pre-dispatch or not. Is "mid-run" honest in the pre-dispatch case? This is for Pass 3.
  2. Is ADR-0024 F1 better fixed as an amendment line in the ADR or left to the ADR's own references? It depends on the repo's ADR immutability convention.
