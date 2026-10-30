# Review brief: plan-rollback epic

Slug: `plan-rollback` (bead epic dpaa2-controlplane-vbg). Reviewed span:
commits dc1fa67..a4a3741 on main — dc1fa67 is the spec-init, then 25
execution commits all carrying a `Change: plan-rollback` trailer; the
span contains NO interleaved commits from other changes and NO `fixup!`
commits (the amendment trail was autosquashed on 2026-10-27; the
NEEDED=G-D staging fix and the fsl_dpaa2_eth detector fix live inside
the sealed rev-4 commit bafe8d1 — a finding treating a squashed
amendment as undocumented churn is a false positive). Epic beads
vbg.1–.12 closed, tasks 27/27 ticked (25 original + group 12's two
sitting-finding pull-ins, beads dpaa2-controlplane-f59 and
dpaa2-controlplane-b4c, both closed; bead dpaa2-controlplane-3qv
settled by ADR-0024; bead dpaa2-controlplane-275 retired by it). Net
epic size (trailer commits only): crates 33 files +3,669/−191; models
(incl. board records and re-frozen traces) 21 files +1,327/−147;
openspec 10 files +1,207/−54; docs 12 files +202/−41 — ~1.7x
resource-aware-orchestration, same four-pass shape, slightly larger
budgets. Mechanical lint findings are NOT the target. Targets: the
change's core promise (a mid-run failure under the default policy
unwinds the executed prefix in reverse from a run journal of observed
values, through a compile-checked total reversal table, leaving only
typed reboot-named residue; overcommit is named distinct from the
planning-time headroom refusal and re-apply refuses while the drain
holds; the two-ledger capacity face closes the census-blind
hardware-side drain; no state persists between runs), the model↔Rust
isomorphism of the rollback action and global ledger (ADR-0002 law),
sans-io discipline across the recording/rollback/render growth, the
busy-portal and live-free-ceiling hardening (f59/b4c), and doc/spec
alignment including the ADR-0024 deferral trail.

## Grounding facts (verified 2026-10-27, re-verify only if main moved)

- Commit order (task-ordered, subjects abbreviated): dc1fa67 (spec-init),
  611d79f (overcommit vocabulary refactor), d54efbd (1.x model: rollback
  action, run-journal projection, ORCH-I3 net identity up to dpio-seat
  residue, re-frozen traces incl. overcommit-rollback), 23d27a6 (2.x api:
  ActuationRecord journal, `#[non_exhaustive]` FailurePolicy
  Rollback-default/Abort, compile-checked total reversal table), 5873746
  (3.x Recording wrappers over McControl/KernelControl inside retry),
  145591c (4.x reverse-order rollback executor: irreversible rows →
  residue, failed undo → unreversed, no in-run retry), d93b34c (5.x the
  shell's one mid-run failure seam), faee318 (6.x fake-MC overcommit
  simulation, both dispositions offline, twin replay re-derives verdicts),
  8355f0b (CLI `--on-failure`, dry-run policy preview, operator-terms
  report), 09804e1 (7.1 V-ROLLBACK-1 authored + vrollback1_intents pins),
  5c1c8d2 + 63c3709 (docs/ADR-0023 amended: census-scoped bound, two
  ledgers recorded), 3211ea6 (spec deltas grown in place), 90cbc87 (8.x
  model: global ledger closes the census-blind drain), 9ebe1a2 (9.x
  mc.global listing capture pinned), d6a7d1a + 134fed9 (9.x capacity face
  carries GlobalBudget rows; admission judges creates against the global
  row), 0c8f763 (10.1 admission table renders `mc-global {family}` row),
  3e41804 (10.2 fake global counter beside the census), a555f6c (10.3
  replay re-derives the verdict from both ledgers, nine re-frozen traces),
  bafe8d1 (11.1 V-ROLLBACK-1 rev 4: runtime-derived staging gate, capped
  writer drain, global-row grep), 5be85f6 + 7d75bb5 (11.2 V-DPCON-2
  staged; sitting verdict banked), 09259cb (12.1 f59: PortalBusy bounded
  retry + honest render voice), c02aeb9 (ADR-0024), a4a3741 (12.2 b4c:
  Counted=live-free split + capacity-census spec delta).
