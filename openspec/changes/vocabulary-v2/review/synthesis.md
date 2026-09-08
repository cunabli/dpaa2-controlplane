# vocabulary-v2 epic review — synthesis (change-judge)

Reviewed span 34e708b..a578470 (7 commits, epic 093). Inputs: `openspec/changes/vocabulary-v2/review/{brief,pass1,pass2,pass3}.md`. Spot-checks performed by the judge: PASS2-F3 (refuse.rs:475-527 read — confirmed), PASS3-F2 (grep of `match.qnt` for `ConfigFacet|pool` — zero hits; matcher.rs:121/:866 claim confirmed wrong), PASS3-F1 (`.git/logs/HEAD` — no `!` on any of the seven subjects, confirmed), PASS1-F4 (parse.rs:373-393 read — confirmed), plus the main loop's independent confirmation of PASS1-F2.

**False-positive filter:** no finding folds a protected duplication (ADR-0014 lockstep copies, parse/compile twins, matcher second pool reading, split kernel materialisation) — PASS1-F4 keeps the twin errors byte-identical, PASS3-F2 repoints the 093.7 note without touching the deliberate second reading, PASS1-F5 deletes a tombstone for a test that does not exist, not a lockstep copy. **Staleness anchors:** every stale claim carries a superseding event; nothing dropped for schema violations. **Deduplication:** PASS3-F2 subsumes Pass 2's non-finding read of the same matcher note (Pass 2 verified the note exists; Pass 3 checked its target and it is wrong — Pass 3 wins on evidence). PASS2-F1/F2/F4 and PASS1-F4 merge into one `""`-encoding hygiene stream. Zero findings were fully duplicated across passes otherwise.

## 1. Merged findings ledger (most severe first)

