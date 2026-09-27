# Review brief: pool-objects epic

Slug: `pool-objects`. Reviewed span: commits df950e0..9abb7fa on main
(1 ADR-0019 preamble + 1 spec-init + ~77 execution commits; every commit
carries a `Change: pool-objects` trailer and a task-phase reference). Epic
960, closed at tasks 4.4 (NOT FIRED, bead 960.13, dff678b) and 5.1 (docs
close-out, bead 960.14, 9abb7fa). Net epic size excluding openspec/
artifacts: ~12,830 inserted / ~540 deleted lines across 115 files in
crates/, models/, docs/ — roughly 2.7x dpni-typestate, so the same
four-pass shape with larger budgets. Quality floor (cargo
build/fmt/clippy/clippy --tests/doc) and the ledger lint are green as of
9abb7fa; mechanical lint findings are NOT the target. Targets: the change's
core promise (the driver allocation pool — dpbp/dpcon/dpmcp with dpio's
seat variant — converges as anonymous P3 capacity: count-declared,
level-triggered, idempotent, with free-only shrink and DPL-born survival),
structural isomorphism of `pool_lifecycle.qnt` + family variants vs the
Rust P3 substrate (ADR-0002 law), the D9 single-provider amendment landing
everywhere the old per-port chain reached, the D10 plug/draw custody split
two-sided, the ADR-0020 scope narrowing (root reclaim → typed
reboot-required residue) consistent across model, twin, suites, and specs,
the D12 loop-breaker refusal, sans-io discipline across the grown shim
(pool.rs, populate.rs, probe.rs are new), ITF replay coverage, and
doc/spec alignment including the 5.1 COVERAGE dispositions.

## Grounding facts (verified 2026-09-27, re-verify only if main moved)

- Commit range: `df950e0` (ADR-0019 quint-module preamble) and `2b562b8`
  (spec-init) → `9abb7fa` (5.1 docs seal). The 4.4 gate closed NOT FIRED at
  `dff678b`: the DPL-defined-child escape did not fire — DPBP-I2's
  kernel-pool half was witnessed at root (V-POOL-7 rev 1), and the
  unreached faces (DPIO-I3 kernel half — seat-saturated
  container-independently per ADR-0008; DPBP-I4 top-up; DPRC-I8
  plug→probe) ride bead dpaa2-controlplane-5y7 / tile #10. A finding
  proposing the DPL edit anyway contradicts the recorded gate outcome.
- Major new surface (current sizes):
  `crates/dpaa2-api/src/families/pool_lifecycle.rs` 1,623 (the P3
  census/sizing/disposition substrate, tasks 2.1/2.2 + 3.10/3.18),
  `families/dpio.rs` 740 (seat variant, task 2.3 — table-pure, no cfg
  facet earned, the ADR-0019 judgment marker),
  `models/families/pool_lifecycle.qnt` 673 + thin family instantiations
  (`dpbp.qnt` 41, `dpcon.qnt` 47, `dpmcp.qnt` 37, `dpio.qnt` 287 with the
  seat laws), `crates/dpaa2-mc/src/pool.rs` 669 (count→individual
  crossing), `populate.rs` 590 (child population + VFIO handoff),
  `probe.rs` 174 (root-bind read-back), `restool.rs` +750 (create/destroy
  verbs for the four families, connect/endpoint, unbind), `dpaa2-hal`
  driver-link + eth-unbind sysfs faces, `crates/dpaa2-tools/src/engine.rs`
  +945 (root pool convergence before the port loop, teardown walk,
  residue reporting), `crates/dpaa2-verify/tests/pool_replay.rs` 466 +
  `dpio_replay.rs` 247 with `src/intent/{pool_itf,dpio_itf}.rs` (ITF
  conformance twins, task 4.1), board suites V-POOL-5/6/7, V-DPIO-1,
  V-MVP-1 under `models/board/`. Docs: designs D1–D12 (D9/D10/D11/D12
  amended mid-epic), ADR-0019 (P3 pattern + dpio row), ADR-0020 (new),
  ADR-0008 §9 (loop-breaker note), `docs/upstream/
  phylink-dpni-churn-crash.md` (finding 49), 5.1's COVERAGE dispositions +
  three baseline promotions + ROADMAP row #6 (9abb7fa).
