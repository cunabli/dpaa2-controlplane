# PASS 4 — Spec/docs alignment: `dprc-encapsulation`

Agent: spec-align (Opus override). Spend: ~113k tokens vs 110k budget.
Saved verbatim by the orchestrator.

## Findings

**PASS4-F1** · `openspec/changes/dprc-encapsulation/specs/reconciler/spec.md:49` (vs `openspec/specs/reconciler/spec.md:61-65`) · **doc-drift** · superseded-by task 1.3 / D8 (bead cd3.15) · **breaks-a-claim** · **amend** (add a `## MODIFIED Requirements` block restating "Ownership is limited to the configured subgraph" with the container carve-out) · verification: `rg -n "SHALL NOT enumerate all MC objects" openspec/specs/reconciler/spec.md` must return a text that admits the label-fingerprinted container prune.
The live capability says reconciliation "SHALL only plan changes to objects reachable from a DPMAC named in the desired topology. It SHALL NOT enumerate all MC objects and delete those absent from desired." The ADDED prune requirement does exactly that for containers (`McControl::observe_containers` at `crates/dpaa2-api/src/port.rs:111` enumerates every root child; `crates/dpaa2-api/src/dprc_plan.rs:839 plan_prune` classifies all of them; `crates/dpaa2-tools/src/engine.rs:249 prune_containers` dispatches destroys). The delta carries no MODIFIED section, so archiving lands two contradictory SHALL/SHALL NOT requirements in one capability. This is the D8 plan working as designed — the defect is that the amended plan never amended the requirement it overrides.

**PASS4-F2** · `docs/adr/0001-dpaa2-provisioning-control-plane-architecture.md:65-70` · **doc-drift** (citation accuracy) · superseded-by task 1.3 / D8 · **misleads-a-reader** · **new ADR note** (amendment section on §4: ownership becomes explicit and label-anchored for containers; the fence survives only for empty-label/zero-overlap) · verification: `rg -n "ADR-0001 §4" crates openspec docs` — every cite should resolve to a §4 text that mentions the fingerprint fence.
§4 is cited six times as the authority for the report-only fence (`specs/reconciler/spec.md:59`, `design.md:136`, `crates/dpaa2-api/src/dprc_plan.rs:686,701,762`, `crates/dpaa2-tools/src/engine.rs:108,242`). Its actual text says foreign objects are "never enumerated, let alone mutated" and that ownership is *implicit*. The epic makes ownership *explicit* (label fingerprint) and mutates foreign-but-fingerprinted containers. The cite is accurate for the report-only half only; the ADR never records the half that changed. `docs/adr/0003-...:54` inherits the same now-partial cite.

