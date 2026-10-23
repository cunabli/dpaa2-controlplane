# Review brief: dpsw-typestate epic

Slug: `dpsw-typestate`. Reviewed span: commits e18316a..8a0c993 on main
(e18316a is the spec-init — the ADR-0004 §8 11→12→10 lock plus
proposal/design/specs/tasks — then 22 execution commits carrying a
`Change: dpsw-typestate` trailer). Two interleaved commits belong to a
DIFFERENT change and are out of scope here: 9e96a23 (`Change:
resource-aware-orchestration`, the #15 proposal + ADR-0023) and its
fixup b971fe8. Two unsquashed `fixup!` commits (4593fc1 → 4e243da,
2d7577b → 49dfad9) are the recorded amendment trail of an amendable
unsealed branch — a finding calling unsquashed fixups a defect is a
false positive. Epic dpaa2-controlplane-dbf; close-out banked at
68598aa (baseline/ledger/ADR bank) and sealed at 8a0c993 (quality
floor + `openspec validate --change dpsw-typestate` recorded green).
Net epic size excluding openspec/ artifacts, frozen traces, and board
corpus: crates 50 files +7,998/−89; models (excl. traces/ and board/)
15 files +964/−51; docs 10 files +513/−21 — roughly 135% of
cross-dprc-links, so the same four-pass shape with the same heavier
Pass 2/3 budgets. Mechanical lint findings are NOT the target.
Targets: the change's core promise (a declared switched domain
converges end to end — the kernel-bindability predicate DPSW-I1 as
construction law in both model and Rust so an unbindable kernel-steered
cfg is unrepresentable, the switched-domain construct compiled with
per-rule provenance, the two dpsw edge kinds as ADR-0022 table rows
with port-bound connects and no severed witness, sever-first
interposition through the dpmac severed-witness law, census +
read-back-only convergence (D6), the 6.4 ensure pass with the
exhaustive-match completeness guard and the teardown unbind law, and
the D7 capture tap banking #10's encode fixtures), structural
isomorphism of `models/families/dpsw.qnt` + the `core/connect.qnt`
edge rows vs the Rust family/plan surface (ADR-0002 law), sans-io
discipline across the grown adapters (restool.rs +866, engine.rs +871,
capture.rs 846), ITF replay coverage of the 8 frozen dpsw traces, and
doc/spec alignment including the ADR-0013/0019/0022/0004 amendments,
the baseline unknowns answered by the sittings, and the two-sitting
board story told consistently across tasks.md, the board README
ledger, and VERDICTS.json.

## Grounding facts (verified 2026-10-22, re-verify only if main moved)

- Commit order (all task-ordered; subjects abbreviated): e18316a
  (spec-init + ADR-0004 §8 verb-scope portal route, 11→12→10 locked),
  4e78a03 (1.1 P1 phase machine, bindability as type law), e8471ed
  (1.2 edge rows — port bound, no severed witness), 79b803f (1.3
  Apalache marking, frozen traces, twin record), e7a6e6b (1.4 COVERAGE
  dispositions), 72a7de0 (2.1 MAC-table sizing with provenance),
  4b280d7 (2.2 typed refusals — unsteerable and empty domains),
  fa62280 (2.3 config parse), 34021e2 (3.1 family tile), 4689f4b (3.2
  plan surface — ordered, bounded, typed rewire), b8daf95 (3.3
  read-back as the only convergence truth), 044de2b (4.1 verbs
  verbatim), 4d94d8d (4.2 info parse, honest typed observation),
  d258420 (5.1 detail row), 9ddad66 (6.1 MBT twins), ce7ee8d (6.2
  board program render), 51d37f1 (6.3 capture tap), 0a1e49f (7.1
  V-DPSW-2 discovery sitting), 4e243da + fixup 4593fc1 (6.4 ensure
  pass — the LATE task: the 7.2 offline pre-check found `dpaa2ctl
  ensure` had no dpsw dispatch at all; bead dbf.22 landed the pass,
  the completeness guard, and the teardown unbind law; its seal also
  amends the 3.2 companion placement to co-residency in the dpsw's own
  container (root), not a child dprc), 49dfad9 + fixup 2d7577b (7.2
  V-DPSW-5 findings bank + ADR-0013 amend), b8defbd (7.2 witness
  sitting banks its structural pass), 68598aa (8.1 doc bank), 8a0c993
  (8.2 seal). A finding re-reporting the pre-6.4 ensure gap as a live
  defect contradicts the 6.4 close; a finding demanding the full
  Quint convergence model of the ensure pipeline now contradicts the
  recorded #15 scheduling decision (ADR-0023, ROADMAP row #15:
  implementation directly after #11 archives).
- Major new surface (span deltas; current sizes where new):
  `models/families/dpsw.qnt` +704 (P1 phase machine with the reset
  facet; DPSW-I1 bindability as type law — ctrl-if ∧ flooding PER_FDB
  ∧ broadcast PER_FDB ∧ max_fdbs ≥ num_ifs; DPSW-I2 vendor-recipe
  refusal; DPSW-I7 census judge on the observable cfg signature;
  DPSW-I9 probe-consumer per design D10a; DPSW-I10 port bound);
  `models/core/connect.qnt` +19 (the two waiting ADR-0022 rows filled:
  `dpni ↔ dpsw.N.M`, `dpsw.N.M ↔ dpmac`, reification policy +
  port-bound connects, NO teardown witness — design D5, revisit
  trigger recorded); intent models (alphabet +20, derive +54, refuse
  +41, invariants +24, main +50) for the switched-domain construct;
  8 frozen traces under `models/traces/families/dpsw/`
  (createBindableAccepted, ctrlIfDisabledRefused, fewFdbsRefused,
  vendorRecipeRefused, resetFacetTotal, probeDrawsAndBinds,
  censusCompetitorDefers, destroySoleResizePath).
  `crates/dpaa2-api/src/families/dpsw.rs` 1024 (new: constructor-gated
  create block per D2, lifecycle typestates, census judgment),
  `plan/dpsw.rs` 948 (new: companion placement → create → port
  connects → plug ordering; destroy-and-recreate disruption classing;
  companion-starvation and port-overflow refusals), `plan/connect.rs`
  +58 (edge-kind rows), `contract/mc.rs` +160 + `fake.rs` +193 +
  `kernel.rs` +31 (verb seam + fake state), intent compiled +72 /
  derive +73 / refuse +~90 / compile_tests +238;
  `crates/dpaa2-config` parse.rs +92, schema.rs +17 (name-keyed
  construct; forced bindability fields ABSENT from the schema — D3);
  `crates/dpaa2-mc` restool.rs +866 (create/destroy/info verbs
  rendered verbatim with create-time labeling), parse.rs +31 +
  fixtures (per-if block, absence ≠ zero, `Unobservable` connection
  face), kernel.rs +148 (bind/unbind machinery incl. the 6.4
  driver-unbind rungs), populate.rs +23;
  `crates/dpaa2-tools` engine.rs +871 (the 6.4 ensure pass:
  switched-domain dispatch after the links pass, interposition
  sever-first, read-back judgment only, undeclared-root-dpsw teardown
  under the double gate in baseline recipe order — port disconnects →
  kernel unbind → destroy, allocator-claimed companions get
  `driver/unbind` before any `--plugged=0`; the exhaustive-match
  completeness guard refusing typed any compiled family without an
  executor), render.rs +209, status.rs +47, main.rs +96,
  `tests/dpsw_detail.rs` 150 + `tests/vdpsw5_intents.rs` 205 (offline
  operand pins for the witness suite) + render.rs +110 + 6 new
  snapshots + dprc_convergence +208 + link_spawns +25;
  `crates/dpaa2-verify` `board/capture.rs` 846 (new: the D7 eBPF
  capture tap — bpftrace on the ioctl tracepoints filtered on the one
  `SEND_MC_COMMAND` constant, loud-fail preamble, offline payload
  parse into the fixture corpus), driver.rs +170, ioctlpolicy.rs +4,
  `intent/dpsw_itf.rs` 260 + `tests/dpsw_replay.rs` 510 (conformance
  twins), board_artifacts +48, ioctl_policy +23, main.rs +15.
  `dpaa2-hal` portal.rs +6 ONLY (the pinned ioctl constant surface) —
  a finding demanding more hal work, or flagging the tap for not
  living in hal or dpaa2-mc, contradicts D7. CHANGELOG untouched in
  span (cliff-managed, the standing pattern). Board records:
  `models/board/V-DPSW-2/` (probes.json + corpus +
  capture-dprc-list.mc), `models/board/V-DPSW-5/` (two intents +
  hand-authored script + corpus), VERDICTS.json V-DPSW-2 rev 2 +
  V-DPSW-5 rev 6, board README :111/:112 ledger rows + :137 (V-DPSW-2..3
  deferral row). Docs: ADR-0004 +46 (§8 lock + capture tap), ADR-0013
  +29 (switched-domain vocabulary), ADR-0019 +10 (P1-instance note),
  ADR-0022 +4 (table rows), `docs/baseline/dpsw.md` +51 (unknowns
  answered, DPSW-I9/I10 in the candidate table),
  `docs/upstream/fsl-mc-no-autobind-child-dprc.md` 107 (new),
  `docs/upstream/phylink-dpni-churn-crash.md` 104 (new, the finding-49
  handover UAF), findings.md +33, ROADMAP row #11 (implemented
  2026-10-22) + row #15. ADR-0023 (132, new) and the ROADMAP #15 row
  belong to change `resource-aware-orchestration` — Pass 4 judges only
  whether dpsw-side docs cite them correctly, never their content.
