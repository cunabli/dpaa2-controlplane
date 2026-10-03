# Review brief: dpseci-typestate epic

Slug: `dpseci-typestate`. Reviewed span: commits 8532cd6..ae23721 on main
(1 spec-init + 15 execution commits, every one carrying a
`Change: dpseci-typestate` trailer). Epic dpaa2-controlplane-lbk, closed
2026-10-03 at the ae23721 seal: 12/12 tasks across 15 beads (lbk.0–.14,
including the mid-epic D9 decision bead .13 and the sitting-divergence
fix .14). Net epic size excluding openspec/ artifacts, frozen traces, and
beads export: ~4,700 inserted / ~70 deleted lines across ~47 files in
crates/, models/, docs/ — roughly 125% of dpmac-typestate, so the same
four-pass shape with dpmac-scale budgets and extra weight on Pass 3 (the
workspace's unsafe debut lands there). Quality floor (cargo build/fmt/
clippy/clippy --tests/doc) is green per the 6.1 close-out; mechanical
lint findings are NOT the target. Targets: the change's core promise
(the crypto interface is a typed, observed surface — the cfg-only P2
family with length-coupled priorities and the closed options vocabulary,
`HAS_CG` as a real convergence observable through the MC-portal read
slice, unobservability typed rather than guessed, and the anonymous
dpseci population converged as a multiset census on the observable cfg
signature), structural isomorphism of `dpseci.qnt` vs the Rust family
and census (ADR-0002 law), the fenced unsafe debut (ADR-0021: a write
command id is unrepresentable, not forbidden by review), sans-io
discipline across the grown shim and the portal primitive, ITF replay
coverage, and doc/spec alignment including the 1.2/6.1 COVERAGE
dispositions, the new ADR-0021, and the ADR-0019 P2-member amendment.

## Grounding facts (verified 2026-10-03, re-verify only if main moved)

- Commit order: 8532cd6 (spec-init, lbk.0), 6c32e8e (1.1 model P2 cfg
  shape, lbk.1), a0fff2f (1.2 COVERAGE sync, lbk.2), 7139a77 (2.1
  family, lbk.3), 1c2b337 (2.2 derivation, lbk.4), 0b69b54 (2.3 MBT
  twins, lbk.5), 11b3a67 (3.1 hal portal primitive, lbk.6), 8393568
  (3.2 mc shim, lbk.7), 8cde7aa (3.3a design D9 recorded, lbk.13),
  bf27100 (4.1 CLI detail — landed before 3.3, display-only so no
  dependency inversion; a finding calling the ordering a defect is a
  false positive), 0513707 (3.3 convergence dispatch, lbk.12), 63b5f2d
  (5.1 Suite A generated, lbk.9), 1ba7038 (3.2fix root-portal routing,
  lbk.14), 22ef093 (5.2 sitting record, lbk.10), f27900a (6.1 docs
  close-out, lbk.11), ae23721 (bead seal).
- Commit 1c2b337 carries the `One-Writer-Exempt` trailer — the ADR-0016
  §4 recorded escape for the atomic `Attributes::Dpseci` cross-crate
  ripple. Commit hygiene context only, not a defect hunt.
