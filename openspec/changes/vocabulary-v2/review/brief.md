# Review brief: vocabulary-v2 epic

Slug: `vocabulary-v2`. Reviewed span: commits 34e708b..a578470 on main
(1 spec-init + 6 execution commits, ~1,300 inserted / ~614 deleted lines,
55 files across crates/, models/, docs/) — the vocabulary-v2 openspec change
(epic 093, closed 2026-09-07). Quality floor (cargo build/fmt/clippy/
clippy --tests/doc, full model ladder incl. Apalache) is green; mechanical
lint findings are NOT the target. This change is ~5% the size of
intent-layer, so the review is three passes, not seven. Targets: retired
vocabulary residue, model/Rust structural-isomorphism violations (the D6 law
this change itself legislated), parity-twin drift, dead code from the
deleted pool pair, and doc/spec alignment.

## Grounding facts (verified 2026-09-07, re-verify only if main moved)

- The change deleted: the `Tenant.pool` `""` sentinel, the
  `PoolWithoutRestricted`/`RestrictedWithoutPool` refusal pair, `effTenant`
  in `intent_raw.qnt` and its parse eff-mapping, the link-end `is_kernel()`
  exemptions in `tenant_absent_refusals`, the reserved `"crypto"`/`"extra"`/
  `"pool"` tokens in `ConstructName` space, and the PASS2-F13 DEVIATION
  sentence. It added: `Isolation::Restricted { pool }` (D1), the shared
  `TenantRef` sum `Kernel | Named` at ports and link ends (D2), the
  `Referrer` sum on `TenantAbsent` (D3), and three parity refusals
  `KernelDeclared` / `LinkSelfLoop` / `RenameDoubleClaim` twinning
  `parse.rs` checks (D4). `REFUSAL_VARIANTS` is now 25 (verified at
  `crates/dpaa2-api/src/refuse.rs:309`).
- A grep for `effTenant|PoolWithoutRestricted|RestrictedWithoutPool|
  pool\.is_empty` outside openspec/ finds 43 hits in 13 files, concentrated
  in the parse-side raw layer (raw_itf.rs 16, intent_raw.qnt 11,
  COVERAGE.md 3). The raw layer's parse rules are deliberately untouched by
  design (D5: "intent_raw.qnt keeps its parse-side rules untouched"), so
  many hits are legitimate parse-side vocabulary or historical prose —
  sorting stale from deliberate is Pass 1's job, not a bulk finding.
- `is_kernel` still appears 51 times across 8 files (derive.rs, refuse.rs,
  compiled.rs, intent.rs, parse.rs, main.rs, compile_props.rs,
  intent_pairing.rs). Task 2.1's verify only required its removal from
  `tenant_absent_refusals`; whether the survivors are the pool-holder check
  (kept by design — a pool names a `TenantName`, not a `TenantRef`),
  parse-boundary normalisation (kept), or missed D2 match-arm conversions
  is Pass 2's first question.
- Key file sizes: refuse.rs 2,060 (25 variants), derive.rs 1,122,
  parse.rs 1,532, intent.rs 418; refuse.qnt 502, types.qnt 293,
  intent_raw.qnt 661. Docs touched: ADR-0002, ADR-0013, ROADMAP.md,
  COVERAGE.md.
- Discovered decisions landed under 093.7/.8: the matcher's
  `ConfigFacet::Tenant.pool` is a DELIBERATE second reading of the
  `Restricted` payload (doc-noted at matcher.rs and match.qnt, decision 11);
  kernel materialisation is deliberately SPLIT (tools shell `complete_kernel`
  owns port-only, core `effective_tenants` owns link-triggered). A finding
  proposing to fold either is a false positive by definition.
- `KernelDeclared` carries a reserved-shape exemption: `complete_kernel`
  and dry-run legitimately inject `kernel_tenant(max_cores)` before
  compile, so the refusal fires only on a kernel-named tenant of
  non-reserved shape. Witness reachability is known-asymmetric:
  LinkSelfLoop 979/3000 random-reachable; RenameDoubleClaim and
  KernelDeclared 0% (directed-test-only) — recorded in COVERAGE.md, not a
  defect.
- Out of scope: everything the intent-layer review already judged and this
  change did not touch (board machinery, module structure verdicts, the
  typestate roadmap, guidelines distillation — the 12-rule set shipped with
  the last review). Parse-side named errors and TOML surface are pinned by
  `raw_conformance` and were a non-goal; proposing changes there is out of
  scope.

## Passes

### Pass 1 — Staleness and residue map
Agent: Explore (medium). Budget ~50k. Runs first; feeds both others.
Build the retired-vocabulary list straight from the design (D1–D4 deletions
listed in grounding above) and sweep the whole tree outside openspec/ and
.git/. Classify every hit of: `effTenant`, `PoolWithoutRestricted`,
`RestrictedWithoutPool`, `pool.is_empty`, `pool: "".into`, `tenant:""` /
`tenant: ""` ITF shapes, reserved-token `ConstructName` literals
(`"crypto"`/`"extra"`/`"pool"` as construct names in refusal paths), and
prose still describing the old shapes (`Tenant.pool` field, the refusal
pair, the `tenant:"kernel"` wrinkle). Classify each: (a) stale — should
have been updated by a named task; (b) deliberate parse-side raw-layer
vocabulary (the raw alphabet deliberately keeps its own encoding);
(c) legitimately historical prose (ADR/COVERAGE recording what was). Also
sweep `ponytail:` markers in diff-touched files for ceilings this change
retired, and re-run every task-level verify grep from tasks.md verbatim
(1.1, 2.1, 3.1, 4.1, 5.2), reporting any that no longer pass.

