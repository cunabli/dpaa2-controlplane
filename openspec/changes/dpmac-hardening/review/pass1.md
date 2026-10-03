# Pass 1 — acceptance re-run and residue (dpmac-hardening)

Agent: Explore (read-only). Reviewed at HEAD 26c0127, 2026-10-03.
Note: the full three-pass review briefed in brief.md was waived by the
owner after Pass 1 (change too small to warrant passes 2–3 + judge);
this pass stands as the review record. Disposition 2026-10-03, owner
call: all three findings fixed by amending the epic commits in place —
PASS1-F1 folded into the B1 commit (now 4a9d8c6: the `Unbind` variant
carries only the proof, `SeveredProof::dpni()` reads the target back,
third doctest pinned `compile_fail,E0559` for retargeting), PASS1-F2/F3
folded into the B2 commit (now 7437324). All verification greps below
pass at the rewritten tip 2fc2667; no beads or tasks were opened.

## Acceptance obligations

| Obligation | Command | Decisive output | Result |
|---|---|---|---|
| MERGED-3 B7 1.1: no "(#7)" routing in dpmac.md | `grep -n '(#7)' docs/baseline/dpmac.md` | no output (the :277 and :280 rows now route to #10, per the f23ad70 diff) | PASS |
| B7 1.2: COVERAGE DPNI-I3 row | `grep -n 'DPNI-I3' models/COVERAGE.md` | :87 reads "shipped in `families/dpmac.rs` `MacRelation` + V-DPMAC-3 rev 1 … Erratum …", no "(#7)" | PASS |
| B7 1.3: no "unknown #1" in invariants.qnt | `grep -n 'unknown #1' models/core/invariants.qnt` | no output; the comment cites V-DPMAC-2 rev 1 and the DPMAC-I1 destroy caveat; only comment lines changed | PASS |
| B7 1.4: board README pointers at :85/:605/:881, sealed prose untouched | `sed -n '85p;605p;881p' models/board/README.md` + `git diff fb831f2..26c0127 -- models/board/README.md` | all three lines end with "re-anchored to #10 by dpmac-typestate 1.3"; the diff is append-only (V-LINK-4 row byte-identical up to the appended clause) | PASS |
| B7 1.5: dpmac_itf.rs reword | `sed -n 18,24p crates/dpaa2-verify/src/intent/dpmac_itf.rs` | "DPMAC-I4 is structural by construction; the MC-view read is deferred to `mc-portal-backend` (#10) by dpmac-typestate design D6" | PASS |
| B7 1.6: dpmac.qnt severAt comment | `git show f23ad70 -- models/families/dpmac.qnt` | +4 comment lines naming the planner's wider Disconnect vs severAt, citing D3 | PASS |
| MERGED-8 B7 1.7: ADR-0019 Status line | `sed -n 3,14p docs/adr/0019-*.md` | "amended 2026-10-03 by dpmac-typestate 6.1 (bead dpaa2-controlplane-e6s.7) …"; diff is an appended trail only | PASS |
| B1: `_severed_edge` gone | `grep -n '_severed_edge' crates/dpaa2-verify/src/board/replay.rs` | no output; the named `severed_proof()` constructor replaces it | PASS |
| B1: "owned by the types" | `grep -rn 'owned by the types' crates/dpaa2-api/src` | one hit at transition.rs:182 (sever doc). True for the `unbind()` constructor path but undercut by the variant-literal path — see PASS1-F1 | PASS (qualified) |
| B1: compile_fail pinned | `grep -n compile_fail crates/dpaa2-api/src/plan/transition.rs` | :197 `compile_fail,E0423` (forge); :207 `compile_fail,E0382` (reuse) | PASS (grep only; doctests not executed in this pass) |
| B3: status.rs clean | `grep -nE 'SameContainerKernelPeer\|MacAddr::ZERO' crates/dpaa2-tools/src/status.rs` | no output | PASS |
| B5: one link-type scan | `grep -rn 'strip_prefix("DPMAC link type:")' crates/` | exactly one hit: crates/dpaa2-mc/src/parse.rs:549 | PASS |
| B6: three enum defs plus one documented seam | `grep -rn 'enum .*Link' crates/dpaa2-api/src` + `grep -rn 'impl From<' crates/dpaa2-api/src \| grep -i link` | defs at model.rs:377, dpmac.rs:60, inventory.rs:37; seam at dpmac.rs:69-98 (two `From` impls, doc names authority, partial reverse in `DPMAC_LINK_TYPES`, collapse trigger) | PASS |
| Ledger lint | `cargo test -q -p dpaa2-verify --test ledger_lint` | `test result: ok. 2 passed; 0 failed` | PASS |
| Scaffolding in epic diff | `git diff fb831f2^..26c0127 -- crates models docs` grepped for `todo!/dbg!/unimplemented!/allow(dead_code)/allow(unused)/ponytail:/FIXME/XXX/TODO` + commented-out-code scan | no output | PASS |
| No new `ponytail:` marker | same diff grep | none added; existing markers (fake.rs:605, model.rs:552, compile_props.rs, fitcheck.rs:112) all predate the epic | PASS |
| Stale wording: Copy-token / positional pairing / three scanners | targeted greps over the touched surface | Copy: only transition.rs:210 ("not `Copy`", true). Positional: only correct negations (dpmac.rs:345, restool.rs:309, port_detail.rs:107 "old positional pairing"). Scanner count: no hits | PASS |

## Findings

**PASS1-F1** · crates/dpaa2-api/src/plan/transition.rs:114, :117-123 (claims at :118-119, :121-122; related :52-53, :182-183) · guard-drift · superseded-by: n/a (the defect is in B1 / 134b303 itself) · **breaks-a-claim** · amend · verification: a third `compile_fail` doctest that builds `Transition::Unbind { dpni: DpniId::new(9), proof }` from `sever(DpniId::new(7))` must fail to compile, and `grep -n 'field is public' crates/dpaa2-api/src/plan/transition.rs` must return nothing.
- Variant fields of a pub enum are always public. A consumer can therefore build `Transition::Unbind { dpni: <other>, proof }` with a mismatched dpni (retarget), and can move `proof` out of an owned `Unbind` by pattern match (re-extraction, defeating consume-once).
- Contradicts design D1 ("stops being extractable from `Unbind`"), task 2.1 ("no longer publicly extractable"), and the field docs "the two never name different dpnis" (:118-119) and "cross-edge reuse [does not] typecheck" (:121-122). The :114 doc itself says "the `proof` field is public".
- The E0423 and E0382 doctests cover forge and reuse-via-move, not literal retargeting.
- Candidate fixes: drop the redundant `dpni` field (keep only `proof`, add a dpni accessor), or wrap the variant payload in a private-field struct.

**PASS1-F2** · crates/dpaa2-api/src/families/dpmac.rs:345 and crates/dpaa2-mc/src/restool.rs:309 (plus the restool.rs doc line "vocabulary with its verbatim names (dpmac-typestate design D4; ADR-0018)" added in 5bcf7ee) · doc-drift · superseded-by: 5bcf7ee (B2, dpmac-hardening design D2) · misleads-a-reader · amend · verification: after the fix, `grep -rn 'positional guess (dpmac-typestate design D4)\|positionally against.*(dpmac-typestate design D4)' crates` returns nothing.
- These comments attribute the "label by carried name, not positional" decision to dpmac-typestate design D4, which says nothing about names or positions. The decision belongs to dpmac-hardening D2 (MERGED-2). Fix: cite dpmac-hardening design D2 for the naming clause.

**PASS1-F3** · crates/dpaa2-tools/src/render.rs:570-571 · stale (minor) · superseded-by: 5bcf7ee (B2) · misleads-a-reader · amend · verification: `grep -n 'in the same order' crates/dpaa2-tools/src/render.rs` returns nothing.
- "value from the readout in the same order" is leftover wording from the positional era; each name now travels in the same tuple as its value.

## Notes (not findings)

- reconcile.rs:106 `dpni.mac.unwrap_or(MacAddr::ZERO)` predates the epic (cfg-drift isolation: the same value is passed on both sides of `drift_disposition`); it is not a zero judgment and does not break D3.
- reconcile.rs:607/:631 `Some(MacAddr::ZERO)` are B4 test fixtures.
- Both MAC arms (reconcile.rs:150, :156) gate on `mac_read_back_observed`.

## Footer

**Read:** review/brief.md, tasks.md, design.md D1 + section headings; transition.rs:40-125 and grep-targeted lines to :290; dpmac.rs:52-100, :335-352; restool.rs:300-335; reconcile.rs:96-115 + grep hits; render.rs and parse.rs comment greps; dpmac_itf.rs:18-24; the replay.rs diff; the f23ad70 diffs (invariants.qnt, dpmac.qnt, dpmac.md, ADR-0019, COVERAGE); the board README diff + lines 85/605/881; COVERAGE:106; formal-models spec:372-374; mc-status.md:81 register row; archived dpmac-typestate D4.

**Deliberately not read:** status.rs body (grep only), fake.rs, port_detail.rs, dpmac_replay.rs, the parse.rs scanner body, proposal.md and the spec deltas, VERDICTS.json, .beads. No cargo commands other than ledger_lint — the doctests and dpmac_replay suite were not executed in this pass.

**Open questions:**
1. Does PASS1-F1 count as D1 not landed (D1 explicitly promised non-extractability), or was the variant-literal path accepted as residual? No record accepts it.
2. Should `cargo test -p dpaa2-api --doc unbind` be re-run to confirm the E0423/E0382 pins fail for the pinned reason?