| # | Origin | Site | Category | Severity | Confirmed | Disposition |
|---|--------|------|----------|----------|-----------|-------------|
| 1 | PASS2-F3 (+P2-Q3) | `crates/dpaa2-api/src/refuse.rs:480` + `tasks.md:33-34` | doc-drift over a real behavioral asymmetry | breaks-a-claim | judge-verified | Amend comment now (scope to the two `TenantRef` sites); bead the design decision for the three `TenantName` sites (Fabric/Crypto/Extra) where parse admits `kernel` (`parse.rs:286`) but compile fires `TenantAbsent` when no injection ran. TOML-reachable. |
| 2 | PASS3-F1 | `openspec/changes/vocabulary-v2/design.md:238-242` vs commit series | unfulfilled promise | breaks-a-claim | judge-verified (reflog) | Reword commits — see adjudication below. |
| 3 | PASS2-F2 | `models/intent/types.qnt:94,103,111,132` (`from: str`, `""`=absent) vs `intent.rs` `Option<_>` | iso-violation of the change's own D6 law | breaks-a-claim (indirect: rule 15 newly built on the divergence, no DEVIATION marker) | pass-verified | Bead: converge encodings or land a DEVIATION marker at `types.qnt:90-93`. |
| 4 | PASS3-F2 | `crates/dpaa2-api/src/matcher.rs:121,866` + `tasks.md:77-78` | doc-drift, born wrong in b25e79a | misleads-a-reader | judge-verified | Amend: repoint the 093.7 note to `models/intent/observed.qnt:32-37` (the actual model-side honesty block, which states the opposite relationship). |
| 5 | PASS2-F4 | `intent.rs:66-72` vs `types.qnt:43-44` | twin-drift on `""` (`tenantRefOf` → Kernel; `from_name` → `Named("")`) | misleads-a-reader (public API doc claims single-site normalisation) | pass-verified | Bead (fold into #3's stream): decide fold-or-note, pin with doctest. |
| 6 | PASS1-F3 | `raw_itf.rs:219-239`, `raw_conformance.rs:41`, `raw_alphabet.qnt:173`, `intent_raw.qnt:391`, `package.json:6`, COVERAGE.md:291 | half-applied rename (`RawLinkSelfLoop` variant, `Kind::LinkSelfLoop` kind, `wLinkSelfLoop` witness) | misleads-a-reader | pass-verified; precedent `RawMemberUnresolved` one line below | Bead: finish the rename to the `Raw*` spelling everywhere on the raw side. |
| 7 | PASS3-F3 | `models/COVERAGE.md:289-296` vs `package.json:6` | doc-drift: run requests 18 raw witnesses, doc records 13 | carries-cost | pass-verified | Amend: add the raw structure-dimensions line (`wKernelDeclared`, four `wRaw*Present`) or state deliberate omission. |
| 8 | PASS3-F4 | `docs/adr/0002:81-83` | law lacks its one standing exemption (093.7 projection/abstract-facet shape) | misleads-a-reader | pass-verified | Amend: one sentence naming the projection exemption / observed.qnt honesty-block mechanism. |
| 9 | PASS2-F6 | `refuse.rs:282-299` | doc-drift: `RenameDoubleClaim`/`KernelDeclared` docs omit their differently-spelled raw twins (`RenamedFromDeclared`, `ReservedKernel`) | misleads-a-reader | pass-verified; `LinkSelfLoop` at :272-277 shows the correct form | Amend to the F3-exemplar form. |
| 10 | PASS2-F1 | `models/intent/refuse.qnt:352-353,367` | copy-drift: `holderPool` returns the `""` sentinel D1 outlawed | misleads-a-reader | pass-verified; clean idiom exists at `derive.qnt:461` | Amend (fold into #3's stream): bool-returning match, keep the hoisted-helper shape. |
| 11 | PASS1-F1 | `models/intent/scenarios/vwire.qnt:32` | stale (42dfe06/task 1.1) | misleads-a-reader | pass3-endorsed | Amend: reword to "unrepresentable since D1". |
| 12 | PASS1-F2 | `docs/adr/0013:462` | stale line pin (a50f412/task 2.1) | misleads-a-reader | main-loop confirmed | Amend by name ("the `Dataplane` comment in `types.qnt`"), not by line. |
| 13 | PASS2-F5 | `refuse.rs:1738,1758,1780` | simplify: `contains` vs the module's stated complete-set `assert_eq!` convention | misleads-a-reader | pass-verified | Amend: `assert_eq!` on the singleton set — free strength. |
| 14 | PASS1-F4 | `crates/dpaa2-config/src/parse.rs:381-382` | simplify: manufactures `""` from an `Option` just to emptiness-test; also the reason task 1.1's verify grep passes vacuously | carries-cost | judge-verified | Amend (fold into #3's stream): `match t.pool.as_deref()`; errors stay byte-identical. |
| 15 | PASS1-F5 | `refuse.rs:1643-1647` | duplicate: sixth tombstone, in a test module, for a test that does not exist | carries-cost | pass-verified | Delete this one; the other five each earn their place. |
| 16 | PASS3-F5 | `openspec/changes/vocabulary-v2/proposal.md:90` | copy-drift: wrong corpus path (`models/traces/` vs `models/intent/traces/`) | carries-cost (minor) | pass-verified | Amend path. |

## 2. D6 compliance verdict: **YES, qualified**

The surface this change shipped is structurally isomorphic. Evidence rows (Pass 2 mandate (a), spot-consistent with Pass 3's alignment table):

| Shape | Model | Rust | Verdict |
|---|---|---|---|
| `TenantRef` | `Kernel \| Named(str)` (`types.qnt`) | `Kernel \| Named(TenantName)` (`intent.rs:49-50`), no `Default` either side | sum-for-sum |
| `Isolation` | `Public \| Restricted(str) \| Isolated` | `Public \| Restricted { pool } \| Isolated` | payload-for-payload (named-vs-positional is spelling, excluded by D6) |
| `Referrer` | six cases (`refuse.qnt:44-50`) | six cases (`refuse.rs:50-63`), documented `Ref` prefix per the ADR-0002 prefix-artifact permission; decoder follows the `MPort` precedent | case-for-case |
| `LinkSelfLoop{link}` / `RenameDoubleClaim{construct,from}` / `KernelDeclared` (nullary) | `refuse.qnt` | `refuse.rs` | payload-identical; the design's open question resolved to the empty payload consistently at all five sites |
| Counting laws | `REFUSALS_USE`/`REFERRERS_USE` 25/6 | `REFUSAL_VARIANTS` `[&str; 25]` at `refuse.rs:309` | complete |

Two qualifications, neither a new shape the change authored: (a) rule 15 (`RenameDoubleClaim`) is newly built atop the one surviving sentinel-for-case encoding (`types.qnt` `from: str` with `""`=absent vs Rust `Option`) — under the change's own amended ADR-0002 law this needs convergence or a DEVIATION marker (ledger #3); (b) the twin normalisation helpers diverge on the `""` input (ledger #5). Both are encoding-hygiene debts, not sum/payload mismatches in the shipped D1–D4 surface.

## 3. PASS3-F1 adjudication: reword the commits (option A)

Recommendation: interactive rebase rewording 42dfe06 (task 1.1) and a50f412 (task 2.1) subjects to the `!` form. Reasons, weighed:

- The series is local and unpushed; the repo convention is explicitly "amendable until sealed". The reflog already shows 42dfe06 was itself an amend. Message-only rewords of the five descendants are mechanical and conflict-free.
- Option B (amend design.md + hand-noted CHANGELOG line) creates two permanent debts: a design that records its own broken promise, and a hand edit to a file CLAUDE.md declares "entirely managed with cliff" — a standing-rule violation to paper over a fixable one.
- Only option A makes the promise actually true: cliff derives the semver-major section from `!`, so the changelog note materialises automatically, which is the outcome the Migration Plan was promising in the first place.

## 4. Open-question dispositions (all eight)

| Q | Disposition |
|---|---|
| P1-Q1 (`Default` on `Isolation`/`Intent` vs D2's no-Default argument) | Fold: one decision line in bead B2's design notes, flagged for the typestate pass. Not an ADR — a derive choice, not an open-ended ambivalence. |
| P1-Q2 (pre/post-normalisation comparison asymmetry stated only in a comment) | Drop: the twin-site comment is exactly the ADR-0014 mechanism this repo uses for such notes; the agreement is parse-guaranteed (`""` link end unrepresentable) and Pass 2 verified same-predicate-on-reachable-domain. No new law needed. |
| P1-Q3 (R11-equivalent lint for the raw alphabet) | Drop the lint, fix the drift: bead B3 finishes the rename; per the no-defensive-machinery rule, the failing thing is the spelling, not the absence of a linter. Revisit only if raw witness names drift a second time. |
| P2-Q1 (cross-family duplicate names as a remaining one-sided D11 row) | Fold into bead B4: one posture row in ADR-0013 §5 / COVERAGE recording duplicate-name as deliberately raw-only. |
| P2-Q2 (`Restricted { pool: "" }` constructible programmatically) | Fold with P1-Q1 into B2's typestate note. |
| P2-Q3 (who closes the kernel-at-`TenantName`-sites asymmetry) | Fold into bead B1 — it is that bead's central design decision. |
| P3-Q1 (which unrecorded raw witnesses are new vs pre-existing) | Drop: immaterial — the wired command reports counts the authoritative doc never records, either way (Pass 3's own words). |
| P3-Q2 (commit reword vs design amend) | Resolved by the adjudication in §3. |

## 5. Follow-up beads (for one just-in-time openspec change; the review changes nothing)

**B1 — Compile and parse agree on `kernel` at the three `TenantName` reference sites.**
Why: `refuse.rs:480`'s claim "no site can ever name `kernel` as absent" is false inside its own function; parse treats kernel as always-resolving (`parse.rs:286`) while compile rule 1 checks only declared names, so `TenantAbsent{Fabric|Crypto|Extra, "kernel"}` is TOML-reachable when no injection ran. Pre-existing behavior; the change's claim text is what broke.
Acceptance: comment scoped to the `TenantRef` sites; a recorded decision (always-resolving vs parse-refuses — either closes it) in ADR-0013; a unit test pinning the decided behavior for a hw fabric `forwarded_by "kernel"` with no kernel port; model twin updated in lockstep.

**B2 — The `""` case-encoding leaves the twins or is DEVIATION-marked.**
Why: the change legislated "sentinel-for-case violates the spec even when every trace matches", then built rule 15 on `types.qnt`'s `from: ""` vs Rust `Option`, left `holderPool` returning the outlawed sentinel, left `from_name`/`tenantRefOf` diverging on `""`, and left parse.rs manufacturing `""` from an `Option`.
Acceptance: `types.qnt` rename field converged or DEVIATION-marked at :90-93; `holderPool` is a bool-returning match (hoisted-helper shape preserved); `TenantRef::from_name("")` behavior decided and pinned by doctest with both twins noting the domain split if kept; `parse.rs:381` uses `as_deref()` with byte-identical errors; typestate note recording the `Default`/empty-`TenantName` constructibility hazards (P1-Q1, P2-Q2). Model ladder + `raw_conformance` green.

**B3 — The raw self-loop spelling finishes its rename.**
Why: task 4.1 renamed the raw variant to `RawLinkSelfLoop` but left `Kind::LinkSelfLoop`, `isLinkSelfLoop`, `wLinkSelfLoop` across six files; the repo's own precedent one line below is fully `Raw`-prefixed.
Acceptance: `grep -rn 'LinkSelfLoop'` on the raw side shows only `Raw`-prefixed spellings; `raw_conformance` green; no new lint machinery.

**B4 — Docs, pointers, and tests say what shipped.**
Why: eight small drifts, five born in this change (ledger #4, #7, #9, #11 narration, #16), all misleading a reader at exactly the sites a reader consults.
Acceptance, one grep each: matcher.rs 093.7 note points at `observed.qnt` (no `match.qnt` `ConfigFacet` claim) and tasks.md 5.2 corrected; COVERAGE.md records all 18 requested raw witnesses or states the omission; ADR-0002 names the projection exemption; `RenameDoubleClaim`/`KernelDeclared` docs name `RenamedFromDeclared`/`ReservedKernel`; vwire.qnt reworded; ADR-0013:462 repointed by name; the three parity tests use complete-set `assert_eq!`; the `refuse.rs:1643` tombstone deleted; proposal.md path fixed; one D11 posture row for duplicate-name (P2-Q1).

## 6. Required action items (checklist)

- [ ] Before sealing: reword 42dfe06 and a50f412 to `!` subjects via message-only rebase; verify `git log --format='%s' 34e708b.. | grep '!:'` non-empty and cliff emits a major section (ledger #2).
- [ ] Immediate comment amend (one line, no design work): scope `refuse.rs:480` truthfully — can ride any commit touching refuse.rs, or land with B1.
- [ ] Propose the JIT openspec change carrying B1–B4 (B1 and B2 carry design decisions; B3 and B4 are mechanical).
- [ ] Rule-amendment nomination (the only one warranted): PASS1-F2 shows a line-number cross-reference going stale within the same change series that wrote it, while every other cross-ref in the repo names a symbol. If the 12-rule set does not already say "cross-references name symbols/functions, never line numbers", amend it to; otherwise no guidelines action. Secondary lesson, attached to B2 not the rules: task 1.1's verify grep (`pool.is_empty`) passed vacuously against `pool.as_str().is_empty()` — acceptance greps should target the concept, not one spelling.

No architectural shifts detected: Pass 3 confirmed zero scope creep (every shipped move traces to D1–D6 or decisions 093.7/.8), and Pass 2 found no missed D2 conversions among the 51 `is_kernel` survivors and no dead code from the deletions. Blast radius of all proposed fixes is comments/docs/tests plus two bounded design decisions (B1, B2) — no type-surface churn beyond the possible `types.qnt` rename-field convergence, which is model-side with one Rust rule already in the `Option` shape.
