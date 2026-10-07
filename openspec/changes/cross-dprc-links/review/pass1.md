# Pass 1: Residue, deferrals, and verify re-run (cross-dprc-links)

Repo root: . Main is at 215801e, so the grounding facts still hold.

The verify suite is all green, but four things need action:
- **PASS1-F1:** one board scenario run now fails. Nothing caught it because no gate runs `quint test` on board modules.
- **PASS1-F2:** the change's own spec delta still adds a `single_sender` knob, which D9 refused. Archiving would write it into the main spec.
- **PASS1-F3–F5:** Rust doc comments still treat LINK_I1 as the disconnect-before-destroy refusal.
- **PASS1-F7:** the dossier bead's text says the opposite of what tasks 3.5 and 7.6 decided.

## Findings

**PASS1-F1** · models/board/V-TRAF-1/vtraf1.qnt:193-198 (plus the comments at :60-63 "the two trace-inexpressible refusals are disconnect-before-destroy (LINK-I1)", :180 "Face 6 carries three teardown laws", :215 "face 6's two refusal laws") · stale (twin-drift with the weakened family model) · superseded-by 27b99e2 (task 7.5). That commit edited the comment at :180-183 and left the run in place. · breaks-a-claim — `run face6DisconnectBeforeDestroyRefusedTest` (`destroyEndAt(dpniLeft).fail()`) imports `link_lifecycle.*`, where destroying a connected end is now ENABLED. `pnpm exec quint test models/board/V-TRAF-1/vtraf1.qnt --main=vtraf1` gives 21 passing and 1 failed (QNT511, "returned false" at :195). Typecheck still passes, and no gate runs `quint test` on `models/board/*/*.qnt` (package.json `model:test` and quality-floor.sh only typecheck them), so this got through the seal. · disposition: amend — retire the run (or flip it to the accepted-destroy law, like `destroyConnectedEndRemovesEdgeTest`) and fix the three comments. Follow-up bead: add the board scenario modules to `model:test`. · verification: `pnpm exec quint test models/board/V-TRAF-1/vtraf1.qnt --main=vtraf1` shows 0 failed; `grep -n 'face6DisconnectBeforeDestroy' models/board/V-TRAF-1/vtraf1.qnt` returns nothing; `cargo test -p dpaa2-verify --test vtraf1_suite` stays green (byte-for-byte regen).

**PASS1-F2** · openspec/changes/cross-dprc-links/specs/intent-compiler/spec.md:13-20,22-24,33-38 · stale / doc-drift · superseded-by 95dd345 (task 3.5: single_sender refused as an intent knob, D9). The spec text was written at 4df869b, and 215801e rewrote the dpcon section of this file but left these lines. · breaks-a-claim — the MODIFIED requirement says an interface "MAY declare" a `single_sender` knob, and adds a "single_sender knob adds exactly one option" scenario. That contradicts D9, the topology-config delta (`specs/topology-config/spec.md:13`, "There is no `single_sender` knob"), the formal-models delta (:116, :139, "no intent knob minted") and the shipped code. The main spec (`openspec/specs/intent-compiler/spec.md:478-500`) does not have the knob yet, so archiving this delta would add a requirement that D9 refuses. · disposition: amend — restore the MODIFIED requirement to the main-spec wording, or drop the MODIFIED block. · verification: `grep -n -i 'single_sender' openspec/changes/cross-dprc-links/specs/intent-compiler/spec.md` shows no knob wording; `pnpm exec openspec validate cross-dprc-links` passes.

**PASS1-F3** · crates/dpaa2-api/src/plan/connect.rs:195-197 and :259-261 · stale (twin-drift in naming) · superseded-by 27b99e2 (task 7.5; LINK_I1 is now the state face "no edge outlives its endpoints", and disconnect-before-destroy is engine POLICY, stricter than hardware, per link_lifecycle.qnt:362-374 and COVERAGE:101) · misleads-a-reader — `WireDisconnected` and `WireTransition::DestroyEnd` docs cite "`LINK_I1` disconnect-before-destroy" as the model law the type enforces. · disposition: amend — reword to "disconnect-before-destroy as typestate policy (stricter than LINK_I1's state face, V-LINK-6)". · verification: `grep -n 'LINK_I1' crates/dpaa2-api/src/plan/connect.rs` shows no pairing with disconnect-before-destroy.

