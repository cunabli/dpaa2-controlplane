# Review brief: dpmac-typestate epic

Slug: `dpmac-typestate`. Reviewed span: commits e2847ee..81f7b86 on main
(1 spec-init + 13 execution commits, every one carrying a
`Change: dpmac-typestate` trailer; one interleaved process commit b6782ed
carries `Change: adr-0016` — see grounding). Epic dpaa2-controlplane-0xu,
closed 2026-10-03 at task 6.1 (81f7b86, docs close-out). Net epic size
excluding openspec/ artifacts and frozen traces: ~3,711 inserted / ~48
deleted lines across 47 files in crates/, models/, docs/ — roughly 80% of
dpni-typestate, so the same four-pass shape with dpni-scale budgets.
Quality floor (cargo build/fmt/clippy/clippy --tests/doc) is green per the
6.1 DoD; mechanical lint findings are NOT the target. Targets: the change's
core promise (the physical port is a typed, observed surface — the P4
boot-born-offer arbitration phases, the firmware-versioned counter
vocabulary where absence ≠ zero is unrepresentable, the typed MAC relation
judgment, and the dpni–dpmac teardown order held by type as a
severed-witness edge law with no driverless interval), structural
isomorphism of `dpmac.qnt` + the `connect.qnt` edge facet vs the Rust
family and planner (ADR-0002 law), display-only observation discipline
(no read-only hook gates convergence), sans-io discipline across the grown
shim and the one hal primitive, ITF replay coverage, and doc/spec
alignment including the 1.3/6.1 COVERAGE dispositions and the ADR-0019
P4/edge-facet amendment.

## Grounding facts (verified 2026-10-03, re-verify only if main moved)

- Commit range: `e2847ee` (spec-init) → `81f7b86` (6.1 docs seal). Order:
  model (d947541 P4 shape, 8c94d84 edge law), COVERAGE sync (409537f),
  Rust core (f4782bf family, c4d95d0 edge type + planner, 42e5d34 twins),
  adapters (c8d0c2b mc shim, c2d61af hal carrier), product (71c2646
  port-detail view), board (537b7be Suite A, 852f697 Suite B, 50ae32f
  sitting record), close-out (81f7b86). The interleaved `b6782ed`
  (`Change: adr-0016`, ADR-0016 §4 one-writer-exempt trailer) is process
  work that rode the span — a finding calling it an untagged dpmac commit
  is a false positive; whether `families/dpmac.rs`'s API ripples justified
  the exemption it introduced is in scope for Pass 3 only as commit
  hygiene context, not as a defect hunt.
- Major new surface (current sizes): `models/families/dpmac.qnt` 612
  (+590: the `dpmac_lifecycle` P4 reference — Offered/KernelOwned/
  RemoteOwned phase sum with observation-judged transitions, directional
  link channels with requests-down typed `Unreadable` by construct,
  firmware-indexed counter vocabulary with 10.39's 28 rows and
  `COUNTERS_1040_EXT` named-unread, MAC immutability anchored to
  BOOT_MAC), `models/core/connect.qnt` +15 (`edgeDemandsSeveredWitness`,
  the dpni–dpmac edge facet), `crates/dpaa2-api/src/families/dpmac.rs`
  731 (new), `plan/transition.rs` +99 and `plan/reconcile.rs` +30 (the
  planner consumes the severed-witness type), `contract/{mc,kernel,
  fake}.rs` read extensions, `crates/dpaa2-hal/src/sysfs.rs` +94 (the one
  policy-free carrier primitive: dpni netdev for KernelOwned, macN
  otherwise, NoObservable for driverless), `crates/dpaa2-mc` kernel.rs
  +52 / parse.rs +63 / restool.rs +122 with two captured fixtures and
  shim.rs tests +188, `crates/dpaa2-tools` status.rs +83 / render.rs +81
  (port-detail view) with `tests/port_detail.rs` 116 (the hooks-never-
  gate engine proof) and `tests/vdpmac3_intents.rs` 85 (offline operand
  pin), `crates/dpaa2-verify/src/intent/dpmac_itf.rs` 202 +
  `tests/dpmac_replay.rs` 422 (conformance twins), board suites
  V-DPMAC-3 (Suite A: 320-line script + intent-a.toml) and V-DPMAC-2
  (Suite B: probes.json) under `models/board/`, VERDICTS.json +28. Docs:
  designs D1–D7 (no mid-epic design amendments), ADR-0019 amendment (P4
  reference landed; phase-marker promotion trigger fired; edge
  teardown-law facet), `docs/baseline/dpmac.md` (unknown #1 answered,
  carrier observability), COVERAGE rows DPMAC-I2/I3/I4/I6/I7 + DPNI-I3,
  `docs/upstream/findings.md` +24 and `phylink-dpni-churn-crash.md` +15
  (finding 52), ROADMAP row #7.
