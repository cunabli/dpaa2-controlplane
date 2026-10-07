# Review brief: cross-dprc-links epic

Slug: `cross-dprc-links`. Reviewed span: commits 904e531..215801e on main
(2 spec-init + 34 execution commits, every one carrying a
`Change: cross-dprc-links` trailer; no `One-Writer-Exempt` escapes).
Epic dpaa2-controlplane-kux, closed 2026-10-07 at the 215801e seal:
30/30 tasks across groups 1–7, including the two post-sitting close-out
tasks the operator added mid-close (7.5 edge-fate probe + model
weakening, 7.6 knob offline pin — commit eb6eb8d records the widening).
Net epic size excluding openspec/ artifacts, frozen traces, and board
records: ~5,900 inserted / ~166 deleted lines across ~60 files in
crates/, models/, docs/ — roughly 125% of dpseci-typestate, so the same
four-pass shape with slightly heavier Pass 3/4 budgets (the board trail
spans four V-TRAF-1 revs plus the V-LINK-6 discovery probe, and the doc
surface gained one ADR, three amendments, an ADR-0008 §10 extension,
and two upstream-finding documents). Quality floor (cargo build/fmt/
clippy/clippy --tests/doc) is green per the 7.4 close-out; mechanical
lint findings are NOT the target. Targets: the change's core promise
(a declared dpni↔dpni pseudo-wire converges end to end — the edge-kind
table with per-kind reification claiming the delivered dpmac machinery
unchanged, container-agnostic wire transitions keyed by ends, the
populate→connect→bind partial order on the container typestate, the
ADR-0017 healing obligations with the consented Disruptive discharge,
root-issued portal-ready connection verbs, frames witnessed EXACT +8 on
the board, and LINK-I1 hardware-anchored as a state law: the edge dies
with its endpoint), structural isomorphism of `link_lifecycle.qnt` vs
the Rust connection surface (ADR-0002 law), the bounded lift (D3: the
dpmac family's frozen traces replay green, unchanged), sans-io
discipline across the grown adapters and the consent-flow engine, ITF
replay coverage of the 13 frozen link traces, and doc/spec alignment
including ADR-0022, the three amendments, the 7.5 LINK-I1 weakening
trail, and the four-rev board story told consistently across tasks.md,
the board README ledger, and VERDICTS.json.

## Grounding facts (verified 2026-10-07, re-verify only if main moved)

- Commit order (all in task order; subjects abbreviated): 904e531 +
  4df869b (spec-init: proposal/design, then specs/tasks), af3b489 (1.1
  wire lifecycle model), 5e96f0b (1.2 container interplay), 09894eb
  (1.3 LINK-I* invariants + Apalache marks), 6c26986 (1.4 V-TRAF-1
  scenario module + frozen MBT traces), 6ba2041 (1.5 COVERAGE), 60d9120
  (2.1 ADR-0022), 1ede746 (2.2 three amendments), 510a474 (2.3 roadmap
  + dossier bead), aae55e0 (3.1 edge-kind table), a57a913 (3.2 wire
  transitions keyed by ends), 2436cca (3.3 container typestate partial
  order), 1e1b0c3 (3.4 obligations), 95dd345 (3.5 plan laws + knob
  type), d747410 (4.1 link ITF twins), 903922d (4.2 bounded-lift
  proof), 578de73 (4.3 declined-residue unit tests), e762dfa (5.1 MC
  connection verbs), bbe4b14 (5.2 peer resolution + root backstop),
  e2bb511 (5.3 consented heal plumbing), 38e71e4 (5.4 config knob),
  762fb49 (5.5 tools render/consent/status), 58f24f5 (6.1 suite
  render), 237a3b9 (6.2 witness + smoke hooks), 450ed66 (6.3 offline
  gates), 4c144ec (7.1 pre-run record), ec9411b (rev 2 regen: census
  competitor unplugged, hook provisioning sequenced), 76646ed (rev 3
  regen: bind_settle, cleanup ladder, probes banked), 7d72f40 (7.2
  V-TRAF-1 rev 3 pass), a0851d5 (7.3 baseline deltas), eb6eb8d
  (7.4–7.6 overnight record: the close-out widens), 27b99e2 (7.5
  LINK-I1 weakening, traces re-frozen), d20fbee (7.6 vdpcon1 provenance
  pin), 215801e (seal: spec deltas promoted). The two mid-sitting regen
  commits (ec9411b, 76646ed) are the recorded rev-divergence feedback
  loop, not defects; a finding calling the rev 1/rev 2 FAILs live
  defects is a false positive.
- Major new surface (current sizes): `models/families/link_lifecycle.qnt`
  600 (wire typestates ends-exist → connected → disconnected →
  end-destroy-legal consuming `core/connect.qnt`; D4 partial order;
  both D5 obligations with the consented discharge as the only modeled
  one; LINK-I* invariants; after 27b99e2 the destroy-of-connected-end
  transition is ENABLED per the V-LINK-6 answer and the refusal face is
  retired); `crates/dpaa2-api/src/plan/connect.rs` 1019 (new: edge-kind
  table with per-kind reification policy, wire transitions keyed by
  ends, typed refusal surfacing, obligation types — eager
  `DeferredVisibility` unconstructible without a planned discharge,
  lazy stale-node residue, declined-consent typed residue);
  `families/dprc.rs` 214 (new: container typestate partial order),
  `families/dpni.rs` +16, `intent/{compiled,derive,tenant}.rs`
  +70/+88/+81 (dpcon priority knob with provenance; single_sender
  refused as a knob), `contract/{mc,fake}.rs` +37/+43 (connection
  verbs seam + fake state); `crates/dpaa2-mc` populate.rs +258
  (generalized peer resolution, link census), restool.rs +41,
  parse.rs +41 (`dprc_get_connection` parse; `endpoint state: -1` =
  no-endpoint, distinct from `0` = down-but-connected — the V-LINK-6
  parser datum); `crates/dpaa2-config` parse.rs +86, schema.rs +9
  (the one knob; links gain no attributes — rates refused by
  omission); `crates/dpaa2-tools` engine.rs +959 (consent flow,
  obligation plumbing, link faces), render.rs +131, main.rs +53,
  `tests/link_faces.rs` 171 + 6 snapshots (dry-run provenance,
  obligation rows, honest-unknown link state), `tests/vdpcon1_intents.rs`
  81 (7.6 pin: `Some(p)` → exactly one provenance node, `None` →
  byte-identical derivation), `tests/dprc_convergence.rs` +22/−11;
  `crates/dpaa2-verify` `intent/link_itf.rs` 243 +
  `tests/link_replay.rs` 564 (conformance twins),
  `tests/vtraf1_suite.rs` 258, `board/generate.rs` +312 (suite + hook
  render). 13 frozen traces under `models/traces/families/
  link_lifecycle/` (wireLifecycle, freshPartialOrder, dpniPairLegal,
  bindBeforeConnect, childToChildWire, crossContainerWire,
  connectAlreadyConnectedRefused, reconnectAfterDisconnect,
  postBindCreateObligated, postBindDestroyStaleNode,
  consentedRebindDischarges, declinedConsentResidue,
  destroyConnectedEndRemovesEdge — the last REPLACING the retired
  destroyConnectedEndRefusedTest at 27b99e2). Board records:
  `models/board/V-TRAF-1/` (six faces + hook + plan + script),
  VERDICTS.json V-TRAF-1 revs 1–4 + V-LINK-6 rev 1, board README :109
  (V-TRAF-1 ledger row) + :110 (V-LINK-6 row). Docs: ADR-0022 new
  (146 — reification policy is a property of the edge kind), ADR-0017
  amended (PASS4-F8 discharged), ADR-0019 amended (edge facet second
  inhabitant), ADR-0003 §8 amended (Mellanox decision point dropped,
  trigger reworded), ADR-0008 +39 (§10 removal-burst rescan race),
  `docs/upstream/fsl-mc-rescan-race-evicts-standing-devices.md` 101
  (new) + `docs/upstream/findings.md` +26, `docs/baseline/dpni.md`
  +42/−4 and `dprc.md` +6/−2 (board answers), ROADMAP rows #9/#10,
  COVERAGE +10/−5. `dpaa2-hal` untouched (verified in span diffstat) —
  a finding demanding hal changes, or flagging their absence,
  contradicts D6. CHANGELOG untouched in span (cliff-managed, the
  standing pattern) — a finding demanding a CHANGELOG edit is a false
  positive. README +1 line only.
- **Board outcomes are recorded facts** — findings that contradict them
  are false positives:
  1. **V-TRAF-1 rev 1 FAIL, evidentiary** (2026-10-05, 32/34): the 24
     restool-surface steps of faces 1–3/5/6 conformed; face 4's
     KernelBind failed because kernel auto-probe stole the census via
     the deferred-probe queue; frame witness vacuous; refusal probes
     inconclusive (fixtures unasserted, MC id recycling). Teardown-era
     rescan race evicted standing dpni.0 (reboot recovery). **Rev 2
     FAIL, evidentiary** (33/35 + hook 18/19): the frame witness read
     EXACT +8 on all four counters and the 3×1000 saturation smoke ran
     clean — the wire law held; the two FAILs share one root cause
     (bind/unbind are asynchronous); **LINK-I1 falsified with the
     fixture asserted** (MC ACCEPTED destroy of still-connected
     dpni.12); dpni.0 evicted both revs, so the destroy-of-connected
     probe was quarantined from rev 3 as the discriminating experiment.
  2. **Rev 3 passed 35/35** (bind_settle poll + drivers_probe/dprc-sync
     kicks; cleanup ladder; probes banked; one benign 0x6). **Rev 4
     passed 35/35** with zero hook-object residue (the ladder's added
     allocator sysfs-unbind rung) — and rev 4 ran NO LINK-I1 probe yet
     still lost standing dpni.0 to the §10 race, refuting rev 3's
     probe attribution: the eviction is the probabilistic removal-burst
     rescan race (ADR-0008 §10, upstream doc), probe-independent.
     Pacing stays declined (suite near retirement; operators recover by
     reboot) — a recorded call, not a gap. The ADR-0011 mcp portal leak
     and the rev-3 hook-allocatable residue are recorded, never gating.
  3. **V-LINK-6 rev 1 passed 9/9** (the quarantined edge-fate probe,
     task 7.5): the MC ACCEPTS `dpni destroy` of a still-connected
     dpni↔dpni root end and auto-removes the edge atomically; the
     survivor reads `endpoint: No object associated`, state -1, link
     down, clean dmesg. LINK-I1's state face (no edge outlives its
     endpoints) is hardware-anchored; only the old refusal-guard claim
     was falsified. The model weakening (27b99e2), the retired refusal
     trace, and the re-frozen family traces are the recorded
     consequence — a finding asking for the refusal guard back, or
     calling the weakening a regression, is a false positive.
  4. Reboots in the sitting record are ADR-0003 operator recovery of
     board-wide residue, not a plan-surface transition — D5's "no
     reboot exists on this surface" is about the modeled discharge; a
     finding calling the recovery reboots a D5 violation is a false
     positive.