**PASS1-F4** · crates/dpaa2-verify/tests/link_replay.rs:264-266 and :280 · stale (twin-drift) · superseded-by 27b99e2. That commit touched this file and rewrote :411-415 correctly as "deliberate typestate policy", but left these lines. · misleads-a-reader — the replayer's destroy-before-disconnect refusal still says "(`LINK_I1`)", in a doc comment and in the error string, so the same file contradicts itself. · disposition: amend — label it policy and not LINK_I1. · verification: `grep -n 'LINK_I1' crates/dpaa2-verify/tests/link_replay.rs` shows only state-face uses; `cargo test -p dpaa2-verify --test link_replay` stays green.

**PASS1-F5** · crates/dpaa2-verify/src/intent/link_itf.rs:12 and :105-107 · stale · superseded-by 27b99e2. The retired `destroyConnectedEndRefusedTest` was the only disconnect-before-destroy `.fail()` trace; the one `.fail()` trace left is `connectAlreadyConnectedRefusedTest`. · misleads-a-reader — module docs and `LinkStep::Refused` docs still list "the cardinality-one / disconnect-before-destroy guard refusals". · disposition: amend — mention only cardinality-one. · verification: `grep -n 'disconnect-before-destroy' crates/dpaa2-verify/src/intent/link_itf.rs` returns nothing.