- Major new surface (current sizes): `models/families/dpseci.qnt` 535
  (the cfg-only P2 reference: queue pairs 1..16, length-coupled
  priorities each 1..8, closed options vocabulary with provenance-
  carrying raw escape, restool-layer refusal surface replaying
  V-DPSECI-1, DPSECI-I1 by construction, DPSECI-I4 as birth
  capability); `crates/dpaa2-api/src/families/dpseci.rs` 547 (new,
  isomorphic typestates + refined newtypes + `compile_fail`
  immutability doctest), `plan/dpseci.rs` 342 (new: observable-subset
  classify + signature-multiset census delta, the D9 pure core),
  `plan/populate.rs` +64 (`PlannedChildDpseci` gather/dispatch arms),
  `contract/mc.rs` +96 and `contract/fake.rs` +145 (observe/destroy
  seam with FakeBackend dpseci state), `intent/{compiled,derive}.rs`
  (priorities `[2; num_queues]`, options `{HAS_CG}`, rule provenance);
  `crates/dpaa2-hal/src/portal.rs` 530 (the unsafe debut: closed
  command sum OPEN/GET_ATTR/GET_API_VERSION/DPSECI_GET_TX_QUEUE/CLOSE,
  64-byte portal encode/decode, three typed outcomes, layouts asserted
  against the pinned `fsl_dpseci.h`/`fsl-mc-uapi.c` constants, fixture
  tests); `crates/dpaa2-mc` restool.rs +572 (create/destroy dispatch
  with computed options mask, presence read-back on destroy, GET_ATTR
  read path over the hal primitive riding the shim's ROOT node, typed
  Unobservable outcome), populate.rs +391 (census dispatch), parse.rs
  +87 (`info` parse with NO options field — the wrong observable does
  not typecheck) + `fixtures/dpseci_info.txt`; `crates/dpaa2-tools`
  status.rs +54 / render.rs +92 (detail row: queues, priorities,
  observed options, API version, binding state, unknown rendered
  honestly) + `tests/dpseci_detail.rs` 84 (hooks-never-gate engine
  proof) and `tests/vdpseci3_intents.rs` 78 (offline operand pin);
  `crates/dpaa2-verify/src/intent/dpseci_itf.rs` 156 +
  `tests/dpseci_replay.rs` 269 (conformance twins); 8 frozen traces
  under `models/traces/families/dpseci/`; board Suite A
  (`models/board/V-DPSECI-3/`: 316-line script + intent-a.toml),
  VERDICTS.json +13. Docs: designs D1–D8 at propose + D9 recorded
  mid-epic (8cde7aa — the recorded exception to no-mid-epic-design-
  amendments, bead lbk.13), ADR-0021 new (userspace MC reads ride the
  kernel whitelist; the read slice precedes #10; #10 owns all growth
  of the command vocabulary), ADR-0019 +6 (P2 member landed),
  `docs/baseline/dpseci.md` +20 (unknowns #2/#3 answered, read-slice
  observability note), COVERAGE +14 (DPSECI-I1/I4 deferred→modeled,
  I3 implemented-at-adapter, I2/I5/I9 re-anchored loud with fences
  named), ROADMAP row #8. CHANGELOG untouched per explicit user call —
  a finding demanding a CHANGELOG edit is a false positive.