- **Board outcomes are recorded facts** — findings contradicting them are
  false positives: **V-DPCON-2 rev 1 passed** (1 PASS / 64 creates / 1
  VERDICT): every `dpcon create --num-priorities=2` drew exactly one
  `swpch.2wq` unit (112→48), no `cqch.*` row moved, symmetric credit-back
  on destroy; the mid-sitting `mcp` 203→202 delta is the known
  ADR-0011/DPMCP-I6 destroyed-portal leak class; post-reboot snapshot 0
  deltas. **V-ROLLBACK-1**: rev 3 proved the machinery (gate, writer,
  convergence under drain, teardown, residents tolerance) with the
  witness not landed; rev4b exposed bead b4c (the engine silently planned
  create=0 on the aged staging); rev4e exposed the writer's
  driver-basename detector defect (board driver `fsl_dpaa2_eth`, fixed
  and squashed into bafe8d1); rev4g hit the single-opener law — `ensure`
  refused at its first verb with the open()-EINVAL RestoolGuard, its
  "5 PASS" partly vacuous. The witness is recorded NOT reachable on the
  restool transport (ADR-0024); the machinery verdict stands; the re-sit
  is retired to the ADR's revisit triggers.
- **Deliberate decisions findings must not "fix"** (each recorded in the
  design, ADR-0023/0024, or the bead trail):
  1. ADR-0024 deferral: a finding demanding the live mid-run overcommit
     witness, a shape-(b) re-stage, or deletion of V-ROLLBACK-1's
     machinery contradicts the ADR; its revisit triggers (ioctl
     MC-portal transport in dpaa2-hal, kernel single-opener relaxation,
     a decisive shape-(b) ordering) are the recorded carriers.
  2. b4c fix shape: the Counted=live-free semantics split is the
     accepted fix; the alternative (splitting live free out of Ceiling
     into a GlobalBudget-only channel, Ceiling static-only) is recorded
     in bead b4c and deliberately NOT done — a finding demanding it
     contradicts the user's scope decision. The Quint model remains a
     static-bracket world where `poolPopulation < CEILING ≡ free > 0`;
     the aged-board divergence is a real-board phenomenon the model does
     not represent. Whether that limitation is recorded somewhere a
     reader would look is a LEGITIMATE Pass 2/4 question; re-modeling it
     is not.
  3. f59 policy: retry fires ONLY on the `open() failed` discriminant;
     `Error::McStatus` and the DPRC-I3 plugged-move RestoolGuard are
     never retried — a finding proposing blanket RestoolGuard retry
     contradicts DPRC-I3. The 5×20ms constants are the named calibration
     knob (`ponytail:` marker on PORTAL_OPEN_BACKOFF). Policy lives in
     dpaa2-mc, never dpaa2-hal (ADR-0018).
  4. Rollback semantics per ADR-0023 (amended): rollback is the default
     and abort the opt-out; residue is grow-only typed reboot-named
     (ADR-0020, ADR-0003 §7 dpio seats); a failed undo continues as
     unreversed with no in-run retry; admission holds no reservation
     (D7) and the one-rollback-per-drain bound for hardware-side drains
     stays live-unwitnessed (ADR-0024 Consequences).
  5. dpcon global budget rows: only `swpch.2wq` at draw 1 per dpcon for
     the fixed `--num-priorities=2` operand is proven; `swpch.8wq` and
     `cqch.*` stay out until their own proving observation (the
     monotonic-growth rule, ADR-0023 Consequences) — a finding demanding
     those rows contradicts V-DPCON-2's recorded bounds.
  6. `FdbBudget::free`'s Counted arm is unreachable today (Counted is
     minted only for Dpbp) and left as defensive — recorded at the b4c
     seal; a finding may propose removing it only as a `simplify` with
     the reachability argument restated.
  7. All protected decisions of prior reviews (resource-aware-
     orchestration, orchestration-hardening, dpsw lineage, and earlier)
     bind this review exactly as before.
- **Pre-logged carries** — the passes CONFIRM, locate precisely, and
  disposition these rather than re-discover them: (1) `openspec validate`
  WARNING: the capacity-census ADDED requirement exceeds 500 chars —
  cosmetic vs split, judge; (2) the model's static-bracket blindness to
  the aged board (grounding item 2 above) — is it recorded
  reader-visible (COVERAGE? ADR-0024?); (3) the dead-defensive
  `FdbBudget::free` Counted arm (plan/capacity.rs); (4) V-ROLLBACK-1's
  suite header and README ledger row now carry TWO bracketed notes each
  (f59 supersession + ADR-0024 settlement) — do the layered annotations
  still read as one story; (5) `render_failure_disposition` now has
  three voices (overcommit / portal-busy / generic refused) — is the
  three-way split exhaustively tested and does any other surface still
  stamp "refused" on a PortalBusy.
- Gates recorded green at the seals (Pass 1 re-runs the offline ones):
  workspace tests (61 suites, incl. dpaa2-api 396, dpaa2-mc 139+26,
  pool_replay 9/9 byte-identical post-b4c, admission_oracle 7/7,
  orchestration replay over nine re-frozen traces, vrollback1_intents
  5/5, ledger_lint 2/2), quality floor, `sh -n` + shellcheck on the
  board suites, `openspec validate plan-rollback` valid with the one
  recorded warning.
