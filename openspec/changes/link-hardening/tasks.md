# Tasks

Each group maps 1:1 to an epic child bead (dpaa2-controlplane-7on); the
bead's notes carry the exact sites and the bead's acceptance criteria
gate the group. One group at a time, through acceptance, in this order.
Standing gates on every group: `scripts/checks/quality-floor.sh`,
`pnpm model:test`, `cargo test -p dpaa2-verify`; D3 byte-diff over
`models/traces/families/dpmac/` + `tests/dpmac_replay.rs` stays empty.

## 1. Fake edge fidelity + symmetric held refusal (bead 0d0 — S5, S6)

- [x] 1.1 Make the contract fake obey the V-LINK-6 symmetric-edge law:
      disconnect drops both endpoints-mirror entries; destroy drops
      both entries AND the pool_objects row (fake.rs:586-594, :636-646,
      :693-707). Verify: new peer-read test — connect pair,
      disconnect/destroy one end, assert
      `dprc_get_connection(peer) == None` and
      `observe_pool(None, Dpni)` omits the destroyed id (transcript
      values, V-LINK-6-rev1).
- [x] 1.2 Widen `plan_wire(a, b, a_observed)` with `b_observed`
      (plan/connect.rs:418); engine held pre-pass reads both ends
      (engine.rs:1391-1401); fake gains the both-ends precondition.
      Verify: a foreign-held b end yields RewireRefused/Held offline
      and in link_faces; `cargo test -p dpaa2-api -p dpaa2-tools`
      green.

## 2. Typed-surface hardening (bead 5a2 — S3, S11)

- [x] 2.1 Make `PostBindCreate` obligation/discharge fields private
      behind accessors; `discharge()` stays the sole RebindCycle yield;
      adjust known readers (connect.rs tests, link_replay.rs:176). Add
      compile_fail doctest forging `plan.discharge` (dpseci idiom).
      Verify: `grep -n 'pub obligation\|pub discharge'
      crates/dpaa2-api/src/plan/connect.rs` empty; doctest fails to
      compile the forgery.
- [x] 2.2 `destroy_resident_stale` takes `VisibleEndpoint` instead of
      bare `WireEnd` (families/dprc.rs:1089-1096); link_replay.rs:203-204
      adjusts to `VisibleEndpoint::observe(we, true)`. Verify:
      `cargo test -p dpaa2-api -p dpaa2-verify --test link_replay`
      green.

## 3. LINK_I1 wording and replay-claim sweep (bead e9k — S4, S10, S14, S15, S16, S24)

- [x] 3.1 Retire the LINK_I1-as-refusal pairing at the stale sites
      (connect.rs:195-197, :259-261, :311, :320-321, :334;
      link_replay.rs:26, :264-266, :280) using the in-file wording
      precedent (:415-416). Fix S14 held-ness overclaim (claim the
      ORDER) and S16 dataflow overclaims (+ attribute_wire_refusal
      banked-pattern note). Verify: greps for the retired pairing,
      'actually held', 'carrier consumes' come back empty.
- [x] 3.2 Drive plan_wire at the Refused sentinel against the prior
      world and assert a non-Connect verdict (~8 lines,
      link_replay.rs:304-313); drop disconnect-before-destroy from the
      link_itf.rs refusal enumeration. Add the D4 refinement note on
      `connectWireAt` (link_lifecycle.qnt:211-216, the :234-235 idiom).
      Rename `unset_knob_derives_byte_identically` to
      `unset_knob_adds_no_node` (vdpcon1_intents.rs:74-81). Verify:
      `pnpm exec quint test models/families/link_lifecycle.qnt --
      main=link_lifecycle` 13/13; `cargo test -p dpaa2-verify --test
      link_replay` green incl. the new sentinel assertion.

## 4. Deferral-ledger and record sweep (bead ceg — S18, S20-S23, S26, S28)

- [x] 4.1 Apply the per-row fixes: ADR-0022:103 policy-vs-hardware
      clause; COVERAGE:121 w01 heal delivered; COVERAGE:129 DPCON-I3
      arrow to mc-portal-backend (#10); dpni.md rows #4/#7/#11/#13;
      dprc.md:350 DPRC-I5 note. Verify: the per-row greps in the bead
      notes; `cargo test -p dpaa2-verify --test ledger_lint` green.
- [x] 4.2 Answer the S26 capture question FIRST, then fix the fixture
      to -1 or record the never-connected baseline nuance; S28 rename
      VERDICTS.json V-TRAF-1 "run" key only if hand-maintained.
      Verify: fixture shows -1 OR the nuance exists;
      `cargo test -p dpaa2-mc` green.

## 5. Link-pass spawn cache (bead wl7 — S27, OPTIONAL)

- [ ] 5.1 Per-pass container-rows cache + reuse the pre-pass connection
      read in the dispatch arms (engine.rs sites in the bead); do NOT
      touch the McControl trait. Verify: RecordingRunner run_verb count
      per link drops ~7 → ~3; `cargo test -p dpaa2-tools` green.

## Workflow follow-up

- Epic close-out: run `/epic-review link-hardening` BEFORE
  `/opsx:archive`, brief from the newest template
  (archive/2026-10-07-cross-dprc-links/review/brief.md), carrying the
  protected decisions forward; budget at the 1.7x correction with the
  1.5x stop-partial rule enforced at acceptance.
- Bead ckx (S12/S13 lift) waits for the #11 mc-portal-backend proposal
  and reparents there; bead cmt (R2/R3 rule nominations) is a user
  call.
