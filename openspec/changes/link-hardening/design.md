# Design

## Context

See proposal.md — Why. The change executes epic dpaa2-controlplane-7on;
every child bead is self-contained (exact sites, chosen alternatives,
inline verifications in its notes), so this design records only the
decisions already made at review synthesis and the constraints that
bound every parcel. The one-task-at-a-time rule applies (children in
priority order: 0d0 → 5a2 → e9k → ceg → wl7 optional); child ckx (S12,
S13 lift) is out of scope — it rides the #11 mc-portal-backend proposal.

Standing constraints inherited from cross-dprc-links:

- **D3 bounded lift**: dpmac machinery is claimed, not retyped. Across
  the whole change, `git diff` over `models/traces/families/dpmac/` and
  `crates/dpaa2-verify/tests/dpmac_replay.rs` is empty and dpmac_replay
  passes 8/8. A diff fails the parcel.
- **D2 refusal authority**: the MC stays the refusal authority — never
  pre-forbid in the model what only the MC refuses.
- **Sans-io**: judgments live in dpaa2-api; adapters execute/observe.
- **Board facts are the oracle**: V-LINK-6 rev 1 (2026-10-05, 9/9),
  record in `models/board/VERDICTS.json` (V-LINK-6-rev1) and board
  README:110. Task 7.5 (27b99e2) already weakened the model to match.

## Goals / Non-Goals

**Goals:**

- Close the fake-vs-board fidelity gap so the fake can no longer hide
  the one-sided held pre-pass gap (the two fix together — bead 0d0).
- Promote two engine disciplines to type law (bead 5a2) per the
  ADR-0021 typestate posture: unrepresentability proven at compile
  time.
- Re-establish the ADR-0002 law-for-law map after the 7.5 weakening
  (bead e9k): every LINK_I1 citation pairs with the state face; the one
  `.fail()` trace binds a real Rust refusal.
- Leave every deferral-ledger arrow ending at a change that can fire
  its trigger (bead ceg).

**Non-Goals:**

- No McControl trait restructuring (wl7 explicitly bounded: per-command
  granularity is the D6 portal seam for #10).
- No model strengthening: the model keeps the board's answer; Rust
  stays the stricter party, recorded as refinement notes.
- No S12/S13 lift (wire class + end census into plan/connect.rs) — that
  is bead ckx under #11.

## Decisions

1. **plan_wire widens to `(a, b, a_observed, b_observed)`** rather than
   an engine-only pre-pass fix: the refusal law belongs in dpaa2-api
   (sans-io); the engine pre-pass reads both ends and feeds the pure
   judgment. Alternative (engine-only second read) rejected — it leaves
   the api surface able to plan a silent overwrite.
2. **Fake fidelity encodes the transcript, not symmetry**: the expected
   side of the new peer-read test is the V-LINK-6 verbatim outcome
   (survivor reads None; destroyed id absent from pool observation). A
   merely-symmetric fake that disagrees with the transcript is wrong.
3. **Unforgeability by compile_fail doctest, not runtime test** (5a2):
   the claim is unrepresentability, so the proof is a forged
   `plan.discharge` and a bare-WireEnd mint failing to COMPILE — idiom
   precedent: the immutability doctest in `families/dpseci.rs`. The
   child twin (`ChildRebindCycle`, fields private, minted only inside
   `discharge_child`) is the pattern the dpni twin adopts.
4. **destroy_resident_stale takes `VisibleEndpoint`** — the witness two
   methods up already exists; the model mints StaleNode only for a
   busVisible end (link_lifecycle.qnt:239–253).
5. **S14 is doc-only**: claim the ORDER, not held-ness. Gating the
   disconnect-proof mint would change the planner seam against the
   bounded posture.
6. **S10 fix is behavioral, not doc softening**: at the Refused
   sentinel drive plan_wire against the prior world and assert a
   non-Connect verdict (~8 lines) — the replayer then actually
   witnesses what link_itf.rs claims.
7. **S24 rename over insta snapshot**: snapshot churns on every
   unrelated derivation change; the real 7.6 tripwire is the
   full-Compiled equality already in the file.
8. **S26 verify-first**: whether dpni_info_unconnected.txt is a
   verbatim never-connected capture decides fixture-fix vs
   baseline-nuance; the fixture is not touched before the question is
   answered.

## Risks / Trade-offs

- [plan_wire signature change ripples to callers/tests] → the bead
  lists the known readers; quality-floor plus `cargo test -p dpaa2-api
  -p dpaa2-tools` gate each parcel.
- [Fake edits could drift from board semantics elsewhere] → the new
  peer-read test pins the transcript values; pnpm model:test loops all
  board models.
- [Doc sweep could over-delete LINK_I1 citations] → correct wording
  precedent is in-file (link_replay.rs:415–416, :433–434); acceptance
  greps for the retired pairing, not for absence of LINK_I1.
- [Hardening leaking into dpmac machinery] → byte-diff gate (D3) fails
  the parcel, not just the epic close.

## Migration Plan

Each bead lands as its own conventional commit (amendable until
sealed), gated by `scripts/checks/quality-floor.sh`, `pnpm model:test`,
and `cargo test -p dpaa2-verify`. Close-out runs `/epic-review
link-hardening` before `/opsx:archive`, with the protected-decisions
list carried forward from the cross-dprc-links brief (V-LINK-6
outcomes, D3 bounded lift, D2 refusal authority, 7.6 provenance-only
knob, rule R1).

## Open Questions

- S26: verbatim capture or hand-typed fixture? Answered inside bead ceg
  before the fixture is touched (affects which of two recorded fixes
  applies, not the approach).
- S28: is the VERDICTS.json V-TRAF-1 "run" key hand-maintained or
  operator-ingest-derived? Same pattern — answered in-bead.
- wl7 (spawn cache) is optional; executed last only if its cost stays
  low at acceptance time.
