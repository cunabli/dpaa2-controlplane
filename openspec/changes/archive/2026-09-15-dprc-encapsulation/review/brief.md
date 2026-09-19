# Review brief: dprc-encapsulation epic

Slug: `dprc-encapsulation`. Reviewed span: commits 0aac741..1051e71 on main
(1 spec-init + 21 execution commits; the range also interleaves 4 process
commits and 1 ledger-marker fix that are NOT part of this change — see
grounding). Epic cd3, closed 2026-09-14 at task 6.1 (bead cd3.14). Net epic
size excluding openspec/ artifacts and board evidence: ~7,800 inserted / ~60
deleted lines across 41 files in crates/, models/, docs/ — roughly 6x
vocabulary-v2, smaller than intent-layer, so four passes. Quality floor
(cargo build/fmt/clippy/clippy --tests/doc/test) is green per the 6.1 DoD;
mechanical lint findings are NOT the target. Targets: structural isomorphism
of the new lifecycle sum (the ADR-0002 law), plan-guard fidelity to the
board-settled containment laws, sans-io discipline (design D2 claims the
pure core carries the change), refusal-typing discrimination (D4), ITF
replay coverage, the D8 prune amendment trail, and doc/spec alignment.

## Grounding facts (verified 2026-09-14, re-verify only if main moved)

- Commit range: `0aac741` (spec-init) → `1051e71` (6.1 seal). Interleaved
  but OUT OF SCOPE: `c00f9f0`, `b1d0787`, `e4b1751`, `5a834fc` (ADR-0016
  development-process modeling: scripts/checks/, .githooks/, CONTRIBUTING.md,
  CLAUDE.md) and `872f509` (board ledger marker). Findings against those
  files or ADR-0016 are out of scope by definition.
- Major new surface (current sizes): `crates/dpaa2-api/src/dprc.rs` 1,402
  (lifecycle typestates, task 2.1), `crates/dpaa2-api/src/dprc_plan.rs`
  1,408 (plan guards + prune buckets, tasks 2.2/2.4),
  `models/families/dprc.qnt` +782 (now 812; tasks 1.1/1.2/1.4),
  `crates/dpaa2-mc/src/restool.rs` +597 (now 1,356; dprc verbs, task 3.1),
  `crates/dpaa2-verify/tests/dprc_replay.rs` 643 + `src/dprc_itf.rs` 176
  (ITF conformance, task 2.3), `crates/dpaa2-mc/src/kernel.rs` +199 (VFIO
  face, task 3.2), `crates/dpaa2-hal/src/sysfs.rs` +149,
  `crates/dpaa2-tools/src/engine.rs` +318 (convergence + prune dispatch,
  tasks 4.2/4.3), `crates/dpaa2-tools/tests/dprc_convergence.rs` 387 +
  3 snapshots. Docs: ADR-0017 (new), ADR-0007 §3 + ADR-0011 amended
(ADR-0001 §4 is cited by D8, not edited — Pass 1 corrected an 0001↔0011
transposition here),
  `docs/baseline/dprc.md` +50, `models/COVERAGE.md` +19, ROADMAP row #4,
  CHANGELOG.
- **Amendment trail (D8)**: the undeclared-consumer prune requirement fell
  through the task 2.2/4.2 seam and was closed mid-epic at 5.3 authoring
  (2026-09-13): design D8 added, spawning tasks 1.3 (spec delta, cd3.15),
  1.4 (DPRC-I12 `createdByUs` model, cd3.16), 2.4 (prune buckets in
  dpaa2-api, cd3.17), 4.3 (`--prune` dispatch behind `--allow disruptive`,
  cd3.18). A finding that the prune surface is "unplanned scope creep" is a
  false positive — it is the amended plan. The D8 enumeration deliberately
  records ONE stranding escape (set-label to empty string drops a managed
  container to report-only) and deliberately does NOT widen the fingerprint;
  proposing to widen it contradicts a recorded decision.
- Deliberate decisions that findings must not "fix": consumer→container
  derivation landed in `dpaa2-api` (`model.rs` +94, task 4.1 commit
  `983cfcb api:`) although the proposal's Impact section names
  `dpaa2-config` — whether that is deliberate placement or doc-drift is
  Pass 4's to disposition, not a code finding; the intent path emits no
  residents (D5 container-only); VFIO bind state rides the plugged face
  kernel-side, one sum, not two machines (D3); the override-propagation
  oracle lives in the `vfio.sh` suite hook, not the core machine — the core
  cannot trace a dprc bind (`o.fam != Dprc` guard) and this is the settled
  open question, recorded in ADR-0017; label-under-lock resolved repairable,
  encoded at `dprc.qnt` `setLabelAt`/`labelUnderLockTest`.
