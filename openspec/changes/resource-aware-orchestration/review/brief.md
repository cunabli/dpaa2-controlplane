# Review brief: resource-aware-orchestration epic

Slug: `resource-aware-orchestration` (#15). Reviewed span: commits
abbc5b2..90d1f4a on main — abbc5b2 is the spec-init (proposal/design/
specs/tasks + ADR-0023), then 13 execution commits carrying a
`Change: resource-aware-orchestration` trailer. Interleaved commits in
the span belong to OTHER changes and are out of scope here: the
dpsw-typestate close-out/archive trail (6b3dea0, 99ba94f, 4c9f011,
f4965d3, ab14d8d, d3f1d6f, b4ab55e), the whole dpsw-hardening series
(dd375d0, 4cdcd21, c5c007d, f9f289a, f5a33db, 3d25737, 2abe0d3,
73b8b0a, 1369406), the trace-home move 5966bf8, and the process-rule
commit aeb27f9. The branch's amendment trail was autosquashed on
2026-10-25: NO `fixup!` commits remain, the amendments live inside the
sealed commits and the recorded trail below — a finding treating a
squashed amendment as "undocumented churn" is a false positive. Epic
dpaa2-controlplane-m7a, all beads m7a.1–.7 closed; 11/11 tasks ticked.
Net epic size (trailer commits only): crates 12 files +2,104/−30;
models (incl. board records and 6 frozen traces) 14 files +1,215/−76;
docs 2 files +247/−16 (ADR-0023 +245); openspec 8 files +708/−27 —
roughly 40% of dpsw-typestate, so the same four-pass shape with
proportionally smaller budgets. Mechanical lint findings are NOT the
target. Targets: the change's core promise (the ensure pipeline's own
origin story cannot recur — a compiled family without an executor is
non-compiling in Rust and invariant-red in the model, an over-budget
intent is refused typed before any verb; the guarded-action model with
ORCH-I1..I6, the compile-checked total dispatch with
Execute/Unimplemented rows, the capacity face + gross admission of
pending creates, the D9 census-read/live-read discipline, the CLI
admission rendering, the MBT twins + frozen-trace replay, the bounded
pairwise confluence check, and the live V-ADMIT-1 witness), structural
isomorphism of `models/orchestration/pipeline.qnt` vs the Rust
dispatch/admission/engine surface (ADR-0002 law, one level up),
sans-io discipline across the grown engine and render surfaces, and
doc/spec alignment including the mid-change D8/capacity-census/ADR-0023
amendment trail.

## Grounding facts (verified 2026-10-25, re-verify only if main moved)

- Commit order (task-ordered; subjects abbreviated): abbc5b2 (spec-init:
  proposal/design/specs/tasks settled + ADR-0023 first cut), 5e85c42
  (design completes — D6..D11 added: prior-art anchor,
  check-without-reservation + environment adversary, gross draw,
  authority-aligned guard discipline, total dispatch rows, census-schema
  state projection), 6bdf15e (1.1/1.2 `models/orchestration/pipeline.qnt`
  — guarded-action model, ORCH-I1..I5), d465a94 (2.1/2.2 retro-oracle:
  mask-a-family turns completeness red; handover-window pair —
  `models/orchestration/retro_oracle.qnt` + `models/helpers/
  orch-retro-oracle.py`, 13 scenarios), a81ee32 (3.1 `plan/dispatch.rs`
  — the compile-checked total TABLE, rows Execute/Unimplemented, dpseci
  the first Unimplemented with its retirement trigger), 6153355 (3.1
  the engine's interim 6.4 reify guard absorbed — reacts to the
  dispatch row, not its own list), 78b12e5 (3.2 the dpaa2-verify
  registry twin + orchestration trace replay —
  `intent/orchestration_itf.rs` 481, `tests/orchestration_replay.rs`
  221, 6 frozen traces under `models/traces/orchestration/`), 4df6102
  (4.1/4.2 `plan/capacity.rs` 236 — CapacityFace, PoolHeadroom
  Free/Spent, FdbBudget over the ADR-0011 Ceiling, Unknown admits —
  and `plan/admission.rs` 250 — gross draw via the dispatch DrawKind,
  data-only sorted AdmissionRefusal; carries the amended
  capacity-census spec + design D8 from the pending-creates fix),
  25ad0b8 (4.x `tests/admission_oracle.rs` — 4 offline scenarios incl.
  rebuild-takes-two-runs and the converged-intent-re-admits scenario),
  e7954b1 (5.1 tools: engine `plan_admission` — ONE `read_inventory`
  feeding face and FDB bracket so compile's UnknownCeiling warning
  cannot drift, `render_admission` + `render_admission_refusal`,
  dry-run verdict beside the plan, ensure gate after `reify_guard`
  before `converge_pools` Grow), 537ed4b (ADR-0023 amended:
  pending-creates admission basis + "shortfall disposition is a
  planning-time choice, default rollback" — rollback DESIGN only, see
  protected item 4), 2864998 (6.1 V-ADMIT-1 authored: hand-authored
  read-only suite + `tests/vadmit1_intents.rs` offline pins + ledger
  row "authored, pending"), 88e8cb5 (7.1 confluence: pipeline.qnt pure
  pass twins — State record + cur, actions = all{guard,
  commitState(applyX)}, public guard names kept as wrappers;
  ORCH_I6_pairwise diamond over the 37-pass alphabet, 666 pairs/state,
  census projection excludes censusOcc/verdict; `model:confluence`
  rung 20x200 chained into `model:validation`; COVERAGE ORCH-I6
  deferred→simulate with cost), 90d1f4a (6.1 sitting banked: ledger
  row flips to passed, VERDICTS.json V-ADMIT-1-rev1, tasks 6.1 tick).