- **Board outcomes are recorded facts** — findings that contradict
  them are false positives:
  1. **V-DPSW-2 rev 2 passed 11/11** (2026-10-18, discovery, bead
     dbf.18, ADR-0003 envelope — class lifecycle, no dpmac named).
     Answers now banked: DPSW-I3/I9 — a bindable dpsw without its dpbp
     defers silently and binds once the dpbp lands, but ONLY via an
     explicit `drivers_probe` kick (auto-rebind fails −12);
     DPSW-I4 — bind-time reset leaves the create-visible attribute
     surface byte-identical, facet stays ResetTotal with the measured
     five-verb reach bound; DPSW-I5 — bpid 2 vs object id 1, the
     V-READBACK-1 bpid==id coincidence bounded to non-law; FDB
     budget — the per-port split is DRIVER CONVENTION, enforced by
     neither MC nor probe (the kernel bound a split-violating shape),
     so the control plane's typed overflow refusal is its own law, not
     hardware's mirror. **Rev 1 FAIL same day, evidentiary**: teardown
     left all four allocator-bound companions plugged (unplug refusals
     swallowed by unconditional success guards, census closed +4);
     rev 2 unbinds before unplugging, judges every refusal, and
     machine-diffs the closing census byte-identical at the 97-object
     baseline. The eth0 carrier flap, paired qbman `no response`
     lines, and teardown-rescan −119 transients are recorded
     non-casualty observations, not open defects.
  2. **V-DPSW-5 rev 6 passed STRUCTURAL 8/8** (2026-10-18, witness,
     bead dbf.16/dbf.19): converge + sever-first interposition (flat
     dpni↔dpmac.7 severed through the typed severed-witness path, no
     driverless interval) + dpsw kernel bind via `bind_settle` +
     idempotence + typed teardown to the pre-suite baseline census;
     operands pinned offline by `vdpsw5_intents.rs`; V-DPSW-1 rev 2
     and V-DPSW-4 rev 1 cited, never re-run. Rev 7 (2026-10-22)
     re-proved steps 0–5 and closed finding 54 (child-dprc unplugged:
     `driver_override` then bind is the working path) but was killed
     at the burst prompt without its teardown trap — recorded, not a
     gating defect. **The reachability-traffic leg is DEFERRED** with
     the named revisit trigger (change #15's driver-occupancy ordering
     guard plus the kernel refresh; the finding-49 phylink handover
     UAF documented at `docs/upstream/phylink-dpni-churn-crash.md`);
     task 7.2's DoD was re-scoped to the structural verdict on bead
     dbf.19. A finding calling the deferral a gap, demanding the
     traffic burst now, or asking for a sitting re-run is a false
     positive.
  3. V-DPSW-1 and V-DPSW-4 are banked prior verdicts (2026-08) cited
     as the bindability predicate's positive/negative board anchors —
     never re-run in this change.
