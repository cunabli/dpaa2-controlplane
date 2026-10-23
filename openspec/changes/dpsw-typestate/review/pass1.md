# Pass 1: residue, deferrals, and the verify re-run (dpsw-typestate)

Every offline verify obligation re-ran green, including Apalache on `dpsw.qnt`. I found no `todo!`, `unimplemented!`, `dbg!`, `#[allow(dead_code)]`, `#[ignore]` or commented-out code in the epic-touched files. The six `ponytail:` markers are the same pre-existing set. The real hits are 4 stale claims, 2 records that are thin or missing, and 1 piece of unmarked new residue.

HEAD is 8a0c993. The working tree has only untracked review/agent dirs, so main has not moved since grounding.

## Verify re-run ledger

| Obligation | Command | Result |
|---|---|---|
| 2.x derivation/refusal unit tests (`compile_tests.rs` lives in the dpaa2-api lib) + compile_props | `cargo test --locked --workspace` (one capture, grepped per target) | PASS: api lib 349/349, compile_props 6/6 |
| 3.x plan tests (`plan/dpsw.rs:775` `companion_starvation_is_planned_away`, `:800` `port_overflow_is_refused_typed_before_dispatch`, `:888` `silent_destroy_failure_does_not_converge`, `:904` `blanked_connection_line_is_not_freedom`) | same | PASS (in api lib) |
| 4.x render and parse (`restool.rs:3478` field-for-field render, `:3617` V-DPSW-4 fixture parse, `:3657` truncation, `:3680` missing connection line → `Unobservable`) + `ioctl_policy.rs:91` dpsw verbs resolve | same | PASS: mc lib 134/134, shim 26/26, ioctl_policy 5/5 |
| 5.1 + render snapshots (incl. the `Unobservable` face snapshot) | same | PASS: dpsw_detail 2/2, render 7/7 |
| 6.1 dpsw ITF replay | same | PASS: dpsw_replay 8/8. All 8 frozen traces are referenced, and `every_committed_trace_is_listed` (`dpsw_replay.rs:488`) guards against orphans. lastverbs_trace 1/1, but see F5: dpsw traces carry no `lastVerbs` |
| 6.3 tap offline tests (`driver.rs:1506` tapped and untapped renders drive identical steps, `:1546` dead tap refuses before any step, `capture.rs:677` dead tap refuses at the preamble, `:586` corpus round-trip, `:548` committed capture parses) | same | PASS: verify lib 149/149 |
| 6.4 engine tests over the fake backend (`engine.rs` create order, idempotence, interposition incl. a child-container dpni, cfg-drift recreate, allow gate, prune teardown with unbind before destroy, prune report-only, `reify_guard` refusal) | same | PASS: tools lib 26/26 |
| 7.2 offline pins | same | PASS: vdpsw5_intents 2/2 |
| COVERAGE ledger lint | same | PASS: ledger_lint 2/2 |
| Rest of the workspace (regression check) | same | PASS: 0 failed, 0 ignored in any target |
| Quint typecheck | `quint typecheck` on `families/dpsw.qnt`, `main.qnt`, `intent/main.qnt`, `intent/alphabet.qnt`; `python3 models/helpers/intent-pairing.py` | PASS (pairing exit 0) |
| Quint test | `quint test models/families/dpsw.qnt --main=dpsw_lifecycle` | PASS: 14 passing |
| Quint simulate | `quint run` on dpsw `stateInvariants` (20 steps / 200 samples), main (40/200), intent_alphabet `intentInvariants` (8/300) | PASS: dpsw prints "[ok] No violation found"; main and alphabet exited 0 with no violation output |
| Marked Apalache | `quint verify models/families/dpsw.qnt --main=dpsw_lifecycle --invariant=stateInvariants --max-steps=1 --apalache-version=0.56.1` | PASS: NoError, about 8.5s |
| Rest of the ladder (other `model:verify` families, `model:test` and `model:simulate` for non-dpsw modules, `model:coverage`) | not run | SKIPPED: these modules are outside the epic, and depth 1 is the ladder bound |

