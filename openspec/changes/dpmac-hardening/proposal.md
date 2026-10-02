# dpmac-hardening

## Why

The dpmac-typestate epic review (openspec/changes/dpmac-typestate/review/
synthesis.md) merged the four passes into 8 findings: core promise YES with
one typed-law qualification, ADR-0002 isomorphism YES. This change lands the
dispositioned findings (epic bead dpaa2-controlplane-e6s). The qualification
is the lead defect: the severed-witness the planner consumes is a token, not
a binding — `SeveredProof` is `Copy`, carries no `DpniId`, and `sever` mints
it without consuming anything, so the sever-then-unbind order is held by
type only at the API boundary while per-edge pairing is planner discipline.
The owner picked the real fix (bind the proof to its edge), not the
doc-softening fallback (decision 2026-10-03, recorded on the review commit
92072e3 and in design D1 here).

## What Changes

- **Bead B1 (e6s.1, leads with B7):** `SeveredProof` binds its edge — the
  proof stores `DpniId` privately, loses `Clone`/`Copy`, the `Unbind` field
  stops being publicly extractable, `unbind` takes the id from the proof;
  the replay mint-and-discard is replaced; the doctest pins
  `compile_fail,E0423` and gains a cross-dpni/reuse case; the transition.rs
  doc claims become true as written (MERGED-1).
- **Bead B2 (e6s.2):** the verbatim restool counter row names ride
  `CounterReadout::Vocabulary` instead of being dropped at the shim, so the
  port-detail view stops pairing board values positionally against the
  3-name model slice — wrong labels on real values today; the same carrier
  is the additive #10/10.40 vocabulary prep. Doc tail: the formal-models
  delta and COVERAGE wording name the representative-slice/adapter split
  (MERGED-2, PASS4-F3; D4 protected and unchanged).
- **Bead B3 (e6s.3):** the status.rs root-peer alphabet draw and the
  `unwrap_or(MacAddr::ZERO)` coercion hoist into pure `families/dpmac.rs`
  judgments — no manufactured observation on the absence-≠-zero surface
  (MERGED-5).
- **Bead B4 (e6s.4):** the reconciler's raw MAC compare gains the
  bind-transient exclusion: a `Some(ZERO)` read-back is unobserved, not an
  Assert mismatch or an Actuate SetMac target (MERGED-4; pre-existing
  dpni-typestate line). Threads with B3's zero-sentinel story.
- **Bead B5 (e6s.5):** one raw `dpmac info` scanner with thin projections
  replaces the three scanners and three token→enum spelling oracles, per
  the file's own `OPTION_BITS` precedent (MERGED-6a).
- **Bead B6 (e6s.6):** the three Rust `LinkType` sums reconcile behind one
  documented `From` seam now; the collapse decision is recorded for the
  next P4 family (MERGED-6b; contingent — shrinks to a doc pointer if a
  design already acknowledges the triplication).
- **Bead B7 (e6s.7, pure docs, parallel with B1):** the doc/ledger
  amendment commit — dpmac.md and board-README #7→#10 re-routes, the
  never-synced COVERAGE DPNI-I3 row, the invariants.qnt unknown-#1 comment,
  the dpmac_itf.rs coverage claim, the severAt-narrowing model comment, the
  ADR-0019 Status line, the design-D2 phase-marker phrase
  (MERGED-3/7/8).
- **No rule amendment**: the synthesis nominated none — the one-writer
  exemption (b6782ed) worked as designed on the epic's four-crate fan-out.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `reconciler`: the severed-witness proof is bound to the edge it severs
  (unforgeable, uncopyable, consumed per-edge); a zero read-back MAC is a
  bind-window transient, never drift or an actuation target.
- `mc-backend`: the dpmac counter readout carries the verbatim restool row
  names beside the values; the `dpmac info` text has one raw scanner.
- `provisioning-cli`: port-detail counter rows render under their verbatim
  board names; the port-detail inference (peer alphabet, absent-MAC
  handling) is a pure family judgment, not shell code.

## Impact

- `crates/dpaa2-api` (plan/transition.rs, plan/reconcile.rs,
  families/dpmac.rs, contract/fake.rs, core link-type seam)
- `crates/dpaa2-mc` (restool.rs readout, parse.rs scanner fold)
- `crates/dpaa2-tools` (status.rs, render.rs, port_detail tests)
- `crates/dpaa2-verify` (board/replay.rs, dpmac_itf.rs comment)
- `models/` (COVERAGE.md, core/invariants.qnt comment, families/dpmac.qnt
  comment, board/README.md pointers), `docs/` (baseline/dpmac.md, adr/0019
  Status line), archived-change design D2 phrase, the dpmac-typestate
  formal-models delta wording (wherever it lives when B2 runs)
- Ordering: B7 ∥ B1 first; then B2; B3+B4 together; B5/B6 anytime. One
  bead at a time through acceptance.