**PASS4-F3** · `openspec/changes/dprc-encapsulation/specs/formal-models/spec.md:11-13` (+ scenario `:15-17`) · **twin-drift** · superseded-by task 1.2 (the split recorded at `models/families/dprc.qnt:106-112`) · **breaks-a-claim** · **amend** the delta sentence before archive (Apalache marks on the state-expressible subset; I1/I5/I7/I9/I11 as simulate-green directed runs) · verification: `rg -n "stateInvariants" -A4 models/families/dprc.qnt` lists exactly the invariants the delta names as marked.
Delta: "Invariants DPRC-I1, I5, I7, I9, I10 and the remaining face of I11 SHALL have named, simulate-green properties, **with Apalache marks**". Shipped: `stateInvariants` (`dprc.qnt:583-586`) holds `DPRC_I10` + `DPRC_I12` only; the header at `:108-110` states outright that "I1/I5/I7/I9/I11 are action-guard, liveness or Breaking-absence properties carried as directed runs (simulate-only)". `models/COVERAGE.md:76,78,80,82` agree with the model (rung `simulate`). The model and COVERAGE are self-consistent and defensible; only the delta text (and `tasks.md:4`'s "mark … with Apalache marks") was left claiming more. ADR-0002's own "Sealed gap" amendment is the precedent for recording this in the change.

**PASS4-F4** · `models/COVERAGE.md:79` · **doc-drift** · superseded-by task 1.2 (which promised the I8 row point at #10) · **breaks-a-claim** (of task 1.2 and four spec/doc statements) · **amend** (either repoint the row or correct the five artifacts to name `pool-objects` (#6) for I8) · verification: `rg -n "DPRC-I8" models/COVERAGE.md docs/baseline/dprc.md openspec/changes/dprc-encapsulation` — all pointers agree on one tile.
The DPRC-I8 deferral row's owning change reads `` `pool-objects` (#6) `` in both the location column and the trailing arrow. Five in-scope artifacts promise #10: `proposal.md:42-44`, `design.md:37-38`, `tasks.md:4`, `specs/mbt-harness/spec.md:22-24` ("emitted as a deferral row pointing at tile #10"), `specs/mc-backend/spec.md:24-26`. `docs/baseline/dprc.md:344` hedges ("a DPL-defined child **or** the raw command path (#10)"), which is probably the true state — so #6 is likely correct and the spec text is the stale side. Note: Pass 1 reported this row as verified at #10; that check was wrong.

**PASS4-F5** · `crates/dpaa2-tools/src/main.rs:69-71` and `:94-96` (also `crates/dpaa2-tools/src/engine.rs:30-31`) · **stale** · superseded-by task 4.3 (`--prune` widened to containers) · **misleads-a-reader** · **amend** the help/doc strings · verification: `dpaa2ctl ensure --help | rg -i container` non-empty; `rg -n "declared absent" crates/dpaa2-tools/src` returns nothing that omits containers.
Both `--prune` flags still read "Tear down ports declared absent (opt-in)". After 4.3 the same flag, with `--allow disruptive`, destroys *undeclared containers* — the single most destructive semantic in the tool, and the one D8 deliberately routed through existing flag surface rather than a new one. The only user-facing statement of that widening is the runtime render header. `ConvergeConfig::prune` likewise still cites "design D7" (the intent-layer anchor) with no D8 pointer.

**PASS4-F6** · `openspec/changes/dprc-encapsulation/tasks.md:35` (vs `models/COVERAGE.md:180`) · **Scope Creep** (doc-drift) · n/a · **misleads-a-reader** · **amend** (add the dpdbg-face row to §6 or to the system-integration delta; otherwise re-attribute the suite out of this change) · verification: `rg -n "V-DPDBG-2" openspec/changes/dprc-encapsulation` returns a task/requirement row.
`COVERAGE.md:180` advances DPDBG-I4's sysfs face and attributes it to "dprc-encapsulation task 6.1" (commits 1eefc75, 1051e71). Task 6.1 authorizes only "baseline amendments from **5.x outcomes**, ADR, roadmap, deferral rows, CHANGELOG, quality floor", and `specs/system-integration/spec.md:5-14` enumerates exactly three suites (lifecycle, VFIO, end-to-end convergence). DPDBG-I4 is outside the change's declared invariant set (DPRC-I1/I5/I7/I9/I10/I11/I12). A fourth board suite advancing an out-of-family invariant has no spec row anywhere.

**PASS4-F7** · `openspec/changes/dprc-encapsulation/proposal.md:78-81` · **stale** · superseded-by task 4.1 (commit 983cfcb, derivation landed in `dpaa2-api`) and task 4.3 (tools surface is not "minor") · **misleads-a-reader** · **amend** the Impact bullet · verification: `rg -n "derive_consumer_containers" crates --files-with-matches` shows `dpaa2-api` only; `rg -n "dpaa2-config" openspec/changes/dprc-encapsulation/proposal.md` returns nothing.
Two stale Impact claims in one bullet: `dpaa2-config` (consumer→container derivation) — shipped at `crates/dpaa2-api/src/dprc_plan.rs:568`, `dpaa2-config` untouched; and `dpaa2-tools` ("convergence path exercised, minor surface") — 4.3 added a prune report, a prune dispatch pass, a renderer and three snapshots.

**PASS4-F8** · `docs/adr/0017-vfio-override-propagation-is-deferred-to-scan.md:53-56` · **doc-drift** · n/a · **carries-cost** · **amend** the consequence (say the pure core makes a post-bind create unrepresentable today) **or follow-up bead** against tile #6 when population lands · verification: `rg -n "ADR-0017" crates/` returns at least the plan/typestate site that carries the obligation.
Decision 3's second half — "a post-bind create is legal but buys a deferred visibility obligation **the plan must carry explicitly**" — has no carrier: `plan_create_resident` (`crates/dpaa2-api/src/dprc_plan.rs:280`) emits only on the unplugged face, so no post-bind create is representable and no obligation exists. Related: ADR-0017 is cited by zero files under `crates/`, against the repo standard "Code is documented with annotations tagged to ADRs" — the propagation observable at `crates/dpaa2-api/src/port.rs:224,250` is the natural anchor.

**PASS4-F9** · `docs/baseline/dprc.md:347` · **doc-drift** · superseded-by task 6.1 (row rewritten) · **misleads-a-reader** · **amend** (name the suite in the antecedent) · verification: `rg -n "corrected at rev 2" docs models` returns nothing without an adjacent suite name.
The DPRC-I11 row is headed "board-settled … (V-DPRC-12 rev 1)" and then says "Earlier rev 1 (V-DPRC-3, 2026-08-29) … — corrected at rev 2". "rev 2" has no antecedent in the row's own frame; a reader parses it as V-DPRC-12 rev 2, which does not exist. `models/COVERAGE.md:82` carries the same phrasing.

## Per-spec-delta table

| Delta requirement | Shipped evidence | Verdict |
|---|---|---|
| formal-models: dprc model carries lifecycle + laws; I1/I5/I7/I9/I10/I11 named, Apalache-marked | `models/families/dprc.qnt:113` (sum + guards), `:583-586` (marks: I10, I12 only), `:106-112` (declared split) | **DRIFTED** (PASS4-F3) |
| intent-compiler: a declared consumer derives its container (mask/placement/label/provenance; kernel derives none) | `crates/dpaa2-api/src/dprc_plan.rs:568`; tests `crates/dpaa2-api/tests/consumer_containers.rs:61,113,153,182` | **ALIGNED** (crate placement ≠ proposal prose; see disposition line) |
| reconciler: child DPRC lifecycle is typestated | `crates/dpaa2-api/src/dprc.rs:250-262,896`; `dprc_plan.rs:324` (plugged-move refusal), `:386` (assign-before-plug by construction) | **ALIGNED** |
| reconciler: containment refusals discriminated | `crates/dpaa2-api/src/dprc_plan.rs:194 attribute_mc` (0x6/0x8/0x4 → typed `Attribution`) | **ALIGNED** |
| reconciler: destroy planning encodes the eviction law | `crates/dpaa2-api/src/dprc_plan.rs:240 predict_eviction`, `:436 plan_teardown` | **ALIGNED** |
| reconciler: visibility only by re-observation | `crates/dpaa2-api/src/dprc_plan.rs:500 verdict`; `crates/dpaa2-tools/src/engine.rs:291-295` (survivor = error) | **ALIGNED** |
| reconciler: undeclared containers pruned under the double gate | `dprc_plan.rs:770 classify_container`, `:839 plan_prune`; `engine.rs:249-289` (both gates), `render.rs:225` (buckets + fields + predicted post-state) | **ALIGNED in code**, delta contradicts the base capability (F1) and the CLI help (F5) |
| reconciler: consumer convergence is container-only | `dprc_plan.rs:600 plan_consumer_container`; test `consumer_containers.rs:153` | **ALIGNED** |
| mc-backend: restool dprc verb surface, typed refusals | `crates/dpaa2-mc/src/restool.rs` (verb fns + 19 unit tests against recorded transcripts) | **ALIGNED** |
| mc-backend: KernelControl VFIO face incl. propagation observable | `crates/dpaa2-mc/src/kernel.rs:83,89,103,109` + test `:238`; `crates/dpaa2-api/src/port.rs:224,250`; `crates/dpaa2-hal/src/sysfs.rs:114` | **ALIGNED** |
| mbt-harness: online discovery sessions; divergence feedback; deferred faces emit deferral rows at #10 | `models/COVERAGE.md:72` (V-DPRC-10 rev 2), `:82` (V-DPRC-12 rev 1), ledger lint `crates/dpaa2-verify/src/ledger.rs`; deferral rows `COVERAGE.md:75,79,82` | **DRIFTED** — I8 row points at #6 (F4); rest aligned |
| system-integration: three operator-launched suites, scratch-first, reference pair | docs cite V-DPRC-9 rev 1 (`docs/baseline/dprc.md:340`), V-DPRC-8 rev 1 (`:236,308,343`), V-DPRC-12 rev 1 (`:347,414`) | **ALIGNED** for the three named suites; an unlisted fourth (V-DPDBG-2) is attributed to the change (F6) |

## dpaa2-config delta disposition

**Deliberate placement to record, not a gap — amend the proposal.** The
intent-compiler capability already declares its model half in `dpaa2-api`
(`openspec/specs/intent-compiler/spec.md:7`: "The `dpaa2-api` crate SHALL
define an `Intent` type…"), and the delta's own requirement text names no
crate; `dpaa2-config` is only the TOML parser. Deriving the container next
to `Tenant::child_dprc` and the plan vocabulary keeps the sans-io core
whole and adds no dependency to `dpaa2-config`. Fix is one line of prose
at `proposal.md:78-81` (swap `dpaa2-config` → `dpaa2-api`, drop "minor
surface" for `dpaa2-tools`); no ADR needed, no code movement proposed.

## Footer

**Read:** `openspec/changes/dprc-encapsulation/{proposal,design,tasks}.md`
and all six deltas under `specs/`;
`openspec/specs/{reconciler,intent-compiler,formal-models}/spec.md`
(targeted); `review/pass1.md`; `docs/adr/0017`, `docs/adr/0007` amendment,
`docs/adr/0011` (in-scope hits), `docs/adr/0001` §4, `docs/adr/0002`
(Apalache gate wording); `docs/baseline/dprc.md` (invariant table +
register + verdict anchors), `docs/baseline/dpdbg.md` (V-DPDBG-2 hit);
`docs/ROADMAP.md:24`; `models/COVERAGE.md` (tally + DPRC/DPDBG rows);
`models/families/dprc.qnt` (header, `stateInvariants`, `setLabelAt`,
ghosts), `models/core/machine.qnt:338-342`;
`crates/dpaa2-api/src/dprc_plan.rs` (derivation + prune + teardown),
`src/dprc.rs` (typestate faces), `src/port.rs` (VFIO/observe seams),
`src/model.rs` (grep); `crates/dpaa2-tools/src/{main,engine,render}.rs`
(flag + gate + render); `crates/dpaa2-mc/src/kernel.rs`;
`crates/dpaa2-config/src/schema.rs`; `crates/dpaa2-verify/src/ledger.rs`
(grep).

**Deliberately not read:** `models/board/` evidence, ITF traces, suite
scripts (operator-sealed, out of scope); ADR-0016 and process files;
`CHANGELOG.md` (bare-stub convention, ruled not-a-finding by the brief);
Rust bodies beyond what a spec claim needed (Passes 2–3 own code quality,
placement and isomorphism); archived changes; test bodies beyond
names/assert targets already confirmed by Pass 1.

**Open questions:**
1. F4 asks a real call: is DPRC-I8 owned by `pool-objects` (#6, a
   DPL-defined child could reach it) or `mc-portal-backend` (#10)? The
   baseline says "either". One of the two sides must move; I do not have
   the authority to pick.
2. F6: whether the V-DPDBG-2 sitting was an intentional opportunistic
   ride-along on a 6.1 board window (in which case a `tasks.md` 6.2 row
   records it) or belongs to a different change entirely. Board ledger is
   out of my scope.
3. Whether `git cliff` output at release time will carry the epic —
   inherited UNCHECKABLE-OFFLINE from Pass 1; unchanged here (no tool
   access).