## Deferral carriers

- **Reachability-traffic leg → change #15 + bead dbf.19:** OK. The dbf.19 close reason names the #15 resource-aware-orchestration driver-occupancy ordering guard plus the kernel refresh. ROADMAP row #11 says the same, and `specs/system-integration/spec.md:34` has the deferred scenario.
- **Banked corpus handoff → bead 94k / ROADMAP #10:** WEAK, see F6. `bd show dpaa2-controlplane-94k` never mentions dpsw, corpus, capture or fixture, and was last updated 2026-10-07, before the epic. ROADMAP #10 only says "encode fixtures pre-banked by the #11/#12 capture tap", with no corpus path. `tasks.md:142` says the corpus goes "alongside" 94k, not inside it.
- **V-DPSW-3:** OK. `models/board/README.md:137` still reads "stays deferred as written".

## Ponytail markers

- The six code markers (`fake.rs:827`, `model.rs:552`, `compile_props.rs:209`, `fitcheck.rs:112`, `invariants.qnt:105`, `observed.qnt:30`) are the same set as at `e18316a~1`. Only `fake.rs` (748→827) and `compile_props.rs` (168→209) shifted line numbers.
- `git blame` on the marker regions shows only pre-span commits (8380277, 549928c, 83f5579). None is stale.
- The epic added no markers. The only unmarked new heuristic I found is in F7.

