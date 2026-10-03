# Review brief: dpmac-hardening epic

Slug: `dpmac-hardening`. Reviewed span: commits fb831f2..26c0127 on main
(9 execution commits, every one carrying a `Change: dpmac-hardening`
trailer). Epic dpaa2-controlplane-e6s, closed 2026-10-03 at the 26c0127
close-out. Net epic size: ~540 inserted / ~222 deleted lines across 24
files — roughly 15% of dpmac-typestate, so a three-pass shape with
proportionally smaller budgets. Quality floor (cargo build/fmt/clippy/
clippy --tests/doc) is green per the close-out; mechanical lint findings
are NOT the target. This change lands the eight merged findings of the
dpmac-typestate epic review (synthesis at
`openspec/changes/archive/2026-10-03-dpmac-typestate/review/synthesis.md`
§4, MERGED-1..8) as dispositioned there and in this change's design
D1–D5. Targets: each finding landed per its recorded disposition; the
severed-witness binding is real (not a token rename); the zero-sentinel
judgment is one predicate on both paths; the folds (counter names,
scanner, LinkType seam) changed structure without changing behavior; the
doc/ledger amendments are pointers and errata, never rewrites of sealed
prose.

## Grounding facts (verified 2026-10-03, re-verify only if main moved)

- Commit order: f23ad70 (B7 doc/ledger amendment, bead e6s.7), 134b303
  (B1 SeveredProof binds its edge, e6s.1), 5bcf7ee (B2 verbatim counter
  names, e6s.2), 34b11a0 (B3 port-detail inference hoists, e6s.3),
  deec9b9 (B4 zero-MAC bind transient, e6s.4), 2910619 (B5 one
  dpmac-info scanner, e6s.5), 2b30a13 (B6 LinkType From seam, e6s.6),
  047f1f1 (refusal register catches V-DPMAC-2, bead 8rs — a
  pre-existing ledger gap found en route: dpmac-typestate 5.3 recorded
  the verdict without the mc-status.md register row; fixing it here was
  deliberate, a finding calling it scope creep is a false positive),
  26c0127 (close-out).
- Commits 134b303 and 34b11a0 carry the `One-Writer-Exempt` trailer —
  the ADR-0016 §4 recorded escape for atomic cross-crate API ripples
  (b6782ed). Commit hygiene context only, not a defect hunt.
- Touched surface: `crates/dpaa2-api` plan/transition.rs (SeveredProof
  stores DpniId privately, no Clone/Copy, manual `impl Clone for
  Transition` re-mints in-crate, two pinned compile_fail doctests E0423
  + E0382), plan/reconcile.rs (unbind(proof) call sites; both MAC arms
  gate on `mac_read_back_observed`), families/dpmac.rs (Vocabulary
  carries `Vec<(String, CounterRead)>`, `DPMAC_1039_COUNTER_ROWS`
  const, three pure port-detail judgments, the LinkType From seam with
  the collapse trigger as doc comments), contract/fake.rs;
  `crates/dpaa2-mc` restool.rs (names ride the readout; observe reads
  the token table) and parse.rs (one raw scanner, offer/info
  projections, `DPMAC_LINK_TYPES` token table per the OPTION_BITS
  precedent); `crates/dpaa2-tools` status.rs (consumes family
  judgments) and render.rs (rows render under carried names; positional
  pairing deleted) + tests/port_detail.rs; `crates/dpaa2-verify`
  board/replay.rs (named severed_proof constructor replaces
  mint-and-discard) + tests/dpmac_replay.rs (call-site fix);
  `models/` COVERAGE.md (DPNI-I3 erratum + :106 slice/adapter wording),
  core/invariants.qnt comment, families/dpmac.qnt severAt-narrowing
  comment, board/README.md forward pointers; `docs/` baseline/dpmac.md
  #7→#10 re-routes, baseline/mc-status.md register row, adr/0019 Status
  line; the archived dpmac-typestate design.md D2 phrase;
  `openspec/specs/formal-models/spec.md` slice wording.