- **The mid-change admission-basis amendment is a recorded fix, not an
  open defect.** As first sealed, admission priced the FULL desired
  plan against observed free headroom, so a converged re-ensure on a
  pool-tight board refused its own fixpoint. A user-driven semantics
  review caught it post-seal on 2026-10-24; the fix (draw basis =
  PENDING creates — the passes' own convergence reads:
  `PoolDeltas::create`, `seat_deficit`, `SwitchVerdict
  Create|Recreate` via `plan_switches` — with `GrossDraw::from_pending`
  the one constructor, routed through the dispatch TABLE) was
  fixup-squashed into 4df6102/25ad0b8/e7954b1 together with the
  capacity-census spec amendment (new scenario "A converged intent
  re-admits") and the design D8 amendment; the model needed no change
  (its admit already folded pending). A finding re-reporting full-plan
  pricing as live, or calling the spec scenario unimplemented,
  contradicts this trail. ADR-0023's amendment commit is 537ed4b.
- **Board outcome is a recorded fact** — findings contradicting it are
  false positives: **V-ADMIT-1 rev 1 passed 6/6** (2026-10-25,
  read-only, rode the sitting; VERDICTS.json `V-ADMIT-1-rev1`,
  source_hash = fnv1a64 of the suite script). dry-run exited 0
  rendering `dpcon draw=232 headroom=16 [refused]`; ensure exited 1 in
  the refused-pool voice having dispatched nothing; pre/post census
  byte-identical. The operand raises dpcon BY DESIGN: its ceiling is
  Unknown to compile, so static feasibility (Infeasible) cannot
  preempt the admission judgment, while dpbp's Counted(63) ceiling
  would refuse at compile — a finding calling the dpcon choice
  arbitrary contradicts the suite header and the ledger row. **Bonus
  live finding, recorded in the ledger row:** the board is dpbp-tight
  (free 1), so the port's base draw alone also refused dpbp (draw 2 >
  free 1) — the live anchor for D7's advisory that a pool-tight board
  refuses modest intents. That advisory (gross admission can refuse a
  pool-tight converged board's re-ensure UNTIL teardown frees) is
  design-recorded (D8: rebuild resolves as two runs) — a finding
  demanding net admission now contradicts D8's named revisit trigger.
- **Pre-logged carries from the group seals** (recorded on beads
  m7a.4/m7a.5) — the passes CONFIRM, locate precisely, and disposition
  these rather than re-discover them; each is a legitimate finding
  seed, none is yet dispositioned: (1) WANT_PORT gross-FDB asymmetry —
  the model prices +1 FDB port wire where Rust treats the edge as
  re-observation (`plan/admission.rs` near the draw fold) — one
  direction must win or trace replay may trip when an FDB-drawing edge
  trace freezes; (2) double `read_inventory` inside one
  `plan_admission` judgment (engine.rs) — two snapshots of one
  judgment's truth; (3) "banked fixtures" wording in the proposal's
  validation story vs hand-built-in-banked-shape reality; (4) the
  spec-delta dpio four-family wording; (5) "shortfall" (spec/ADR
  vocabulary) vs the rendered "headroom" voice; (6) dry-run does not
  preview the Unimplemented refusal — the dispatch-path reading was
  adopted (the refusal fires at dispatch, which dry-run never
  reaches); judge whether any user-facing doc contradicts it.
- Gates recorded green at the seals (Pass 1 re-runs the offline ones):
  `model:typecheck`/`test`/`simulate`, `model:confluence` [ok] 20x200,
  retro-orch 13/13, ledger_lint 2/2, orchestration_replay 3/3,
  admission_oracle 4 scenarios, quality floor
  (`scripts/checks/quality-floor.sh`), spec-align ACCEPT per group.
- **Deliberate decisions findings must not "fix"** (each recorded in
  design D1–D11, the proposal, ADR-0023, or the bead trail):
  1. D1/Non-Goals: Quint is the only formalism; Petri-net vocabulary is
     explanatory prose only. A finding proposing Petri tooling or a
     second model contradicts D1/ADR-0023.
  2. D2: exactly two guard kinds (convergence predicates, capacity
     counts), re-minted from observation every loop — no persisted
     ledger of any kind (ADR-0008). A finding proposing reservation
     state, a capacity cache, or a third guard kind contradicts D2/D7.
  3. D8: admission is GROSS over pending creates; frees never count.
     Net admission (free-before-draw scheduling) is a named revisit
     trigger. Rebuild-against-exhausted-budget refusing and resolving
     as two runs is the designed shape, matching pool-objects D12.
  4. ADR-0023 shortfall decision: rollback (run-scoped journal of
     observed results via a decorator on the backend traits, reverse
     unwalk through a compile-checked total reversal table, typed
     reboot-named residue) is DESIGNED in the ADR and deliberately NOT
     shipped here — it is bead dpaa2-controlplane-axq, its own openspec
     change after this one archives. A finding demanding the rollback
     implementation, or calling the ADR's design dead weight,
     contradicts that recorded scheduling decision.
  5. D9: exactly ONE live-read guard exists today (dpmac driver
     occupancy before switch-port wiring); all capacity counts are
     census-read because the firmware is the executing authority. A
     finding demanding more live-reads, or a general re-sample layer,
     contradicts D9's authority-alignment derivation.
  6. D10: dpseci is the first `Unimplemented` row, a typed whole-intent
     refusal naming its retirement trigger — crypto provisioning rides
     the sibling VPP scripts until its own change. A finding calling
     the dpseci row a gap contradicts D10.
  7. D11: the orchestration model's state is the census-schema
     projection, NOT an import of the family machines — family
     internals are trusted as atomic verdicts (assume-guarantee). A
     finding demanding the composed product model contradicts D11.
  8. Non-Goals: the existing pool grow/shrink passes keep their
     delivered shape (no migration onto the capacity face here); no
     concurrency (serialized dispatch; #10+ territory); only the two
     board-proven budgets (container pools, dpsw FDB) — bpid space,
     qbman seats, portal counts join with the changes that prove them.
  9. The design's open question "FDB ceiling on unbanked boards" was
     resolved as **characterize** (`Unknown` admits) at the m7a.4 seal
     — a finding demanding a conservative constant contradicts the
     resolution; a finding that the resolution is recorded nowhere
     reader-visible is legitimate for Pass 4.
  10. Confluence is bounded, pairwise, simulator-first (20x200), cost
     recorded in COVERAGE; the full treatment is an ADR-0023 revisit
     trigger. The pure-twin refactor kept the public guard names as
     wrappers precisely so `retro_oracle.qnt` and frozen traces were
     undisturbed — replay 3/3 and retro-orch 13/13 after it are the
     recorded semantics-preservation evidence.
  11. All protected decisions of prior reviews (dpsw-typestate +
     hardening, cross-dprc-links, dpseci, dpmac, dpni, pool, dprc
     lineage) bind this review exactly as before.
- `ponytail:` markers: the pre-existing repo set only; Pass 1 confirms
  the epic added none that hide an unmarked ceiling and none of the
  existing set went stale through epic-touched lines.
- Out of scope: everything prior epic reviews already judged that this
  change did not touch; dpsw-hardening's own commits (reviewed at its
  close); the rollback change (bead axq, future); #12 dpdmux and #10
  portal work; the proposal's forward-proof DoD item 4 (judged when
  #12 lands, not here); re-running board suites; VPP integration.

## Passes

### Pass 1 — Residue, deferrals, and verify re-run
Agent: Explore (medium). Budget ~40k. Runs first; feeds the other three.
Re-run every task-level verify obligation checkable offline:
`cargo test -p dpaa2-verify` (orchestration_replay — all 6 frozen
orchestration traces replayed, none orphaned, no trace without a
replay — and admission_oracle's 4 scenarios incl.
converged-intent-re-admits), `cargo test -p dpaa2-tools`
(vadmit1_intents pins, render snapshots), the retro-oracle
(`pnpm model:retro-orch` — 13/13, mask-a-family red), the Quint gate
(`pnpm model:typecheck` and, if runtime allows, `model:confluence`
20x200), the COVERAGE ledger lint, and the quality floor script.
Verify the deferral promises point at their carriers: rollback →
ADR-0023 + bead axq; net admission → D8's revisit trigger; full
confluence → ADR-0023 revisit trigger + COVERAGE ORCH-I6 row; budgets
beyond the two → Non-Goals. Sweep the epic-touched files (not
out-of-scope surfaces) for leftover scaffolding: `todo!`,
`unimplemented!` (distinguish the DESIGNED `Unimplemented` dispatch row
from scaffolding), `dbg!`, `#[allow(dead_code)]`, `#[ignore]`,
commented-out code, and stale wording predating the late answers — a
full-desired-plan draw claim predating the pending-creates fix, a
"pending sitting" claim predating the 2026-10-25 verdict, an
FDB-conservative-constant claim predating the characterize resolution.
Locate the six pre-logged carries (grounding) precisely (file:line)
and classify each. Confirm no new unmarked `ponytail:`-class
simplification shipped.

### Pass 2 — Isomorphism and the orchestration law
Agent: software-architect. Budget ~80k. After Pass 1; parallel with 3, 4.
Scope: `models/orchestration/pipeline.qnt`, `retro_oracle.qnt`,
`models/helpers/orch-retro-oracle.py`, the 6 frozen traces,
`crates/dpaa2-api/src/plan/{dispatch,capacity,admission}.rs`,
`crates/dpaa2-verify/src/intent/orchestration_itf.rs`,
`tests/{orchestration_replay,admission_oracle}.rs`. Four mandates:
(a) **Isomorphism (ADR-0002 law, one level up)**: the model's pass
alphabet, guards, and census projection vs the engine's dispatch TABLE
and pass set — row-for-row via the 3.2 registry twin; ORCH-I1
completeness vs the D10 exhaustive match (does masking a family
actually fail compilation, not just the model); ORCH-I2 order-as-guards
vs the engine's actual sequencing; names converge on the same readable
English.
(b) **Admission is the model's admit**: gross draw over pending creates
on both sides — locate the WANT_PORT asymmetry (pre-logged carry 1),
state which direction is correct and what trace would trip; the
refusal's stability (ORCH-I3) vs the Rust data-only sorted refusal;
capacity face types honest (Free/Spent, Unknown admits, Ceiling from
ADR-0011).
(c) **ITF replay coverage**: map the 6 frozen traces onto the model's
guarded transitions; name every guard/invariant with no replayed trace;
confirm the environment-adversary action (D7) and the confluence twins
are exercised or their absence recorded in COVERAGE with cost.
(d) **The confluence refactor is semantics-preserving**: the pure pass
twins vs the original actions — guard wrappers really just wrap; the
census projection's exclusion of censusOcc/verdict is justified (no
engine pass writes them); the 666-pairs/state diamond actually compares
final census, not intermediate state.

