# PASS 1: dpseci-typestate. Residue, deferrals, and verify re-run

HEAD = ae23721, so main has not moved and the grounding stands. The only untracked paths are `.claude/agents/` and `review/`. Nothing was modified; cargo and quint wrote only to `target/`.

## Verify obligations re-run (all PASS)

| Obligation | Command | Result line |
|---|---|---|
| 1.1 model typecheck | `npx quint typecheck models/families/dpseci.qnt` | exit 0 |
| 1.1 model tests | `npx quint test models/families/dpseci.qnt --main=dpseci_lifecycle` | `17 passing (274ms)` |
| 1.1 simulate | `npx quint run models/families/dpseci.qnt --main=dpseci_lifecycle --invariant=stateInvariants --max-steps=20 --max-samples=200` | `[ok] No violation found` |
| 1.2 ledger lint | `cargo test -p dpaa2-verify` → tests/ledger_lint.rs | `test result: ok. 2 passed` |
| 2.1 `compile_fail` immutability doctest | `cargo test -p dpaa2-api --doc` | `families::dpseci::DpseciCfg (line 256) - compile fail ... ok`; `11 passed` |
| 2.1 family unit tests + 3.3 census (core) | `cargo test -p dpaa2-api dpseci` | `test result: ok. 26 passed` |
| 2.2 derivation (`[2; n]`, `{HAS_CG}`, provenance) | same run, `intent::refuse::compile_tests::dpseci_per_crypto_block_sized_by_its_own_flows` | `ok`. It asserts the full `DpseciCfg` for flows 4 and 8 and the provenance value 2. It does not assert the provenance `anchor` text (minor, not a finding). |
| 2.3 ITF replay | `cargo test -p dpaa2-verify` → tests/dpseci_replay.rs | `test result: ok. 5 passed`. `every_committed_trace_is_listed` checks both directions (each listed trace exists on disk, and the on-disk count equals `TRACES.len()`). The 8 on-disk traces equal the 8 `TRACES` entries, which equal the 8 names in the `model:freeze-dpseci` match list. None is orphaned or unreplayed. |
| 3.1 portal fixture tests | `cargo test -p dpaa2-hal portal` | `test result: ok. 11 passed` (golden frames for all 5 ids, request code, all 3 outcomes) |
| lbk.14 root-routing pin (`dprc.64998`) | `cargo test -p dpaa2-mc observe_dpseci_reads_a_child_resident_over_the_root_portal` | `test result: ok. 1 passed` |
| 3.2 shim tests | `cargo test -p dpaa2-mc` | lib `113 passed`, tests/shim.rs `26 passed` |
| 4.1 hooks-never-gate proof | `cargo test -p dpaa2-tools` → tests/dpseci_detail.rs | `test result: ok. 2 passed` |
| 5.1 operand pin | same → tests/vdpseci3_intents.rs | `test result: ok. 1 passed` |

## Deferral carriers in both documents

| Promise | `models/COVERAGE.md` | `docs/baseline/dpseci.md` |
|---|---|---|
| I2 MC layer → #10, CREATE unreachable by construction | OK (:135) | OK (:228) |
| I5 board face → #10, whitelist fence stated | OK (:138) | **MISSING**: :231 still routes to V-DPSECI-2/#8 (F2) |
| I9 → block-global counter law row | OK (:142) | **MISSING**: :235 says only "candidate" (F4) |
| Unknown #1 fenced → #10 | n/a | **PARTIAL**: :239–245, carrier not named (F5) |
| Unknowns #5–#8 fenced | n/a | **MISSING**: :260–267, no carriers anywhere (F6) |