- Out of scope: everything prior epic reviews already judged that this
  change did not touch; re-running board suites; the GlobalBudget
  channel split (bead b4c records it); the ioctl portal transport
  (roadmap `mc-portal-backend`); results/ capture archival (operator's
  call); VPP integration; the `.claude/agents/` and `.beads.gate.lock`
  untracked entries (session tooling, not the change).

## Passes

### Pass 1 — Residue, deferrals, and verify re-run
Agent: Explore (medium). Budget ~40k. Runs first; feeds the other three.
Re-run every task-level verify obligation checkable offline:
`cargo test --workspace` headline counts (name any suite not green),
the Quint gate (`pnpm model:typecheck`; `model:test` if runtime
allows), the COVERAGE ledger lint, `sh -n` on
`models/board/V-ROLLBACK-1/V-ROLLBACK-1.sh` and
`models/board/V-DPCON-2/V-DPCON-2.sh`, and
`scripts/checks/quality-floor.sh`. Verify the deferral promises point
at their carriers: the mid-run witness → ADR-0024 revisit triggers;
the GlobalBudget split → bead b4c text; dpcon's unproven rows →
V-DPCON-2 ledger bounds; the retired re-sit → bead 275 + the
annotations. Sweep the epic-touched files for leftover scaffolding:
`todo!`, `unimplemented!`, `dbg!`, `#[allow(dead_code)]`, `#[ignore]`,
commented-out code, and stale wording predating the late answers — a
"pending sitting"/"operator-gated rev 4" claim predating ADR-0024, a
population-bracket ceiling claim predating b4c, a "refused" claim on
the portal-busy path predating f59. Locate the five pre-logged carries
(grounding) precisely (file:line) and classify each. Confirm the epic
added no unmarked `ponytail:`-class simplification (the
PORTAL_OPEN_BACKOFF marker is the recorded one).

### Pass 2 — Isomorphism and the rollback/ledger law
Agent: software-architect. Budget ~90k. After Pass 1; parallel with 3, 4.
Scope: `models/orchestration/` (the rollback action, run-journal
projection, global ledger, re-frozen traces), the ADR-0023 amendment,
`crates/dpaa2-api/src/plan/{reversal,admission,capacity}*` and the
rollback/journal modules wherever they live (locate via the commit
subjects), `crates/dpaa2-api/src/families/pool_lifecycle.rs` (the b4c
split), `crates/dpaa2-verify/src/intent/*` and
`tests/{orchestration_replay,admission_oracle,pool_replay}.rs`. Four
mandates:
(a) **ORCH-I3 as shipped**: the model's rolled-back run nets to
identity up to the typed dpio-seat residue — map each model journal
entry kind onto the Rust reversal table rows (total? compile-checked?
any family reversible in the model but residue in Rust or vice versa);
the no-oscillation claim's witness.
(b) **The two-ledger law**: the model's global counter beside the
census vs `CapacityFace::globals` + `GlobalBudget` — the mid-run guard
refusing on the global row when the census shows headroom, on both
sides; the b4c split's model mapping (static CEILING ≡ Observed; the
live-free Counted has no model twin — confirm where that is recorded).
(c) **Rule 6 self-application**: the converged-intent fixpoint tests
and the `check_pair` verdict re-derivation actually re-derive (not
structure-trust) over the NINE re-frozen traces, overcommit step
included; name every model action/invariant with no replayed trace.
(d) **Replay honesty**: the re-frozen trace set — what changed in the
re-freeze, is each re-freeze justified by a model change (never by a
harness patch); the pool_replay Observed re-typing preserved
byte-identical replays.

### Pass 3 — Architecture, sans-io discipline, and code quality
Agent: software-architect. Budget ~90k. After Pass 1; parallel with 2, 4.
Scope: `crates/dpaa2-api/src/core/{error,engine}*`, the Recording
wrappers and rollback executor, `crates/dpaa2-mc/src/restool.rs` (f59
retry + the mc.global read), `crates/dpaa2-tools/src/{engine,render,
main}.rs` deltas, `tests/{vrollback1_intents,rollback_cli}.rs`,
`models/board/V-ROLLBACK-1/V-ROLLBACK-1.sh` and
`models/board/V-DPCON-2/V-DPCON-2.sh`. Four mandates:
(a) **Sans-io discipline**: judgment (reversal totality, disposition,
admission, the Counted/Observed split) lives in dpaa2-api; policy
(retry bounds, error mapping) in dpaa2-mc; the shell composes at ONE
mid-run failure seam — no second decision layer, no policy leaked into
dpaa2-hal.
(b) **The journal records observed truth**: Recording wrappers sit
inside retry so the journal carries the actuation that finally
succeeded; a PortalBusy exhaustion mid-run reaches the failure seam
without fabricating an MC verdict; the three render voices (overcommit
/ portal-busy / generic) are mutually exclusive and each pinned by a
test.
(c) **Suite quality**: V-ROLLBACK-1 rev 4 and V-DPCON-2 against the
board-suite protocol (ADR-0003: repo-root cd, total-deny self-check,
runtime-derived counts, EXIT-trap teardown, exit-2 precondition skip as
the offline witness, PASS/FAIL face log); the offline pins
(vrollback1_intents) pin the operands and grep shapes the scripts
actually run, including the post-f59/ADR-0024 annotations.
(d) **Reuse and the established voices**: the rollback report, the
admission table's global row, and the PortalBusy give-up reuse the
established render seams and refusal idioms; nothing reimplements a
helper that exists; name any simplification that cut an unmarked
corner.