### Pass 2 — Isomorphism, twins, and code quality
Agent: software-architect. Budget ~130k. After Pass 1.
Scope: the diff-touched model files (types.qnt, refuse.qnt, derive.qnt,
alphabet.qnt, intent_raw.qnt, main.qnt, observed.qnt) and Rust files
(intent.rs, refuse.rs, derive.rs, compiled.rs, matcher.rs, parse.rs,
intent_itf.rs, raw_itf.rs, tools main.rs, tests). Four mandates:
(a) **Structural isomorphism (D6 law)**: for each new/changed shape —
`Isolation`, `TenantRef`, `Referrer`, the three parity refusals — verify
the Rust type is sum-for-sum, payload-for-payload identical to its Quint
twin, and that names converge on the same readable English both sides. This
change legislated the law; it must itself be clean under it.
(b) **Parity-twin fidelity (D4)**: diff each of the three new compile
refusal predicates against its named parse twin (`parse.rs:172-179`,
`parse.rs:472-476`, `check_renames`) — same predicate, not merely similar;
each site carries the F3-form doc note naming its twin; the
`KernelDeclared` reserved-shape exemption matches the design's stated rule
and nothing wider.
(c) **is_kernel survivors**: classify all remaining `is_kernel` call sites
(51 hits, 8 files) as pool-holder check / parse normalisation / test
fixture / missed D2 conversion. A string test on a `TenantRef`-carrying
path that a match arm should own is a finding.
(d) **Deletion completeness**: dead arms, dead helpers, dead test fixtures
left by the pool-pair deletion and the `effTenant` removal; refuse.rs grew
by a net ~350 lines — flag per-variant boilerplate the change added that an
existing helper already provided (reuse-before-write), but judged against
the deliberate ADR-0014 lockstep duplication, which is protected.

### Pass 3 — Spec/docs alignment
Agent: spec-align. Budget ~90k. After Pass 1; parallel with Pass 2.
Scope: `openspec/changes/vocabulary-v2/{proposal,design,tasks}.md` and
specs/ deltas vs shipped code; ADR-0013 §5 (one amendment, records all four
moves) and §2 (pool spellings only where pool already appeared); ADR-0002
isomorphism law wording vs design D6; COVERAGE.md witness counts vs the
committed corpus and package.json model:coverage wiring; ROADMAP.md row;
CHANGELOG semver-major note (Migration Plan promised `!` conventional
commit — verify it exists or flag); the two discovered-decision doc notes
(093.7 matcher/match.qnt, 093.8 complete_kernel/effective_tenants twins)
present at BOTH twins each. Disposition Pass 1's category-(a) doc hits.
Verify the design's Open Question resolution: `KernelDeclared` shipped with
empty vs `tenant: TenantName` payload — whichever, spec/ADR/model/Rust must
agree.

## Ordering

1 → {2, 3 concurrent} → synthesis. Pass 1's classification feeds both:
Pass 2 judges code hits, Pass 3 dispositions doc hits.

## Report schema (every pass; include verbatim in each pass prompt)

One finding per row, no essays:
**ID** (PASSn-Fm) · **file:line** (absolute) · **category** (stale / dead /
duplicate / iso-violation / twin-drift / copy-drift / doc-drift / simplify)
· **superseded-by** (task number or commit that made it stale — MANDATORY
for stale claims; a staleness claim without a superseding event is dropped)
· **severity** (breaks-a-claim / misleads-a-reader / carries-cost) ·
**disposition** (delete / amend / fold / new ADR note / follow-up bead) ·
**verification** (exact grep/command/test confirming the fix).
Duplication findings cite BOTH sites (file:line pairs) and state the fold
mechanism plus readability cost in one line — remembering the protected
deliberate duplications named in grounding.
Fixed footer: what was read, what was deliberately not read, open questions.

## Synthesis mandate (change-judge, ~60k)

Deduplicate and rank findings across passes (Passes 1 and 3 will both find
doc-side hits); drop unanchored staleness claims and any finding that folds
a protected deliberate duplication (ADR-0014 lockstep copies, the parse/
compile twins, the matcher's second pool reading, the split kernel
materialisation); deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. A verdict on the change's own D6 compliance: is the shipped surface
   structurally isomorphic, yes or no, with the evidence rows.
3. Proposed follow-up work as bead-shaped items (title + why + acceptance)
   for a just-in-time openspec change — the review itself changes nothing.
No guidelines deliverable: the 12-rule set shipped with the intent-layer
review and this change is too small to re-open it; at most nominate a rule
amendment if a finding shows an existing rule failed to prevent a defect.

## Budgets

P1 50k · P2 130k · P3 90k · synthesis 60k ≈ 330k total across 3 pass
agents + judge. A pass exceeding ~1.5x its budget stops and reports partial.
