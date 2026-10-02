# dpmac-hardening — tasks

Ordering per design D5: B7 ∥ B1 first, then B2, then B3+B4 together (one
zero-sentinel story, design D3), B5/B6 anytime after B2. One bead at a time
through acceptance; each group is one bead → one parcel → one commit.
Verification commands live on the beads; synthesis cross-reference in
openspec/changes/archive/2026-10-03-dpmac-typestate/review/synthesis.md §4.

## 1. Bead B7 — doc/ledger amendment (dpaa2-controlplane-e6s.7)

- [ ] 1.1 `docs/baseline/dpmac.md:277`: raw link reads re-route "(#7)" →
  `mc-portal-backend` (#10); optional one-clause #10 pointer for bulk
  statistics (PASS1-F1, F2-downgraded)
- [ ] 1.2 `models/COVERAGE.md:87` DPNI-I3: replace the never-synced
  "→ dpmac-typestate (#7)" with `families/dpmac.rs` `MacRelation` +
  V-DPMAC-3 rev 1 (PASS1-F4; brief erratum)
- [ ] 1.3 `models/core/invariants.qnt:154-159`: comment cites the DPC-gated
  answer (V-DPMAC-2 rev 1); destroy caveat moves to its real anchor;
  invariant body untouched (PASS4-F1)
- [ ] 1.4 `models/board/README.md:85,:605,:881`: forward pointer only
  ("re-anchored to #10 by dpmac-typestate 1.3"); sealed V-LINK-4 verdict
  prose never rewritten (PASS1-F5)
- [ ] 1.5 `crates/dpaa2-verify/src/intent/dpmac_itf.rs:20-22`: comment
  reworded to "structural by construction; MC-view read deferred to #10
  (D6)" (PASS2-F3)
- [ ] 1.6 `models/families/dpmac.qnt`: one comment naming the planner's
  wider Disconnect emission vs `severAt`'s guard, citing D3's single-edge
  scope (MERGED-7)
- [ ] 1.7 ADR-0019 Status trail gains the one-line dpmac-typestate 6.1
  entry; dpmac-typestate design D2's "in the P1 phase-marker idiom" phrase
  softened to the shipped observation-judged enum (MERGED-8)
- [ ] 1.8 Gates: ledger lint green; `scripts/checks/quality-floor.sh` PASS;
  close bead e6s.7; commit

## 2. Bead B1 — SeveredProof binds its edge (dpaa2-controlplane-e6s.1)

- [ ] 2.1 `crates/dpaa2-api/src/plan/transition.rs`: `SeveredProof` stores
  `DpniId` privately; `Clone`/`Copy` dropped; `Unbind`'s proof field no
  longer publicly extractable; `unbind` takes its target from the proof
- [ ] 2.2 `plan/reconcile.rs` call sites (:120-123, :209-212) pass the
  bound proof; behavior unchanged
- [ ] 2.3 `crates/dpaa2-verify/src/board/replay.rs:113-115`: mint-and-
  discard replaced (named constructor or the sever delta emitted in the
  same window)
- [ ] 2.4 Doctest pinned `compile_fail,E0423`; second compile_fail for the
  cross-dpni/reuse face; transition.rs doc claims (:45/:52, :147-149) true
  as written
- [ ] 2.5 Gates: `cargo test -p dpaa2-api --doc unbind`, lib transition +
  dpmac tests, `dpmac_replay` green; quality floor PASS; close bead
  e6s.1; commit

## 3. Bead B2 — verbatim counter names (dpaa2-controlplane-e6s.2)

- [ ] 3.1 `CounterReadout::Vocabulary` carries the verbatim restool row
  names (shim stops dropping them at `restool.rs:328`); fake's literal 28
  resolves to a named const (PASS3-F5)
- [ ] 3.2 `crates/dpaa2-tools/src/render.rs:536-594`: rows render under
  their carried names; positional pairing against the model slice deleted
- [ ] 3.3 Render/port_detail test: non-uniform scripted readout asserts the
  correct name at the pause position
- [ ] 3.4 Doc tail: the dpmac-typestate formal-models delta wording +
  `models/COVERAGE.md:106` name the representative-slice/adapter split
  (PASS4-F3)
- [ ] 3.5 Gates + close bead e6s.2; commit

## 4. Beads B3 + B4 — the zero sentinel settles (e6s.3, e6s.4)

- [ ] 4.1 B3: pure `peer_observation_from_root(Option<&ObservedDpni>)` and
  an `Option<MacAddr>`-taking relation judge in `families/dpmac.rs` with
  unit tests; `status.rs:128-134` consumes them —
  `SameContainerKernelPeer` and `MacAddr::ZERO` leave status.rs
- [ ] 4.2 B4, model side first per house ordering: decide the Actuate
  posture for a zero read-back (skip vs defer), then
  `reconcile.rs:146/:152` treat `Some(MacAddr::ZERO)` as unobserved via
  the same family predicate; unit test: zero read-back + Assert ⇒ no
  mismatch
- [ ] 4.3 Gates + close beads e6s.3, e6s.4; one commit each

## 5. Bead B5 — one dpmac-info scanner (dpaa2-controlplane-e6s.5)

- [ ] 5.1 `RawDpmacObservation` (plus the endpoint line) becomes the single
  raw scanner in `crates/dpaa2-mc/src/parse.rs`; offer/info projections
  derive from it; one token→enum table per the `OPTION_BITS` precedent
- [ ] 5.2 Gates: the link-type strip_prefix appears once; `cargo test -p
  dpaa2-mc` green; quality floor PASS; close bead e6s.5; commit

## 6. Bead B6 — LinkType sums reconcile (dpaa2-controlplane-e6s.6)

- [ ] 6.1 Search designs/ADRs for an acknowledged triplication (design D4
  here); if recorded deliberate → doc pointer naming the collapse trigger;
  else → `From` impls at one documented seam
  (`families/dpmac.rs:59` / `core/model.rs:377` / `core/inventory.rs:37`)
- [ ] 6.2 Collapse decision recorded for the next P4 family; gates + close
  bead e6s.6; commit

## 7. Close-out

- [ ] 7.1 Epic bead e6s acceptance check (all children closed); ROADMAP
  touch only if #7-adjacent state changed
- [ ] 7.2 `openspec validate --strict` green; epic review per standing
  practice; ready for archive