### Pass 4 — Spec/docs alignment
Agent: spec-align. Budget ~80k. After Pass 1; parallel with 2, 3.
Scope: `openspec/changes/plan-rollback/{proposal,design,tasks}.md`, the
FIVE spec deltas under `specs/` (rollback, orchestration-model,
mbt-harness, provisioning-cli, capacity-census — the last added at the
12.2 pull-in) vs shipped code; `docs/adr/0023-*.md` (the census-scoped
amendment) and `docs/adr/0024-*.md` vs the shipped deferral;
`models/board/README.md` V-ROLLBACK-1 + V-DPCON-2 ledger rows and
VERDICTS entries vs tasks.md groups 7/11/12 and the suite scripts —
the records tell ONE story (authored → rev 3 machinery → rev 4
staging → ADR-0024 retirement; V-DPCON-2 staged → passed → verdict
bounds); `docs/ROADMAP.md` row 15's tail; the group-12 task rows vs
the sealed commits. Verify the capability claims in proposal.md are
each true as shipped (the reversal table IS total and compile-checked;
dry-run DOES preview the failure policy; the overcommit refusal voice
IS distinct; re-apply refuses while the drain holds — offline
evidence). Verify no doc still claims the sitting is pending, claims
the witness will land on restool, stamps "refused" on the portal-busy
voice, or speaks the population-bracket ceiling for dpbp. Disposition
pre-logged carries (1)(4) and Pass 1's doc-side hits.

## Ordering

1 → {2, 3, 4 concurrent} → synthesis. Pass 1's classification feeds
all three: Pass 2/3 judge code hits, Pass 4 dispositions doc hits.

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

## Synthesis mandate (change-judge, ~60k)

Deduplicate and rank findings across passes (Passes 1 and 4 will both
find doc-side hits; 2 and 3 both touch the journal/admission seam);
drop unanchored staleness claims and any finding that "fixes" a
protected decision (the seven grounding items, the board outcomes, the
amendment trail, and everything prior reviews protected); deliver:
1. The merged findings ledger, most severe first, with dispositions —
   the five pre-logged carries MUST each appear with a final
   disposition (confirmed-fix-worthy → bead shape, or
   recorded-acceptable → where recorded).
2. A verdict on the change's core promise: does a mid-run failure under
   the default policy unwind the executed prefix in reverse with typed
   residue and an honest operator report; is overcommit named distinct
   from the headroom refusal and does re-apply refuse while the drain
   holds; does the two-ledger face close the census-blind drain; does
   the engine survive a concurrent operator (f59) and an aged board
   (b4c) — yes or no, evidence rows.
3. A verdict on ADR-0002 compliance: is the shipped
   journal/reversal/admission surface structurally isomorphic to the
   orchestration model's rollback action and global ledger, and is rule
   6 (fixpoint + verdict re-derivation) genuinely self-applying over
   the re-frozen traces — yes or no, evidence rows.
4. Proposed follow-up work as bead-shaped items (title + why +
   acceptance) for a just-in-time openspec change — the review itself
   changes nothing. Candidates to judge, not rubber-stamp: the
   model's static-bracket blindness to the aged board; the dead
   FdbBudget Counted arm; the >500-char spec requirement; the layered
   annotation readability on V-ROLLBACK-1's records.
No guidelines deliverable. At most nominate a rule amendment if a
finding shows an existing rule failed to prevent a defect (candidates:
b4c shipped through sealed groups with green gates and was caught only
on the board — does any offline gate exercise an aged-board census
shape; rev4e's detector defect burned two sittings before the
common-trigger rule fired — is there a pre-sitting lint for
board-vs-fixture name drift?).

## Budgets

P1 40k · P2 90k · P3 90k · P4 80k · synthesis 60k ≈ 360k total across
4 pass agents + judge. A pass exceeding ~1.5x its budget stops and
reports partial.
