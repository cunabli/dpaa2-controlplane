# dpseci-hardening — design

## Context

dpseci-typestate shipped the P2 crypto surface and its review (synthesis in
the 2026-10-03 archive) merged sixteen findings into four beads; the markdown
record bead is closed, three remain. The sharpest finding is latent: design
D4 claims an unknown firmware bit is "attributed, never silently merged",
but `decode_dpseci_options` returns `None` on any unnamed bit — not-merged
holds, attributed does not. The model side carries escapes on the observable
face (`createRawEscapeReadbackTest`), so the Rust decode is the drifted
twin. Separately, the whole-census-poisoning law exists only in
`populate.rs`, against ADR-0002's quint-is-the-spec, and the census/destroy
surface lacks the frozen-trace replay oracle the create surface has.

## Goals / Non-Goals

**Goals:**

- The D4 attribution claim holds end to end on the display face: decode
  keeps the unknown bit's identity, the detail row shows it by value.
- One `complete_kernel` definition site; the four operand pins stay green
  and cannot silently diverge.
- The census-poisoning law stated in Quint with a directed run, and the
  census/destroy surface trace-frozen with a replay arm (or a recorded
  sufficiency note).
- The five stale doc comments and the fixture string corrected with zero
  behavior change.

**Non-Goals:**

- No census-signature change: an attributed bit never enters the census
  projection (destroy-loop risk, V-LIFE-DPSECI-1).
- No new MC commands, no whitelist change, no board sitting — offline gates
  only.
- CLAUDE.md:28 (user-gated, out of every parcel).

## Decisions

- **D1 — attribution arm (i) over honest-gap arm (ii)** (synthesis B4).
  The decoded mask type gains a raw-escape carrier so an unnamed bit rides
  through decode with its bit position intact; render shows it (e.g.
  `options=[HasCg, raw:bit7]`) instead of `options=[unknown]`, closing the
  render asymmetry between the desired (:170) and observed (:621) faces.
  Arm (ii) — recording the gap in COVERAGE/ADR and rewording
  `families/dpseci.rs:92-94` — was the fallback; (i) is chosen because D4's
  text is the agreement and the model twin already carries escapes, so the
  code moves to the spec, not the spec to the code. The census projection
  is untouched: attribution is display-face only.
- **D2 — census stays conservative.** An unknown bit still projects the
  object Unobservable for judgment purposes exactly as today; only the
  detail/display face gains information. This is the synthesis's own safety
  bound (attributing INTO the census signature risks a destroy loop).
- **D3 — `complete_kernel` lifts into the dpaa2-tools testkit** (lib-side,
  `cfg(test)`-reachable from integration tests), the single definition the
  binary and all four pins consume. Alternative — a shared fixture crate —
  rejected: one function does not earn a crate.
- **D4 — the poisoning law lands as a pure predicate + directed run** in
  `models/families/dpseci.qnt`
  (`censusUnobservableMemberJudgesNothingTest`), twinning
  `populate.rs:150-157`'s guard; a one-line corpus check records whether
  dpni/pool share the idiom gap (follow-up bead if yes, note if no).
- **D5 — freeze-or-record for census/destroy traces.** Extend
  `model:freeze-dpseci` with census/destroy runs plus a dpaa2-verify replay
  arm; if the pure-operator surface genuinely cannot yield a meaningful
  trace, the why-not note lands in COVERAGE instead. The implementing task
  decides from what the freeze run shows, per synthesis B7's explicit
  either/or.

## Risks / Trade-offs

- [Raw-escape carrier widens the decoded type] → display-face only; the
  census projection and family judgment signatures are asserted unchanged
  by the existing replay/pin tests.
- [Comment-sync parcel drifts into behavior] → trait signatures
  byte-for-byte is an acceptance grep; quality-floor green gates each bead.
- [Freeze run produces trivial traces] → D5's either/or lands the recorded
  note instead of ceremony traces.

## Migration Plan

Three beads, one at a time, dependency-ordered dsx → ne4 → vje; each closes
through its acceptance greps plus `scripts/checks/quality-floor.sh` before
the next starts. No deployment surface; no board run.
