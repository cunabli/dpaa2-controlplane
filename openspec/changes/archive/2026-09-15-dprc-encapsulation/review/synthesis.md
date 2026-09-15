# Synthesis — `dprc-encapsulation` epic quality review

Agent: change-judge (Fable 5, pinned). Spend: ~58k tokens vs 80k budget.
Saved verbatim by the orchestrator.

## Arbitration rulings (repo re-read, not guessed)

1. **DPRC-I8 tile pointer**: `models/COVERAGE.md:79` reads
   `` `pool-objects` (#6) `` in the location column and closes with
   `→ pool-objects (#6)`. **Pass 4 is right; Pass 1's PASS on this row was
   wrong.** Five in-scope artifacts (proposal.md:42-44, design.md:37-38,
   tasks.md:4, specs/mbt-harness/spec.md:22-24,
   specs/mc-backend/spec.md:24-26) still promise #10. Which tile *should*
   own it is a genuine call (baseline hedges "DPL-defined child **or** raw
   command path (#10)") — the amend is mechanical once decided; the
   decision goes to the open-items list.
2. **Prune-dispatch cluster**: verified in
   `crates/dpaa2-mc/src/restool.rs:427-481` — the bare-`ResidentId` keying
   (:451-454), the `CreatedIn` hardcode (:449-456), and the state
   classification in the shim (:462-468) all exist as claimed. Merged into
   one fix cluster (M1/M2 below), not seven independent beads.
3. **PASS3-F3 staleness confirmed**:
   `crates/dpaa2-tools/src/engine.rs:305,322` says "restool resident
   read-back is deferred" and dead-ends every `UnplugResident` step in
   `Error::Backend` — while `restool.rs:443-460` demonstrably reads
   residents back. The comment is stale (superseded by task 3.1); the
   *real* ceiling is the family-less `ResidentId` (no `ObjectRef` to hand
   `dprc_unassign`), which is exactly PASS3-F14. The stale comment and the
   collision share one fix.
4. **PASS2-F5 (guard order under lock)**: no grounding fact or protected
   decision settles duplicate-id-under-lock ordering — V-DPRC-12 proved
   create-under-lock ⇒ 0x4 but never with a duplicate id. Genuinely open;
   routed to ADR/RFC with a board trigger, not forced pass/fail.

**Schema/protection filter**: no finding lacked mandatory fields (all
stale claims carry superseded-by). No finding attacks a protected
decision: PASS2-F9 freezes *existing* runs without widening the
fingerprint (compliant); PASS3-F14 explicitly keys the *observation* type
by `ObjectRef` and leaves `dprc::ResidentId` untouched, preserving the
ADR-0014 twin (compliant, kept); PASS4's `dpaa2-api` derivation
disposition (deliberate-to-record, prose amend only) is honored. Zero
findings dropped; Pass 1's I8 row is *corrected*, not a new finding.

---

## 1. Merged findings ledger (most severe first)

### Cluster M1 — Prune dispatch is unsound on the two states it will actually meet (breaks-a-claim)
Constituents: **PASS2-F3, PASS2-F4, PASS3-F3, PASS3-F4, PASS3-F5,
PASS3-F14, PASS3-F15.** One surface, one story: the census can
under-count residents (family-key collision, `restool.rs:451`),
`plan_teardown` (`dprc_plan.rs:436-449`) ignores `Locked` and plans steps
the model refuses 0x4 — violating the module's own "doomed step is never
emitted" doctrine (:279) — the engine's `UnplugResident` arm dead-ends on
a stale "read-back deferred" claim (`engine.rs:321-324`), a refusal
aborts the whole prune pass untyped with no re-observation
(`engine.rs:283-292`), MC `0x10` never reaches the already-typed
`Teardown::ResidentPlugged`, and the fake (`fake.rs:352-355`) plus every
fixture (`dprc_convergence.rs:188-215`, all `plugged:false`) make the
whole gap invisible to `cargo test`. PASS2-F4 folds in (same predicate,
sibling planner). Disposition: **two code beads + one fixture bead**
(below). Verification: the plugged-resident/Locked-orphan fixtures named
per constituent; the F15 fixture must fail before the F5 fix.

### Cluster M2 — The shim judges lifecycle where the core should (breaks-a-claim via F2)
Constituents: **PASS3-F1, PASS3-F2.** `restool.rs:462-468` decides
`Created`/`Populated` in the shim (the sibling VFIO face deliberately
reports raw and lets `VfioBind::classify` judge); `restool.rs:449-456`
hardcodes `ResidentKind::CreatedIn` for an *unobservable* fact, and the
one consumer of that guess is the undeclared-container prune — where an
assigned-in resident would be predicted "released" when the eviction law
says it is evicted to the parent (`render_prune` post-state lie).
V-DPRC-11 board evidence covered only the CreatedIn case, so this is
unproven, not disproven. Disposition: fold classification into a core
`ContainerState::classify`; report origin as `Option<ResidentKind>` + ADR
note. Verification: PASS3-F1/F2 rows.

### M3 — Archived spec would carry two contradictory SHALLs (breaks-a-claim)
**PASS4-F1.** The D8 prune delta enumerates-and-deletes containers; the
base capability (`openspec/specs/reconciler/spec.md:61-65`) says SHALL
NOT. The delta has no MODIFIED block. Working-as-designed code; the
amendment never amended the requirement it overrides. Disposition: amend
delta with a `## MODIFIED Requirements` block before archive. Gates
archive.

### M4 — formal-models delta overclaims Apalache marks (breaks-a-claim)
**PASS4-F3.** Delta promises Apalache marks on I1/I5/I7/I9/I10/
I11-remainder; `dprc.qnt:583-586` marks I10+I12 only, and the model
header + COVERAGE.md consistently declare the rest simulate-only directed
runs. Model/COVERAGE are the defensible side; delta + tasks.md:4 are the
stale side. Disposition: amend delta sentence (ADR-0002 "Sealed gap"
precedent).

### M5 — DPRC-I8 tile pointer split-brain (breaks-a-claim)
**PASS4-F4 + Pass 1 correction.** COVERAGE.md:79 says #6; five change
artifacts say #10 (ruling 1 above). Disposition: amend after the tile
call (open item OI-2). Verification: `rg -n "DPRC-I8"` across the five
artifacts agrees on one tile.

### M6 — The ITF conformance assertion is a tautology (misleads-a-reader)
**PASS2-F1 + Pass 2 open question 2.** `dprc_replay.rs:239-252` binds all
six `Attribution` arms to `r.mc_status()` and asserts it against itself;
a mutated `attribute_mc` stays green. Fold in the silent `if let`-no-else
skip at :263. Disposition: amend — per-arm expected-status map + loud
else-arm.

### M7 — Attribution is verb-blind and half of it lives in the wrong crate (misleads / carries-cost)
**PASS2-F2 + PASS3-F6.** `attribute_mc` (`dprc_plan.rs:208-216`)
attributes 0x4 by mask alone — under the child DEFAULT mask a lock-strip
refusal of a create verb misattributes as `PermissionGap{TopologyChanges}`
(model: `dprc.qnt:326-340` has no topology branch on create). Latent
until the engine wires it. `attribute_refusal` (`engine.rs:364-374`) is a
pure map over two `dpaa2-api` types stranded in the shell (D4 places
interpretation in `dpaa2-api`). Disposition: one bead — verb parameter +
relocation; M1's `PruneOutcome::Refused` consumes it.

### M8 — `--prune` help text hides the tool's most destructive semantic (misleads-a-reader, user-facing)
**PASS4-F5.** `main.rs:69-71,94-96` still reads "Tear down ports declared
absent"; after 4.3 the flag also destroys undeclared containers.
Superseded by task 4.3. Disposition: amend help/doc strings + D8 pointer
on `ConvergeConfig::prune`.

### M9 — Unwitnessed model arms and branches (carries-cost)
**PASS2-F7, PASS2-F8, PASS2-F9** + the five untested `Container<Locked>`
refusing methods (Pass 2 trace map). Accepted move-out/unplug have no
trace; `Unlocked::Empty` unwitnessed on both sides; task-1.4 runs
unfrozen so replay never sees a voiding `setLabelAt("")`. F9 freezes
*existing* runs only — fingerprint untouched. Disposition: one
freeze-and-witness bead.

### M10 — Shim error-path hygiene (carries-cost)
**PASS3-F7, PASS3-F8.** Every non-dprc verb still exits through raw
`Runner::run` (the default a tile #5–8 author inherits — a D4
collapse-by-omission trap); `code: None` (signal death) becomes
`RestoolClientGuard` instead of `Error::Backend`. Disposition: single
classified exit + signal-death fix.

### M11 — Mechanical duplicate folds (carries-cost)
**PASS3-F9, F10, F11, F12.** Option-bit table, `CannedRunner` vs
`ScriptedRunner`, byte-identical `dprc_info` helper, hand-copied
inventory vs `testkit::ref_inventory(16)`. None protected (ADR-0014
lockstep is model↔Rust, not these). F12 carries a behavior caveat
(`Counted(18)` → `Observed{18}` + a labels seed) — verify under cargo
before sealing. Disposition: fold, low priority.

### M12 — Observation seam is root-only/all-at-once (carries-cost, seam for #5–8)
**PASS3-F13** (+ its dpmcp-budget open question, OI-3).
`observe_containers` rescans everything (1+2N spawns ×4 per ensure); doc
promises "re-querying the affected container" with no verb to do it.
Disposition: follow-up bead, may ride tile #5 planning.

### M13 — Doc/ADR citation accuracy (misleads-a-reader, doc-only)
**PASS4-F2** (ADR-0001 §4 cited six times for a fence the epic
half-changed — amendment section), **PASS4-F6** (V-DPDBG-2 ride-along has
no task/spec row — record or re-attribute; operator call, OI-4),
**PASS4-F7** (proposal Impact: `dpaa2-config` → `dpaa2-api`, honoring the
deliberate-placement disposition; "minor surface" for tools is false),
**PASS4-F8** (ADR-0017 Decision 3's obligation has no carrier and zero
crate citations), **PASS4-F9** ("rev 2" with no antecedent), **PASS2-F6**
(bind/unbind accept-as-no-op is a real refinement of a disabled model
guard — one recorded sentence). Disposition: one doc-amend bead.

---

## 2. Verdict — ADR-0002 compliance (lifecycle sum isomorphism)

**YES — structurally isomorphic, with two guard-level refinements to
record.** Evidence (Pass 2 map): all 22 rows match state-for-state,
guard-for-guard, payload-for-payload; parity is *enforced*, not asserted
(`dprc.rs:1026` `container_states_match_the_enum_and_the_model`, `:1050`
typestate-marker parity, `dprc_plan.rs:1087` eviction triple-binding,
`prune_buckets_match_the_enum_and_the_model`); compile-time claims are
honestly scoped (module doc audit clean); DPRC-I12 bucket parity holds
arm-for-arm with the empty-label escape landing ReportOnly on both sides
and ghosts staying model-side. The two non-matches are refinements, not
structural breaks: `bindVfio` accept-as-no-op vs disabled guard
(PASS2-F6, one doc sentence) and the Locked-face guard *ordering*
(PASS2-F5, open item OI-1 — the sum and the guards themselves match; only
the evaluation order under lock is unpinned). Coverage caveat: the
isomorphism is proven where witnessed; M9 lists the unwitnessed arms.

## 3. Verdict — D2 claim (pure core carries the change)

**YES — holds, with three localized leaks to fold back.** Evidence (Pass
3 mandate verdicts): (a) sans-io HOLDS-WITH-FINDINGS — no planning logic
in `engine.rs`; gate → dispatch → re-observe → judge is the documented
boundary and `dprc_plan.rs:822-837` explicitly assigns the double gate to
the shell; the plan logic is off-board-replayable today (FakeBackend + 3
snapshots, ITF replay, prune classifier all in `dpaa2-api`). (b) D4
refusal typing HOLDS-WITH-FINDINGS — no string-matching anywhere;
converge path reaches discriminated `Attribution`
(`dprc_convergence.rs:149-169`); the collapses are by *omission* (prune
path, M1/M7). The three leaks: shim-side state classification (M2/F1),
shim-side origin guess (M2/F2), shell-side `attribute_refusal` (M7/F6).
All are single-function moves, not rewrites — the architecture rule
("encapsulation without heavy rewrites") survived contact.

## 4. Follow-up work (bead-shaped, for a just-in-time openspec change)

**Code/test — ordered by severity:**

- **B1 — Prune dispatch honors Locked and plugged residents (cluster M1,
  part 1: plan+engine).** Why: the prune path plans steps the model
  refuses and aborts untyped on the exact states an undeclared container
  will present. Acceptance: `plan_teardown` on a `Locked` observed
  container yields empty steps + `Attribution::LockGate` gap (PASS2-F3);
  `plan_move_out(Destroyed,…) == Refused(FaceNotAssignable)` (PASS2-F4);
  `PruneOutcome::Refused{id, attribution}` exists, MC `0x10` maps to
  `Teardown::ResidentPlugged`, a refusing candidate does not abort the
  pass, and the `:292` re-observation always runs naming the survivor
  (PASS3-F4/F5).
- **B2 — Observation keyed by `ObjectRef`; shim stops judging (cluster M1
  part 2 + M2).** Why: family-key collision is reachable through the
  shipped census; two lifecycle judgments live in the shim. Acceptance:
  `dprc_plan::ObservedContainer.residents` keyed by `ObjectRef` while
  `dprc::Container` keeps `ResidentId` (ADR-0014 twin untouched); a child
  listing `dpbp.0 plugged` + `dpmcp.0 unplugged` yields two residents and
  an `UnplugResident` step that dispatches via `dprc_unassign`
  (PASS3-F14); state classification moves to a core
  `ContainerState::classify` (PASS3-F1); origin reported
  `Option<ResidentKind>` with conservative core prediction + ADR note
  (PASS3-F2); the stale "read-back is deferred" comment in
  `engine.rs:305,322` replaced by the real ceiling (PASS3-F3).
- **B3 — Fixture that fails before B1 (M1 part 3).** Why: every current
  fixture seeds `plugged:false`, so the whole cluster is test-invisible.
  Acceptance: `fake.rs` `dprc_destroy` returns `McStatus{0x10}` while any
  resident is plugged; a plugged-resident orphan fixture exists;
  `cargo test -p dpaa2-tools prune` fails on pre-B1 code (PASS3-F15).
  Land B3's fixture first, TDD-style, per repo tenet 5.
- **B4 — Attribution carries the verb; replay asserts expected status
  (M6+M7).** Acceptance: `attribute_mc` takes the refused verb;
  `attribute_mc(TopologyLockGate, DEFAULT, CreateResident) == LockGate`
  (PASS2-F2); `attribute_refusal` relocated to `dpaa2_api::dprc_plan`
  (PASS3-F6); `dprc_replay.rs` maps each arm to its expected status and a
  mutated `attribute_mc` fails the suite; the silent `if let` gets a loud
  else (PASS2-F1).
- **B5 — Freeze the missing witnesses (M9).** Acceptance: one directed
  run frozen covering accepted move-out + unplug, one lock-strip sweep
  covering the five untested Locked refusals, the three task-1.4 runs
  frozen (states only, ghosts ignored by `world_view` — no fingerprint
  change); `created().lock().unlock()` matches `Unlocked::Empty`;
  `every_committed_trace_is_listed` green with 17+ traces
  (PASS2-F7/F8/F9).
- **B6 — Shim exit hygiene + duplicate folds (M10+M11).** Acceptance:
  every `McControl` verb exits through the classifying path,
  `Runner::run` stays raw transport (PASS3-F7); `code: None` ⇒
  `Error::Backend` (PASS3-F8); the four folds land with `rg`
  verifications as written; F12 fold confirmed under `cargo test` given
  the `Observed{18}`/labels caveat.
- **B7 — `observe_container(id)` seam (M12).** May be deferred into tile
  #5 planning; acceptance per PASS3-F13, plus the dpmcp-budget
  measurement (OI-3) recorded either way.

**Doc-only amends (cheap — two beads, the first gates archive):**

- **B8 — Archive-gating spec amends (M3, M4, M5).** Acceptance:
  reconciler delta carries a MODIFIED block restating the ownership fence
  with the container carve-out (PASS4-F1); formal-models delta states the
  Apalache/simulate split (PASS4-F3); all six DPRC-I8 pointers agree on
  the tile chosen in OI-2 (PASS4-F4).
- **B9 — Doc polish (M8, M13).** Acceptance: `--prune` help names
  containers + `--allow disruptive` and D8 (PASS4-F5); ADR-0001 §4
  amendment section (PASS4-F2); proposal Impact corrected
  `dpaa2-config`→`dpaa2-api`, tools surface honest (PASS4-F7); ADR-0017
  consequence amended or obligation carrier filed against tile #6, plus
  one crate citation anchor (PASS4-F8); "rev 2" antecedent named in
  baseline + COVERAGE (PASS4-F9); one sentence recording the bind/unbind
  idempotent refinement citing the disabled model guard (PASS2-F6);
  V-DPDBG-2 recorded per OI-4's call (PASS4-F6).

## 5. Open items → ADR/RFC with board-verify triggers (not pass/fail)

- **OI-1 (PASS2-F5)** Lock-strip vs enabling-precondition ordering: does
  the real MC's permission check precede the duplicate-id check? Trigger:
  one V-DPRC directed sitting — `create` a duplicate id under
  `set-locked 1`, observe 0x4 vs config error; then amend whichever side
  (model or `dprc.rs:937-980` doc) lost, as an ADR-0002 note.
- **OI-2 (PASS4-F4)** DPRC-I8 ownership: `pool-objects` (#6) vs
  `mc-portal-backend` (#10). Decision call (Pass 4 correctly declined
  authority). Trigger: tile #6 scoping, or the first DPL-defined-child
  board window, whichever comes first; B8 executes the winner.
- **OI-3 (PASS3-F13 OQ)** Whether each restool spawn draws from the
  never-returned per-boot dpmcp budget — turns the 1+2N×4 scan from
  latency into a resource leak. Trigger: one board measurement in the
  next sitting.
- **OI-4 (PASS4-F6 OQ)** Whether V-DPDBG-2 was an intentional ride-along
  on the 6.1 board window (add a tasks row) or belongs elsewhere
  (re-attribute COVERAGE.md:180). Operator/ledger call.
- **OI-5 (Pass 1/4)** `git cliff` CHANGELOG generation remains
  UNCHECKABLE-OFFLINE. Trigger: next release-time cliff run.

**Rule amendment (one, nominated per mandate):** M3 shows a spec-delta
rule gap — a delta that overrides a base-capability SHALL must carry a
`MODIFIED Requirements` block; nothing in the current review rules or
delta-authoring checks caught the SHALL/SHALL-NOT contradiction until
Pass 4. The Pass 1 I8 misread was pass-execution error, not a rule
failure; no amendment warranted there.

---

## Footer

**Read (full):**
`openspec/changes/dprc-encapsulation/review/{pass1,pass2,pass3,pass4,brief}.md`.
**Arbitrated by repo re-read:** `models/COVERAGE.md:73-84` (ruling 1: I8
row says #6 — Pass 4 right, Pass 1 wrong; also confirmed the V-DPRC-11
CreatedIn-only board evidence bounding M2);
`crates/dpaa2-tools/src/engine.rs:301-326` (ruling 3: stale "read-back
deferred" comment + dead `UnplugResident` arm confirmed);
`crates/dpaa2-mc/src/restool.rs:427-481` (ruling 2: ResidentId collision,
CreatedIn hardcode, shim-side state classification all confirmed).
Nothing else re-reviewed; pass evidence taken as reported elsewhere.
**Open questions:** OI-1 through OI-5 above; plus whether B7 rides tile
#5 or lands standalone — orchestrator's sequencing call.