- **Board outcomes are recorded facts** — findings that contradict them
  are false positives:
  1. **V-DPSECI-3 rev 1** (2026-10-03, binary at 63b5f2d) ran the e2e
     clean — converge, population exact, idempotent second ensure,
     typed teardown, census clean, dmesg quiet — but the scratch
     dpseci's portal face typed no-observable (`open /dev/dprc.2
     ENOENT`): observe_dpseci had routed GET_ATTR over the VFIO
     child's own container node, which a VFIO-bound child never
     exposes. Triaged implementation-first, fixed at 1ba7038 (lbk.14):
     the observe read rides the shim's root node (`root_portal()`,
     `DprcId::ROOT` fallback; MC DPSECI_OPEN is id-addressed). The
     recorded deviation: `read_dpseci_over_portal`'s open-context
     widened to carry the node path tried — the only observable that
     distinguishes root-vs-child routing without /dev access. A
     finding calling the rev-1 ENOENT a live defect, or the
     open-context widening scope creep, is a false positive.
  2. **V-DPSECI-3 rev 2 passed 8/8, hook 2/2** (binary at 1ba7038):
     portal face reads `options=[HasCg] version=5.4`; V-DPSECI-2 rev 1
     witnessed as the dual-transport read-back (restool
     queues=2/priorities=2,2 + portal options/version — DPSECI-I3
     live). **Recorded landing deviation**: V-DPSECI-2 lives as a
     Suite A hook (board README :132) inside the V-DPSECI-3-rev2
     VERDICTS.json row (`hook: 2/2`), not as its own VERDICTS key —
     the epic close reason and README row both record this; a finding
     demanding a separate VERDICTS.json entry re-litigates a recorded
     call, but Pass 4 verifies the three records (tasks.md 5.2 wording,
     README :108/:132, VERDICTS.json) tell one consistent story.
  3. **Boot-object hooks banked, both revs consistent**: boot dpseci =
     dpseci.0 under driver dpaa2_caam, GET_API_VERSION = 5.4 (baseline
     unknown #2 confirmed), boot dpseci GET_ATTR options=[] (unknown
     #3 answered: the kernel dpseci does NOT carry HAS_CG), 16 queues
     all-1 priorities. The scratch dpseci staying kernel-unbound is
     the V-LIFE-DPSECI-1 rev 2 expected shape, cited not re-run.
     Non-gating UnknownCeiling {Dprc, Dpio, Dpseci} warnings and the
     grow-only dpio seat residue (reboot per run-LAST) are recorded,
     never gating.
- **Deliberate decisions findings must not "fix"** (each recorded in
  design D1–D9 or the bead trail):
  1. D1: the model is cfg-only P2 plus consumer-facing consequences —
     no consumer steering machine; those states would demand Rust
     types nothing can inhabit. A finding demanding runtime-state
     modeling contradicts D1 and the ADR-0019 P2 rationale.
  2. D2/ADR-0021: the portal primitive's command vocabulary is a
     closed sum over exactly the five whitelisted reads; a write id is
     unrepresentable. hal stays policy-free (encode, ioctl, decode,
     three typed outcomes); retry/tolerance/mapping live in dpaa2-mc.
     The unsafe confinement (ioctl call + struct transmutes, layouts
     asserted, fixture-tested) is the recorded debut shape; #10 owns
     all vocabulary growth. A finding proposing reads beyond the
     whitelist, a write probe, or hal-side policy contradicts D2.
  3. D3: priorities derive as the constant `[2; num_queues]` (verified
     deployed profile); no intent knob exists because baseline unknown
     #4 means no board reading can discriminate — the typed range
     stays 1..8 so the kernel's all-1 reads back Known. A finding
     demanding a priority knob or a different constant contradicts D3.
  4. D4: derivation emits `HAS_CG` only; `HAS_OPR,OPR_SHARED` are
     deliberately not derived (unknown #6); the vocabulary observes
     all three verified bits plus the raw escape so unknown firmware
     bits are attributed, never silently merged.
  5. D5: options drift is judged from GET_ATTR only; the dpseci `info`
     parse type carries no options field at all. Portal-read
     unavailability is a typed Unobservable — absence of evidence,
     never drift. Cfg mismatch on an existing object is immutable-cfg
     repair (destroy+create disruption class); no setter exists.
  6. D6: one suite, one sitting, two verdicts; the production VPP
     child container is never touched; the boot dpseci is read, never
     unbound. The VFIO RemoteOwned leg reads back as RECORDED
     findings, not PASS/FAIL (the V-DPMAC-3 precedent).
  7. D9: convergence is a multiset census on the observable signature
     `(num_queues, options)` per container — matching by
     ordinal-derived labels is REJECTED (ADR-0015 decision 5 +
     V-LIFE-DPSECI-1 makes a spurious recreate permanently degrading);
     priorities are NOT in the observable subset (no read-back on any
     userspace transport; adapters report, never judge); the future
     named-table path is recorded not taken. A finding proposing
     per-object identity, label matching, or priority drift judgment
     contradicts D9.
  8. Banked verdicts V-LIFE-DPSECI-1 rev 2, V-DPSECI-1 rev 1 are
     cited, never re-run; re-running board suites is out of scope.
  9. All protected decisions of prior reviews (dpmac-typestate D1–D7,
     dpni, pool, dprc lineage) bind this review exactly as before.
- Six `ponytail:` markers exist in the repo (fake.rs:689,
  compile_props.rs:168, fitcheck.rs:112, model.rs:552, observed.qnt:30,
  invariants.qnt:105) — the same pre-existing set the dpmac briefs
  carried (fake.rs's line shifted). Pass 1 confirms none went stale
  through epic-touched lines and the epic added none that hide an
  unmarked ceiling.
- Frozen traces (8): congestionBirthWithCgTest,
  congestionBirthWithoutCgTest, createAcceptedReadbackTest,
  createRawEscapeReadbackTest, priorityZeroRefusedTest,
  priorityAboveEightRefusedTest, priorityCountMismatchRefusedTest,
  queueCountOutOfRangeRefusedTest — all under
  `models/traces/families/dpseci/`, replayed by `dpseci_replay.rs`.
  Board evidence under `models/board/` is operator-sealed.
- Out of scope: everything the prior epic reviews already judged that
  this change did not touch; every MC write over the ioctl path, the
  differential gate, per-family migration, MC-layer create validation
  (unknown #1), and the DPSECI-I5 post-unbind board face — all #10;
  consumer-owned runtime state (rx steering, congestion thresholds,
  enable/disable — vpp-dpaa2-support ADRs 0005/0007/0008, DPSECI-I6);
  the priority knob / OPR derivation / SEC counter reads / DPL (#14);
  dpdmux (#12), tier-c (#13); the repo-wide anchored-refs `--tree`
  debt (carried bead, pre-existing).

## Passes

### Pass 1 — Residue, deferrals, and verify re-run
Agent: Explore (medium). Budget ~50k. Runs first; feeds the other three.
Re-run every task-level verify obligation from tasks.md that is
checkable offline: the ledger lint green (1.2 acceptance), the
`compile_fail` immutability doctest in families/dpseci.rs, the
derivation tests (priorities `[2; num_queues]`, options `{HAS_CG}`,
provenance), the ITF replay suite (`dpseci_replay.rs` — every frozen
trace replayed, none orphaned, and no trace exists without a replay),
the `dpseci_detail.rs` hooks-never-gate proof, the `vdpseci3_intents.rs`
operand pin, the portal fixture tests, and the lbk.14 root-routing pin
test (sentinel root dprc.64998). Verify the deferral promises point at
their carriers in BOTH `models/COVERAGE.md` and
`docs/baseline/dpseci.md`: DPSECI-I2 MC layer → #10 (CREATE
unreachable from the read slice by construction), I5 board face → #10
with the whitelist fence stated, I9 → the block-global counter law row,
unknowns #1/#5–#8 re-anchored with fences named. Sweep the epic-touched
files (not out-of-scope surfaces) for leftover scaffolding: `todo!`,
`unimplemented!`, `dbg!`, `#[allow(dead_code)]`, `#[ignore]`,
commented-out code, stale wording predating the board outcomes (an
unknown-#2/#3-still-open claim, a child-node portal-routing description
predating 1ba7038, a V-DPSECI-2-as-separate-verdict claim predating the
hook landing). Confirm the six `ponytail:` markers are all pre-existing
and none sits stale inside an epic-touched region; flag any new
unmarked deliberate simplification the epic introduced.

