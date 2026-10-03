# dpseci-typestate — Four-pass review synthesis (change-judge)

HEAD ae23721, unmoved through all passes. 28 raw findings in → 16 merged items. Nothing dropped for missing schema fields (every staleness claim carried its superseding commit). Nothing contradicted a protected decision after the rulings below; the one proposal that would have (deleting `classify_cfg_mismatch`) is rejected on D9's own text.

## Cross-pass tension rulings (verified in-repo)

**T1 — PASS2-F4 vs PASS3-F5 (observe-side unknown-bit handling).** Ruled for Pass 2's twin-drift reading, with Pass 3's safety assessment adopted. The D4 text (openspec/changes/dpseci-typestate/design.md:138-141) is explicitly an observe-side claim: "observing the production child object parses `Known` and an unknown firmware bit is attributed, never silently merged." Shipped `decode_dpseci_options` (restool.rs:260-274) returns `None` on any unnamed bit — not-merged holds, attributed does not: the bit identity is discarded, render shows `options=[unknown]`, and no read-side `RawEscape` is ever minted (model twin `createRawEscapeReadbackTest` carries escapes on the observable face). The bead trail could not be consulted (`.beads/issues.jsonl` passive export absent; only the Dolt DB exists), so no recorded honest-gap call rescues the doc-only reading. Behavior is safe and latent (unknown bit → Unobservable → zero actions; sole constructor takes no options; derivation emits HAS_CG only), so this is a follow-up bead (B4), not a live defect — with the doc-amend exit left open if the change owner records the gap instead. PASS3-F5's families/dpseci.rs:92-94 amendment folds into the same bead (it becomes true as written if the code lands, needs rewording if the gap is recorded).

**T2 — classify_cfg_mismatch triple hit (PASS2-F1/F2 + PASS4-F8).** Merged (S4). Deletion is OFF the table: design.md:218-219 reads verbatim "`classify_cfg_mismatch` (full cfg) remains the plan-layer repair law for desired-versus-desired comparisons" — D9 keeps the function by name. Disposition is doc-only: fix the pre-D9 module header (plan/dpseci.rs:6-8, "executor … task 3.2"), reword the fn doc away from "against the observed one", rename the `observed` parameter to the desired-vs-desired role, cite D9.

**T3 — observe_dpseci `container` parameter (PASS1-F7/F8 + PASS3-F3).** Merged (S5). Pass 3's keep-the-parameter ruling stands; no contrary evidence found in Pass 2 (which confirmed from the census side) or Pass 4. Docs-only amendment at contract/mc.rs:111-117 and restool.rs:1054; trait signature byte-for-byte.

**T4 — PASS4-F7 amend-at-archive.** Verified: openspec archive merges delta text VERBATIM. The archived dpmac mbt-harness delta (archive/2026-10-03-dpmac-typestate/specs/mbt-harness/spec.md:32,38) appears character-identical in the living spec (openspec/specs/mbt-harness/spec.md:238,244); no witness citation was rewritten at archive. So the dpseci delta's "clean census (V-DPSECI-3 rev 1)" (changes/dpseci-typestate/specs/mbt-harness/spec.md:26) WILL flow into the living spec citing the rev whose portal face failed. Disposition upgraded from "amend at archive" to "amend the delta file before `/opsx:archive`" (B2, archive-blocking). The :25 "V-DPSECI-2 rev 1" is correct; proposal.md:59 / design.md:163 stay frozen.

## Merged findings ledger (most severe first)