## Findings

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS1-F1 | models/COVERAGE.md:136 | stale | 22ef093 (5.2 sitting: V-DPSECI-2 rev 1 witnessed as the V-DPSECI-3-rev2 hook, 2/2) | misleads-a-reader | amend: verified stamp citing the V-DPSECI-3 rev 2 hook (README :132, VERDICTS `V-DPSECI-3-rev2`). The brief expected a 6.1 verified stamp, but f27900a never touched COVERAGE. | `grep -n 'open: V-DPSECI-2' models/COVERAGE.md` → no hit; ledger_lint green |
| PASS1-F2 | docs/baseline/dpseci.md:231 | stale | 22ef093 / f27900a (unknown #2 answered; the board face moved to #10 in COVERAGE at a0fff2f) | breaks-a-claim (the I5 → #10-with-whitelist-fence promise is absent from the baseline) | amend: replace "board reset path stays open … → V-DPSECI-2 under dpseci-typestate (#8)" with "API 5.4 confirmed (V-DPSECI-3 rev 2); post-unbind dirt face → mc-portal-backend (#10), get_rx_queue/get_congestion on no userspace whitelist" | `grep -n 'DPSECI-I5' docs/baseline/dpseci.md \| grep -c '#10'` ≥ 1 |
| PASS1-F3 | docs/baseline/dpseci.md:227,229,230 | stale | 7139a77/0b69b54 (I1/I4 modeled), 0513707 + 22ef093 (I3 implemented at adapter and board-witnessed) | misleads-a-reader | amend: status column for I1/I3/I4 still reads "candidate"; mirror COVERAGE as I2/I5 already do | `awk -F'\|' '/DPSECI-I[134] /{print $5}' docs/baseline/dpseci.md` → no "candidate" |
| PASS1-F4 | docs/baseline/dpseci.md:235 | doc-drift | a0fff2f (I9 re-anchored to `SEC_COUNTERS_BLOCK_GLOBAL` in COVERAGE only) | misleads-a-reader | amend: point the I9 row at the `families/dpseci.qnt` `SEC_COUNTERS_BLOCK_GLOBAL` law row | `grep -n 'DPSECI-I9' docs/baseline/dpseci.md \| grep SEC_COUNTERS_BLOCK_GLOBAL` |
| PASS1-F5 | docs/baseline/dpseci.md:239-245 | stale | 11b3a67 (the ioctl portal now exists as a read slice, so "the ioctl portal is needed to reach it" is ambiguous) | misleads-a-reader | amend: "needs ioctl CREATE, excluded from the ADR-0021 read slice by construction → mc-portal-backend (#10)" | `sed -n 239,246p docs/baseline/dpseci.md \| grep -c '#10'` ≥ 1 |
| PASS1-F6 | docs/baseline/dpseci.md:260-267 (claim at openspec/changes/dpseci-typestate/design.md:263-265) | doc-drift | — | breaks-a-claim: design says "#1, #5–#8 are re-anchored loud (D7) with their fences named", but neither the baseline nor COVERAGE names a fence or carrier for #5 (sec_if_id, DPL → #14), #6 (OPR → #14, D4), #7 (dpseci_reset, not whitelisted → #10) or #8 (sec_attr, `-EACCES` per baseline :155 → #10) | amend the register entries (or amend design.md:264 if the re-anchor was deliberately dropped) | `sed -n 260,268p docs/baseline/dpseci.md \| grep -cE '#1[04]'` = 4 |
| PASS1-F7 | crates/dpaa2-api/src/contract/mc.rs:111-117 | stale | 1ba7038 (lbk.14 root-portal routing; it touched only restool.rs) | misleads-a-reader (#10 implementers read the trait doc) | amend doc only, keeping the trait signature byte-for-byte: "when the `/dev/dprc.N` node of `container` is reachable … `container` names the dprc whose device node carries the portal read" → the read rides the backend's root node; `container` is not the portal | `grep -n "node of \`container\`\|whose device node carries" crates/dpaa2-api/src/contract/mc.rs` → no hit |
| PASS1-F8 | crates/dpaa2-mc/src/restool.rs:1054 | doc-drift | 1ba7038 | misleads-a-reader | amend: the comment says `container` is "the trait seam the fake keys on", but fake.rs:542 also ignores `_container`. No implementor reads it, so the parameter is now dead on both impls (callers: populate.rs:149, status.rs:184). Fix the comment now; keeping or removing the parameter is a #10 seam call. | `grep -n 'fake keys on' crates/dpaa2-mc/src/restool.rs` → no hit |
| PASS1-F9 | crates/dpaa2-tools/tests/dpseci_detail.rs:70,82 | stale | 1ba7038 (portal never opens a child node such as /dev/dprc.5 now; under root routing the reason names the root node) | carries-cost (low; a display-only fixture string) | amend fixture reason to `/dev/dprc.1` | `grep -n 'dprc.5' crates/dpaa2-tools/tests/dpseci_detail.rs` → no hit; `cargo test -p dpaa2-tools --test dpseci_detail` green |

## Sweep results with no finding

- **Scaffolding:** in the added lines of `git diff -U0 8532cd6..ae23721` across crates/, models/ and docs/ there are zero instances of `todo!`, `unimplemented!`, `dbg!`, `#[allow(dead_code)]`, `#[ignore]`, `#[expect`, `TODO`, `FIXME`, `XXX` or `HACK`.
  - The only new `#[allow(...)]` is the scoped `#[allow(unsafe_code)]` on the portal ioctl call. That is the recorded D2/ADR-0021 shape.
  - Grepping for commented-out code (`//` followed by code syntax) hits only prose comments.
- **Stale board wording:**
  - Nothing outside the docs above claims unknowns #2/#3 are still open. The V-DPSECI-3.sh mentions are pre-run hook labels and are correct.
  - Nothing claims V-DPSECI-2 is a separate verdict. The script, ROADMAP :28 and vdpseci3_intents.rs:66 all describe it as a hook.
  - Child-node routing descriptions survive only at F7, F8 and F9. restool.rs:800-830, ADR-0021:85 and baseline :108-113 are correct after 1ba7038.
- **ponytail markers:** the six in crates/ and models/ match the brief exactly (fake.rs:689, compile_props.rs:168, fitcheck.rs:112, model.rs:552, observed.qnt:30, invariants.qnt:105). The 3 in scripts/checks are outside the epic. The epic diff adds 0 `ponytail` strings.
  - fake.rs:689 (whole-set `in_use` clear in the dpni `destroy`) sits between the epic hunks at 539-550 and 840-864. The new `destroy_dpseci` (fake.rs:857) does not touch `in_use`, so the marker is not stale.
  - The epic adds no new unmarked ceiling. The derive clamp (derive.rs:137) and the `root_portal` `DprcId::ROOT` fallback are each documented in place, and the fallback is the recorded lbk.14 deviation.

## Footer

**Read:**
- review/brief.md, tasks.md, design.md (scope, D7, Open Questions)
- COVERAGE DPSECI rows, the whole of the baseline dpseci.md except the kernel section, and the mc-ioctl-policy dpseci rows
- tests/dpseci_replay.rs :120-222, derive.rs :120-155 and :1038-1056, compile_tests.rs :943-986
- restool.rs :300-320, :798-849, :1053-1066, :2745-2783
- contract/mc.rs :105-125, fake.rs (dpseci hunks and the :689 context), shim.rs :88-104, populate.rs judge_dpseci, status.rs dpseci_details
- ADR-0021 routing lines, README :65/:108/:131-132, VERDICTS keys, ROADMAP :28, commit stats for a0fff2f, f27900a, 22ef093 and 1ba7038

**Deliberately not read:**
- portal.rs internals, plan/dpseci.rs census logic and the dpseci.qnt body (Pass 2/3 scope)
- V-DPSECI-3.sh beyond the grep hits
- ADR-0019, frozen trace contents, board evidence archives, out-of-scope surfaces

**Not re-run:**
- the full `pnpm model:validation`; only the dpseci model was run
- clippy/fmt; per the brief, the quality floor is not the target

**Open questions:**
1. F6: was the #5–#8 re-anchor intended for the baseline register, or did D7 silently drop it? If it was dropped, the fix is to design.md:264 instead.
2. F8: the `container` parameter of `McControl::observe_dpseci` is read by no implementor. Pass 3 should decide whether it is a #10 seam to keep, given the byte-for-byte contract claim.
3. F1: the brief expects "6.1 verified stamps citing the sitting" in COVERAGE, but COVERAGE was touched only at a0fff2f. Pass 4 should confirm whether any verified stamp exists anywhere.