- Refusal statuses: the permission matrix distinguishes Configuration error
  0x6 / No resources 0x8 / No privilege 0x4, plus restool client-side
  guards, typed in `dpaa2-mc` and interpreted in `dpaa2-api` (D4). V-DPRC-2/6
  proved the statuses carry causal information; collapsing them anywhere on
  the path is a finding.
- `models/COVERAGE.md` tally now: 55 modeled / 47 deferred / 7 board-settled
  of 109 candidates; 12 DPRC-I rows. Promised deferral rows → tile #10:
  DPRC-I8 batch ordering, DPRC-I11 unlock face, OBJ_CREATE_ALLOWED gate.
- Exactly one `ponytail:` marker in epic-touched code:
  `crates/dpaa2-mc/src/restool.rs:451` (bare ResidentId collides across
  families) — its ceiling and disposition belong to Pass 3.
- Out of scope: everything intent-layer/vocabulary-v2 reviews already judged
  that this change did not touch (refuse.rs vocabulary, matcher, derive
  internals beyond the container hook); board evidence under `models/board/`
  (operator-sealed, per-task verdicts committed); re-running board suites;
  the ADR-0016 process system; portal/ioctl work (explicit non-goal).

## Passes

### Pass 1 — Residue, deferrals, and verify re-run
Agent: Explore (medium). Budget ~60k. Runs first; feeds the other three.
Re-run every task-level verify obligation from tasks.md that is checkable
offline (typecheck/test names cited, grep-able assertions; report any that
no longer hold). Verify the three promised deferral rows exist and point at
tile #10 in `models/COVERAGE.md` AND in `docs/baseline/dprc.md`. Sweep the
41 epic-touched files (not the out-of-scope process files) for leftover
scaffolding: `todo!`, `unimplemented!`, `dbg!`, `#[allow(dead_code)]`,
`#[ignore]`, commented-out code, stale `create-only` wording that predates
the D8 amendment, and orphaned fixtures. Verify the frozen traces
`dprc_replay.rs` replays are committed under `models/traces/families/` and
referenced, none orphaned. Sweep `ponytail:` markers in touched files.
Classify every hit: (a) stale — a named task should have updated it;
(b) deliberate (grounding lists the protected decisions); (c) historical
prose recording what was.

### Pass 2 — Isomorphism and the typestate law
Agent: software-architect. Budget ~150k. After Pass 1; parallel with 3, 4.
Scope: `models/families/dprc.qnt`, `crates/dpaa2-api/src/dprc.rs`,
`dprc_plan.rs`, `crates/dpaa2-verify/src/dprc_itf.rs`,
`tests/dprc_replay.rs`, `crates/dpaa2-api/src/port.rs`. Four mandates:
(a) **Lifecycle sum isomorphism (D3, ADR-0002 law)**: the Quint sum
(Declared → Created/unplugged → Populated → Plugged|Locked → Emptied →
Destroyed, VFIO bind state on the plugged face) vs the Rust typestates —
state-for-state, guard-for-guard, payload-for-payload; names converge on
the same readable English. Spot-check the "invalid transitions
unrepresentable" claim: which illegal transitions does the type system
actually reject at compile time vs merely refuse at runtime — a runtime
refusal advertised as a typestate is a finding.
(b) **Containment-law guard fidelity**: the permission matrix
(0x6/0x8/0x4), eviction law (ADR-0007 §3: created-resident dies,
assigned-in resident evicted unplugged to parent), visibility law (DPRC-I6:
no sync-implies-visibility, convergence by re-observation only), and
plugged-move precondition — model guard vs `dprc_plan.rs` plan guard, same
predicate, not merely similar.
(c) **ITF replay coverage**: map `dprc_replay.rs` traces onto the model's
guarded transitions; name every guard/refusal arm with no replayed trace.
(d) **DPRC-I12 bucket parity**: the model's `createdByUs` ghost bit and
bucket invariant (task 1.4) vs the Rust prune classifier's bucket
boundaries (converged / candidate full+partial / report-only, task 2.4) —
the empty-label stranding escape must land in report-only on BOTH sides.