## Findings

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F1 | docs/adr/0013-accepted-intent-vocabulary.md:594 | stale | 4e243da (6.4: ensure reifies the switched domain) | misleads-a-reader | amend: fabric now has an executor; only crypto/dpseci stays plan-only (matches the `engine.rs:654` guard) | `grep -n "plan-only until their executors land (#11" docs/adr/0013-*.md` → empty |
| PASS1-F2 | docs/adr/0013-accepted-intent-vocabulary.md:595-596 | stale | e7a6e6b (1.4: COVERAGE DPSW-I1/I2 → `modeled`; V-DPSW-1/4 anchors) | misleads-a-reader | amend: say the ledger records I1/I2 as modeled, with board faces V-DPSW-1 rev 2 and V-DPSW-4 rev 1 | `grep -n "board-pending" docs/adr/0013-*.md` → no DPSW hit; `grep "^\| DPSW-I1 " models/COVERAGE.md` shows `modeled` |
| PASS1-F3 | crates/dpaa2-api/src/intent/compiled.rs:508-513 (also `render.rs:337-338`; shows up in `tests/snapshots/render__dry_run_switched_domain.snap`, which lists `dpsw x1` as plan-only and `0 planned transition(s)`) | stale | 4e243da (6.4: `converge_switches` actuates the dpsw) | misleads-a-reader | amend the doc comments ("never actuated" and "families the reconciler does not execute" are false for dpsw now). Pass 3 decides whether dry-run should exclude engine-actuated families from plan-only or render the switched pass (dry-run vs ensure parity) | `cargo test --locked -p dpaa2-tools --test render`; snapshot no longer reports the dpsw as unexecuted, or the wording names the engine pass |
| PASS1-F4 | openspec/changes/dpsw-typestate/design.md:176-179 | stale | 0a1e49f (7.1: V-DPSW-2 rev 2 answered FDB split = driver convention) | misleads-a-reader | amend: move the item out of Open Questions as answered, citing `dpsw.md` unknown 6 | `grep -n "MC-enforced or driver" openspec/changes/dpsw-typestate/design.md` → only in resolved wording |
| PASS1-F5 | openspec/changes/dpsw-typestate/specs/mbt-harness/spec.md:11-12 (scenario :16-17 "state and verb set match") | doc-drift | n/a | misleads-a-reader | amend the spec: none of the 8 dpsw traces carries `lastVerbs` (no family trace dir does; `lastverbs_trace.rs:24-41` scans only retro + board). Name `ioctl_policy.rs:91` `dpsw_verbs_resolve_through_the_whitelist` as the gate that actually covers it | `grep -c lastVerbs models/traces/families/dpsw/*.json` (all 0 today); spec text names the ioctl_policy test |
| PASS1-F6 | bead dpaa2-controlplane-94k (`bd show`) + docs/ROADMAP.md:30 | doc-drift | n/a | carries-cost | follow-up bead or bd note on 94k: name `models/board/V-DPSW-2/corpus/` + `models/board/V-DPSW-5/corpus/` + `capture.rs` `Corpus::parse` as #10's banked fixture input | `bd show dpaa2-controlplane-94k \| grep -i corpus` → non-empty |
| PASS1-F7 | crates/dpaa2-verify/src/board/capture.rs:423-426 | simplify | n/a | carries-cost | amend: `let _ =` throws away a restool spawn failure, and `unwrap_or_default()` turns an unreadable capture file into the "dead tap" message. Still a loud fail, but the cause is wrong. Surface the io error, or add a `ponytail:` marker for the fixed 1s flush sleep and the swallowed errors | `grep -n "unwrap_or_default" crates/dpaa2-verify/src/board/capture.rs` → empty, or a ponytail marker next to it |
| PASS1-F8 | models/board/V-DPSW-5/V-DPSW-5.sh:11,25-27 | stale | 49dfad9 / b8defbd (7.2 re-scope to the structural verdict; dbf.19) | misleads-a-reader | amend the header with one line: traffic leg deferred (#15 trigger, dbf.19); the verdict is structural 8/8. Last touched at 51d37f1, before the re-scope | `grep -n -i "deferred" models/board/V-DPSW-5/V-DPSW-5.sh \| head -3` hits the header |

The other stale-wording sweeps came back clean:
- **strace:** the only epic-surface hit is `tasks.md:88`, which the brief excludes as history. `design.md:97` records strace as rejected.
- **Child-dprc companion placement:** `plan/dpsw.rs:334` and `proposal.md:39-40` both say root / own container.
- **Pre-6.4 no-ensure-dispatch:** no live claim apart from the plan-only accounting in F1/F3.
- **FDB budget as hardware law:** none. `refuse.qnt:72` is the 16-bit field bound, and the "kernel per-port FDB split" anchor in `derive.rs:888` is accurate.
- **Traffic witness:** none outside F8.

## Footer

**Read:**
- The brief in full, `tasks.md`, the `models/README.md` ladder section, and the `package.json` model scripts.
- `dpsw_replay.rs` structure, `lastverbs_trace.rs` scope, and the ioctl_policy dpsw test.
- `capture.rs:390-430`, the `engine.rs` reify guard and the uplink lookup (`engine.rs:612-660`, `775-800`), and `compiled.rs:500-535`.
- The dry-run snapshot, the `V-DPSW-5.sh` header, and the `vdpsw5_intents.rs` header.
- `dpsw.md` invariant table and unknowns, the ADR-0013 consequences, the ROADMAP #10/#11 rows, README :137, `bd show` for dbf.19 and 94k, and `git blame` on the marker regions.
- Greps over the epic-touched set: 75 files in crates/models/docs, excluding traces and board corpora.

**Deliberately not read:**
- Out-of-scope surfaces: ADR-0023 content, resource-aware-orchestration, dpdmux.
- The board corpora contents and VERDICTS.json (Pass 4).
- The full engine/plan/model bodies (isomorphism and sans-io belong to Passes 2 and 3).

**Open questions:**
1. Should `tasks.md:88` ("strace-wrapped spawns") be re-sealed? The brief carves it out as history, but the synthesis mandate asks this exact rule question.
2. The `derive.rs:864` / `derive.qnt:742` / ADR-0013:252 provenance anchor says "read-not-verified", while COVERAGE cites the V-DPSW-1/4 board faces and `dpsw.md:246` still marks DPSW-I1 as "candidate". Pass 4 should decide which record is wrong. Any change to the anchor string also needs `fabricAcceptedTrace` and the dry-run snapshot regenerated.
3. F3's dry-run "0 planned transition(s)" against ensure creating a dpsw goes beyond wording: it is a dry-run/ensure parity question for Pass 3.

About 62k tokens used, a little over the 50k budget and under the 75k stop line.
