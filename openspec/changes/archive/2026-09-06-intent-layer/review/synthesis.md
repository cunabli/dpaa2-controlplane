# Synthesis — intent-layer epic review (648f804..4272019, judged 2026-09-06)

## Verdict

The epic is structurally sound: the typestate witness pattern, the R11–R15 lint spine, the frozen-trace MBT harness, and the doc-example pinning all held under seven adversarial passes — no memory-safety, no logic defect in the shipped compile/reconcile/match pipeline, and every crate earned or defended its structure verdict. The defect population is overwhelmingly *amendment residue*: the 2.3a/2.6x/3.3x/6.6 rename trail left ~35 stale comments, headers, and spec sentences, each with a named superseding commit (no unanchored staleness claims survived filtering). Two findings are substantive: the tenant-emission order-dependence that both the model and Rust share while their own docs claim the opposite (the D11 programmatic surface is exposed), and the num-queues seam leak where the dry-run and `ensure` disagree on a dpni attribute. One hard constraint governs sequencing: five spec-delta amendments must land before `pnpm openspec archive intent-layer`, or a retired label law and a stale compile signature get promoted verbatim into `openspec/specs/`.

## Merged findings ledger

### Pre-archive — must land before `pnpm openspec archive intent-layer`

| # | ID(s) | Sites | Defect | Severity | Disposition |
|---|-------|-------|--------|----------|-------------|
| A1 | PASS6-F1 | `openspec/changes/intent-layer/specs/intent-compiler/spec.md:263-273,128-129` (+ `design.md:212`, `proposal.md:45`, `tasks.md:41`) | Promotable delta still mandates the retired `<tenant>/<family>/<ordinal>` label law (`router/dpio/3`); shipped label = construct name (6.6, ADR-0015 d3/d13) | breaks-a-claim | amend before archive |
| A2 | PASS1-F11 + PASS6-F2 | `specs/intent-compiler/spec.md:92-93`; `design.md:167,187` | `compile` SHALL return `Result<Plan, Refusal>` — shipped is `Result<Compiled, BTreeSet<Refusal>>` (`refuse.rs:876`); `Plan` is a different type | misleads-a-reader | amend both sites; spec site is archive-blocking |
| A3 | PASS6-F3 | `specs/intent-compiler/spec.md:138-140` | Refuse list opens with phantom dpci/dpdcei constructs; `TenantAbsent` is the generalised rule | misleads-a-reader | amend before archive |
| A4 | PASS6-F4 + PASS1-F12 | `specs/intent-compiler/spec.md:136-162`; `design.md:168-184` | SHALL-refuse enumeration under-counts `REFUSAL_VARIANTS` (spec misses `SelfMember`; design D5 misses 6 of 24) | misleads-a-reader | amend; one check — every variant maps to a clause |
| A5 | PASS1-F16 | `specs/intent-compiler/spec.md:47`; `design.md:267` | "its own regime companion draw" — subject is a tenant, field is `dataplane` (2.3a) | carries-cost | amend; spec site is archive-blocking |
| A6 | PASS6-F8 | `specs/reconciler/spec.md:86-99` | Scenarios promise converge-level rename behavior; the MatchPlan→Transition lowering is unwired (see B2) | misleads-a-reader | scope scenario wording to the dpaa2-api relation at/before archive; the lowering itself is bead B2 |

### Substantive gaps (post-archive OK, fix soon)