- **Amendment trail** — four mid-epic design amendments, each
  evidence-driven; findings that call them scope creep are false
  positives, and findings proposing their reversal contradict recorded
  decisions:
  1. **D9 single provider** (task 3.5, e3af5c2; beads 960.17–.20):
     V-POOL-6 rev 3 exposed the double-feed — the port chain's companion
     dpmcp counted into the pool census. `create_dpni` sheds the per-port
     provisioning chain (f06fbb3); per-declared-port draws fold into the
     pool requirement (0aed0f5); the census nets DPL-born out of the draw
     guard (eebf972).
  2. **D10/D11 custody split** (tasks 3.9–3.14, f1c0e65 audit
     2026-09-24): plugged ≠ drawn; the plug facet splits from draw
     (f3c2838, b2193f8); reclaim unplugs before it destroys (bf838f1);
     ports actuate in their planned container; the one-label membership
     law — a labelled stray is pruned, a bare empty-label foreign
     deliberately escapes prune (V-POOL-6 rev 7 witnesses both arms).
  3. **D12 loop-breaker** (task 3.15, 7201578 + afd4d3a): V-MVP-1 rev 1's
     kernel Oops/UAF churn decided the same-run rebuild is a typed
     refusal, teardown severs first; ensure exits typed (5dca220);
     ADR-0008 §9 records it. The phylink race itself is upstream's
     (finding 49 fired once at the ordered teardown, recorded in
     docs/upstream/, power-cycle applied) — a churn workaround is
     explicitly rejected (4fdf8e9).
  4. **ADR-0020 grow-only root reclaim** (tasks 3.17–3.20, 1f0d5df →
     6bf7a58): real unbind surfaced the kernel's refusal to return root
     capacity; the reclaim law narrows to child scope, root surplus is a
     typed reboot-required residue with destroy=0, reported on every
     operator surface, labeled plugged capacity counted for the residue.
- Deliberate decisions findings must not "fix": pool counts are owned
  solely by the pool construct — a port never provisions its own trio
  (D9); anonymous capacity is count-converged, never identity-matched —
  the count↔individual boundary stays in design D2, deliberately not an
  ADR (its no-distortion condition did not fire; verified at 5.1);
  free-only shrink judged from the census, refusal (`ShrinkBelowDraw`)
  not teardown; DPL-born structurally exempt from prune; the bare
  empty-label foreign escapes prune by the one-label law (D10) — not a
  leak; root surplus destroy=0 residue (ADR-0020 decisions 1–2); dpio has
  no cfg facet and seats are grow-only with typed residue (ADR-0019 row,
  ADR-0008); unbind is level-triggered on the driver read-back and
  tolerates losing the release race (567ad44, 214a7a1); observe fills the
  MC default for an unset request and project models it board-pinned
  (bee68a1, 1ebd61a); the grow is not paced — the kernel session owns
  both kernel bugs (270a22e); VFIO rebind-drift healing is deferred to
  tile #9 (design D11, bead w01); V-POOL-6/V-MVP-1 are hand-authored
  dpaa2ctl suites with no SuitePlan — their verdicts live as prose in
  `models/board/README.md`, deliberately absent from VERDICTS.json
  (lint-consistent, settled at 5.1); CHANGELOG.md untouched — cliff is
  not set up yet (user directive 2026-09-27), not a gap this review
  reopens.
- Six `ponytail:` markers exist in the repo (model.rs:552, fake.rs:535,
  compile_props.rs:168, fitcheck.rs:112, observed.qnt:30,
  invariants.qnt:105); which sit in epic-touched files and whether each
  names its ceiling honestly is Pass 1's to classify — a marker is a
  recorded deliberate simplification, not a defect per se.
- Frozen traces live under `models/traces/families/{dpbp,dpio,dpni,dprc}`;
  the pool walks replay through `pool_replay.rs`/`dpio_replay.rs`
  (task 4.1). Board evidence under `models/board/` is operator-sealed —
  re-running board suites is out of scope.