**PASS1-F6** · models/board/V-TRAF-1/vtraf1.hook.sh:4-5, :214, :222 and vtraf1.qnt:66-67 · stale (doc-drift) · superseded-by V-LINK-6 rev 1 / 27b99e2 (only the refusal-guard claim was falsified; LINK-I1's state face is hardware-anchored) · misleads-a-reader — the comments say "falsifying LINK-I1" and the echo prints "LINK-I1 falsified rev 1-2" without qualification. They are rev-3-era record text; the board README:109-110 and COVERAGE:101 tell it correctly. · disposition: amend the comments to "LINK-I1's refusal guard falsified". Leave the executed echo string alone unless Pass 4 wants a rev note; it is part of the run artifact. · verification: `grep -n 'falsif' models/board/V-TRAF-1/vtraf1.hook.sh models/board/V-TRAF-1/vtraf1.qnt` shows every hit qualified with "guard"/"refusal".

**PASS1-F7** · bead dpaa2-controlplane-94k (description; `.beads/` store) · stale · superseded-by 95dd345 (task 3.5, single_sender is not a knob) and eb6eb8d/d20fbee (task 7.6, the knob does not ride the ping face) · misleads-a-reader — the carrier exists and is OPEN, and ROADMAP row #10 (docs/ROADMAP.md:30) names it correctly. But its adoption paragraph says "Adopted by #9: DPCON-I3 (dpcon priority knob, same ping face) and SINGLE_SENDER … — both small additive intent knobs with derivation pass-through", which contradicts D9 and 7.6. · disposition: amend the bead description (`bd update`) at #10 reparenting, or now. · verification: `bd show dpaa2-controlplane-94k | grep -i -E 'single_sender|ping face'` shows no knob/ping-face claim.

**PASS1-F8** · models/COVERAGE.md:129 (DPCON-I3 row, last column) · doc-drift · superseded-by d20fbee (task 7.6 moved the caveat onto the consumer rig fed by #10's transport) · misleads-a-reader — the open caveat names its concrete trigger correctly, but its owner arrow still ends "→ `cross-dprc-links` (#9)". #9 is about to be archived, while the trigger is the dpaa2-verify consumer rig on #10's transport. · disposition: amend — point the owner at the consumer-rig/#10 carrier, or say why #9 keeps it after archive. Pass 4 decides. · verification: `sed -n 129p models/COVERAGE.md | grep -o '→ .*'`; `cargo test -p dpaa2-verify --test ledger_lint` stays green.

**PASS1-F9** · crates/dpaa2-api/src/contract/fake.rs:691-698 (`dprc_disconnect`) and :709-719 (`destroy`), compared with :632-645 (`dprc_connect`) · twin-drift (the fake against the board's symmetric edge and LINK_I1) · not a staleness claim · carries-cost — `dprc_connect` records the dpni↔dpni edge on both ends (the task 5.2 comment), but `dprc_disconnect` removes only `endpoints[dpni]`, so the peer still reads the disconnected wire through `dprc_get_connection`. `destroy` removes neither end, so a destroyed end leaves its peer's edge behind, which the post-27b99e2 law ("the edge dies with its endpoint") forbids. The engine's disconnect-first policy hides the destroy half today; the disconnect asymmetry is reachable by any test that reads the peer after a disconnect. · disposition: amend — remove the mirror in both verbs (about 4 lines), plus one fake unit test that reads the peer after disconnect/destroy. Pass 3 judges. · verification: `grep -n 'endpoints.remove' crates/dpaa2-api/src/contract/fake.rs` hits the peer mirror in both `dprc_disconnect` and `destroy`; `cargo test -p dpaa2-api` stays green.

**PASS1-F10** · crates/dpaa2-verify/tests/vtraf1_suite.rs:231-234 · stale · superseded-by 76646ed (rev 3 regen: the refusal probes are banked and the hook runs no refusal command) · misleads-a-reader — the assert message says "the hook probes disconnect-before-destroy", and it only checks that the string "LINK-I1" appears, which the banked comment satisfies. · disposition: amend the message to "the hook banks the LINK-I1/DPRC-I5 disposition". · verification: `grep -n 'probes disconnect-before-destroy' crates/dpaa2-verify/tests/vtraf1_suite.rs` returns nothing.

**PASS1-F11** · crates/dpaa2-tools/tests/vdpcon1_intents.rs:74-81 · simplify (the pin is weaker than its name) · not a staleness claim · misleads-a-reader — `unset_knob_derives_byte_identically` only checks that the provenance key is absent. Byte-identity against today's derivation is never checked: the `Some(p)` test compares against the same `None` compile, so a drift in the `None` path would pass both tests. The brief's 7.6 obligation ("None → byte-identical derivation") is only met relative to itself. · disposition: amend — rename to `unset_knob_adds_no_node`, or pin the `None` compile with an insta snapshot. · verification: `cargo test -p dpaa2-tools --test vdpcon1_intents`, plus a deliberate mutation of the `None` derivation now fails.

No `todo!`, `unimplemented!`, `dbg!`, `#[allow(dead_code)]`, `#[ignore]`, TODO/FIXME, or commented-out code was added in the epic's crate/model/doc files. The only lint allows added are three justified clippy allows (`needless_pass_by_value` ×2, `too_many_arguments` with a D2 comment).

None of the six `ponytail:` markers is stale:
- fake.rs:716 is outside the epic's fake.rs changes (nearest is the one-line change at :693).
- compile_props.rs:168 is outside its changes at :576/:696.
- model.rs:552, fitcheck.rs:112, observed.qnt:30 and invariants.qnt:105 are in files the epic did not touch.

No new unmarked shortcut with a hidden limit was found in the added lines; F9 is the closest.

I found no claim that the probe caused the eviction: README:109, ADR-0008:391 and the hook all state rev 4's refutation or only historical facts. The only knob-board-witness and single_sender-knob claims are F2 and F7.

## Verify-obligation ledger

- 4.1 link ITF replay (13 traces, none orphaned, none unreplayed): `cargo test -p dpaa2-verify --test link_replay` — PASS (11/11, including `every_committed_trace_is_listed` and `link_traces_replay_green`). The directory has exactly 13 `.itf.json` files, matching the 13 entries in TRACES and the 13 names in `model:freeze-link`.
- Retired `destroyConnectedEndRefusedTest` gone from traces and replays: `ls models/traces/families/link_lifecycle/`; `grep -rn destroyConnectedEndRefused crates models package.json` — PASS. It survives only in historical record prose at COVERAGE:101/:104 and board README:110.
- 4.2 bounded lift (dpmac frozen traces byte-unchanged and green): `git diff --stat 4df869b..215801e -- models/traces/families/dpmac/ crates/dpaa2-verify/tests/dpmac_replay.rs` gives an empty diff; `cargo test -p dpaa2-verify --test dpmac_replay` passes 8/8 — PASS.
- 4.3 obligation, partial-order and refusal unit tests: `cargo test -p dpaa2-api` — PASS (302 lib tests; `#[test]` count is plan/connect.rs 23, families/dprc.rs 33, plan/dprc.rs 29).
- 7.6 pin: `cargo test -p dpaa2-tools --test vdpcon1_intents` — PASS 2/2. The `None` half is weaker than its name (F11).
- link_faces snapshot suite: `cargo test -p dpaa2-tools --test link_faces` — PASS 6/6, all 6 snapshots present.
- vtraf1_suite render checks: `cargo test -p dpaa2-verify --test vtraf1_suite` — PASS 4/4.
- Full replay rung (`model:replay`, which is `cargo test -p dpaa2-verify`): PASS, every binary green.
- Rest of the epic's adapters: `cargo test -p dpaa2-mc -p dpaa2-config -p dpaa2-tools` — PASS.
- Quint typecheck: `pnpm exec quint typecheck models/families/link_lifecycle.qnt` and `models/board/V-TRAF-1/vtraf1.qnt` — PASS.
- Quint test (family): `pnpm exec quint test models/families/link_lifecycle.qnt --main=link_lifecycle` — PASS 13/13.
- Quint test (board scenario, not in any gate): `pnpm exec quint test models/board/V-TRAF-1/vtraf1.qnt --main=vtraf1` — FAIL 21/22 (`face6DisconnectBeforeDestroyRefusedTest`, F1).
- Quint simulate: `pnpm exec quint run models/families/link_lifecycle.qnt --main=link_lifecycle --invariant=stateInvariants --max-steps=20 --max-samples=200` — PASS.
- Marked Apalache: `pnpm exec quint verify models/families/link_lifecycle.qnt --main=link_lifecycle --invariant=stateInvariants --max-steps=1 --apalache-version=0.56.1` — PASS (NoError, about 16 s; Apalache 0.56.1 was available locally).
- COVERAGE ledger lint: `cargo test -p dpaa2-verify --test ledger_lint` — PASS 2/2.
- Deferral carriers:
  - DPCON-I4 and unknowns #4/#11: COVERAGE:130 and :100 point at #10 with their fences; ROADMAP:30 names `dpaa2-controlplane-94k`; `bd show` shows the bead exists and is OPEN — PASS, though the bead text is stale (F7).
  - DPCON-I3 caveat: re-anchored to the consumer rig (COVERAGE:129) — PASS, with an owner-pointer question (F8).
  - LINK-I* rows: settled to the sitting's verdicts (I1 board-settled by V-LINK-6, I2 verified rev 4, I3 banked unresolvable, I4 part-settled) — PASS.

## Footer

**Read:**
- review/brief.md and tasks.md in full.
- package.json model scripts and models/README.md (Running section).
- link_lifecycle.qnt header, LINK_I1 region and grep hits; vtraf1.qnt header, face-6 region and imports; vtraf1.hook.sh header and SECTION C.
- link_replay.rs (TRACES table and grep hits), link_itf.rs (docs), vdpcon1_intents.rs (full), link_faces.rs (test names), vtraf1_suite.rs:220-240.
- fake.rs epic changes (:105-115, :580-720); connect.rs:192-266.
- COVERAGE rows 76, 99-104, 129-130; ROADMAP:30; board README:109-110 (by clause).
- intent-compiler, formal-models and topology-config spec deltas (grep and excerpts); main intent-compiler spec :478-500.
- 27b99e2 and 215801e stats and diffs; bead 94k.

**Deliberately not read:** engine.rs, render.rs, populate.rs and generate.rs bodies (Pass 3's scope); ADR-0022/0017/0019/0003 bodies beyond grep, ADR-0008 §10, upstream docs, baseline dpni/dprc deltas, proposal.md and design.md (Pass 4's scope; design.md:108/133 still read disconnect-before-destroy as an invariant, which is historical design text left to Pass 4); VERDICTS.json; prior-epic out-of-scope surfaces.

**Open questions:**
1. F1: retire the face-6 destroy run, or turn it into an accepted-destroy run? It is the pre-run record of revs 1-2. Either way, should board scenario modules join `model:test` (a rule-amendment candidate: typecheck-only gates let a weakened family model silently break dependent scenario runs)?
2. F2: was the single_sender MODIFIED block meant to be dropped at the 215801e seal? Archive will apply it as written.
3. F8: who owns DPCON-I3's open caveat once #9 is archived?