- **Deliberate decisions findings must not "fix"** (each recorded in
  design D1–D10, the proposal Out-of-scope, or the bead trail):
  1. D1: dpsw gets its own P1 family module, no shared "switch
     substrate" with dpdmux — sharing waits for #12 to prove a common
     mechanism. A finding proposing the shared substrate contradicts
     D1/D8.
  2. D2/D3: the bindability predicate gates CONSTRUCTION (constructor-
     refused, not dispatch-validated); flooding/broadcast/ctrl-if are
     derivation outputs with NO northbound knob. A finding demanding
     the knob, or dispatch-time validation, contradicts D2/D3.
  3. D4: non-kernel steering owner refused at compile, typed
     (DPSW-I6) — not deferred to scan/observation.
  4. D5: the two dpsw edge rows carry NO teardown witness (baseline
     records no hazard; revisit trigger recorded). A finding minting
     the witness now contradicts D5.
  5. D6: convergence is census + read-back only; no verb exit code is
     trusted (DPSW-I7). The detail row is display-only — a detail-row
     field gating convergence is a finding; demanding it gate is not.
  6. D7: the capture tap is eBPF/bpftrace in dpaa2-verify's suite
     driver — never in dpaa2-mc, never a hal transport; strace was
     REJECTED ON EVIDENCE (no ioctl-buffer dump facility; bead
     dbf.17), LD_PRELOAD and restool patches rejected (oracle stays
     stock, ADR-0004 §1), kernel source patch rejected (Kconfig
     fragment only). A finding proposing any of those contradicts D7.
     (Task 6.3's "strace-wrapped spawns" wording predates the D7
     evidence — if it still reads that way anywhere outside tasks.md
     history, that IS a legitimate staleness hit for Pass 1/4.)
  7. Proposal Out-of-scope: the runtime switchdev surface (~17
     function groups, consumer-owned, DPSW-I8), S_VLAN + LAG, writable
     counters, PER_VLAN flooding semantics, the taildrop UPDATE verb
     (unwitnessable dead code by the dpmac-typestate D6 law; taildrop
     stays read-only observation), and ALL dpdmux work — #12 inherits
     the D8 frame, nothing more. A finding demanding any of these
     contradicts the proposal.
  8. D9 witness classes: `mem_size` and `max_meters_per_if` stay
     undeclared dead baseline fields; taildrop/counters are
     observation-only offline-pinned. A finding demanding their knobs
     or board witnesses is a false positive.
  9. The 6.4 companion-placement amendment (co-residency at root, not
     a child dprc) is the recorded shape — sealed into the 3.2/6.4
     trail; `docs/upstream/fsl-mc-no-autobind-child-dprc.md` carries
     the kernel-side why. A finding calling the root placement wrong,
     or the child-dprc rejection undocumented, is a false positive.
  10. All protected decisions of prior reviews (cross-dprc-links
      D1–D10 + its nine grounding items, dpseci, dpmac, dpni, pool,
      dprc lineage) bind this review exactly as before.
