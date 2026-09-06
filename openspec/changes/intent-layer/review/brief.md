# Review brief: intent-layer epic (full version)

Slug: `intent-layer`. Reviewed span: commits 648f804..4272019 on main
(~58 commits, ~24,900 inserted lines, 151 files) — the intent-layer openspec
change (epic gqf, closed 2026-09-06). Quality floor (cargo build/fmt/clippy/
clippy --tests/doc, model ladder) is green; mechanical lint findings are NOT
the target. Targets: staleness, dead code, duplication (intra- and
cross-crate), simplification, module structure, sans-io seam leaks, model/Rust
copy drift, doc/ADR drift, typestate alignment, and a distilled Rust
guidelines set for `.claude/agents/rust-developer.md`.

## Grounding facts (verified 2026-09-06, re-verify only if main moved)

- Staleness risk concentrates in the epic's amendment trail: tasks 2.3a, 2.5a,
  2.6a–e, 3.3a–e, and phase 6 renamed or retired vocabulary mid-flight
  (regime→dataplane, `kernel`→`kernel-netlink`, `[[limit]]` deleted,
  `[[extra]]`/`[[tenant]]`/`[[port]]` array→keyed-table migrations, max-fold
  ceiling retired, fixed `dpaa2ctl` label tag retired in 6.6).
- A grep for retired vocabulary already finds ~73 hits across 25 files outside
  openspec/ (e.g. docs/adr/0012-* 11 "regime" hits, models/core/companions.qnt
  17, models/main.qnt, docs/baseline/object-model.md, README.md). Some are
  legitimately pre-change vintage; sorting that is Pass 1's job.
- Crate sizes: dpaa2-verify 17 flat modules ~14,400 lines, two vintages side
  by side (board-verify from change #2: adapter.rs 2362, ledger.rs 2356,
  generate.rs 2066, driver.rs 1417; intent-layer: intent_itf.rs 753,
  raw_itf.rs 764, edits_itf.rs 183, fitcheck.rs 463). dpaa2-api 19 flat
  modules ~7,500 lines (refuse.rs 1,860 / 24 variants; derive.rs 1,129).
  dpaa2-config parse.rs 1,481 vs schema.rs 328. dpaa2-mc 5 files,
  dpaa2-tools 6, dpaa2-config 3. models/intent ~20 .qnt + 20 traces.
  ADRs touched: 0002, 0005, 0012, 0013, 0014, 0015.
- mc/tools/config are almost certainly correctly flat — reviewers are told so
  up front; any grouping proposal there must justify itself, not be
  manufactured.
- Out of scope in any version: re-reviewing pre-existing dpaa2-verify board
  machinery (snapshot.rs, safety.rs, ioctlpolicy.rs, mcstatus.rs) beyond a
  diff-touched check — it predates the epic.

## Passes