| # | Merges | file:line | Category | Severity | Disposition |
|---|---|---|---|---|---|
| S1 | PASS2-F4 + PASS3-F5 | crates/dpaa2-mc/src/restool.rs:260-274; crates/dpaa2-api/src/families/dpseci.rs:92-94; crates/dpaa2-tools/src/render.rs:621-629 | twin-drift | breaks-a-claim (latent) | Bead B4: attribute unknown bits as `RawEscape` on the display face; census projection stays conservative; or record the honest gap and amend the family doc |
| S2 | PASS1-F2 + PASS4-F2 | docs/baseline/dpseci.md:231 | stale (22ef093/a0fff2f) | breaks-a-claim (the one doc claiming a whitelist-denied capability, under the wrong change) | Bead B1: I5 row → API 5.4 confirmed (V-DPSECI-3 rev 2); post-unbind face → #10, reads not whitelisted |
| S3 | PASS1-F6 + PASS4-F6 | docs/baseline/dpseci.md:260-267 (claim at design.md:263-265) | doc-drift (never fulfilled) | breaks-a-claim | Bead B1: four one-line carrier pointers (#5 DPL→#14, #6 OPR→#14/D4, #7 reset→#10, #8 sec_attr→#10); intent proven by proposal.md:62-70, so the register is the fix, not design.md |
| S4 | PASS2-F1 + PASS2-F2 + PASS4-F8 | crates/dpaa2-api/src/plan/dpseci.rs:6-8,26-36 | doc-drift (8cde7aa/0513707) | misleads-a-reader (invites the exact role swap D9 forbids) | Bead B3: doc/param-name reword to desired-vs-desired, cite D9. Deletion rejected (T2) |
| S5 | PASS1-F7 + PASS1-F8 + PASS3-F3 | crates/dpaa2-api/src/contract/mc.rs:111-117; crates/dpaa2-mc/src/restool.rs:1054 | stale (1ba7038) | misleads-a-reader (#10 implementers read this seam) | Bead B3: docs only; keep the parameter (T3) |
| S6 | PASS4-F7 | openspec/changes/dpseci-typestate/specs/mbt-harness/spec.md:26 | stale (1ba7038/22ef093) | misleads-a-reader (merges verbatim into living spec) | Bead B2: cite rev 2 or drop the rev number BEFORE archive (T4) |
| S7 | PASS2-F3 | crates/dpaa2-mc/src/populate.rs:150-157; plan/populate.rs:64-75; gap at models/families/dpseci.qnt:453-477 | twin-drift | misleads-a-reader (convergence-affecting law is Rust-only) | Bead B5: model the whole-census-poisoning law (unobservable member ⇒ judge nothing) in Quint; check dpni/pool for the same corpus idiom gap |
| S8 | PASS1-F1 + PASS4-F1 | models/COVERAGE.md:136 | stale (22ef093) | misleads-a-reader | Bead B1: I3 verify cell → verified stamp citing the V-DPSECI-3-rev2 hook 2/2 |
| S9 | PASS1-F3 + PASS4-F3 | docs/baseline/dpseci.md:227,229,230 | stale (7139a77/0b69b54/0513707/22ef093) | misleads-a-reader | Bead B1: I1/I3/I4 status cells off "candidate", mirror COVERAGE |
| S10 | PASS1-F4 + PASS4-F4 | docs/baseline/dpseci.md:235 | doc-drift (a0fff2f) | misleads-a-reader | Bead B1: I9 row → `SEC_COUNTERS_BLOCK_GLOBAL` law row |
| S11 | PASS1-F5 + PASS4-F5 | docs/baseline/dpseci.md:239-245 | stale (11b3a67) | misleads-a-reader | Bead B1: unknown #1 → "ioctl CREATE, excluded from the ADR-0021 read slice → #10" |
| S12 | PASS3-F1 | crates/dpaa2-hal/src/lib.rs:6; CLAUDE.md:28; docs/adr/0018…:112-113,121 | stale (11b3a67) | misleads-a-reader | Bead B3: portal read slice is present today; ADR-0018:121 row-10 attribution → row 8 per ADR-0021. CLAUDE.md edit is user-gated |
| S13 | PASS3-F2 | crates/dpaa2-hal/src/portal.rs:121-122 | doc-drift (8393568) | misleads-a-reader | Bead B3: vocabulary decode is dpaa2-mc's job, not dpaa2-api's |
| S14 | PASS3-F4 | crates/dpaa2-tools/tests/vdpseci3_intents.rs:41-45 vs src/main.rs:583-590 (+3 prior-epic siblings) | duplicate | carries-cost (4 hand-mirrored copies silently de-pin together) | Bead B6: lift `complete_kernel` into the tools lib/testkit, one definition site |
| S15 | PASS2-F5 | models/families/dpseci.qnt:220-223,489-569 | simplify | carries-cost (low) | Bead B7: freeze census/destroy ITF traces + replay arm, or record that hand-twins suffice for pure operators |
| S16 | PASS1-F9 | crates/dpaa2-tools/tests/dpseci_detail.rs:70,82 | stale (1ba7038) | carries-cost (low) | Bead B3: fixture string dprc.5 → dprc.1 |
| — | PASS4-F9 | design.md:107-109 vs portal.rs | informational | none | No action: design anticipated transmutes, shipped transmute-free; ADR-0021 is the durable correct record |

Dropped/absorbed: Pass 1's provenance-anchor note (concurred not-a-finding by Pass 2); Pass 3's note 2 (portal queue counts projected away at the trait seam — recorded dual-transport witness, additive for #10, not a finding); PASS4's three-record consistency, capability claims, ADR-0021/0019, D9-vs-shipped, ROADMAP checks — all clean confirmations feeding the verdicts below.

## Verdict 1 — Core promise: **YES**, the crypto interface is a typed, observed surface

| Claim | Evidence |
|---|---|
| Cfg complete enough that the shim creates one | Board V-DPSECI-3 rev 2: converge + exact population + idempotent second ensure + typed teardown, 8/8. Offline: Pass 1 shim 26 tests + lib 113 green; derivation `[2; n]`/`{HAS_CG}`/provenance test-pinned |
| HAS_CG witnessed through the fenced read slice, unobservability typed | Board rev 2 portal face `options=[HasCg] version=5.4`; sum-typed end to end `SigUnobservable` ↔ `ObservedSig::Unobservable` ↔ `DpseciPortalReadout::Unobservable` ↔ `ChildDpseci::Unobservable` (Pass 2 d); no sentinel/string compare; absence of evidence judges nothing (`dpseci_unobservable_portal_judges_nothing`) |
| Anonymous population converged as signature multiset census | D9 law shipped exactly: `Sig=(num_queues, options)`, priorities excluded, no identity minted; two-blocks-per-tenant converges at model/core/adapter layers; position-independence tested at all three (Pass 2 b, Pass 4 D9 check) |
| Board e2e dual-transport | V-DPSECI-2 rev 1 as the Suite A hook 2/2 (restool queues/priorities + portal options/version), VERDICTS `V-DPSECI-3-rev2`; three records (tasks 5.2 / README :108/:132 / VERDICTS) tell one consistent story (Pass 4) |
| Laws on both sides of the sans-io seam | Census/classify live in dpaa2-api core with Quint twins; dpaa2-mc plumbs only; hal policy-free; tools display-only with no hal dep edge (Pass 3 b/c) |

Caveats that do not flip it: S1 (read-side attribution latent gap, unreachable today) and S7 (census-poisoning law lacks its Quint twin).

## Verdict 2 — ADR-0002 / ADR-0021: **YES** on both

| Claim | Evidence |
|---|---|
| Family structurally isomorphic to dpseci.qnt (ADR-0002) | Law-for-law/guard-for-guard/refusal-for-refusal (Pass 2 a): classify order transcribed into `DpseciCfg::new`, bound bidirectionally by the proptest twin straddling all three bounds; refusal names verbatim; I1 by construction both sides (no mutating verb + `compile_fail` doctest); I4 both polarities; 8/8 traces replayed, none orphaned (Pass 1). Relational length-coupling encoded at block level is the only structural encoding — judged satisfied |
| Write unrepresentable | `DpseciRead` is a closed five-variant sum; cmdids are private consts reachable only through the `encode` match; `execute` takes only the sum; `Token` mintable only from an Open response (Pass 3 a) |
| Whitelist as single source | Every constant cites its pinned source (`fsl_mc_cmd.h`, `fsl_dpseci_cmd.h`, `fsl_mc_ioctl.h:47`, mc-ioctl-policy rows); tests pin the V1 macro, the whitelist mask rule, request code 0xc040_52e0, golden frames for all five commands; ADR-0021 names the table as single source and #10 as sole vocabulary owner — matches shipped exactly (Pass 4) |
| Unsafe confinement | Sole unsafe is the one `libc::ioctl` call under scoped `#[allow(unsafe_code)]` with crate-level deny; zero transmutes — stricter than the designed shape (PASS4-F9); decode total, no panic/UB path; three outcomes honestly disjoint |

## Proposed follow-up beads (for a just-in-time openspec change; the review changes nothing)

- **B2 — Fix the mbt-harness delta's rev citation before archive.** Why: archive merges delta text verbatim (verified against the dpmac archive), so "V-DPSECI-3 rev 1" would enter the living spec citing the rev whose portal face failed. Acceptance: `grep -n 'V-DPSECI-3 rev 1' openspec/changes/dpseci-typestate/specs/mbt-harness/spec.md` → no hit; archived spec cites rev 2 or no rev. **Sequencing: must land before `/opsx:archive` of dpseci-typestate.**
- **B1 — baseline/COVERAGE dpseci catch-up** (S2, S3, S8–S11). Why: the 6.1 close-out under-synced two files; one cell claims a whitelist-denied capability. Acceptance: the per-item greps in the ledger rows all pass; `cargo test -p dpaa2-verify --test ledger_lint` green.
- **B4 — Read-side unknown-bit attribution honors D4** (S1). Why: D4 claims observe-side attribution; shipped decode discards the bit identity (display shows `options=[unknown]`). Acceptance: either (i) `decode_dpseci_options` mints `RawEscape` into the decoded mask for the detail/display face, census projection unchanged-conservative, a decode test attributes `RawBits`, render shows the escape name and the :621 vs :170 asymmetry closes, families/dpseci.rs:92-94 true as written; or (ii) the honest gap is recorded (COVERAGE/ADR note) and families/dpseci.rs:92-94 reworded to the desired-side carry — owner's call at spec time.
- **B3 — Code-side doc comment sync** (S4, S5, S12, S13, S16). Why: five stale/pre-D9 doc comments sit exactly where #10 implementers will read. Acceptance: ledger-row greps return no hits; trait signatures byte-for-byte; quality-floor green; CLAUDE.md line gated by the user.
- **B5 — Model the census-poisoning law in Quint** (S7). Why: "any unobservable custody row ⇒ the dpseci face judges nothing" is convergence-affecting and Rust-only, against ADR-0002 quint-is-the-spec. Acceptance: census stated over observed faces with a `censusUnobservableMemberJudgesNothingTest` directed run, quint test green; one-line check whether dpni/pool share the gap (corpus idiom vs dpseci patch).
- **B6 — Lift `complete_kernel` to one definition site** (S14). Why: the epic added the fourth hand-mirrored copy; one change de-pins four operand tests silently. Acceptance: `grep -rn 'port_names_kernel' crates/dpaa2-tools` → one definition; all four pins green.
- **B7 — Census/destroy ITF coverage or record-why** (S15). Why: the census twins are hand-mirrored, a weaker binding than the replay oracle the create surface gets. Acceptance: either trace count grows with a replay arm green, or a recorded note that hand-twins suffice for pure-function operators.

## Rule amendment nomination (one, per mandate)

The rev-1 portal-routing miss (observe over the VFIO child's nonexistent `/dev/dprc.N` node) shipped through every offline gate and was caught only on the board. The FakeBackend contract could not represent the board fact "a VFIO-bound child exposes no device node." Nominated rule: **a new transport read path must land with an offline witness of its node-routing law** — a fixture/contract test asserting which device node the read opens under each binding state (the lbk.14 sentinel-root pin, `dprc.64998`, is exactly this shape, added post-hoc; the rule makes it pre-sitting). Scoped to new transports/routing laws so it costs nothing on steady-state epics; #10 is the first consumer.

Key paths: openspec/changes/dpseci-typestate/review/{brief,pass1,pass2,pass3,pass4}.md (inputs); design.md:138-141 and :218-219 (T1/T2 rulings); openspec/changes/archive/2026-10-03-dpmac-typestate/specs/mbt-harness/spec.md:32,38 vs openspec/specs/mbt-harness/spec.md:238,244 (T4 evidence); openspec/changes/dpseci-typestate/specs/mbt-harness/spec.md:26 (B2 target).