- Six `ponytail:` markers exist in the repo (fake.rs:827,
  model.rs:552, compile_props.rs:209, fitcheck.rs:112,
  invariants.qnt:105, observed.qnt:30) — the same pre-existing set the
  cross-dprc-links brief carried (three lines shifted). Pass 1
  confirms none went stale through epic-touched lines and the epic
  added none that hide an unmarked ceiling.
- Out of scope: everything prior epic reviews already judged that this
  change did not touch; the `resource-aware-orchestration` change
  (9e96a23 + b971fe8 + ADR-0023 + ROADMAP #15 — its own future
  review); the quint-update commits below the span (1787055, 76af06b);
  the portal read slice and every MC command-vocabulary addition
  (ADR-0021: #10); dpdmux (#12); tier-c (#13); the repo-wide
  anchored-refs `--tree` debt (carried bead, pre-existing); VPP
  integration; re-running board suites.

## Passes

### Pass 1 — Residue, deferrals, and verify re-run
Agent: Explore (medium). Budget ~50k. Runs first; feeds the other three.
Re-run every task-level verify obligation from tasks.md that is
checkable offline: the dpsw ITF replay suite (`dpsw_replay.rs` — all 8
frozen traces replayed, none orphaned, no trace without a replay;
`lastVerbs` resolving against the ioctl-policy module), the 2.x
derivation/refusal unit tests (`compile_tests.rs`, compile_props), the
3.x plan unit tests (companion-starvation, port-overflow,
silent-destroy-failure, blanked-connection-line), the 4.x render and
parse tests (field-for-field render; fixture parses incl. truncation
and missing-line), the 5.1 + render snapshots (incl. the
`Unobservable` port face), the 6.3 tap offline tests (tapped and
untapped renders drive identical steps; dead-tap fails the preamble;
corpus parse round-trip), the 6.4 engine tests over the fake backend
(create, interposition, teardown, guard refusal), the 7.2 offline pins
(`vdpsw5_intents.rs`), the Quint gate (typecheck + simulate + marked
Apalache on `dpsw.qnt` within ladder bounds), and the COVERAGE ledger
lint. Verify the deferral promises point at their carriers: the
reachability-traffic leg → change #15 + bead dbf.19's close reason;
the banked corpus handoff → bead 94k / ROADMAP row #10; V-DPSW-3 stays
deferred as written (README :137). Sweep the epic-touched files (not
out-of-scope surfaces) for leftover scaffolding: `todo!`,
`unimplemented!`, `dbg!`, `#[allow(dead_code)]`, `#[ignore]`,
commented-out code, and stale wording predating the late answers — a
strace-tap claim predating D7's bpftrace evidence, a child-dprc
companion-placement claim predating the 6.4 amendment, a
no-ensure-dispatch mention predating 6.4, an FDB-budget-as-hardware-law
claim predating V-DPSW-2's driver-convention answer, a traffic-witness
claim predating the 7.2 re-scope. Confirm the six `ponytail:` markers
are pre-existing and none sits stale inside an epic-touched region;
flag any new unmarked deliberate simplification the epic introduced.

### Pass 2 — Isomorphism and the construction law
Agent: software-architect. Budget ~110k. After Pass 1; parallel with 3, 4.
Scope: `models/families/dpsw.qnt`, `models/core/connect.qnt` delta,
the intent model deltas (alphabet/derive/refuse/invariants),
`crates/dpaa2-api/src/families/dpsw.rs`, `plan/dpsw.rs`,
`plan/connect.rs` delta, `intent/{compiled,derive}.rs` +
`intent/refuse/`, `crates/dpaa2-verify/src/intent/dpsw_itf.rs`,
`tests/dpsw_replay.rs`, the 8 frozen traces. Four mandates:
(a) **Isomorphism (ADR-0002 law)**: the P1 phase machine with the
reset facet, DPSW-I1 as type law, DPSW-I2 refusal, the census
judgment, and the plan ordering (companions → create → port connects →
plug; destroy-and-recreate the only repair) vs `families/dpsw.rs` +
`plan/dpsw.rs` — law-for-law, guard-for-guard, refusal-for-refusal;
names converge on the same readable English; the edge rows mirror
`core/connect.qnt` (`legalPair`, `edgeDemandsSeveredWitness` false for
both dpsw kinds) rather than re-deriving policy.
(b) **The construction law is typed, not checked**: an unbindable
kernel-steered cfg is unconstructible (constructor-refused, D2);
flooding/broadcast/ctrl-if unreachable from the northbound schema
(D3); the regime refusal compile-time typed (D4); the derivation rules
carry per-rule provenance; no dispatch-time re-validation duplicating
the constructor.
(c) **ITF replay coverage**: map the 8 frozen traces onto the model's
guarded transitions; name every guard/refusal arm with no replayed
trace; check the property twins bind the Rust-side refusals to the
model's; confirm the census-competitor (DPSW-I9 probe-consumer, D10a)
and reset-facet behaviors replay.
(d) **The interposition rides the dpmac law**: sever-first planning
reuses the existing severed-witness machinery (nothing retyped, no
witness logic duplicated for the dpsw kinds — they carry none by D5);
port-bound connects enforce DPSW-I10; flag any dpmac code moved or
re-keyed.

### Pass 3 — Architecture, sans-io discipline, and code quality
Agent: software-architect. Budget ~110k. After Pass 1; parallel with 2, 4.
Scope: `crates/dpaa2-mc/src/{restool,parse,kernel,populate}.rs` +
fixtures, `crates/dpaa2-api/src/contract/{mc,fake,kernel}.rs`,
`crates/dpaa2-config/src/{parse,schema}.rs`,
`crates/dpaa2-tools/src/{engine,render,status,main}.rs` +
`tests/{dpsw_detail,vdpsw5_intents,dprc_convergence,render,link_spawns}.rs`
+ snapshots, `crates/dpaa2-verify/src/board/{capture,driver,
ioctlpolicy}.rs` + `tests/{dpsw_replay,board_artifacts,ioctl_policy}.rs`,
`crates/dpaa2-hal/src/portal.rs` delta. Four mandates:
(a) **The verbs are portal-ready**: McControl dpsw verbs named 1:1
with whitelisted MC commands, typed returns, honest-unknown
observation faces (`Unobservable` connection, absence ≠ zero
counters); no restool text leaks through the trait; ioctl-policy
resolution covers the new verbs; no portal read-slice addition snuck
in (ADR-0021: #10 owns every addition).
(b) **Sans-io discipline**: all judgment (bindability, census,
disruption classing, plan ordering, the completeness guard) lives in
dpaa2-api; the adapters execute and observe. engine.rs grew +871 —
verify the ensure pass is imperative-shell composition over api-owned
plan transitions, not a second decision layer; the teardown recipe
order (port disconnects → kernel unbind → destroy; allocator-claimed
companions `driver/unbind` first) executes an api-stated plan rather
than deciding in the shell; render/status stay display composition
with the honest-unknown idiom; no detail-row field gates convergence
(sweep for side doors).
(c) **The capture tap stays an observer**: capture.rs 846 lives
entirely in dpaa2-verify's suite driver; preamble proves the chain
loud-fail BEFORE mutation; the only hal contact is the pinned
constant (portal.rs +6); nothing in dpaa2-mc knows the tap exists;
the offline payload parse and corpus round-trip are self-contained;
the tap render leaves untapped suites byte-identical.
(d) **Reuse-before-write and seam quality for #12/#10**: did the
suite/hook render reuse the established render seams; did
dpsw_detail/snapshots follow the existing insta idiom; fake.rs +193
vs existing fake-state idioms; does the D8 frame actually stand —
would dpdmux land as table rows + a mirrored regime law rather than
new surfaces, and does the banked corpus + ioctl-policy coverage give
#10's portal backend its drop-in seam as ADR-0004 §8 claims?

### Pass 4 — Spec/docs alignment
Agent: spec-align. Budget ~90k. After Pass 1; parallel with 2, 3.
Scope: `openspec/changes/dpsw-typestate/{proposal,design,tasks}.md`,
the spec deltas under `specs/` (8 capabilities) vs shipped code;
COVERAGE DPSW-I1…I10 dispositions (anchors per the named-disposition
requirement: consumer-owned → ADR-0004 §8, board rows → the group-7
suite ids) vs the witnesses they cite; ADR-0013 (switched-domain
vocabulary) vs the shipped construct; ADR-0022 rows vs
`core/connect.qnt` + `plan/connect.rs`; ADR-0019 P1-instance note;
ADR-0004 §8 (11→12→10 lock, capture-tap mandate, verb-scope portal
route) vs what shipped; `docs/baseline/dpsw.md` deltas (unknowns
answered by V-DPSW-2: reset scope, bpid law, FDB budget as driver
convention; DPSW-I9/I10 in the candidate table; the `has_endpoint`
misread corrected) vs the sitting record; the two upstream docs vs
the evidence that spawned them (no-autobind-child-dprc vs the 6.4
amendment trail; phylink-dpni-churn-crash vs the rev-7 kill and the
deferral); ROADMAP row #11 (implemented 2026-10-22, deferral named)
and row #10 (corpus pre-banked claim); the board README :111/:112/:137
rows + VERDICTS.json V-DPSW-2/V-DPSW-5 vs tasks.md 7.1/7.2 — verify
the records tell ONE consistent story: rev 1 evidentiary FAIL with the
unbind-before-unplug fix, rev 2 census-clean pass, V-DPSW-5 rev 6
structural 8/8 with rev 7 closing finding 54, the reachability
deferral with its named trigger recorded where a reader would look
(tasks.md, ledger, ROADMAP, bead dbf.19). Verify the
modified-capability claims in proposal.md are each true as shipped
(formal-models, intent-compiler, topology-config, reconciler,
mc-backend, provisioning-cli, mbt-harness, system-integration).
Verify no doc still treats the answered unknowns as open, none claims
a traffic witness, none claims strace for the tap, none claims
child-dprc companion placement, and none re-reports the pre-6.4
ensure gap as live. Disposition Pass 1's doc-side hits.

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

## Synthesis mandate (change-judge, ~70k)

Deduplicate and rank findings across passes (Passes 1 and 4 will both
find doc-side hits; 2 and 3 may both touch the plan/engine seam); drop
unanchored staleness claims and any finding that "fixes" a protected
decision (the ten grounding items above, the three board outcomes, and
everything prior reviews protected); deliver:
1. The merged findings ledger, most severe first, with dispositions.
2. A verdict on the change's core promise: does a declared switched
   domain converge end to end — bindability as construction law on
   both sides of the sans-io seam, the construct compiled with
   provenance, the two edge kinds as table rows with sever-first
   interposition through the dpmac law, census + read-back-only
   convergence, the ensure pass with its completeness guard and
   teardown law, the capture tap banking #10's fixtures — yes or no,
   evidence rows.
3. A verdict on ADR-0002/ADR-0022 compliance: is the shipped surface
   structurally isomorphic to `dpsw.qnt` + the `core/connect.qnt`
   rows, and is reification policy a property of the edge kind as
   shipped (both dpsw rows witness-free per D5, dpmac machinery
   claimed not retyped), yes or no, evidence rows.
4. Proposed follow-up work as bead-shaped items (title + why +
   acceptance) for a just-in-time openspec change — the review itself
   changes nothing.
No guidelines deliverable. At most nominate a rule amendment if a
finding shows an existing rule failed to prevent a defect (candidates:
the 6.4 executor gap — a compiled construct family shipped through
five task groups with no ensure dispatch and only the 7.2 offline
pre-check caught it; ADR-0023/#15 answers the class, but should the
offline pre-check itself be a standing gate in the board-suite
protocol? and the 6.3 tasks wording shipped naming strace while the
design's D7 evidence had settled bpftrace — should task text be
re-sealed when a design decision it names is overturned?).

## Budgets

P1 50k · P2 110k · P3 110k · P4 90k · synthesis 70k ≈ 430k total across
4 pass agents + judge. A pass exceeding ~1.5x its budget stops and
reports partial.
