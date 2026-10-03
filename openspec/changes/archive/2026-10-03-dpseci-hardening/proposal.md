# dpseci-hardening

## Why

The dpseci-typestate review (synthesis, archive 2026-10-03-dpseci-typestate)
left three actuation-grouped beads open after the record amendments landed:
the shipped read-side decode discards the identity of an unknown firmware
bit, breaking design D4's attribution claim (latent, S1); five stale doc
comments and one wrong fixture string sit exactly where the mc-portal-backend
(#10) implementers will read (S4, S5, S12, S13, S16); and the
whole-census-poisoning law — convergence-affecting — lives Rust-only against
ADR-0002's quint-is-the-spec, while the census/destroy surface has no frozen
ITF trace (S7, S15). The epic also added the fourth hand-mirrored
`complete_kernel` copy, so one change silently de-pins four operand tests
(S14).

## What Changes

- Unknown firmware option bits are attributed as `RawEscape` into the decoded
  mask on the detail/display face (synthesis B4 arm (i)): the decode keeps
  the bit identity, the render shows it, and `families/dpseci.rs:92-94`
  becomes true as written. The census projection stays conservative —
  attributing into the census signature risks a V-LIFE-DPSECI-1 destroy loop.
- `complete_kernel` is lifted to one definition site in the dpaa2-tools
  lib/testkit; the four hand-mirrored copies collapse onto it (B6).
- The whole-census-poisoning law (any unobservable custody row ⇒ the dpseci
  face judges nothing) is stated in `models/families/dpseci.qnt` with a
  directed run, twinning `populate.rs`; dpni/pool are checked for the same
  idiom gap (B5).
- The census/destroy surface gains frozen ITF traces and a replay arm in
  `model:freeze-dpseci`, or a recorded why-not note if hand-twins suffice for
  pure operators (B7).
- Doc-comment/fixture sync, zero behavior: `plan/dpseci.rs` reworded to the
  desired-vs-desired role per D9, `contract/mc.rs`/`restool.rs` container-doc
  per the T3 keep-the-parameter ruling, hal portal/lib attribution per
  ADR-0021, `dpseci_detail.rs` fixture string `dprc.5` → `dprc.1` (B3). The
  CLAUDE.md:28 line is user-gated and excluded.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mc-backend`: the observe-side options decode attributes an unknown
  firmware bit as a raw escape instead of collapsing the whole mask to
  unknown; not-merged AND attributed both hold (design D4).
- `provisioning-cli`: the status-detail dpseci row renders an attributed raw
  bit by value, never a bare `options=[unknown]`, while unobservable stays
  honest-unknown.
- `formal-models`: the model carries the whole-census-poisoning law with a
  directed run, and the census/destroy surface is trace-frozen (or the
  hand-twin sufficiency is recorded).

## Impact

- `crates/dpaa2-mc/src/restool.rs` (decode_dpseci_options, observe docs),
  `crates/dpaa2-api/src/families/dpseci.rs`, `plan/dpseci.rs`,
  `contract/mc.rs` — decode behavior + doc sync; trait signatures
  byte-for-byte.
- `crates/dpaa2-tools` — render of attributed bits, `complete_kernel` lift,
  `tests/vdpseci3_intents.rs`, `tests/dpseci_detail.rs`.
- `crates/dpaa2-hal/src/lib.rs`, `src/portal.rs`, `docs/adr/0018` — stale
  attribution notes.
- `models/families/dpseci.qnt`, `model:freeze-dpseci`, `dpaa2-verify` replay
  tests, `models/COVERAGE.md`.
- Beads: dpaa2-controlplane-dsx (B3), -ne4 (B4+B6), -vje (B5+B7), worked one
  at a time in that order.
