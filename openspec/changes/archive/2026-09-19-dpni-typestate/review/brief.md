# Review brief: dpni-typestate epic

Slug: `dpni-typestate`. Reviewed span: commits ced8de8..5508c75 on main
(1 spec-init + 25 execution commits; one interleaved docs commit carries no
task tag — see grounding). Epic guu, closed 2026-09-19 at task 6.1 (bead
guu.9), trace corpus landed one commit after the seal (5508c75, in scope).
Net epic size excluding openspec/ artifacts and board evidence: ~4,720
inserted / ~330 deleted lines across 95 files in crates/, models/, docs/ —
roughly 60% of dprc-encapsulation, so the same four-pass shape with smaller
budgets. Quality floor (cargo build/fmt/clippy/clippy --tests/doc/test) is
green per the 6.1 DoD; mechanical lint findings are NOT the target. Targets:
the change's core promise ("an invalid dpni create configuration is
unrepresentable"), structural isomorphism of the dpni model vs the Rust
family (ADR-0002 law), the guu.4a envelope-fence amendment landing two-sided,
sans-io discipline across the grown shim, refusal-parity discrimination for
the eleven dead options, ITF replay coverage, the D5/z5z rider, the D6
hazard closure, and doc/spec alignment including the untagged ADR-0019.

## Grounding facts (verified 2026-09-19, re-verify only if main moved)

- Commit range: `ced8de8` (spec-init) → `5508c75` (trace-corpus chore, lands
  after the 6.1 seal `dc64596` and is part of the change: it moves frozen
  traces into `models/traces/{families,retro}/`). Every commit in the range
  carries a task tag except `9373b40` (ADR-0019, four state patterns cover
  the sixteen families, 280 lines) — drafted mid-epic between tasks 3.2 and
  4.1, cited today only by `docs/baseline/dpdbg.md` and itself, not by any
  dpni-typestate artifact or code. Whether it belongs to this change record
  or should have been its own change is Pass 4's to disposition; its content
  (atemporal style, family-pattern claims vs the shipped dpni shape) is in
  scope for Pass 4.
- Major new surface (current sizes): `crates/dpaa2-api/src/families/dpni.rs`
  1,375 (create-surface typestates, tasks 2.1/2.2),
  `models/families/dpni.qnt` 776 (option surface + invariants, task 1.1 +
  guu.4a), `crates/dpaa2-mc/src/restool.rs` +453 (now 1,789; dpni create /
  primary-MAC / observe_container verbs, tasks 4.1/4.2),
  `crates/dpaa2-verify/tests/dpni_replay.rs` 336 +
  `src/intent/dpni_itf.rs` 236 (ITF conformance, tasks 4.1/5.1/5.2),
  `crates/dpaa2-mc/src/parse.rs` +144 (now 774; dpni_attr read-back
  mapping), `crates/dpaa2-api/src/intent/{derive,compiled}.rs` +58/+76
  (profile derivation, task 3.1), `intent/tenant.rs` +24/−20 (now 190;
  hazard closure, task 3.2), `crates/dpaa2-tools/src/engine.rs` +52/−34
  (now 636; per-candidate re-observation, task 4.2), plus
  `models/intent/{main,derive,refuse,invariants}.qnt` deltas mirroring the
  derivation. Docs: `docs/baseline/dpni.md` +87/−16, `models/COVERAGE.md`
  +39 (DPNI-I1..I12 rows dispositioned), ADR-0011 +5, ADR-0013 +4, ADR-0019
  (new, untagged), ROADMAP row #5, CHANGELOG. Frozen traces: 7 dpni scenario
  ITF files under `models/traces/families/dpni/`.