### Pass 3 — Architecture and code quality
Agent: software-architect. Budget ~130k. After Pass 1; parallel with 2, 4.
Scope: `crates/dpaa2-mc/src/{restool,kernel,parse,runner}.rs`,
`crates/dpaa2-hal/src/sysfs.rs`, `crates/dpaa2-api/src/{port,fake,error,
model}.rs`, `crates/dpaa2-tools/src/{engine,render,main}.rs`, tests. Four
mandates:
(a) **Sans-io discipline (D2)**: `dpaa2-mc` verbs must be thin plumbing —
any lifecycle/containment decision living in the shim instead of the pure
core is a finding; `engine.rs` (+318) is imperative shell — planning logic
that leaked into it is a finding.
(b) **Refusal typing end-to-end (D4)**: the typed MC statuses and restool
client-guard refusals survive from `restool.rs` through `error.rs` to plan
drift/permission-gap reporting without string-matching or collapse.
(c) **Reuse-before-write**: restool.rs grew +597 — flag parsing/plumbing
the existing `parse.rs`/`runner.rs` seams already provided; the prune
classifier vs the existing matcher precedent; `fake.rs` seeding vs existing
fixture helpers. Judged against protected deliberate duplication (ADR-0014
lockstep, parse/compile twins).
(d) **Seam quality for #5–8**: `port.rs` grew +164 — do the McControl/
KernelControl trait additions pre-shape tiles #5–8 as D2 promises, or
encode dprc-only assumptions the next family must rewrite? Disposition the
`restool.rs:451` ponytail ceiling (bare ResidentId family collision): still
safe, or does the prune census make it reachable?

### Pass 4 — Spec/docs alignment
Agent: spec-align. Budget ~110k. After Pass 1; parallel with 2, 3.
Scope: `openspec/changes/dprc-encapsulation/{proposal,design,tasks}.md`,
the six spec deltas under `specs/` (formal-models, intent-compiler,
mbt-harness, mc-backend, reconciler, system-integration) vs shipped code;
ADR-0017 (new) and the ADR-0007 §3 / ADR-0011 amendments vs the code that
cites them (ADR-0001 §4 is citation-only grounding for the D8 report-only
fence — verify the citation is accurate, not that the file changed); `docs/baseline/dprc.md` amendments anchored to named board
verdicts (V-DPRC-8/9/12, V-DPDBG-2); `models/COVERAGE.md` dispositions for
DPRC-I1/I5/I7/I9/I10/I11 match the model's Apalache marks and the 5.4
online-MBT outcomes; ROADMAP row #4 status; CHANGELOG entry. Disposition
the proposal-Impact vs reality delta: derivation in `dpaa2-api`, not
`dpaa2-config` — deliberate (amend proposal prose / record decision) or
gap. Verify both design Open Questions marked settled actually have their
claimed encodings (`setLabelAt`/`labelUnderLockTest` in dprc.qnt +
COVERAGE.md DPRC-I11 row; ADR-0017 + baseline amendment for propagation).
Verify the D8 trail is complete: design amendment ↔ tasks 1.3/1.4/2.4/4.3 ↔
spec deltas ↔ shipped flags (`--prune`, `--allow disruptive`, report-only
fence). Disposition Pass 1's category-(a) doc hits.

## Ordering

1 → {2, 3, 4 concurrent} → synthesis. Pass 1's classification feeds all
three: Pass 2/3 judge code hits, Pass 4 dispositions doc hits.

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

## Synthesis mandate (change-judge, ~80k)

Deduplicate and rank findings across passes (Passes 1 and 4 will both find
doc-side hits; 2 and 3 may both touch dprc_plan.rs); drop unanchored
staleness claims and any finding that "fixes" a protected decision (the D8
non-widened fingerprint and recorded stranding escape, the dpaa2-api
derivation placement pending Pass 4's disposition, container-only
population, the one-sum D3 machine, the hook-carried propagation oracle,
ADR-0014 lockstep copies); deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. A verdict on the change's ADR-0002 compliance: is the shipped lifecycle
   sum structurally isomorphic to the Quint sum, yes or no, evidence rows.
3. A verdict on the D2 claim: does the pure core actually carry the change
   (plan logic off-board-replayable, shim thin), yes or no, evidence rows.
4. Proposed follow-up work as bead-shaped items (title + why + acceptance)
   for a just-in-time openspec change — the review itself changes nothing.
No guidelines deliverable: the 12-rule set stands; at most nominate a rule
amendment if a finding shows an existing rule failed to prevent a defect.

## Budgets

P1 60k · P2 150k · P3 130k · P4 110k · synthesis 80k ≈ 530k total across
4 pass agents + judge. A pass exceeding ~1.5x its budget stops and reports
partial.