### Pass 2 — Isomorphism and the census law
Agent: software-architect. Budget ~110k. After Pass 1; parallel with 3, 4.
Scope: `models/families/dpseci.qnt`, `models/intent/derive.qnt` (+21),
`crates/dpaa2-api/src/families/dpseci.rs`, `plan/dpseci.rs`,
`plan/populate.rs`, `intent/{compiled,derive}.rs`,
`crates/dpaa2-verify/src/intent/dpseci_itf.rs`, `tests/dpseci_replay.rs`.
Four mandates:
(a) **P2 isomorphism (ADR-0002 law)**: the Quint cfg block (queue pairs
1..16, length-coupled priorities 1..8 each, closed options vocabulary
with raw escape, the V-DPSECI-1 refusal surface, I4 birth capability)
vs `families/dpseci.rs` — law-for-law, guard-for-guard,
refusal-for-refusal; names converge on the same readable English;
DPSECI-I1 holds by construction on both sides (no mutating verb
representable). The refined newtypes must make the coupled-length
violation and the out-of-range priority unrepresentable, not checked.
(b) **The census is lawful two-sided**: the D9 signature-multiset
delta and the observable-subset classify in `plan/dpseci.rs` ↔ their
Quint twins — create-missing/destroy-surplus/mismatch-as-
destroy+create, with the observable subset exactly (num_queues,
options) and priorities excluded; no per-object identity minted
anywhere on the path; `classify_cfg_mismatch` (full cfg) stays the
desired-vs-desired law and the two never swap roles. The
two-blocks-per-tenant case must converge in tests at both layers.
(c) **ITF replay coverage**: map the 8 frozen traces onto the model's
guarded transitions; name every guard/refusal arm with no replayed
trace; check the property twins (length-coupling, range judgments)
actually bind the Rust-side refusals to the model's.
(d) **Unobservability and the escape are typed, not stringly**: the
Unobservable-this-run outcome (D5) and the provenance-carrying raw
options escape hold as sum types end to end; absence of evidence never
reaches the census as drift; an unknown firmware bit is attributed,
never merged; no sentinel or string compare stands in for either.