- **Deliberate decisions findings must not "fix"** (each recorded in
  design D1–D10, ADR-0022, or the bead trail):
  1. D1/D6: frame witness rides netns + ping in operator-reviewed hook
     scripts; zero Rust netlink, zero new dependencies, hal untouched;
     no VPP anywhere in suites or writings. A finding proposing a Rust
     netlink path, a VPP leg, or a hal transport contradicts D6.
  2. D2: the construct is container-agnostic; connect issues at a
     common ancestor (root constant `CONNECT_ANCESTOR` today); MC-
     refused patterns surface as typed refusals, nothing pre-forbidden
     except intent's `LinkSelfLoop`. A finding demanding model-side
     pre-forbidding of MC-refused shapes contradicts D2.
  3. D3: the lift is bounded — dpmac transitions, executor, and
     `SeveredProof` minting stay where they are, keyed as they are;
     the edge-kind table claims them (`edgeDemandsSeveredWitness`
     false for dpni↔dpni: disconnect-only teardown, no driver
     handback by construction). The acceptance proof is the unchanged
     dpmac frozen-trace replay. A finding demanding the dpmac
     machinery be retyped into the new table contradicts D3.
  4. D4: post-bind connect/disconnect of already-visible endpoints is
     additionally legal; the kernel-end `ENDPOINT_CHANGED` → `-EPERM`-
     discarded dmesg law is recorded, not treated as an error.
  5. D5: the only modeled discharge of `DeferredVisibility` is the
     consented Disruptive rebind cycle; declined consent is typed
     standing residue; the destroy-side stale node is the lazy mirror
     and may stand indefinitely. A finding demanding a silent rebind,
     an eager stale-node sweep, or a second discharge route (that is
     #10's open question) contradicts D5.
  6. D9: the dpcon priority knob is the ONE additive knob;
     SINGLE_SENDER deliberately has NO intent knob (consumer-typed,
     `Profile::Pmd`, dpni-typestate D3) — the wire-variant board face
     carries it; DPCON-I4 and dpni unknowns #4/#11 are assigned to
     #10 via the dossier bead named in ROADMAP row #10. A finding
     demanding a single_sender knob or in-#9 delivery of the three
     #10 rows contradicts D9.
  7. Task 7.6 (recorded at eb6eb8d, shipped at d20fbee): the dpcon
     priority knob emits ONLY a provenance node — no dpcon create
     operand exists and no restool read-back exists, so the board
     witness is unbuildable; the knob is pinned offline as a tripwire
     (`vdpcon1_intents.rs`), and DPCON-I3's consumer-at-priority>0
     caveat is re-anchored to its concrete trigger (hal QBMan portal +
     mc-portal-backend feeding the -verify consumer rig). A finding
     demanding a board read-back for the knob, or calling the
     provenance-only emission dead code, is a false positive.
  8. Rates/link attributes are refused by omission at the schema (no
     NXP script ever used them); V-DPCI-1's child-connect refusal is
     replayed from the bank, never re-run; no PHY dpmac appears in any
     face (finding 49 stays unpoked); re-running board suites is out
     of scope.
  9. All protected decisions of prior reviews (dpseci D1–D9, dpmac,
     dpni, pool, dprc lineage) bind this review exactly as before.
- Six `ponytail:` markers exist in the repo (fake.rs:716,
  compile_props.rs:168, fitcheck.rs:112, model.rs:552, observed.qnt:30,
  invariants.qnt:105) — the same pre-existing set the dpseci brief
  carried (fake.rs's line shifted). Pass 1 confirms none went stale
  through epic-touched lines and the epic added none that hide an
  unmarked ceiling.
- Out of scope: everything the prior epic reviews already judged that
  this change did not touch; the portal read slice and every MC
  command-vocabulary addition (ADR-0021: #10), the differential gate,
  DPCON-I4 and dpni unknowns #4/#11 (the #10 dossier bead), dpdmux
  (#12), tier-c (#13), the repo-wide anchored-refs `--tree` debt
  (carried bead, pre-existing); VPP integration (post-archive product
  use); netns/addressing dataplane config (outside the intent
  vocabulary, like MTU).

## Passes

### Pass 1 — Residue, deferrals, and verify re-run
Agent: Explore (medium). Budget ~50k. Runs first; feeds the other three.
Re-run every task-level verify obligation from tasks.md that is
checkable offline: the link ITF replay suite (`link_replay.rs` — all
13 frozen traces replayed, none orphaned, and no trace exists without
a replay; the retired `destroyConnectedEndRefusedTest` gone from both
traces and replays), the 4.2 bounded-lift proof (dpmac family frozen
traces replay green, byte-unchanged in the span), the 4.3 obligation/
partial-order/refusal unit tests, the `vdpcon1_intents.rs` 7.6 pin
(Some(p) → exactly one provenance node; None → byte-identical
derivation), the `link_faces.rs` snapshot suite (dry-run provenance,
obligation rows, honest-unknown link state), the `vtraf1_suite.rs`
render checks, the Quint gate (typecheck + simulate + marked Apalache
on `link_lifecycle.qnt`), and the COVERAGE ledger lint. Verify the
deferral promises point at their carriers: DPCON-I4 + dpni unknowns
#4/#11 → the dossier bead named in ROADMAP row #10 (and the bead
exists), DPCON-I3's caveat re-anchored to its concrete trigger, the
LINK-I* COVERAGE rows settled to the sitting's verdicts. Sweep the
epic-touched files (not out-of-scope surfaces) for leftover
scaffolding: `todo!`, `unimplemented!`, `dbg!`, `#[allow(dead_code)]`,
`#[ignore]`, commented-out code, and stale wording predating the late
board answers — a LINK-I1-as-refusal claim predating 27b99e2 and
V-LINK-6, a knob-board-witness claim predating eb6eb8d/d20fbee, a
probe-caused-eviction claim predating rev 4's refutation, a
single_sender-knob mention. Confirm the six `ponytail:` markers are
all pre-existing and none sits stale inside an epic-touched region;
flag any new unmarked deliberate simplification the epic introduced.

### Pass 2 — Isomorphism and the connection-surface law
Agent: software-architect. Budget ~110k. After Pass 1; parallel with 3, 4.
Scope: `models/families/link_lifecycle.qnt`, `models/core/connect.qnt`
(consumed, pre-existing), `crates/dpaa2-api/src/plan/connect.rs`,
`families/dprc.rs`, `families/dpni.rs` delta,
`intent/{compiled,derive,tenant}.rs`,
`crates/dpaa2-verify/src/intent/link_itf.rs`, `tests/link_replay.rs`,
`models/board/V-TRAF-1/vtraf1.qnt` (scenario module). Four mandates:
(a) **Isomorphism (ADR-0002 law)**: the wire lifecycle (ends-exist →
connected → disconnected → end-destroy-legal, post-7.5 shape: destroy
of a connected end enabled and edge-removing), the D4 partial order,
both D5 obligations and the consented discharge vs `plan/connect.rs` +
`families/dprc.rs` — law-for-law, guard-for-guard, refusal-for-
refusal; names converge on the same readable English; the edge-kind
table mirrors `core/connect.qnt` (`legalPair`,
`edgeDemandsSeveredWitness`) rather than re-deriving it.
(b) **The obligations are typed, not checked**: `DeferredVisibility`
unconstructible without a planned discharge; declined consent a typed
standing residue that can speak its own name (the 578de73 proof);
the stale-node mirror blocks nothing; no silent rebind representable;
the Disruptive consent rides the existing ADR-0015 machinery rather
than a parallel consent path.
(c) **ITF replay coverage**: map the 13 frozen traces onto the model's
guarded transitions; name every guard/refusal arm with no replayed
trace; check the property twins actually bind the Rust-side refusals
to the model's; verify the 27b99e2 re-freeze left no trace replaying
the retired refusal.
(d) **The bounded lift held structurally**: the table CLAIMS the dpmac
machinery (transitions, executor, SeveredProof untouched, keyed as
before) — flag any retyping, any dpmac code moved or re-keyed, any
severed-witness logic duplicated for the dpni↔dpni kind instead of
policy-switched; the dpni↔dpni row must carry disconnect-only teardown
with no handback path representable.

### Pass 3 — Architecture, sans-io discipline, and code quality
Agent: software-architect. Budget ~110k. After Pass 1; parallel with 2, 4.
Scope: `crates/dpaa2-mc/src/{populate,restool,parse}.rs` + fixtures,
`crates/dpaa2-api/src/contract/{mc,fake}.rs`,
`crates/dpaa2-config/src/{parse,schema}.rs`,
`crates/dpaa2-tools/src/{engine,render,status,main}.rs` +
`tests/{link_faces,vdpcon1_intents,dprc_convergence}.rs` + snapshots,
`crates/dpaa2-verify/src/board/generate.rs` + `tests/vtraf1_suite.rs`.
Four mandates:
(a) **The verbs are portal-ready**: McControl connection verbs named
1:1 with whitelisted MC commands (`DPRC_GET_CONNECTION`,
`DPRC_CONNECT/DISCONNECT`, `DPNI_GET_LINK_STATE`), ancestor-explicit,
typed returns (`ObjectRef`, `LinkState`); no restool text leaks
through the trait; the `endpoint state: -1` vs `0` parser datum typed,
not sentinel-compared; no portal read-slice addition snuck in
(ADR-0021: #10 owns every addition).
(b) **Sans-io discipline**: all judgment (edge-kind policy, census
delta, obligation minting, partial-order law) lives in dpaa2-api; the
adapters execute and observe; any link-census decision or obligation
judgment living in dpaa2-mc or dpaa2-tools instead of the core is a
finding. engine.rs grew +959 — verify the consent flow is imperative-
shell composition over api-owned plan transitions, not a second
decision layer; render/status stay display composition with the
honest-unknown idiom; no detail-row field gates convergence (sweep for
side doors).
(c) **Generalized peer resolution stayed general**: populate.rs +258 —
the dpni↔dpni resolution generalizes the existing port-edge path
rather than forking it; the root-pass backstop (bbe4b14) composes with
child populate rather than duplicating it; `CONNECT_ANCESTOR` is one
constant, not scattered.
(d) **Reuse-before-write and seam quality for #10/#11/#12**: did the
suite/hook render in generate.rs +312 reuse the established render
seams; did link_faces/snapshots follow the existing insta idiom;
fake.rs +43 vs existing fake state idioms; does the edge-kind table's
shape let #11/#12 (dpsw, dpdmux kinds) land as table rows rather than
new surfaces, and does the verb seam let #10's portal backend drop in
under the differential gate exactly as D6 claims?

### Pass 4 — Spec/docs alignment
Agent: spec-align. Budget ~90k. After Pass 1; parallel with 2, 3.
Scope: `openspec/changes/cross-dprc-links/{proposal,design,tasks}.md`,
the spec deltas under `specs/` vs shipped code; COVERAGE dispositions
(LINK-I* rows settled to the sitting's verdicts, DPCON-I3 re-anchored,
the three #10 re-pointings) vs the witnesses they cite; ADR-0022 vs
the shipped table (edge kinds, reification rows including the healing
row, atemporal per the recorded style directive); the ADR-0017
amendment (PASS4-F8 discharged with the pointer), ADR-0019 (edge facet
second inhabitant), ADR-0003 §8 (Mellanox dropped, trigger reworded,
no tile) — each vs the code/roadmap that cites it; ADR-0008 §10 + the
upstream rescan-race doc vs the four-rev eviction evidence (including
rev 4's refutation of the probe attribution — no doc may still claim
the probe caused the eviction); `docs/baseline/dpni.md`/`dprc.md`
deltas (post-bind connect dmesg law, destroy-mirror answer, the
single_sender wire-variant answer, the `endpoint state: -1` datum) vs
the sitting record; ROADMAP rows #9/#10 (dossier-bead line present,
bead exists); the board README :109/:110 ledger rows + VERDICTS.json
V-TRAF-1 revs 1–4 + V-LINK-6 vs tasks.md 7.2/7.5 — verify the records
tell ONE consistent story: rev 1/2 evidentiary FAILs with the fix
trail (ec9411b, 76646ed), rev 3 quarantine justified by F4 alone,
rev 4 zero-residue pass refuting the probe attribution, V-LINK-6
anchoring LINK-I1's state face with the refusal retired at 27b99e2,
and the 7.6 re-scope (eb6eb8d → d20fbee) recorded where a reader
would look. Verify the modified-capability claims in proposal.md are
each true as shipped (reconciler, mc-backend, intent-compiler,
topology-config, provisioning-cli, formal-models). Verify no doc
still treats the destroy-mirror or dmesg-law unknowns as open, none
claims a knob board witness, and none claims LINK-I1 as a refusal
law. Disposition Pass 1's doc-side hits.

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
find doc-side hits; 2 and 3 may both touch the obligation path); drop
unanchored staleness claims and any finding that "fixes" a protected
decision (the nine grounding items above, the four board outcomes, and
everything prior reviews protected); deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. A verdict on the change's core promise: does a declared dpni↔dpni
   link converge end to end — edge-kind table claiming the dpmac
   machinery unchanged, container-agnostic transitions, the partial
   order and both healing obligations typed with the consented
   discharge, frames witnessed on the board, LINK-I1 hardware-anchored
   as the edge-dies-with-its-endpoint state law — with the laws on
   both sides of the sans-io seam, yes or no, evidence rows.
3. A verdict on ADR-0002/ADR-0022 compliance: is the shipped surface
   structurally isomorphic to `link_lifecycle.qnt` + `core/connect.qnt`,
   and is reification policy actually a property of the edge kind as
   ADR-0022 claims (dpmac claimed not retyped, dpni↔dpni
   disconnect-only, healing as a reification row), yes or no,
   evidence rows.
4. Proposed follow-up work as bead-shaped items (title + why +
   acceptance) for a just-in-time openspec change — the review itself
   changes nothing.
No guidelines deliverable. At most nominate a rule amendment if a
finding shows an existing rule failed to prevent a defect (candidates:
the LINK-I1 refusal guard shipped through offline gates and was
falsified only on the board — was the guard ever more than an
assumption, and could the brief's unknown-labeling rule have kept it
typed as a board-answerable unknown like the destroy-mirror was? and
the 7.6 knob witness was discovered unbuildable only at close-out —
should derivation-only knobs declare their witness class at design
time?).

## Budgets

P1 50k · P2 110k · P3 110k · P4 90k · synthesis 70k ≈ 430k total across
4 pass agents + judge. A pass exceeding ~1.5x its budget stops and
reports partial.