### Pass 1 — Staleness map
Agent: Explore (very thorough). Budget ~60k. Runs first; feeds everything.
Build the retired-vocabulary list straight from the amendment tasks in
`openspec/changes/intent-layer/tasks.md` (2.3a, 2.5a, 2.6a–e incl. 2.6b's
`[[limit]]`, 3.3b–d table shapes, 6.6's fixed tag) and sweep the whole tree.
Classify every hit: (a) intra-change stale — written by an early phase,
superseded by a later one; (b) pre-change vintage the change should have
updated (ADR-0012's title is a candidate); (c) legitimately historical (ADR
prose recording a decision's history). Also flag every `ponytail:` marker
whose ceiling was since retired (2.6e retired the max-fold one; markers remain
in models/intent/observed.qnt, crates/dpaa2-api/src/model.rs,
crates/dpaa2-verify/src/fitcheck.rs — verify each is current).

### Pass 2 — Model layer
Agent: software-architect. Budget ~150k. After Pass 1.
Scope: models/intent/*.qnt, scenarios/, traces/, seam into
models/core/companions.qnt and models/main.qnt. The model is the source of
truth for phases 3–6, so it reviews before the Rust. Look for: defs left over
from pre-2.6 vocabulary; the 3.3e raw layer (intent_raw.qnt, raw_alphabet.qnt,
raw_replay.qnt) vs the phase-6 edit layer (observed.qnt, edits.qnt, match.qnt)
— two generations of "surface + replay" machinery: duplication, and whether
both replay paths are still load-bearing; trace files nothing replays;
invariant list vs ADR-0013 §6 (I1–I10); Quint defs that grew clause-by-clause
across the 2.6a–e amendments. PLUS: catalog the shapes existing in triplicate
(Quint def / Rust type / verify lint) so Pass 7 can judge which triplication
is ADR-0014-deliberate and which accidental.

### Pass 3 — dpaa2-api
Agent: software-architect. Budget ~220k. After Pass 2. Largest pass.
Scope: all 19 files, anchored on types.rs, intent.rs, derive.rs, compiled.rs,
refuse.rs, plan.rs, matcher.rs, reconcile.rs, model.rs, fake.rs. Look for:
model/Rust drift beyond what R11–R13 lints cover (lints check enumerations
bijectively, not semantics); the 3.1-era DesiredTopology/retro-adapter shapes
vs the 6.5 disruption-classed Plan — stale parallel path?; from_parts (3.1b)
coherence claim vs current facets; typestate claims ("does not compile")
actually enforced — spot-verify; dead constructors/tests from superseded
phases; both-old-and-new logic left by 3.3a/3.3d reworks; String vs newtype at
every seam (known regression pattern). PLUS: (a) module-structure verdict for
19 flat files at 7.5k lines — would the compile pipeline
(intent→derive→compiled→refuse→plan) and the phase-6 layer (matcher, model,
port) read better grouped, or does flat still communicate; (b) intra-crate
duplication with file evidence — refuse.rs and derive.rs are the named
suspects for per-variant/per-family boilerplate collapsible via macro or
table-driven helper, judged against the repo tenet AND readability at 3am;
(c) the runtime-check inventory (schema below) — every validate/check/
refusal-producing branch, noting whether a witness-constructor type already
exists (3.1 pattern: constructors take the deriving construct as witness).

### Pass 4 — dpaa2-verify
Agent: software-architect. Budget ~170k. After Pass 2; parallel with 3 and 5.
Scope: 17 files — itf.rs, raw_itf.rs, intent_itf.rs, edits_itf.rs, replay.rs,
ledger.rs, fitcheck.rs, adapter.rs; board files only diff-touched-checked.
Four sibling ITF modules accreted across 3.2/3.3e/6.7 is a formal duplication
finding: would one replayer with per-alphabet adapters fold them? Do the
rejected quint-connect revisit triggers recorded in intent_itf.rs still
describe reality? Verify R11–R13 lints and negative drift tests still name
what ADR-0013 names post-6.x. Check adapter.rs's retro-model leg for staleness
against the reshaped api. PLUS: strongest structure candidate in the repo —
17 flat modules, 14.4k lines, two separable concerns (board verification vs
intent lints/replay): deliver an explicit grouped-layout proposal (board/ vs
intent/ submodules, or a crate split) WITH churn cost, or an explicit
keep-flat verdict. Emit its (small) runtime-check inventory — verify checks
are its product, not typestate candidates.

### Pass 5 — Adapters and shell (config/mc/tools)
Agent: software-architect. Budget ~140k. After Pass 1; parallel with 3 and 4.
Scope: 14 files. Look for: sans-io seam leaks (IO types or restool spellings
reaching dpaa2-api types; compile/reconcile logic leaking into tools'
engine.rs); parse.rs/schema.rs residue from the three table-shape migrations
(3.3b/c/d) — hand-rolled validation the keyed-table serde now makes
unreachable (3.3c deleted some; audit for stragglers, incl. the remaining
`[[tenant]]`-family hit in schema.rs); 6.6's ownership rework (declared_names
\+ raw labels) leaving fixed-tag paths in dpaa2-mc; render.rs/status.rs
printing pre-2.6a vocabulary; stale insta snapshots for retired output shapes.
PLUS: (a) structure verdict expected "keep flat" for all three — justify any
deviation; (b) is parse.rs's 1,481 lines migration residue or irreducible
serde boundary; (c) runtime-check inventory at the trust boundary — with the
caution that parse-boundary validation is SUPPOSED to be runtime checks and
must not be proposed for typestate removal; interesting rows are checks
duplicated on both sides of the config→api seam.

### Pass 6 — Spec/docs alignment
Agent: spec-align. Budget ~120k. After Passes 2–5 (needs code-truth to say
which side of a divergence is wrong).
Scope: openspec change deltas vs shipped code; ADR-0002/0005/0012/0013/0014/
0015; README example vs shipped parser (3.3 claimed a re-check, 3.3d changed
shapes after); ROADMAP row 3; models/COVERAGE.md rows vs actual lint coverage;
CHANGELOG. Disposition Pass 1's category-(b) hits: ADR-0012 amendment vs
pointer, per the capture-ambivalence rule (open-ended findings → ADR/RFC with
revisit triggers, never forced pass/fail).

### Pass 7 — Cross-crate duplication + typestate roadmap
Agent: software-architect. Budget ~180k. After 3, 4, 5 (consumes their
inventories); parallel with 6.
(a) Duplication across crates: parser/model/verify twins (config schema.rs
types ↔ api types ↔ verify serde copies in adapter.rs), per-object/per-family
boilerplate repeated in mc's parse.rs/restool.rs and api's family.rs. For
each: collapse via trait/generic/macro per the repo tenet, or keep — hard
constraint: ADR-0014 linted copies and design D10 (api stays serde-free) make
some duplication deliberate and protected; a finding proposing to fold a
linted copy is a false positive by definition.
(b) Typestate gap: from the merged runtime-check inventories, a ranked list of
invariants runtime-checked in api that could move to types (session-typed
connect ends, availability states, disruption classes as phantom params,
builder-with-witness extensions), each scored by rewrite blast radius, and an
incremental path: which 2–3 moves are cheap and high-value now, which want
their own OpenSpec change, which are not worth it because model+lint already
carries the guarantee. Explicitly NOT: typestating the parse boundary or the
MC adapter's observed world (external input can't be made unrepresentable).

## Ordering

1 → 2 → {3, 4, 5 concurrent} → {6, 7 concurrent} → synthesis.
Model-before-Rust matters: if Pass 2 finds the model stale, Pass 3 judges the
Rust against the corrected reading, not the file.

## Report schema (every pass; include verbatim in each pass prompt)

One finding per row, no essays:
**ID** (PASSn-Fm) · **file:line** (absolute) · **category** (stale / dead /
duplicate / dup-cross / seam-leak / copy-drift / doc-drift / simplify /
structure / typestate-gap) · **superseded-by** (task number or commit that
made it stale — MANDATORY for stale claims; a staleness claim without a
superseding event is dropped) · **severity** (breaks-a-claim /
misleads-a-reader / carries-cost) · **disposition** (delete / amend / fold /
new ADR note / follow-up bead) · **verification** (exact grep/command/test
confirming the fix) · **guideline** (optional, Passes 3/4/5/7 only: one-line
imperative rule draft + anchor — a known regression, an ADR-carried
constraint, or ≥2 findings from this review showing the same mistake; no
anchor, no candidate; ≤5 nominations per pass).
Duplication findings cite BOTH sites (file:line pairs) and state the fold
mechanism (helper/generic/macro/leave-it) plus readability cost in one line.
Fixed footer: what was read, what was deliberately not read, open questions.

Additional mandatory tables from Passes 3–5:
- **Runtime-check inventory**: invariant name · where checked (file:line) ·
  currently type-enforced? (yes / witness-exists / no) · typestate candidate?
  (yes / no / boundary-exempt) · blast radius (local / crate / cross-crate).
- **Structure verdict** (one per crate): keep-flat / group / split · proposed
  layout if not flat · files-moved count · what the grouping communicates
  that flat doesn't (a proposal without this column is churn — rejected).

## Synthesis mandate (change-judge, ~115k)

Deduplicate and rank findings across passes (Passes 1 and 3 will both find
api-side vocabulary hits); drop unanchored staleness claims; deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. **Guidelines section**: merge guideline candidates, delete any rule that
   clippy/fmt/ledger lints already gate mechanically (the lint IS the rule),
   and deliver (a) a paste-ready "Repo rules" section for
   `.claude/agents/rust-developer.md` — HARD BUDGET 12 rules, one line each,
   each ending in its anchor (ADR number, memory slug, or finding ID);
   (b) a retire list — which CLAUDE.md lines, agent-file lines, and
   per-parcel boilerplate sentences each rule supersedes, so net guidance
   line count goes DOWN; (c) rejected candidates with one-line reasons.
   If the merged set exceeds 12, that signals over-acceptance, not a docs
   file. No separate docs file: the agent file is the only artifact
   guaranteed to load in a dispatched subagent's context.
3. Proposed follow-up work as bead-shaped items (title + why + acceptance),
   for a just-in-time openspec change — the review itself changes nothing.

## Budgets

P1 60k · P2 150k · P3 220k · P4 170k · P5 140k · P6 120k · P7 180k ·
synthesis 115k ≈ 1.18M total across 7 pass agents + judge. A pass exceeding
~1.5x its budget stops and reports partial.