### Pass 3 — Architecture, the unsafe debut, and code quality
Agent: software-architect. Budget ~100k. After Pass 1; parallel with 2, 4.
Scope: `crates/dpaa2-hal/src/portal.rs` (+ lib.rs, Cargo.toml delta),
`crates/dpaa2-mc/src/{restool,parse,populate}.rs` + `tests/shim.rs` +
fixtures, `crates/dpaa2-api/src/contract/{mc,fake}.rs`,
`crates/dpaa2-tools/src/{status,render,main}.rs` +
`tests/{dpseci_detail,vdpseci3_intents}.rs`. Four mandates:
(a) **The fence is structural**: the portal command sum encodes exactly
the five whitelisted reads and nothing else is representable — check
the encode path cannot be fed an arbitrary command id, the unsafe is
confined to the ioctl call and the transmutes, every layout assertion
cites its pinned source constant, a decode mismatch types as an
observation failure (never a panic or UB path), and the three outcomes
(response / MC status ≠ OK / transport refusal) are honestly disjoint.
This is the workspace's first unsafe — judge it as the precedent #10
inherits.
(b) **Sans-io discipline**: hal stays policy-free (no retry, tolerance,
or judgment in portal.rs); all policy (root-node routing law, error
mapping, Unobservable typing) lives in dpaa2-mc; any options judgment,
census decision, or signature computation living in dpaa2-mc or
dpaa2-tools instead of the dpaa2-api core is a finding; status/render
stay display composition only. The lbk.14 routing law (root_portal(),
DprcId::ROOT fallback) is shim policy — verify it sits in restool.rs,
touches no trait seam, and the FakeBackend contract stayed
byte-for-byte as the close reason claims.
(c) **Observation stays display-only end-to-end**: no detail-row field
gates convergence anywhere — `dpseci_detail.rs` proves the engine
seam, but sweep for side doors (a portal read gating a plan step, an
API-version check before actuation). The census consumes the typed
observation seam only.
(d) **Reuse-before-write and seam quality for #10**: restool.rs +572
and populate.rs +391 — flag plumbing the existing runner/parse/census
seams already provided (the pool trio census precedent: did the
signature census reuse its delta shape or re-derive it?); the fixture
pattern vs existing shim tests; fake.rs +145 vs existing fake state
idioms. Does the portal primitive's shape let #10 land its write path
and differential gate additively (trait-seamed behind dpaa2-mc policy
exactly where the backend lands), or does it encode read-slice
assumptions #10 must rewrite?

### Pass 4 — Spec/docs alignment
Agent: spec-align. Budget ~80k. After Pass 1; parallel with 2, 3.
Scope: `openspec/changes/dpseci-typestate/{proposal,design,tasks}.md`,
the spec deltas under `specs/` vs shipped code; the COVERAGE
dispositions (1.2 deferred→modeled rungs with fences named, the 6.1
verified stamps citing the sitting) vs the witnesses they cite;
`docs/baseline/dpseci.md` (unknowns #2/#3 answers, the read-slice
observability note, the I5 routing row) vs the sitting record;
ROADMAP row #8; ADR-0021 vs the shipped primitive (the whitelist table
as single source, #10 owning vocabulary growth, atemporal per the
recorded style directive); the ADR-0019 P2-member amendment vs the
code that cites it; the D9 design record (8cde7aa) vs `plan/dpseci.rs`
as shipped and vs ADR-0015 decisions 4/5; the board README :108 Suite
A row + :132 V-DPSECI-2 hook row + VERDICTS.json V-DPSECI-3-rev2 vs
tasks.md 5.2's "V-DPSECI-2 rev 1 and V-DPSECI-3 rev 1 into
VERDICTS.json and the suite ledger" — verify the three records tell
one consistent story and the rev-1→rev-2 divergence trail (lbk.14) is
recorded where a reader would look. Verify the modified-capability
claims in proposal.md are each true as shipped. Verify no doc still
treats unknowns #2/#3 as open and none claims a capability the
whitelist fence denies (an MC write, a congestion read, SEC counters).
Disposition Pass 1's doc-side hits.

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

Deduplicate and rank findings across passes (Passes 1 and 4 will both
find doc-side hits; 2 and 3 may both touch the observation path); drop
unanchored staleness claims and any finding that "fixes" a protected
decision (the nine grounding items above, the three board outcomes, and
everything prior reviews protected); deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. A verdict on the change's core promise: is the crypto interface a
   typed, observed surface — the cfg complete enough that the shim
   creates one, HAS_CG witnessed through the fenced read slice with
   unobservability typed, the anonymous population converged as a
   signature multiset census, and the board e2e witnessed dual-
   transport — with the laws on both sides of the sans-io seam, yes or
   no, evidence rows.
3. A verdict on ADR-0002/ADR-0021 compliance: is the shipped family
   structurally isomorphic to `dpseci.qnt`, and is the unsafe debut
   actually fenced the way ADR-0021 claims (write unrepresentable,
   whitelist as single source), yes or no, evidence rows.
4. Proposed follow-up work as bead-shaped items (title + why +
   acceptance) for a just-in-time openspec change — the review itself
   changes nothing.
No guidelines deliverable. At most nominate a rule amendment if a
finding shows an existing rule failed to prevent a defect (candidate:
the rev-1 portal-routing miss shipped through offline gates and was
caught only on the board — could a fixture or contract test have
represented the VFIO-child-has-no-node fact before the sitting?).

## Budgets

P1 50k · P2 110k · P3 100k · P4 80k · synthesis 70k ≈ 410k total across
4 pass agents + judge. A pass exceeding ~1.5x its budget stops and
reports partial.
