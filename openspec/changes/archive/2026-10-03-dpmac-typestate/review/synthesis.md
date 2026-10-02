# Synthesis: dpmac-typestate four-pass review

Repo: the repo root. Span e2847ee..81f7b86, main. All four passes completed within budget; no pass reported partial. All severities below use the brief's scale (breaks-a-claim > misleads-a-reader > carries-cost).

## Brief erratum (record, not a code finding)

The brief's grounding said COVERAGE rows "DPMAC-I2/I3/I4/I6/I7 **+ DPNI-I3**" were synced. Pass 1's diff check and Pass 4's file check both show `models/COVERAGE.md:87` (DPNI-I3) was never touched — it still forward-routes "MAC value semantics → dpmac-typestate (#7)". The capability shipped (D5 `MacRelation`, V-DPMAC-3 rev 1 inheritance face); the ledger row lagged. The brief overstated the sync; the finding itself is MERGED-7 below.

## 1. Merged findings ledger (deduplicated, most severe first)

**MERGED-1 — The severed-witness is a token, not a binding (breaks-a-claim).**
Join of PASS1-F3 + PASS2-F1 + PASS3-F4. Spot-checked and confirmed at `crates/dpaa2-api/src/plan/transition.rs:56-57, :146-152, :161-169` and `crates/dpaa2-verify/src/board/replay.rs:113-115`.
- `SeveredProof(())` derives `Clone, Copy`, carries no `DpniId`, and `Unbind`'s `proof` field is public — any holder of any plan can destructure, extract, and reuse one proof for arbitrary dpnis.
- `sever` consumes no typestate and is uncoupled from its emitted `Disconnect`; `board/replay.rs:113` is already an in-tree mint-and-discard (`let (_severed_edge, proof) = Transition::sever(...)`).
- The model's law is two guards plus a state change (`dpmac.qnt:338-347, :356-360` — severAt consumes KernelOwned∧KernelFaceBound, yields Offered + witness); Rust carries one demand: "some sever ran".
- The doc claims "sever consumes the bound edge" (:45, :52) and "the ordering is owned by the types, not by planner discipline" (:147-149). As written these are false at edge granularity; correct pairing today is planner discipline (reconcile.rs:120-123/:209-212 — which IS correct).
- The negative-face doctest at :161 is plain `compile_fail` with no error code — it would also pass on an unrelated compile failure.
Disposition: amend (bead B1). Bind `DpniId` privately into the proof, drop `Copy`, make the field private or have `unbind` take the id from the proof, pin the doctest `compile_fail,E0423`, add a cross-dpni/reuse compile_fail, and give replay.rs a named constructor or emit the sever delta in the same window. Fallback (if the ripple is judged too wide now): `ponytail:` marker naming the ceiling + soften both doc claims so the text is true.
Verification: `cargo test -p dpaa2-api --doc unbind` (2 pinned compile_fail cases); `grep -n "owned by the types" crates/dpaa2-api/src/plan/transition.rs` matches only a true claim; `grep -n '_severed_edge' crates/dpaa2-verify/src/board/replay.rs` empty.