- **Board outcomes are recorded facts** — findings that contradict them
  are false positives:
  1. **V-DPMAC-3 (Suite A) rev 1 passed 8/8** 2026-10-02: one ensure
     converged both regimes hitless (kernel dpni.1/eth0 on dpmac.7,
     child dprc.2 VFIO on dpmac.5), every hook conformed, the typed
     sever-then-unbind teardown left dpmac.7 standalone-bound with no
     driverless interval, the ADR-0020 grow-only residue was loudly
     declared and reclaimed by the closing reboot.
  2. **V-DPMAC-2 (Suite B) rev 1 settled baseline unknown #1**: the MC
     refuses a dpmac create for a DPC-absent mac-id with Invalid state
     (status 0xc) — creation is DPC-gated; the contingency legs never
     ran and the root face is recorded untaken. A finding proposing to
     exercise the contingency legs anyway contradicts the recorded
     outcome.
  3. **Finding 52**: the teardown window produced a detached-timer WARN
     in `__run_timers` (LIST_POISON2), milder than finding 49,
     mechanically unattributed — recorded as its own upstream finding
     with a second-signature note in `phylink-dpni-churn-crash.md`. It
     is upstream kernel work, not a gap this review reopens.
- Deliberate decisions findings must not "fix": the P4 reference is
  single-family with no shared offer substrate — extraction waits for a
  second P4 family (design D1, the ADR-0019 pattern-ledger judgment);
  arbitration is phase markers judged from observation, not a cfg facet
  (D2); the teardown law lives on the edge kind, not the family — other
  edge kinds carry no new law (D3); the counter vocabulary is
  firmware-version-indexed, `Known | NotInVocabulary` with absence ≠
  zero unrepresentable, the 10.40 extension named but unread, and the
  28-of-62 row count is the only observable of the silent refusal (D4,
  V-DPMAC-1 banked); the MAC relation is a typed judgment
  (Inherited/Overridden/Pending/Mismatched) with Pending explicitly not
  drift and no new verb (D5); link observation is sysfs-only — the
  MC-view dpni link read, `dpmac_get_link_cfg` + `SET_LINK_STATE`, and
  bulk `get_statistics` are named deferrals on `mc-portal-backend` #10
  as restool-absence ledger rows (D6, COVERAGE DPMAC-I4/I7); link and
  counters are display-only and never gate convergence (proof:
  `port_detail.rs`); the RemoteOwned leg reads back as RECORDED
  findings, not PASS/FAIL — RemoteOwned is not derivable from
  root-scoped observation (task 4.1); the dead-spawn `assert(false)`
  restool hazard is a typed observation failure the adapter never
  inherits (task 3.1 risk row); DPRTC-I4 is re-anchored off #7 to #13
  (lifecycle-foreign); banked verdicts V-LINK-2/V-LINK-4/V-DPMAC-1 are
  cited, never re-run; CHANGELOG rides commits (cliff), no file edit
  expected.