| # | ID(s) | Sites | Defect | Severity | Disposition |
|---|-------|-------|--------|----------|-------------|
| L1 | PASS2-F3 + PASS2-F4 + PASS3-F1 (+ Pass 7 row 3) | `models/intent/types.qnt:122-125`, `derive.qnt:617-619`, `intent_raw.qnt:315`, `invariants.qnt:298-302`; `crates/dpaa2-api/src/intent.rs:245-257`, `derive.rs:566-572,1111-1116`; `tests/compile_props.rs:948-956` | Tenant emission order is declaration-dependent on BOTH model and Rust (Pass 3 corrected Pass 2's framing); the position-independence claim is enforced only by dpaa2-config's `BTreeMap` — the D11 programmatic surface gets order-dependent `order` from two "equal" intents; `positionIndependence`/I10 never permute tenants/fabrics | breaks-a-claim | lockstep fix: `sortByRank` tenants in `fullOrder` + `toIntent`; one-line name-sort in `effective_tenants`; widen I10 prop + `positionIndependence` to tenants+fabrics; fix the `intent.rs` doc |
| L2 | PASS5-F2 | `crates/dpaa2-mc/src/restool.rs:469,119` vs `dpaa2-api/src/derive.rs:505`; `render__dry_run_reference.snap:329`; `render.rs:19-20` | Two dpni-sizing authorities: dry-run shows compiled `num_queues=5`, `ensure` re-derives from host nproc (creates 16); render.rs "exact plan `ensure` executes" over-claims | breaks-a-claim | bead B3 (carry the attribute); amend the render.rs claim now |
| L3 | PASS3-F6 | `crates/dpaa2-api/src/compiled.rs:9-19,340-346`; `compile_props.rs:958-961` | "double connect … not a value this module admits" is false — `attach_point()` re-mints; single-connect is I2-runtime-guarded, not type-level | breaks-a-claim | scope both docs to the handle-level consume lock; no registry (over-machinery) |
| L4 | PASS1-F1 | `design.md:74-77` | D1 claims crypto shares "the same positional convention" as ports/links — INTENT_I10 proves the opposite post-3.3d | breaks-a-claim | amend |
| L5 | PASS2-F5 | `models/intent/derive.qnt:55-58` | `labelOf` — zero callers AND restates the retired label rendering, contradicting its own file's 6.6 header | breaks-a-claim | delete |
| L6 | PASS6-F7 | `models/COVERAGE.md:256-270` | Raw-surface laws table restates `intent_raw.qnt` names with no lint and no bead named in the table text — ADR-0014 rule 2 unmet | breaks-a-claim | extend the R15 mechanism to point at `intent_raw.qnt` inside the amendments change (Pass 6: cheaper than a bead) |
| L7 | PASS7-F2 (+ Pass 2 catalog row) | `models/intent/refuse.qnt:66` ↔ `refuse.rs:328-345` ↔ `intent_itf.rs:545,552` | Warning vocabulary spelled 3× with zero lint — and synthesis grep confirms **zero frozen traces exercise a warning** (see Residual checks), so the decoder tags are entirely unexercised; severity raised per Pass 7 Q1 | breaks-a-claim (upgraded) | `Warning::name()` exhaustive match + R14 Warning leg + decoder keys off `name()`; negative lint test |
| L8 | PASS7-F1 | `models/intent/intent_raw.qnt:153-157` ↔ `family.rs:56-75` | `FAMILY_NAMES` is the one hand-maintained lowercase family list, unlinted; synthesis grep confirms `raw_alphabet.qnt` draws only `{dpio, dpni, dpwidget}` — 14/16 entries conformance-unexercised | carries-cost (confirmed necessary) | add R14 leg bijecting `FAMILY_NAMES` ⇄ `ALL_FAMILIES.map(as_str)`; keep both copies |

### Staleness / copy-drift amends (all anchored to a superseding commit)

| # | ID(s) | Sites | Defect | Severity | Disposition |
|---|-------|-------|--------|----------|-------------|
| L9 | PASS2-F1 + PASS4-F1 + PASS2-F2 + PASS4-F7 | `match.qnt:58-69`; `intent_itf.rs:13-15`; 10 scenario sites (`scenarios/*.qnt`/`.toml`); `ledger.rs:1154-1156` | Spent forward-looking notes: "RED RUST TWINS … AWAITS" (twins shipped 6.5–6.7); D9 revisit trigger fired at 6.7, never re-judged; "by convention until task 3.4" / "is task 3.4" (pairing test exists) | misleads-a-reader | amend all; record the 6.7 re-decision in the D9 note |
| L10 | PASS2-F9 + PASS3-F3 + PASS4-F4 + PASS6-F6 (+ PASS2-F8 cousin) | `invariants.qnt:12,21-22`, `main.qnt:10,701`, `alphabet.qnt:246-247`; `compile_props.rs:6-14`; `ledger.rs:1110-1111`; `docs/adr/0014:84-88`; `COVERAGE.md:201-202` | Invariant-count drift: "I1..I9"/"all nine" survives I10 (3.3d) on four surfaces incl. ADR-0014's own compliance exhibit; Apalache subset spelled I7/I8 vs I7/I8/I9 in three places | misleads-a-reader | one sweep; align Apalache marking on I7/I8/I9 (ADR-0002 agrees); go unnumbered where possible (rule 4) |
| L11 | PASS1-F2 | `proposal.md:70` | `[[extra]]` array idiom outlived 3.3b | misleads-a-reader | amend |
| L12 | PASS1-F3/F4/F5 | `derive.rs:133`; `derive.qnt:193`; `invariants.qnt:289-290` | "`[[port]]` position" phrase — syntax deleted by the same commit that wrote the comments (3.3d) | misleads-a-reader | amend |
| L13 | PASS1-F6 | `refuse.qnt:83` | "port owner, fabric owner, crypto owner" — V3-retired role names, written after the rename | misleads-a-reader | amend |
| L14 | PASS3-F5 | `model.rs:243-247,275-281` | `Presence`/`immutable` docs describe a deleted config path ("phase 1" `DesiredTopology` from config) | misleads-a-reader | rewrite against the D10 projection |
| L15 | PASS3-F7 | `compile_props.rs:158-183,546-557` | The kernel-pooled-Restricted I1 hole is marker-recorded model-side (`invariants.qnt:104`), silent on the Rust twin | misleads-a-reader | mirror the marker on `intent_i1` + strategy |
| L16 | PASS6-F5 | `docs/adr/0013:509-517` | "slated for the ledger lint … bead gqf.34 … until that lands" — lints and pairing test shipped and green | misleads-a-reader | rewrite as delivered cross-references |
| L17 | PASS4-F6 + PASS5-F5 | `dpaa2-verify/src/{lib,mcstatus,ioctlpolicy}.rs`, `ledger.rs:1110,1289`; `dpaa2-tools/src/link.rs:8`, `tests/link_gen.rs:1` | Three changes now own a "task 6.3/6.4"; bare task numbers unresolvable | misleads-a-reader | qualify with change slug, once per module header |
| L18 | PASS1-F15 | `docs/baseline/object-model.md` | Cited anchor of INTENT_I1/I2/I4 with zero bridge to the renamed vocabulary | misleads-a-reader | one head-of-file bridge note (Pass 6 scoping: no per-section edits) |
| L19 | PASS2-F10 + PASS3-F8 | `refuse.qnt:254`; `refuse.rs:671-755` | Rule comments run …8, **11**, 9, 10, 12 on both twins (2.6e insertion) | carries-cost | renumber lockstep (rule 3) |
| L20 | PASS2-F12 + PASS4-F8 | `intent_raw.qnt:7`; `raw_itf.rs:6` | "post-3.3b/3.3c" header missing 3.3d — identical phrase, one commit missed both twins | carries-cost | one-word amend, both files |
| L21 | PASS2-F11 | `observed.qnt:101` vs `derive.qnt:106` | Two unrelated `type Plan` in one corpus; the natural follow-on law forces importing both | carries-cost | rename match layer's to `MatchPlan` (Rust already is) |
| L22 | PASS2-F13 | `refuse.qnt:86-105` ↔ `intent_raw.qnt:238-258` | Deliberate parse/compile dup (keep), but `TenantAbsent{tenant:"kernel"}` for an ownerless port is undocumented UX | carries-cost | one DEVIATION sentence now; variant change deferred to bead B1 |
| L23 | PASS2-F7 | `alphabet.qnt:325,55`; `package.json:6` | `wInterruptDrawn` carries V5-retired naming; not R11-pinned | carries-cost | rename `wEventDrawn`, 3 sites + coverage re-run |
| L24 | PASS3-F2 | `compile_props.rs:832-835` | "every other field's order IS the ordinal source" contradicts the I10 prop 110 lines below | carries-cost | amend |
| L25 | PASS3-F4 | `model.rs:197-204`; `lib.rs:48` | `ObjectKind` dead since 3.2 keyed plans by `Family` | carries-cost | delete |
| L26 | PASS4-F5 | `adapter.rs:37-38` | Miscites ADR-0013 §5 (refusals) for the family vocabulary | carries-cost | cite `types.qnt` `Family` instead |
| L27 | PASS1-F13 | `tasks.md:260` | "ForeignAnchor" — accepted spelling is `Foreign` | carries-cost | amend |
| L28 | PASS1-F14 + PASS6-F9 | `docs/ROADMAP.md:23`; `docs/adr/0005:3-5` | Row 3 date 09-05 vs 09-06 phase-6 deliverables; Delivers omits ADR-0014; ADR-0005 status carries the same stale date | carries-cost | one coordinated amend |
| L29 | PASS1-F9 + PASS1-F10 | `docs/adr/0012` (head); `docs/adr/0013` §6 | ADR-0012 lacks the forward pointer; `companionCountsByRegime` is the deliberate ADR-0012 bridge | carries-cost | Pass 6 dispositions verbatim: two relates-to lines in ADR-0012 + one gloss sentence in ADR-0013 §6; **no rename** (R12 untouched) |
| L30 | PASS1-F17 | `README.md:75` | "poll-mode regime" at the first-reader surface | carries-cost | one-word amend |
| L31 | PASS7-F3 | `intent_raw.qnt:121-131` | RawRefusal DEVIATION posture correct (Pass 7 ruling: no ADR section, no R-rule) but the graduation trigger is unwritten | carries-cost | one sentence: graduates to ADR-0013 §5 + R14 when config exposes a typed parse-error enum |
| L32 | PASS5-F6 | `parse.rs:547-551` | Crypto errors are the only construct errors that can't name their block | carries-cost | include the 1-based index (= dpseci ordinal, d4) |
| L33 | PASS5-F7 | `dpaa2-tools/src/status.rs:18,44` | `PortStatus.name: String` — the exact known-regression pattern | carries-cost | type as `ConstructName`; Display at print site |
| L34 | PASS5-F3 | `parse.rs:369-378,440-443,466-470,496-499,522-543` | Deliberate seam duplication undocumented at 4 of 5 sites; a dedup breaks raw-conformance silently | carries-cost | one twin-naming doc line per site (the `parse_family` pattern) |

### Fold set (code, no behavior change)

| # | ID(s) | Sites | Defect | Disposition |
|---|-------|-------|--------|-------------|
| L35 | PASS3-F9 | `refuse.rs:716-740` ↔ `:836-853` | Needed-count loop twice; `derive()` runs 3× per compile | fold: `family_counts` helper, derive once |
| L36 | PASS3-F10 + PASS7-F4 | `refuse.rs:952-996` ↔ `compile_props.rs:466-510` | Two hand copies of `REF_INVENTORY` | fold: one testkit-feature helper; ADR-0014 then satisfied, no lint |
| L37 | PASS3-F11 | `derive.rs:588-651` | Two hand-synced emission walks | fold: push keys in the same walk |
| L38 | PASS4-F2 | `intent_itf.rs:193-199` ↔ `edits_itf.rs:73-79` ↔ `adapter.rs:46-50` | Family-by-tag scan ×3 | fold: `family_of_tag` in `itf.rs` |
| L39 | PASS4-F3 | `edits_itf.rs:109-112` | `opt_cname` re-implements private `opt_name` | fold: `pub(crate)` the generic |
| L40 | PASS4-F9 | `edits_itf.rs:43` | Re-declares `dpaa2_api::KERNEL` | fold: import it |
| L41 | PASS4-F10 | `intent_itf.rs:133-189` | Generic ITF helpers live in one alphabet's module | fold into `itf.rs` — **inside** the B6 reorganization commit (Pass 4 Q3) |
| L42 | PASS5-F1 | `restool.rs:406-418` ↔ `inventory.rs:147-154` | `"dpl"` judgment twinned; mc judges under a "never judges" comment | fold: mc reports raw label, `availability_of` judges once |

### Accepted leave-its (recorded so they aren't re-litigated)

PASS3-F12 (`REFUSAL_VARIANTS` triple — lint-backed, macro would hide the teeth), PASS3-F15 (`origin_list` O(n²) clones — board-scale bounded, memoize only on profile evidence), PASS4-F11 (four ITF readers ≠ one framework), PASS5-F9 (pipeline in the binary — fold when a second frontend lands), PASS7-F5 (parser/model/verify serde triplet is D10-protected; any future fold proposal is a false positive by mandate), PASS7-F3's no-ADR ruling for RawRefusal, structure keep-flat for dpaa2-api/config/mc/tools.

### Dropped / re-dispositioned

- **PASS1-F8** (empty CHANGELOG) — overturned by Pass 6: cliff manages it; empty `[Unreleased]` pre-first-tag is the mechanism's normal state. Becomes repo-wide bead B5, not charged to this epic.
- Pass 2's model-vs-Rust framing of the tenant-order defect — corrected by Pass 3 (both sides diverge from the claim); merged as L1.
- The review brief's "`[[tenant]]`-family hit in schema.rs" — false positive (Pass 5): every `[[…]]` is `[[crypto]]`, current by ADR-0015 d4.
- No unanchored staleness claims survived: every stale row above names its superseding commit or ADR event.

## Guidelines

### (a) Paste-ready "Repo rules" for `.claude/agents/rust-developer.md` (9 of 12 budget)

```markdown
## Repo rules

1. Name slots use dpaa2-api newtypes, never `String`; convert only at the serde/display boundary. (memory: name-slots-use-newtypes; PASS5-F7)
2. A "by construction" claim names its constructor — the sort, consume, or private field lives in the claiming crate, or the claim is scoped to the boundary that enforces it. (PASS3-F1)
3. Model and Rust twins edit in lockstep: rule numbers, "post-X" lineage phrases, and known-hole markers change on both surfaces in the same commit. (PASS2-F10)
4. Prose never hard-counts a linted enumeration — write "the INTENT invariants", not "all nine" or "I1..I9". (PASS2-F9)
5. A forward-looking note ("awaits X", "revisit if Y", "until task Z lands") is closed or re-judged in the commit that makes it true. (PASS4-F1)
6. Task-number citations in code carry their openspec change slug — three changes already own a "task 6.3". (PASS4-F6)
7. A check deliberately duplicated across the config→api seam names its twin and the raw_itf spelling pin in a doc note at each site. (PASS5-F3)
8. Adapters report, never judge: classification rules and sentinel constants live once, core-side. (PASS5-F1)
9. Every hand-maintained copy of a model vocabulary is lint-bijected, generated, or carries its bead — two-entry copies included. (ADR-0014; PASS7-F2)
```

### (b) Retire list (net guidance goes down: +9 lines in, ~13 lines out)

- `/workspace/dev/clearfog-cx-lx2/src/dpaa2-controlplane/.claude/agents/rust-developer.md` — Guiding-examples bullet 1 (the 7-line newtype example, lines 23–28) → superseded by rule 1.
- Same file — Guiding-examples bullet 2 (the 2-line "fix names in both places, quint and Rust" example, lines 29–30) → superseded by rule 3.
- `/workspace/dev/clearfog-cx-lx2/src/dpaa2-controlplane/CLAUDE.md` — final team-standards bullet ("Tables that restate rules from a source of truth are linted (ADR-0014)…") → superseded by rule 9, which is broader (any vocabulary copy, not just tables); ADR-0014 remains the source of truth for the main loop.
- Per-parcel boilerplate — the standing "put the newtype rule in every crate parcel spec" sentence (memory: name-slots-use-newtypes) is no longer repeated per parcel once rule 1 lives in the agent file; the acceptance-time `grep` audit stays with the main loop.

### (c) Rejected / folded candidates

- Pass 3 G2 (renumber rules), Pass 4 #2 (lineage phrase both twins), Pass 3 G3 (mirror hole markers) — folded into rule 3; three symptoms of one lockstep discipline.
- Pass 5 #3 (task slugs) — duplicate of Pass 4 #3 → rule 6.
- No candidate deleted as mechanically gated today: none is covered by clippy/fmt or the current ledger lints. Rule 9 becomes partly mechanical once L7/L8's R14 legs land — revisit it then; the lint is the rule for the surfaces it covers.

## Follow-up work (bead-shaped, for a just-in-time openspec change)

**B0 — `intent-layer-review-amendments` (the closing change; the review itself changes nothing).**
Why: 40+ ledger rows are amends/folds/deletes with per-row verification greps; five are archive-blocking.
Acceptance: (1) rows A1–A6 land first, then `pnpm openspec archive intent-layer` merges the deltas; (2) every ledger verification grep passes; (3) L6's raw-law lint leg and L7/L8's R14 legs land with negative tests; (4) `cargo build|fmt|clippy|clippy --tests|doc` green; (5) L1 lands as one lockstep commit (model + Rust + widened props). B4 and B7 below may ride as tasks of this change.

**B1 — vocabulary v2 / programmatic-surface hardening (own openspec change; Pass 7 rows 4–6 bundled).**
Why: three items edit the same linted spine (`refuse.qnt` + ADR-0013 §5 + `REFUSAL_VARIANTS` + R11 witnesses + alphabet); three separate changes churn it three times.
Scope: `Isolation::Restricted { pool: TenantName }` deleting the `""` sentinel and two refusal variants (PASS5-F4); compile-side parity refusals for self-loop links, rename double-claim, and declared `kernel` closing the D11 surface (Pass 5 inventory one-sided rows); `TenantAbsent` payload enum replacing the `"crypto"`/`"extra"`/`"pool"` sentinel tokens and the `tenant:"kernel"` wrinkle (PASS3-F13, PASS2-F13/Q3).
Acceptance: `grep -rn 'pool.is_empty\|pool: "".into' crates/` empty; parse-side named errors unchanged (`raw_conformance` green); R11/R14 green; ADR-0013 §5 amended once.

**B2 — MatchPlan→Transition lowering (PASS3-F14 + PASS6-F8).**
Why: the phase-6 identity layer is delivered but unwired — nothing lowers a `MatchPlan`, and no roadmap row owns it; it will rot beside reconcile's label repair.
Acceptance: bead names the owning change (#4 dprc-encapsulation or #9 cross-dprc-links) and the `specs/reconciler/spec.md` scenarios; converge exercises the matcher path; **first check `bd show gqf.57`** for overlap before filing.

**B3 — dpni attribute fidelity across `McControl::create_dpni` (PASS5-F2).**
Why: compile derives `num_queues`; the transition drops it; the shim re-derives from host nproc — dry-run says 5, `ensure` creates 16.
Acceptance: `Attributes::Dpni` carried through `Transition::Create`; shim test asserts `--num-queues` equals the compiled attribute; render.rs "exact plan" claim true again. Check `bd` for a duplicate first (gqf.57 covers unanchored families only; this facet is executed, so it looks unclaimed).

**B4 — witness-count snapshot single-sourcing (PASS2-F14).**
Why: seed-20260831 counts hand-pasted in `alphabet.qnt:51-63` and `COVERAGE.md:206-244` (+ partial in `package.json:6`), unlinted — ADR-0014 requires a lint or a bead.
Acceptance: `alphabet.qnt` block reduced to a pointer at COVERAGE.md, or an R-rule cross-checks the counts, or the bead id appears in the table's own text.

**B5 — repo-wide cliff wiring (PASS1-F8, re-dispositioned by Pass 6; not charged to this epic).**
Why: nothing runs `git cliff`; the identical unmet DoD clause sits in the archived restool-baseline change too.
Acceptance: cliff generation wired into the release/tag flow; ROADMAP DoD 5 reworded "CHANGELOG flows from conventional commits via cliff at release".

**B6 — dpaa2-verify structure group (Pass 4 verdict; crate split rejected).**
Why: the dependency graph is already two clusters (`itf.rs` the only shared node); `src/board/` vs `src/intent/` makes the ADR-0003 safety question answerable from the tree.
Acceptance: 13 moves + ledger.rs split into `board/ledger.rs` + `intent/lint.rs`; L41's helper move inside the same commit; zero logic changes; all tests green in one mechanical sitting.

**B7 — `owner` identifier sweep (PASS1-F7 + PASS2 addendum `alphabet.qnt:180,203,218` + PASS5-F8 `render.rs:74`, `main.rs:292,297`).**
Why: the V3-retired role name survives as identifiers now overloaded against the live DPL sense.
Acceptance: every `\bowner\b` hit in `models/intent/`, `derive.rs`, and the tools test helpers is either `forwarded_by`/`tenant` or explicitly DPL-qualified.

## Residual checks

- **Ran — PASS7 Q1:** `grep -l "UnknownCeiling|UnmeasuredCombination" models/traces/*.itf.json` → **no files**. No frozen trace exercises a warning; L7's severity upgraded accordingly (the warning decoder tags in `intent_itf.rs` are dead code against the committed corpus).
- **Ran — PASS7 Q2:** `raw_alphabet.qnt:61` draws families from `Set("dpio", "dpni", "dpwidget")` only — 14 of 16 `FAMILY_NAMES` entries are conformance-unexercised; L8's R14 leg is confirmed necessary, not optional.
- **Manual (needs Bash, not available to the judge):** `git log --oneline 648f804..4272019 -- crates/dpaa2-verify/src/{snapshot,safety,ioctlpolicy,mcstatus,driver,generate}.rs` — Pass 4's content-grep proxy shows no intent vocabulary reached the board files, but a semantic behavior tweak in the span is unruled-out.
- **Manual (`bd`):** duplicate checks before filing B2 (vs gqf.57 and the open design bead gqf.40) and B3.
- **Flag while editing the agent file:** `.claude/agents/rust-developer.md` Hard Rule 3 still names VPP/DPDK interfacing — carried over from the sibling VPP effort and stale for this repo; fix in the same edit that lands the Repo rules section.