- Out of scope: everything the intent-layer, vocabulary-v2,
  dprc-encapsulation, and dpni-typestate reviews already judged that this
  change did not touch; portal/ioctl work (#10); dpmac/dpseci surfaces
  (#7/#8); traffic and cross-dprc links (#9); the repo-wide
  anchored-refs `--tree` debt (carried bead, pre-existing); kernel-side
  fixes (the phylink race and the root-reclaim kernel remedy are recorded
  upstream work).

## Passes

### Pass 1 — Residue, deferrals, and verify re-run
Agent: Explore (medium). Budget ~60k. Runs first; feeds the other three.
Re-run every task-level verify obligation from tasks.md that is checkable
offline (typecheck/test names cited, grep-able assertions; report any that
no longer hold). Verify the deferral promises point at their carriers: the
5y7/#10 routing for DPRC-I8/DPBP-I4/DPIO-I3-kernel-half and DPMCP portal
state, the #9 routing for DPCON-I3/I4 runtime dynamics, the w01/#9 VFIO
rebind-drift rider — in `models/COVERAGE.md` AND the family baseline docs.
Sweep the epic-touched files (not out-of-scope surfaces) for leftover
scaffolding: `todo!`, `unimplemented!`, `dbg!`, `#[allow(dead_code)]`,
`#[ignore]`, commented-out code, stale wording predating the four
amendments (a per-port provisioning claim, a root-reclaim-destroys claim,
plugged-equals-drawn phrasing, an unpaced-grow caveat), and orphaned
fixtures. Verify every frozen pool/dpio trace is replayed by
`pool_replay.rs`/`dpio_replay.rs` and none is orphaned. Classify the six
`ponytail:` markers: in-scope vs pre-existing, and each in-scope one as
(a) stale — a named task should have retired it; (b) deliberate with an
honest ceiling; (c) historical prose.

### Pass 2 — Isomorphism and the pool law
Agent: software-architect. Budget ~140k. After Pass 1; parallel with 3, 4.
Scope: `models/families/{pool_lifecycle,dpbp,dpcon,dpmcp,dpio}.qnt`,
`crates/dpaa2-api/src/families/{pool_lifecycle,dpio}.rs`,
`crates/dpaa2-verify/src/intent/{pool_itf,dpio_itf}.rs`,
`tests/{pool_replay,dpio_replay}.rs`. Four mandates:
(a) **P3 isomorphism (ADR-0002 law, ADR-0019 pattern)**: the Quint pool
machine (census, grow/shrink/refuse disposition, prune with the one-label
membership judge, DPL-born survival, born-drawn netting, environment
adversary) vs the Rust substrate — law-for-law, guard-for-guard,
refusal-for-refusal; names converge on the same readable English. The
four thin family instantiations must actually be thin: family-specific
law leaking into the shared substrate (or substrate law re-implemented
per-family) is a finding.
(b) **The amendments landed two-sided**: the D10 plug/draw split (drawn
distinct from plugged in census AND model), the ADR-0020 scoped reclaim
(child-scope reclaim law, root surplus as typed residue with destroy=0,
`shrinkBelowDrawRefusedTest` both sides), the D12 same-run-rebuild
refusal (a6c1dfb model ↔ 8f93bd5 core), and the D9 born-drawn netting —
each must hold as the same predicate in Quint and Rust. A one-sided law
reopens the divergence its amendment closed.
(c) **ITF replay coverage**: map the frozen traces onto the model's
guarded transitions; name every guard/refusal arm with no replayed trace
(the disposition table in COVERAGE lines ~217-232 names twelve laws with
`itf-replay` marks — how many arms does each replay actually drive?).
(d) **dpio seat variant**: the seat arithmetic (regime-typed ceilings,
-ERANGE refusal, grow-only seats, typed reboot-required `seatDisposition`
residue) holds identically in `dpio.qnt` and `families/dpio.rs`, and the
no-cfg-facet judgment (ADR-0019 marker) is structural — a cfg facet
sneaking in through a side door is a finding on the recorded judgment.

### Pass 3 — Architecture and code quality
Agent: software-architect. Budget ~140k. After Pass 1; parallel with 2, 4.
Scope: `crates/dpaa2-mc/src/{pool,populate,probe,restool,parse}.rs`,
`crates/dpaa2-hal` (driver-link + eth-unbind faces),
`crates/dpaa2-api/src/{contract/mc,contract/fake,core/model,
plan/reconcile}.rs`, `crates/dpaa2-tools/src/{engine,render,main}.rs`,
tests. Four mandates:
(a) **Sans-io discipline**: pool.rs crosses counts into individuals,
populate.rs populates and hands off, probe.rs reads the bind back —
all three must be thin plumbing driven by pure-core decisions; any
census judgment, disposition arithmetic, victim selection, or ordering
decision living in the shim instead of `pool_lifecycle.rs` is a finding;
engine.rs (+945) is imperative shell — planning logic that leaked into
it is a finding. hal faces stay policy-free primitives.
(b) **Refusal and residue typing end-to-end**: `ShrinkBelowDraw`, the
same-run-rebuild refusal (named diff), the seat -ERANGE, and the
reboot-required residue survive from the pure core through contract/mc
and the shim to every operator surface (ensure, teardown, dry-run
render) without string-matching or collapse; teardown reports the
capacity it cannot return (8566110) rather than silently succeeding.
(c) **Reuse-before-write**: restool.rs grew +750 — flag plumbing the
existing runner/parse seams already provided; pool.rs vs the
dprc-encapsulation crossing precedent; populate.rs vs the existing child
lifecycle; fake.rs (+379) seeding vs existing fixture helpers. Judged
against protected deliberate duplication (ADR-0014 lockstep twins).
(d) **Seam quality for #7/#8/#9/#10**: does the P3 substrate pre-shape
the next families (dpmac/dpseci typestates, the portal backend) or
encode trio-specific assumptions the next change must rewrite? Does the
teardown walk (consumers before pools, 156a37f) generalize? Is the
root-only projection (7735eba) a reusable slice or a one-off?

### Pass 4 — Spec/docs alignment
Agent: spec-align. Budget ~100k. After Pass 1; parallel with 2, 3.
Scope: `openspec/changes/pool-objects/{proposal,design,tasks}.md`, the
five spec deltas under `specs/` (formal-models, mbt-harness, mc-backend,
reconciler, system-integration) vs shipped code; the 5.1 COVERAGE
dispositions and the three baseline promotions (dpbp/dpio/dpcon) vs the
witnesses they cite; ROADMAP row #6; ADR-0019 (P3 pattern + dpio row) and
ADR-0020 vs the code that cites them; ADR-0008 §9; the phylink upstream
record. Verify each amendment trail is complete: D9 ↔ beads 960.17–.20 ↔
the shed provisioning chain ↔ V-POOL-6 rev 7; D10/D11 ↔ the custody
commits ↔ the one-label witnesses; D12 ↔ ADR-0008 §9 ↔ the refusal
commits ↔ V-MVP-1 rev 5; ADR-0020 ↔ the 3.17–3.20 commits ↔ the narrowed
suites (88f7c09) ↔ the 5.1 rows. Verify the specs no longer promise root
reclaim the kernel cannot give (d6eaa58) anywhere — a surviving stale
promise is a finding. Verify ADR-0019/0020 are atemporal per the recorded
style directive. Verify the 4.4 NOT FIRED close is consistently reflected
(no doc still treats the DPL-child escape as pending inside #6).
Disposition Pass 1's category-(a) doc hits.

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
doc-side hits; 2 and 3 may both touch the refusal path); drop unanchored
staleness claims and any finding that "fixes" a protected decision (the
D9 single provider, the one-label prune law, the ADR-0020 destroy=0
residue, the D12 refusal, the no-cfg-facet dpio judgment, the 4.4 NOT
FIRED outcome, the hand-authored-suite verdict placement, the untouched
CHANGELOG); deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. A verdict on the change's core promise: does the pool converge as
   anonymous P3 capacity — count-declared, level-triggered, idempotent,
   free-only shrink, DPL-born survival, typed root residue — with the
   laws held on both sides of the sans-io seam, yes or no, evidence rows.
3. A verdict on ADR-0002 compliance: is the shipped P3 substrate
   structurally isomorphic to `pool_lifecycle.qnt` including the four
   amendments, yes or no, evidence rows.
4. Proposed follow-up work as bead-shaped items (title + why + acceptance)
   for a just-in-time openspec change — the review itself changes nothing.
No guidelines deliverable: the 12-rule set stands; at most nominate a rule
amendment if a finding shows an existing rule failed to prevent a defect
(candidates: the V-POOL-6 rev 1–3 census double-feed — three board revs
to catch a model/census operand mismatch; the 3.14 audit finding suite
legs asserting laws the shipped census could not execute — does a rule
fence either class?).

## Budgets

P1 60k · P2 140k · P3 140k · P4 100k · synthesis 80k ≈ 520k total across
4 pass agents + judge. A pass exceeding ~1.5x its budget stops and reports
partial.
