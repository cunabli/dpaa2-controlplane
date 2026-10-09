# Proposal

## Why

The cross-dprc-links epic review (synthesis sealed at e1ff69b, change archived
at adcdc36) judged the core promise and ADR-0002/ADR-0022 compliance YES but
left ranked findings the archive window did not cover: the contract fake
contradicts the board-anchored symmetric-edge law (V-LINK-6), two type-law
gaps let the Rust surface under-enforce the model, post-7.5 wording still
pairs LINK_I1 with a retired refusal reading, and deferral-ledger rows still
point at the archived change as a live carrier. This change carries epic
dpaa2-controlplane-7on (children 0d0, 5a2, e9k, ceg, wl7 optional; ckx waits
for the #11 proposal and is out of scope).

## What Changes

- **Fake edge fidelity (S5, S6)**: the contract fake reproduces the V-LINK-6
  rev 1 board transcript — disconnect and destroy drop BOTH endpoints-mirror
  entries, destroy drops the pool row, so the survivor reads
  `dprc_get_connection == None` and the destroyed id leaves
  `observe_pool(None, Dpni)`. `plan_wire` widens to take both ends'
  observations; the engine held pre-pass reads both ends; a foreign-held b
  end yields the typed held refusal instead of planning Connect.
- **Typed-surface hardening (S3, S11)**: `PostBindCreate` obligation and
  discharge fields go private behind accessors so `discharge()` (the
  Class::Disruptive consent gate) is the sole RebindCycle yield — proven by a
  compile_fail doctest, not a runtime test. `destroy_resident_stale` takes
  the existing `VisibleEndpoint` witness so the StaleNode residue mints only
  for a bus-visible end, matching the model.
- **LINK_I1 wording and replay-claim sweep (S4, S10, S14, S15, S16, S24)**:
  retire the disconnect-before-destroy-as-LINK_I1 pairing at the five stale
  sites; make the Refused sentinel drive `plan_wire` against the prior world
  so the one `.fail()` trace binds the Rust refusal to the model guard; fix
  the held-ness and dataflow overclaims; add the D4 stricter-Rust refinement
  note on `connectWireAt`; rename the vdpcon1 tripwire test.
- **Deferral-ledger and record sweep (S18, S20–S23, S26, S28)**: re-point
  owner arrows from archived #9 to changes that can fire their triggers,
  record the delivered ADR-0017 heal and the rev-4 partial answers, qualify
  the ADR-0022 ordering law as engine policy stricter than hardware, resolve
  the dpni_info_unconnected fixture's endpoint-state contradiction.
- **Optional (S27)**: per-pass container-rows cache in the link pass, ~7
  restool spawns per edge down to ~3, without touching the McControl trait.

## Capabilities

### New Capabilities

None — this change hardens surfaces the cross-dprc-links change introduced.

### Modified Capabilities

- `reconciler`: the connection surface's held refusal becomes two-sided
  (`plan_wire` judges both observed ends); the post-bind rebind consent gate
  becomes type law (private obligation bundle, compile_fail-proven); the
  stale-node mint demands the bus-visibility witness; the contract fake obeys
  the V-LINK-6 symmetric-edge law verbatim.
- `formal-models`: the link replayer witnesses the refused action against
  the prior world — the Refused sentinel drives the planner and asserts a
  non-Connect verdict, binding the Rust refusal to the model guard
  (ADR-0002 isomorphism); the model carries the D4 stricter-Rust
  refinement note on `connectWireAt`.

## Impact

- `crates/dpaa2-api`: `plan/connect.rs` (plan_wire signature, PostBindCreate
  privacy, doc sweep), `families/dprc.rs` (VisibleEndpoint parameter),
  `contract/fake.rs` (edge symmetry on disconnect/destroy, both-ends
  precondition).
- `crates/dpaa2-tools`: `engine.rs` held pre-pass reads both ends;
  `tests/vdpcon1_intents.rs` rename; optional spawn cache.
- `crates/dpaa2-verify`: `tests/link_replay.rs` sentinel drive + wording;
  `tests/link_itf.rs` claim fix.
- `crates/dpaa2-mc`: `tests/fixtures/dpni_info_unconnected.txt` (after the
  capture-provenance question is answered).
- `models/`: one D4 refinement comment in `families/link_lifecycle.qnt`;
  `COVERAGE.md` rows; `board/VERDICTS.json` key (conditional).
- `docs/`: ADR-0022 ordering-law clause; baseline dpni.md/dprc.md rows.
- Bounded-lift constraint: `models/traces/families/dpmac/` and
  `tests/dpmac_replay.rs` stay byte-unchanged across the whole change
  (design D3 of cross-dprc-links).