- Six `ponytail:` markers exist in the repo (fake.rs:607, model.rs:552,
  compile_props.rs:168, fitcheck.rs:112, observed.qnt:30,
  invariants.qnt:105) — all six predate this epic (the pool-objects
  brief listed the same set; fake.rs's shifted line is the same marker).
  Pass 1 confirms none went stale through epic-touched lines and the
  epic added none that hide an unmarked ceiling.
- Frozen traces live under `models/traces/families/dpmac/`; they replay
  through `tests/dpmac_replay.rs`. Board evidence under `models/board/`
  is operator-sealed — re-running board suites is out of scope.
- Out of scope: everything the intent-layer, vocabulary-v2,
  dprc-encapsulation, dpni-typestate, and pool-objects reviews already
  judged that this change did not touch; portal/ioctl work (#10,
  including every named restool-absence deferral); dpseci (#8), traffic
  and cross-dprc links (#9), dpdmux (#12), tier-c families (#13);
  `set_protocol`/`set_params`/MDIO (no intent need, proposal
  out-of-scope); kernel-side fixes (findings 49 and 52 are recorded
  upstream work); the repo-wide anchored-refs `--tree` debt (carried
  bead, pre-existing).

## Passes

### Pass 1 — Residue, deferrals, and verify re-run
Agent: Explore (medium). Budget ~50k. Runs first; feeds the other three.
Re-run every task-level verify obligation from tasks.md that is checkable
offline (typecheck/test names cited, grep-able assertions; report any
that no longer hold — e.g. the negative face "unbind-before-sever does
not typecheck" in `compile_props.rs` or wherever task 2.2 landed it, the
`port_detail.rs` hooks-never-gate proof, the `vdpmac3_intents.rs` operand
pin, the replay suites). Verify the deferral promises point at their
carriers: the #10 routing for the requests-down channel, the MC-view
link read, and bulk statistics as restool-absence ledger rows; the #13
routing for DPRTC-I4 — in `models/COVERAGE.md` AND `docs/baseline/
dpmac.md`. Sweep the epic-touched files (not out-of-scope surfaces) for
leftover scaffolding: `todo!`, `unimplemented!`, `dbg!`,
`#[allow(dead_code)]`, `#[ignore]`, commented-out code, stale wording
predating the board outcomes (a pending-unknown-#1 claim, a
phantom-create-semantics-unknown claim outside the recorded answer, a
driverless-interval hazard stated as open), and orphaned fixtures.
Verify every frozen dpmac trace is replayed by `dpmac_replay.rs` and
none is orphaned. Confirm the six `ponytail:` markers are all
pre-existing and none sits stale inside an epic-touched region; flag any
new unmarked deliberate simplification the epic introduced.

### Pass 2 — Isomorphism and the port law
Agent: software-architect. Budget ~110k. After Pass 1; parallel with 3, 4.
Scope: `models/families/dpmac.qnt`, `models/core/connect.qnt` (edge
facet), `crates/dpaa2-api/src/families/dpmac.rs`,
`plan/{transition,reconcile}.rs`, `crates/dpaa2-verify/src/intent/
dpmac_itf.rs`, `tests/dpmac_replay.rs`. Four mandates:
(a) **P4 isomorphism (ADR-0002 law, ADR-0019 pattern)**: the Quint
`dpmac_lifecycle` machine (arbitration phase sum with observation-judged
transitions, MAC immutability anchored to BOOT_MAC, attribute constancy
with the eth_if/IPG exceptions quantified away, the firmware-indexed
vocabulary, the directional link channels) vs `families/dpmac.rs` —
law-for-law, guard-for-guard, refusal-for-refusal; names converge on the
same readable English. The requests-down channel must be unreadable by
construct on both sides (one payload-free constructor in the model; no
Rust path can produce a read).
(b) **The edge law lands two-sided**: `edgeDemandsSeveredWitness` +
`DPMAC_SeverOrder` in the model ↔ the severed-witness type the planner
consumes in `plan/transition.rs`; sever consumes KernelOwned and yields
Offered plus the witness; unbind demands the witness for dpmac-facing
edges ONLY — a law leaking onto other edge kinds is a finding, and so is
a one-sided law. The negative face (unbind-before-sever refused) must
hold as the same predicate in Quint (`hazardUnbindBeforeSeverRefusedTest`)
and Rust (the compile-time or typed-refusal assertion from task 2.2).
(c) **ITF replay coverage**: map the frozen traces onto the model's
guarded transitions; name every guard/refusal arm with no replayed trace
(COVERAGE's DPMAC rows cite the directed runs — `arbitrationLawTest`,
`macAddressImmutableTest`, `attributeConstancyTest`,
`counterVocabularyFirmwareIndexedTest`, `firmwareExtensionReadableAt1040Test`,
`severThenUnbindTest`, `severOrderLawTest`, `hazardUnbindBeforeSeverRefusedTest`,
`linkStateUpReadableTest` — how many arms does each replay actually
drive in `dpmac_replay.rs`?).
(d) **The judgments are typed, not stringly**: the MAC relation
(Inherited/Overridden/Pending/Mismatched, Pending never drift) and the
counter cell (`Known | NotInVocabulary`, absence ≠ zero) hold as sum
types on both sides, with the drift exclusion for Pending structural in
the reconciler, not a string compare or a sentinel zero anywhere on the
path.

### Pass 3 — Architecture and code quality
Agent: software-architect. Budget ~90k. After Pass 1; parallel with 2, 4.
Scope: `crates/dpaa2-mc/src/{kernel,parse,restool}.rs` + `tests/shim.rs`
+ fixtures, `crates/dpaa2-hal/src/sysfs.rs`,
`crates/dpaa2-api/src/contract/{mc,kernel,fake}.rs`,
`crates/dpaa2-tools/src/{status,render,main}.rs` +
`tests/{port_detail,vdpmac3_intents}.rs`, `crates/dpaa2-verify/src/board/
replay.rs`. Four mandates:
(a) **Sans-io discipline**: the shim reads (one spawn per dpmac,
vocabulary-checked parse, the deviating row count as a typed
version-signal, the dead-spawn hazard as a typed observation failure)
must be thin plumbing — any vocabulary judgment, arbitration inference,
or MAC-relation decision living in `dpaa2-mc` or `dpaa2-tools` instead
of `families/dpmac.rs` is a finding; the hal carrier primitive stays
policy-free (resolution rule in, no retry/tolerance/judgment); status/
render are imperative shell — display composition only.
(b) **Observation stays display-only end-to-end**: no port-detail field
(arbitration, MAC relation, carrier, counters) feeds a convergence gate
anywhere — `port_detail.rs` proves the engine seam, but sweep for side
doors (a carrier check before actuation, a counter read gating a plan
step). The severed-witness is the ONE typed demand the planner makes of
this surface; anything else it consumes from dpmac observation is a
finding against D7's display-only line.
(c) **Reuse-before-write**: restool.rs +122 and parse.rs +63 — flag
plumbing the existing runner/parse seams already provided; the fixture
pattern vs existing shim tests; fake.rs +78 vs existing fixture helpers;
the sysfs primitive vs dpaa2-hal's existing faces (driver-link,
eth-unbind — does the carrier read share their idiom?). Judged against
protected deliberate duplication (ADR-0014 lockstep twins).
(d) **Seam quality for #8/#10/#12**: does the P4 family pre-shape dpseci
and the later tier-c families, or encode dpmac-specific assumptions the
next P4 family must rewrite (the ADR-0019 promotion trigger fired — is
the recorded extraction path honest about what moves)? Do the
restool-absence deferral seams (vocabulary named-unread, channel typed
Unreadable) let #10 land additively as COVERAGE promises (one added
`McControl` method, one additive type change)?

### Pass 4 — Spec/docs alignment
Agent: spec-align. Budget ~80k. After Pass 1; parallel with 2, 3.
Scope: `openspec/changes/dpmac-typestate/{proposal,design,tasks}.md`, the
six spec deltas under `specs/` (formal-models, mbt-harness, mc-backend,
provisioning-cli, reconciler, system-integration) vs shipped code; the
COVERAGE dispositions (1.3 deferred→modeled rungs, the 6.1 verified
stamps citing V-DPMAC-3/V-DPMAC-2 rev 1) vs the witnesses they cite;
`docs/baseline/dpmac.md` (unknown #1's answer, carrier observability)
vs the sitting record; ROADMAP row #7; the ADR-0019 amendment (P4
reference landed, phase-marker promotion trigger fired, edge
teardown-law facet) vs the code that cites it; the finding-52 record
(findings.md + the phylink doc's second signature) vs the 50ae32f
commit account; VERDICTS.json + the board README rows for V-DPMAC-2/3.
Verify the out-of-scope re-anchors are recorded loudly where the
proposal promised (requests-down channel, MC-view link read, bulk
statistics → #10 restool-absence ledger rows; DPRTC-I4 → #13). Verify
ADR-0019's amendment is atemporal per the recorded style directive.
Verify no doc still treats baseline unknown #1 or the phantom-create
semantics as open, and none claims a capability the D6 sysfs-only route
cannot give (an MC-view link truth). Disposition Pass 1's category-(a)
doc hits.

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
doc-side hits; 2 and 3 may both touch the witness path); drop unanchored
staleness claims and any finding that "fixes" a protected decision (the
single-family P4 with no offer substrate, the edge-kind home of the
teardown law, the sysfs-only link route, the display-only hook line, the
Pending-not-drift judgment, the named-unread 10.40 vocabulary, the
RECORDED-not-gated RemoteOwned leg, the DPC-gated phantom answer, the
finding-52 upstream routing, the CHANGELOG-via-commits posture); deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. A verdict on the change's core promise: is the physical port a typed,
   observed surface — arbitration phases observation-judged, counter
   absence ≠ zero unrepresentable, MAC relation typed with Pending not
   drift, and the sever-then-unbind order held by type with no
   driverless interval — with the laws on both sides of the sans-io
   seam, yes or no, evidence rows.
3. A verdict on ADR-0002 compliance: is the shipped family structurally
   isomorphic to `dpmac.qnt` plus the `connect.qnt` edge facet, yes or
   no, evidence rows.
4. Proposed follow-up work as bead-shaped items (title + why + acceptance)
   for a just-in-time openspec change — the review itself changes nothing.
No guidelines deliverable: the 12-rule set stands; at most nominate a rule
amendment if a finding shows an existing rule failed to prevent a defect
(candidate: the one-writer exemption fired mid-epic on this change's API
ripple — does a rule fence atomic-API commit shaping before the hook has
to bend?).

## Budgets

P1 50k · P2 110k · P3 90k · P4 80k · synthesis 70k ≈ 400k total across
4 pass agents + judge. A pass exceeding ~1.5x its budget stops and reports
partial.