- **Deliberate decisions findings must not "fix"** (each recorded in
  this change's design or the parcel reports):
  1. D1: the proof binds (stores `DpniId`, loses Clone/Copy); the
     manual `impl Clone for Transition` re-minting the witness in-crate
     is the mechanical consequence — the planner is the proof's
     authority; a finding demanding Clone back on SeveredProof or a
     clone-proof Transition contradicts D1.
  2. D2: counter names travel as data on the readout; the fake uses
     placeholder `counter-{i}` names because the verbatim table is
     adapter-only (ADR-0018 dependency direction) and no third
     vocabulary table may be born; counters never reconcile.
  3. D3: one zero-sentinel predicate (`mac_read_back_observed`), both
     paths; B4's Actuate posture is skip-this-run (stateless
     level-triggered re-judge), justified from dpni.qnt ZERO_MAC; the
     reconciler still never consumes MacRelation (protected).
  4. D4 fork fired ELSE: no ADR/archived design records the LinkType
     triplication as deliberate, so one documented From seam landed
     with the family sum as authority; the three enum definitions
     REMAIN (acceptance allows "one definition OR one documented
     seam"); the inventory reverse is partial and stays data in the
     token table; collapse trigger recorded at the seam.
  5. B5: the eth_if maps stay split on purpose — disjoint token sets
     into different enums, nothing to single-source; parse_dpmac_info's
     exact-token match is stricter than the old `contains` only on
     impossible malformed tokens.
  6. B7: sealed board verdict prose (V-LINK-4 and every other verdict
     row) gained appended pointers only — byte-identical otherwise; a
     finding proposing to rewrite sealed prose is a false positive.
  7. All dpmac-typestate protected decisions (its D1–D7, board
     outcomes V-DPMAC-2/3, deferral routings to #10/#13) bind this
     review exactly as they bound the last one.
- The dpmac-typestate review synthesis dispositions are the acceptance
  oracle: MERGED-1→B1, MERGED-2→B2, MERGED-3/7/8→B7, MERGED-4→B4,
  MERGED-5→B3, MERGED-6a→B5, MERGED-6b→B6. No rule amendment was
  adopted (the synthesis nominated none).
- Frozen traces replay through `tests/dpmac_replay.rs`; board evidence
  under `models/board/` is operator-sealed; re-running board suites is
  out of scope.
- Out of scope: everything the dpmac-typestate review already judged
  that this change did not touch; portal/ioctl (#10) and every named
  restool-absence deferral; dpseci (#8), dpdmux (#12), tier-c (#13);
  the repo-wide anchored-refs `--tree` debt (carried bead,
  pre-existing, includes replay.rs:1 "task 3.2" and mc-status.md:66
  "task 5.9").

## Passes

### Pass 1 — Acceptance re-run and residue
Agent: Explore (medium). Budget ~40k. Runs first; feeds the other two.
Re-run every offline-checkable acceptance obligation from tasks.md and
the seven bead criteria: the MERGED-3/7/8 greps (no "(#7)" routing in
dpmac.md or the COVERAGE DPNI-I3 row; no "unknown #1" in
invariants.qnt; board README pointers present at :85/:605/:881 with
V-LINK-4 verdict prose otherwise untouched; ADR-0019 Status line), the
B1 greps (`_severed_edge` gone from replay.rs; "owned by the types"
only as a true claim; the two compile_fail doctests pinned E0423/
E0382), the B3 grep (`SameContainerKernelPeer|MacAddr::ZERO` out of
status.rs), the B5 grep (one `strip_prefix("DPMAC link type:")`), the
B6 grep (three enum defs + one documented seam), the ledger-lint test
green. Sweep the epic-touched files for leftover scaffolding (`todo!`,
`dbg!`, `#[allow(dead_code)]`, commented-out code) and for stale
wording predating the landed findings (a Copy-token claim, a
positional-pairing description, a three-scanner mention). Confirm no
new `ponytail:` marker appeared and no unmarked deliberate
simplification hides in the touched lines.

### Pass 2 — The witness, the sentinel, the folds
Agent: software-architect. Budget ~70k. After Pass 1; parallel with 3.
Scope: plan/{transition,reconcile}.rs, families/dpmac.rs,
contract/fake.rs, board/replay.rs + tests/dpmac_replay.rs,
mc/src/{parse,restool}.rs, tools/src/{status,render}.rs + tests.
Four mandates:
(a) **The binding is real**: no path forges, retargets, or double-uses
a SeveredProof — check the manual Clone impl (does a cloned Plan's
re-minted witness open a reuse the doctests miss?), the replay named
constructor (honest mint documenting the adjacent-window seam, not a
backdoor), pub surface (no extraction path from Unbind), and whether
the compile_fail pair actually covers forge + cross-edge/reuse.
(b) **One sentinel, one predicate**: `mac_read_back_observed` is the
only zero/absent judgment on both paths; no local zero test or ZERO
sentinel remains in status.rs, render.rs, or reconcile.rs; the
reconciler consumes the boolean only, never MacRelation.
(c) **Folds preserve behavior**: the single scanner + token table
projections reproduce the three old scanners' outputs for the fixture
corpus; the render path labels by carried name with no positional
remnant; the From seam conversions are total where claimed and the
partial reverse stays unreachable as a typed conversion.
(d) **Sans-io discipline held through the hardening**: the hoisted
judgments live in families/dpmac.rs as pure functions; nothing crept
back into the shell; the shim still only plumbs.

### Pass 3 — Spec/docs alignment
Agent: spec-align. Budget ~50k. After Pass 1; parallel with 2.
Scope: this change's proposal/design/tasks and three spec deltas
(mc-backend, provisioning-cli, reconciler) vs shipped code; the
dpmac-typestate synthesis §4 dispositions vs what landed (every
MERGED finding: disposition honored, severity addressed, no silent
narrowing); the B7 amendments vs the synthesis wording (including the
recorded deviation: the DPNI-I3 erratum drops the literal "(#7)" token
to satisfy its own grep); the 047f1f1 register row vs VERDICTS.json
and the board README V-DPMAC-2 row; `openspec/specs/formal-models/
spec.md` + COVERAGE:106 slice/adapter wording vs D4; ADR-0019's
amended Status trail (atemporal body preserved, dated trail only);
the archived design.md D2 softening vs the shipped observation-judged
enum. Verify the three modified-capability claims in proposal.md
(reconciler, mc-backend, provisioning-cli) are each true as shipped.
Disposition Pass 1's doc-side hits.

## Ordering

1 → {2, 3 concurrent} → synthesis. Pass 1's classification feeds both:
Pass 2 judges code hits, Pass 3 dispositions doc hits.

## Report schema (every pass; include verbatim in each pass prompt)

One finding per row, no essays:
**ID** (PASSn-Fm) · **file:line** (absolute) · **category** (stale / dead /
duplicate / iso-violation / guard-drift / twin-drift / leak — logic on the
wrong side of the sans-io seam / doc-drift / simplify) · **superseded-by**
(task number or commit that made it stale — MANDATORY for stale claims; a
staleness claim without a superseding event is dropped) · **severity**
(breaks-a-claim / misleads-a-reader / carries-cost) · **disposition**
(delete / amend / fold / new ADR note / follow-up bead) · **verification**
(exact grep/command/test confirming the fix).
Duplication findings cite BOTH sites (file:line pairs) and state the fold
mechanism plus readability cost in one line — remembering the protected
deliberate duplications named in grounding.
Fixed footer: what was read, what was deliberately not read, open questions.

## Synthesis mandate (change-judge, ~40k)

Deduplicate and rank findings across passes; drop unanchored staleness
claims and any finding that "fixes" a protected decision (the seven
grounding items above, plus everything dpmac-typestate's review
protected); deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. A verdict on the change's promise: did the eight merged review
   findings land per their recorded dispositions — the witness
   edge-bound and consume-once by type, the counter rows named not
   positional, the zero sentinel one predicate on both paths, the
   scanners and sums folded or seamed, the record amended without
   rewriting sealed prose — yes or no, evidence rows per MERGED item.
3. A verdict on protection: did the change leave every protected
   decision of dpmac-typestate (D1–D7, board outcomes, deferral
   routings) and its own design forks (D1–D5) untouched — yes or no.
4. Proposed follow-up work as bead-shaped items (title + why +
   acceptance) for a just-in-time openspec change — the review itself
   changes nothing.
No guidelines deliverable. At most nominate a rule amendment if a
finding shows an existing rule failed to prevent a defect (candidate:
the comment-density and one-writer gates both fired mid-epic on parcel
output — do the parcel-spec templates need the gate ceilings stated up
front?).

## Budgets

P1 40k · P2 70k · P3 50k · synthesis 40k ≈ 200k total across 3 pass
agents + judge. A pass exceeding ~1.5x its budget stops and reports
partial.