- **Amendment trail (guu.4a)**: the 5.2 board sitting exposed a latent
  isomorphism divergence — task 3.1's refined `NumQueues` (HI=32) meant an
  accepted-path T > 32 fell to `NumQueues::DEFAULT` while the model's
  `CreateCfg.numQueues` stayed an unbounded int; unreachable by any frozen
  trace or the cpus=16 reference board. Bead guu.4a (2026-09-19) chose a
  two-sided sizing refusal, shipped in the three 5.2 commits: `3861a64`
  (project() predicts the read-back, not the request), `61a3a9e` (the queue
  envelope fences derived num_queues), `58f0ccd` (the ITF parser decodes
  QueueEnvelopeExceeded), with frozen trace
  `scenarioEnvelopeRefusedTest.itf.json`. A finding that the envelope fence
  is "unplanned scope creep" is a false positive — it is the amended plan;
  proposing a different fence (model-side range only, documented ceiling)
  contradicts the recorded decision. Revisit trigger stays tile #10 or a
  board with T > 32.
- Deliberate decisions that findings must not "fix": runtime surface is
  primary MAC only — the other 35 `dpni_set_*` setters are a recorded
  deferral to tile #10, typing them now is speculation the design rejected
  (D1); cfg drift plans destroy + create, MAC-only drift plans the mutation,
  never repair-in-place (D1, ADR-0001 §4); the raw-mask escape
  (`0x80000000` PFDR_IN_PEB) is a first-class provenance-carrying
  constructor, not a backdoor to close (D2); options derive purely from
  `Dataplane` + construct with exactly two profiles and no per-interface
  override — a third profile is a documented amendment, not a flexibility
  gap (D3); `dist_key_size` is write-only by construct and excluded from
  observation comparison because `dpni_attr` omits it (D4);
  `observe_container(id)` per-candidate re-observation is a latency
  improvement, not a leak fix — OI-3 settled that serial restool spawns
  return their portal (COVERAGE OI-3 row, V-DPRC-13) (D5/z5z); no dpni
  table-content management and no queue/DPCON wiring (non-goals, #6/#9/#10).
- Refusal parity: the eleven dead options and `num_rx_tcs` have no
  constructor AND a programmatic refusal naming each one (vocabulary-v2
  precedent, design D2); frozen traces `scenarioDeadOptionRefusedTest` and
  `scenarioNumRxTcsRefusedTest` replay the refusal side. Collapsing the
  refusals to a generic error, or a dead option acquiring a constructor
  path, is a finding on the change's core promise.
- `models/COVERAGE.md` DPNI rows: I1/I2/I4/I8/I9/I11/I12 modeled, I3
  modeled (retro), I5/I6/I10 deferred/adapter-scoped, I7 board-settled —
  each with named model cites and check column; DPNI-I12 records the D4
  write-only law structurally. Deferral promises from D7/task 6.1: #1
  TX_CONF v1 handler → #10, #12 num_rx_tcs-via-DPL → #14, #4's QoS/FS half
  and #7/#11 → earliest reachability (#9/#10), 35 runtime setters → #10.
- Zero `ponytail:` markers in epic-touched code (grep verified) — one
  appearing in a touched file is a Pass 1 hit.
- Out of scope: everything the intent-layer, vocabulary-v2, and
  dprc-encapsulation reviews already judged that this change did not touch;
  board evidence under `models/board/` (operator-sealed, per-task verdicts
  committed); re-running board suites; portal/ioctl work (explicit
  non-goal, #10); the pre-range yfg/api-modularization surface (sealed
  2026-09-17); mechanical import-path churn from the trace-corpus move.

## Passes

### Pass 1 — Residue, deferrals, and verify re-run
Agent: Explore (medium). Budget ~50k. Runs first; feeds the other three.
Re-run every task-level verify obligation from tasks.md that is checkable
offline (typecheck/test names cited, grep-able assertions; report any that
no longer hold). Verify the promised deferral rows exist and point at their
tiles (#10 runtime setters + TX_CONF, #14 num_rx_tcs-via-DPL, table/traffic
items) in `models/COVERAGE.md` AND in `docs/baseline/dpni.md` — the
baseline's grep surface for `#10`/`#14` looked thin at grounding time
(2 hits); confirm whether the rows live under different wording or are
missing. Sweep the epic-touched files (not the out-of-scope surfaces) for
leftover scaffolding: `todo!`, `unimplemented!`, `dbg!`,
`#[allow(dead_code)]`, `#[ignore]`, commented-out code, stale wording that
predates the guu.4a amendment (an unbounded-num_queues claim), and orphaned
fixtures. Verify the 7 frozen dpni traces under
`models/traces/families/dpni/` are each replayed by `dpni_replay.rs` and
none is orphaned; verify the trace-corpus move (5508c75) left no dangling
path references. Sweep `ponytail:` markers in touched files (expected:
zero). Classify every hit: (a) stale — a named task should have updated it;
(b) deliberate (grounding lists the protected decisions); (c) historical
prose recording what was.

### Pass 2 — Isomorphism and the typestate law
Agent: software-architect. Budget ~120k. After Pass 1; parallel with 3, 4.
Scope: `models/families/dpni.qnt`, `models/intent/{derive,refuse}.qnt`,
`crates/dpaa2-api/src/families/dpni.rs`, `intent/{derive,compiled}.rs`,
`crates/dpaa2-verify/src/intent/dpni_itf.rs`, `tests/dpni_replay.rs`.
Four mandates:
(a) **Create-surface isomorphism (D8, ADR-0002 law)**: the Quint option
surface (twelve live options with ranges, typed flag vocabulary + raw-mask
escape, PMD/kernel profiles, named invariants: create-range refusal,
profile totality, dead-option parity, write-only field law) vs the Rust
typestates — option-for-option, range-for-range, refusal-for-refusal;
names converge on the same readable English. Spot-check the "invalid
create configuration unrepresentable" claim: which invalid configurations
does the type system actually reject at compile time vs merely refuse at
runtime — a runtime refusal advertised as a typestate is a finding.
(b) **guu.4a fence two-sided**: the queue envelope refusal
(T > NumQueues::HI) must hold as the same predicate in `dpni.qnt` and the
Rust core, with `project()` predicting the read-back (not the request) on
both sides, and `scenarioEnvelopeRefusedTest` replaying the refusal arm.
A one-sided fence reopens the divergence guu.4a closed.
(c) **ITF replay coverage**: map the 7 dpni traces onto the model's
guarded transitions; name every guard/refusal arm with no replayed trace
(the dead-option refusal has eleven named arms — how many are replayed?).
(d) **D4 write-only law both sides**: the model's observation omits
`dist_key_size` (DPNI-I12 structural claim) and the Rust observation
comparison excludes it by construct, not by a runtime skip; the read-back
asymmetries (split rx/tx TCs, added key sizes, `wriop_version`) map in the
shim's observation type without leaking into the pure core's equality.

### Pass 3 — Architecture and code quality
Agent: software-architect. Budget ~100k. After Pass 1; parallel with 2, 4.
Scope: `crates/dpaa2-mc/src/{restool,parse,runner}.rs`, `tests/shim.rs`,
`crates/dpaa2-api/src/{contract/mc,contract/fake,core/types,core/model,
plan/reconcile,plan/transition}.rs`, `intent/tenant.rs`,
`intent/refuse/{mod,tenant,compile_tests}.rs`,
`crates/dpaa2-tools/src/{engine,render}.rs`, tests. Four mandates:
(a) **Sans-io discipline**: the dpni create verb, primary-MAC set, and
`observe_container` in `restool.rs` (+453) must be thin plumbing — any
option-derivation, range, or drift decision living in the shim instead of
the pure core is a finding; `parse.rs` (+144) maps `dpni_attr` shape only;
`engine.rs` re-observation (+52/−34) is imperative shell — planning logic
that leaked into it is a finding.
(b) **Refusal typing end-to-end**: the dpni parity refusals, envelope
refusal, and create-range refusals survive from the pure core through
`contract/mc.rs` and the shim to drift/refusal reporting without
string-matching or collapse; `QueueEnvelopeExceeded` stays discriminated.
(c) **Reuse-before-write**: restool.rs grew +453 — flag rendering/plumbing
the existing `runner.rs`/`parse.rs` seams already provided; the dpni
observation mapping vs the dprc observation precedent; `contract/fake.rs`
seeding vs existing fixture helpers. Judged against protected deliberate
duplication (ADR-0014 lockstep, parse/compile twins).
(d) **Seam quality for #6/#7/#9/#10**: does the `families::dpni` shape
(immutable cfg type parameter, runtime-state-within-type) pre-shape tile
#10's setter surface as D1 promises, or encode create-only assumptions the
portal backend must rewrite? Does `observe_container(id)` generalize to
the next family or hardcode dpni? Disposition the D6 hazard closure: are
the two tenant.rs hazards (zero-value Default path, constructible empty
TenantName) actually closed by construct, and did the closure ripple
(named empty constructor, 6fdd3c8/11e961b) leave any call site on the old
path?

### Pass 4 — Spec/docs alignment
Agent: spec-align. Budget ~90k. After Pass 1; parallel with 2, 3.
Scope: `openspec/changes/dpni-typestate/{proposal,design,tasks}.md`, the
six spec deltas under `specs/` (formal-models, intent-compiler,
mbt-harness, mc-backend, reconciler, system-integration) vs shipped code;
`docs/baseline/dpni.md` amendments anchored to named board verdicts (the
V-DPNI series extension and the 5.2 sitting's probe outcomes for
unknown-registers #3/#6/#8); `models/COVERAGE.md` DPNI-I1..I12
dispositions match the model's marks and the 5.2 outcomes; ADR-0011/0013
amendments vs the code that cites them; ROADMAP row #5 status; CHANGELOG
entry. Disposition ADR-0019: untagged mid-epic commit, cited only by
`docs/baseline/dpdbg.md` — does it belong in this change record, do its
four state patterns actually cover the shipped dpni shape, and is it
atemporal per the recorded style directive? Verify the guu.4a trail is
complete: bead ↔ the three 5.2 commits ↔ model + Rust + ITF + frozen trace
↔ any baseline/ADR ceiling note its acceptance promised. Verify the D5/z5z
rider's disposition cites the OI-3 outcome as the bead prescribed. Verify
both design Open Questions have their recorded landing: the
`num_cgs = num_queues + 8` heuristic (narrowed by the #3 walks or recorded
as deployed heuristic with a #10 revisit trigger) and `HAS_REPLICATION`
(#8 accept/reject answered in the baseline). Disposition Pass 1's
category-(a) doc hits.

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

## Synthesis mandate (change-judge, ~70k)

Deduplicate and rank findings across passes (Passes 1 and 4 will both find
doc-side hits; 2 and 3 may both touch the refusal path); drop unanchored
staleness claims and any finding that "fixes" a protected decision (the
primary-MAC-only runtime surface, the destroy+create drift plan, the
raw-mask escape, the two-profile pure derivation, the D4 write-only
construct, the guu.4a refusal fence as shipped, the D5 latency-not-leak
disposition, the table/queue non-goals); deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. A verdict on the change's core promise: is an invalid dpni create
   configuration actually unrepresentable in `families::dpni` (compile-time
   where claimed, typed refusal where recorded), yes or no, evidence rows.
3. A verdict on ADR-0002 compliance: is the shipped create surface
   structurally isomorphic to the dpni.qnt model including the guu.4a
   fence, yes or no, evidence rows.
4. Proposed follow-up work as bead-shaped items (title + why + acceptance)
   for a just-in-time openspec change — the review itself changes nothing.
No guidelines deliverable: the 12-rule set stands; at most nominate a rule
amendment if a finding shows an existing rule failed to prevent a defect
(guu.4a is a candidate: a refined range landed one-sided and only the
board caught it — does a rule fence that class?).

## Budgets

P1 50k · P2 120k · P3 100k · P4 90k · synthesis 70k ≈ 430k total across
4 pass agents + judge. A pass exceeding ~1.5x its budget stops and reports
partial.