**MERGED-2 — Counter render shows wrong names on real values (misleads-a-reader; operator-facing).**
PASS3-F1, linked to PASS4-F3 (distinct spec-wording defect, same vocabulary split). `crates/dpaa2-tools/src/render.rs:536-594` pairs the 28 board values positionally against the 3-name MODEL slice (`counter_vocabulary(Mc1039)`), so the operator reads "IngressByteCount = <rx all frames>", "IngressPauseFrames = <rx frame errors>" — the board's byte counter is row 19, pause row 18 (`crates/dpaa2-mc/src/restool.rs:258-287`; names dropped at :328).
Disposition: amend (bead B2) — carry the verbatim restool row names inside `CounterReadout::Vocabulary` (Pass 3 mandate-d notes this is also the additive #10/10.40 prep), or render positional `counter-N`. Linked doc fix: PASS4-F3 — the formal-models delta (:13-15) implies 28 model rows; the model deliberately carries a representative 5-row slice with the 28-row vocabulary in the adapter (protected D4 shape); soften the delta parenthetical and COVERAGE:106's "named in the model" wording. PASS3-F5 folds in: fake.rs:459's literal `28` gets a canonical const from the same fix (or stays with its anchor comment — defensible).
Verification: port_detail/render test with a non-uniform scripted readout asserting the rendered name at the pause position; delta wording names the slice/adapter split.

**MERGED-3 — Doc/ledger staleness set (misleads-a-reader; one amendment commit).**
Pass 4's dispositions of Pass 1's hits are honored verbatim:
- PASS1-F1 UPHELD: `docs/baseline/dpmac.md:277` still routes raw link reads to "(#7)"; superseded by 1.3/D6 → re-route to `mc-portal-backend` (#10).
- PASS1-F4 UPHELD: `models/COVERAGE.md:87` DPNI-I3 row never synced (see erratum); cite `families/dpmac.rs` `MacRelation` + V-DPMAC-3 rev 1 instead of "(#7)".
- PASS4-F1: `models/core/invariants.qnt:154-159` comment still calls destroy acceptance "board-pending (dpmac.md unknown #1)" — doubly wrong (unknown #1 was the create question, and it is answered DPC-gated, V-DPMAC-2 rev 1). Comment-only reword; invariant body untouched.
- PASS1-F5 UPHELD (low): `models/board/README.md:85, :605, :881` — forward pointer only ("re-anchored to #10 by dpmac-typestate 1.3"); never rewrite sealed V-LINK-4 verdict prose.
- PASS1-F2 DOWNGRADED (honored): the #10 ledger promise is satisfied at COVERAGE:106; optionally fold a one-clause #10 pointer into dpmac.md in the same commit.
- PASS2-F3 (low): `dpmac_itf.rs:20-22` claims DPMAC-I4 is "covered in the Rust tile's own tests"; no `LinkStateUp` test exists — reword to "structural by construction; MC-view read deferred to #10 (D6)".

**MERGED-4 — Reconciler MAC compare has no bind-transient exclusion (carries-cost; PRE-EXISTING, not minted by this epic).**
PASS2-F5 = Pass 3 open question 1. `crates/dpaa2-api/src/plan/reconcile.rs:146, :152` with `crates/dpaa2-mc/src/parse.rs:363-364`: parse yields `Some(ZERO)` during the bind window; `Assert` reports a one-run spurious mismatch (flipping the status exit code), `Actuate` plans a SetMac against the transient. D5's Pending-never-drift is structural only on the display path; the reconciler never consumes `MacRelation` (protected — not a fix target). Ranked below the misleads class per PASS2-F5's pre-existing note; routed as its own bead (B4), not an epic amend.

**MERGED-5 — Shell-side inference leak in status.rs (carries-cost).**
PASS3-F3. `crates/dpaa2-tools/src/status.rs:128-134`: the root-peer→`PeerObservation` alphabet draw and the `unwrap_or(MacAddr::ZERO)` coercion are judgment in the imperative shell — and the ZERO coercion manufactures an observation on the one surface whose motto is absence ≠ zero (benign today only because `judge_mac_relation` maps ZERO→Pending). Hoist a pure `peer_observation_from_root(...)` and an `Option<MacAddr>`-taking relation judge into `families/dpmac.rs` (bead B3; threads with MERGED-4's zero-sentinel story).

**MERGED-6 — Two duplication clusters (carries-cost; refactor beads).**
- PASS3-F2: three `dpmac info` scanners + three token→enum spelling oracles in `crates/dpaa2-mc/src/parse.rs` (:487-520 vs :425-457 vs :557-573; tables :429-443 vs restool.rs:293-312 vs :561-566). Fold to one raw scanner + thin projections per the file's own `OPTION_BITS` precedent. Not ADR-0014-protected (lockstep is model↔Rust, not intra-adapter). Bead B5.
- PASS2-F4: third Rust `LinkType` sum added (`families/dpmac.rs:59` vs `core/model.rs:377` vs `core/inventory.rs:37`) — the qnt twin is protected vs the MODEL, but the two standing Rust sums were not reconciled. `From` seam now; collapse when the next P4 family lands. Bead B6. (Pass 2's open Q1 — check whether a design already acknowledges the triplication; if yes the bead shrinks to a doc pointer.)

**MERGED-7 — Model guard narrowing on severAt (misleads-a-reader, low; comment-only).**
PASS2-F2. The planner emits Disconnect for never-bound dpnis; the model's `severAt` is enabled only at KernelOwned∧KernelFaceBound, so guard-for-guard holds only on the unbind side. No hardware hazard. Disposition: one comment in `dpmac.qnt` naming the planner's wider emission (cite D3's single-edge scope if it already names it). Folds into the MERGED-3 amendment commit.

**MERGED-8 — ADR-0019 Status trail + D2 wording (synthesis calls, both low).**
- PASS4-F2: the trail lists the dated 2026-09-20 amendments but not the dpmac-typestate 6.1 one, so a reader misattributes the edge facet. **Call: add the one-line 6.1 entry.** The trail already carries dated entries; adding one line is less churn than retro-converting the trail to atemporal, and the body stays atemporal per the style directive.
- Pass 4 open Q2 / Pass 2 confirmation: the family ships a plain `Arbitration` enum judged from observation with no family-internal phase-marker typestates — matching D3/ADR-0019 ("trigger met without family-internal markers"). D2's phrase "in the P1 phase-marker idiom" is a one-line design-doc softening; the D2 decision itself (observation-judged markers, not a cfg facet) is protected and untouched. Folds into MERGED-3's commit.
- Pass 1 open Q1 (dprtc.md): resolved by Pass 4 — COVERAGE:180 suffices, no dprtc.md edit.

**Dropped / not-findings.** No pass attacked a protected decision; nothing dropped on that ground. No unanchored staleness claims survived (every stale row carries its superseding commit). Pass 2's liveness note (NoPeer handback wedges the model branch witness-less) is recorded as a note only — no bead until random simulation depth grows (YAGNI). The b6782ed interleave was correctly treated as context, not a defect, by Pass 3.

## 2. Verdict — core promise: **YES, with one typed-law qualification**

| Law | Evidence | Holds |
|---|---|---|
| Arbitration phases observation-judged | P2(a): `Arbitration`/`judge_arbitration` twins `observeArbitrationAt`, all 3 arms replayed (`arbitrationLawTest`); P3(a): judgment pure in `families/dpmac.rs`, shim/hal plumbing only | Yes |
| Counter absence ≠ zero unrepresentable | P2(d): `CounterRead::NotInVocabulary` distinct constructor, proptested `!= Known(0)` for every raw value; P3(b): zero counter consumers in plan/reconcile/engine | Yes (render *labels* defective — MERGED-2 — but the type law holds) |
| MAC relation typed, Pending not drift | P2(d): `MacRelation` sum, Pending first structural branch (`dpmac.rs:526-527`), no string compare; reconciler never consumes it (protected D5) | Yes on the judgment path; the reconciler's separate raw compare lacks a transient guard (MERGED-4, pre-existing) |
| Sever-then-unbind by type, no driverless interval | Negative face in 3 forms (Quint `.fail()` dpmac.qnt:583-586, `compile_fail` transition.rs:161, `inverted_edge_trace_is_rejected` replay:273); board fact V-DPMAC-3 rev 1 8/8, dpmac.7 left standalone-bound, no driverless interval | Qualified: order held by type at the API boundary (no consumer can unbind with zero severs), but per-edge pairing is planner discipline — the proof is Copy, unbound, free-minted (MERGED-1). Behavior correct; the "owned by the types" claim overreaches |
| Laws two-sided across the sans-io seam | P2(b) + P3(a/b): edge law dpni↔dpmac only on both sides, no leak, display-only end-to-end with the witness the planner's one typed demand | Yes |

## 3. Verdict — ADR-0002 compliance: **YES**

| Facet | Evidence |
|---|---|
| Name-for-name family map | P2(a): every model sum/payload/pure-def has a same-named Rust twin in `families/dpmac.rs` (BOOT_MAC…SeveredWitness list); camelCase→snake_case only rename |
| Requests-down unreadable by construct, both sides | One payload-free constructor each side (dpmac.qnt:152, dpmac.rs:177-181); zero Rust consumers |
| Edge facet two-sided, dpmac-only | `edgeDemandsSeveredWitness` dpni↔dpmac only (connect.qnt:42-43, vacuous elsewhere); one Rust `Unbind`, always dpmac-facing |
| Replay coverage | 8 traces = 8 rows, orphan-guarded; I2/I3/I6/SeverOrder re-asserted at every state; refusal arm the law targets is the one replayed; unreplayed arms are the D6/#10 deferral and two disabled faces — COVERAGE says "directed run", accurate |
| Recorded asymmetries (not breaks) | MERGED-1 (Rust proof weaker than the model's consume-and-yield — the one iso-violation, dispositioned amend) and MERGED-7 (severAt narrower than planner emission — comment-only) |
| Rust-only types | Each documented as a D4/D5/D6 decision outside the qnt — protected, not iso gaps |

## 4. Proposed follow-up (bead-shaped, for a just-in-time openspec change)

- **B1 — Bind SeveredProof to its edge.** Why: MERGED-1; the shipped doc claim is false at edge granularity and replay.rs already mint-and-discards. Acceptance: proof stores `DpniId` privately, no `Copy`, no public-field extraction path; doctest pinned `compile_fail,E0423` plus a cross-dpni/reuse compile_fail; replay.rs mint-and-discard replaced; transition.rs doc claims true as written; `cargo test -p dpaa2-api --doc unbind` and dpmac_replay green.
- **B2 — Verbatim counter names ride CounterReadout::Vocabulary.** Why: MERGED-2 shows wrong operator-facing labels; same change is the additive #10/10.40 prep. Acceptance: render test with non-uniform scripted readout asserts the correct name at the pause position; fake's `28` sourced from a named const; formal-models delta + COVERAGE:106 wording names the slice/adapter split (PASS4-F3).
- **B3 — Hoist status.rs port-detail inference into the family.** Why: MERGED-5; shell-side judgment + a manufactured ZERO observation. Acceptance: `grep -n 'SameContainerKernelPeer\|MacAddr::ZERO' crates/dpaa2-tools/src/status.rs` empty; pure fns in families/dpmac.rs; dpaa2-tools tests green.
- **B4 — Zero-MAC bind-transient exclusion in reconcile (pre-existing).** Why: MERGED-4; spurious Assert mismatch / SetMac against the bind-window transient. Acceptance: unit test observed `Some(MacAddr::ZERO)` + Assert ⇒ no mismatch; Actuate posture decided and tested in the bead.
- **B5 — One dpmac-info scanner.** Why: MERGED-6a. Acceptance: `grep -c 'strip_prefix("DPMAC link type:")' crates/dpaa2-mc/src/parse.rs` = 1; single token table per OPTION_BITS precedent; dpaa2-mc tests green.
- **B6 — Reconcile the three Rust LinkType sums.** Why: MERGED-6b. Acceptance: `From` impls at one documented seam now; collapse decision recorded for the next P4 family; grep shows one definition or the documented seam.
- **B7 — Doc/ledger amendment commit.** Why: MERGED-3 + MERGED-7 + MERGED-8. Acceptance: all verification greps pass — dpmac.md:277 → #10; COVERAGE:87 cites MacRelation + V-DPMAC-3; invariants.qnt comment cites V-DPMAC-2; board README forward pointers; dpmac_itf.rs comment names D6/#10; dpmac.qnt severAt narrowing comment; ADR-0019 Status gains the 6.1 line; D2 "phase-marker idiom" phrase softened; optional dpmac.md #10 clause for bulk statistics.

Suggested order: B7 (pure docs, no deps) ∥ B1; then B2 (its doc tail joins B7 if sequenced first); B3/B4 together (shared zero-sentinel story); B5/B6 anytime.

## Rule amendment: **none nominated**

The one-writer exemption (b6782ed) fired on a four-crate API fan-out (f4782bf → c8d0c2b → c2d61af → 71c2646); Pass 3 found no defect tracing to it — the exemption worked as designed. No finding shows a rule failed to prevent a defect, so the 12-rule set stands unamended. If B1 lands, its API ripple will exercise the same exemption; that is usage, not a rule gap.

Tokens consumed: ~35k of the ~70k budget.