### Pass 3 — Architecture, sans-io discipline, and code quality
Agent: software-architect. Budget ~80k. After Pass 1; parallel with 2, 4.
Scope: `crates/dpaa2-tools/src/{engine,render,status,main}.rs` deltas,
`tests/vadmit1_intents.rs` + snapshots,
`crates/dpaa2-api/src/plan/{dispatch,capacity,admission}.rs` (seam
quality view), `models/board/V-ADMIT-1/` (script + operand). Four
mandates:
(a) **Sans-io discipline**: all judgment (dispatch totality, draw
derivation, admission verdict, capacity typing) lives in dpaa2-api;
engine.rs composes — `plan_admission` folds the face from `plan_pools`
and reads inventory ONCE (locate the pre-logged double-read carry 2 and
cost it); the ensure gate sits after `reify_guard` and before
`converge_pools` Grow with no second decision layer; render stays
display composition in the established refusal voice.
(b) **The dispatch table is the single door**: every plan-entry family
flows through it (grep for bypasses); the absorbed 6.4 interim guard
left no duplicate list; `GrossDraw::from_pending` is the only
admission constructor and it routes through the TABLE's DrawKind.
(c) **Suite quality**: V-ADMIT-1.sh against the board-suite protocol
(ADR-0003 — repo-root cd, total-deny self-check, provenance capture,
reference-pair assertion, kmsg window, PASS/FAIL face log, nonzero on
FAIL); the offline pins in vadmit1_intents.rs actually pin the operand
the script runs; the dpbp bonus refusal is consistent with the script's
checks (its greps demanded dpcon only — is that fragile or correct?).
(d) **Reuse-before-write and the #12 seam**: did render/admission reuse
the established render seams and refusal idioms; does the D10 frame
stand — would dpdmux land as one Execute row + its pass with zero
orchestration-layer rework (the proposal's DoD forward-proof), and
name anything that would force rework now rather than when #12 lands.

### Pass 4 — Spec/docs alignment
Agent: spec-align. Budget ~70k. After Pass 1; parallel with 2, 3.
Scope: `openspec/changes/resource-aware-orchestration/{proposal,design,
tasks}.md`, the 4 spec deltas under `specs/` (orchestration-model,
capacity-census NEW; reconciler, provisioning-cli MODIFIED) vs shipped
code; `docs/adr/0023-*.md` (both the original decisions and the
2026-10-24 amendment) vs the shipped admission basis and the deferred
rollback; COVERAGE ORCH-I1..I6 dispositions vs the witnesses they cite
(retro-oracle scenarios, replay traces, the confluence simulate row
with cost); `models/board/README.md` V-ADMIT-1 ledger row +
VERDICTS.json entry vs tasks.md 6.1 and the suite script — the records
tell ONE story: authored 2026-10-24, passed rev 1 6/6 2026-10-25, the
dpcon-not-dpbp reasoning, the dpbp bonus refusal; ROADMAP row #15 says
implemented and names what follows (axq rollback, #12 forward proof);
`openspec/specs/formal-models/spec.md` delta. Verify the
modified-capability claims in proposal.md are each true as shipped
(reconciler: dispatch IS the exhaustive registry, ordering constraints
ARE guards; provisioning-cli: dry-run DOES render admission verdicts).
Verify no doc still prices the full desired plan (the pending-creates
amendment reached every reader surface), none claims the sitting is
pending, none claims rollback shipped, and the characterize resolution
(grounding item 9) is recorded somewhere a reader would look.
Disposition the wording carries (3)(4)(5)(6) from grounding and Pass
1's doc-side hits.

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
find doc-side hits; 2 and 3 both touch the dispatch/admission seam);
drop unanchored staleness claims and any finding that "fixes" a
protected decision (the eleven grounding items, the board outcome, the
amendment trail, and everything prior reviews protected); deliver:
1. The merged findings ledger, most severe first, with dispositions —
   the six pre-logged carries MUST each appear with a final
   disposition (confirmed-fix-worthy → bead shape, or
   recorded-acceptable → where recorded).
2. A verdict on the change's core promise: can the origin story recur —
   is a compiled family without an executor non-compiling AND
   invariant-red, is an over-budget intent refused typed before any
   verb on both the offline oracle and the live board, does admission
   admit its own fixpoint — yes or no, evidence rows.
3. A verdict on ADR-0002 compliance one level up: is the shipped
   dispatch/admission surface structurally isomorphic to
   `pipeline.qnt` (registry twin green being necessary but not
   sufficient — judge names, guards, and the WANT_PORT asymmetry), and
   did the confluence refactor preserve semantics, yes or no, evidence
   rows.
4. Proposed follow-up work as bead-shaped items (title + why +
   acceptance) for a just-in-time openspec change — the review itself
   changes nothing. Candidates to judge, not rubber-stamp: the
   WANT_PORT asymmetry, the double read_inventory, the
   dpbp-tight-board operational posture (any port-adding intent on
   this board refuses until headroom exists — operator guidance or
   axq's concern?).
No guidelines deliverable. At most nominate a rule amendment if a
finding shows an existing rule failed to prevent a defect (candidate:
the admission-basis defect shipped through a sealed group with
spec-align ACCEPT and was caught only by a user-driven semantics
review — should the acceptance gate include a "fixpoint re-admits"
style self-application check for every judgment the engine renders?).

## Budgets

P1 40k · P2 80k · P3 80k · P4 70k · synthesis 60k ≈ 330k total across
4 pass agents + judge. A pass exceeding ~1.5x its budget stops and
reports partial.
