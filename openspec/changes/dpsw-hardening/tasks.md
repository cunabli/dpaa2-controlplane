# Tasks

Each group maps 1:1 to an epic child bead; the bead's description
carries its group verbatim, so the bead is self-contained at execution
time. One group at a time, through acceptance, in this order:
1 → {2, 3, 4, 6 in any order} → 5 → 7. Group 7 gates
`/opsx:archive dpsw-typestate`.

Standing gates on every group: `scripts/checks/quality-floor.sh`,
`pnpm model:test`, `cargo test --workspace --locked`; byte-diff over
`models/traces/families/dpsw/` and `models/traces/families/dpmac/`
stays empty (only group 4 re-freezes `models/intent/traces/
fabricAcceptedTrace.itf.json`, nothing else).

## 1. The engine dispatches the typed switch plan (bead A — M1)

- [x] 1.1 `dispatch_switch_create` (engine.rs:813-852) builds its
      moves by matching `plan_switched_domain`'s `SwitchStep`s
      (plan/dpsw.rs:274-376) instead of re-stating
      create → connects → plug inline from `plan.edges`; the
      interposition dispatch (engine.rs:836-841) matches ALL
      `InterposeStep` arms (plan/dpsw.rs:425-493) with the severed
      proof bound — no `if let` that silently drops ConnectUplink/
      ConnectMember. Verify: `grep -n plan_switched_domain
      crates/dpaa2-tools/src/engine.rs` non-empty; clippy-exhaustive
      match over `InterposeStep`; existing engine tests
      (create→connect→plug order, plug-last, interposition
      sever-first) and `vdpsw5_intents.rs` green unchanged.
- [x] 1.2 Fold the hardcoded `Class::Disruptive` (engine.rs:908-909)
      onto `DriftRepair::class()`; add one engine test proving a
      beyond-num_ifs edge refuses typed (`SwitchPlan::PortOverflow`)
      before any dispatch — DPSW-I10 joins the live path. Verify: the
      new test green; `grep -n "Class::Disruptive"
      crates/dpaa2-tools/src/engine.rs` shows no literal on the switch
      pass.

## 2. Member-count bound as compile law (bead B — M2)

- [x] 2.1 Add the typed fabric refusal (the `MacTableOverflow` shape)
      bounding membership at the `max_fdbs` field envelope (0..=255):
      `models/intent/refuse.qnt` + `crates/dpaa2-api/src/intent/
      refuse/fabric.rs:104-142`; extend `dpswDerivationLawful`
      (models/intent/invariants.qnt:333-336) and
      `dpsw_attrs_well_formed` with the bound — both sides in one
      step (design D2). Verify: a `compile_tests.rs` case with 256
      members yields the new typed refusal; `grep -n "255"
      crates/dpaa2-api/src/intent/refuse/fabric.rs` non-empty;
      `pnpm model:test` + ITF replay green.
- [x] 2.2 Re-word plan/dpsw.rs:219-227 — `cfg_from_attributes`'s
      `Error::Config` arm is now truly internal (unreachable from a
      compiled intent), so its "malformed attribute only" claim
      becomes true. Verify: the comment names the compile bound as
      the reason.

## 3. ADR-0022 row truth + edge-kind negative witnesses (bead C — M3+M15)

- [x] 3.1 Fix docs/adr/0022-reification-policy-is-a-property-of-the-
      edge-kind.md:100 — the dpsw.N.M↔dpmac row must NOT claim
      `edgeDemandsSeveredWitness` true for the kind: attribute the
      interposition witness to the displaced dpni↔dpmac edge (that
      kind's law, `core/connect.qnt:55-56`) and state both dpsw kinds
      carry none (design D5 of dpsw-typestate). Note the bead-yfa
      R-rule lint as the mechanization anchor. Verify:
      `grep -n "edgeDemandsSeveredWitness true" docs/adr/0022-*.md`
      → empty.
- [x] 3.2 Extend `severed_witness_is_demanded_only_by_dpni_dpmac`
      (crates/dpaa2-api/src/plan/connect.rs:826-831) with the two
      missing negative assertions for `(Dpni, Dpsw)` and
      `(Dpsw, Dpmac)` — the exact assertions that would have exposed
      3.1. Verify: `cargo test -p dpaa2-api
      severed_witness_is_demanded_only_by_dpni_dpmac` green.

## 4. DPSW-I1 evidence trail sync (bead D — M6, regen-bearing)

- [x] 4.1 Replace the "read-not-verified" DPSW-I1 provenance anchor
      with the board-anchored wording — positive face V-DPSW-1,
      flooding refusal face V-DPSW-4 rev 1, remaining refusal faces
      (ctrl-if, max_fdbs) kernel-source-read. Sites move in lockstep
      (ADR-0002 twins): crates/dpaa2-api/src/intent/derive.rs:864 ↔
      models/intent/derive.qnt:742, comment copies at derive.qnt:563,
      compiled.rs:145/:838, models/intent/scenarios/fabric.qnt:15,
      docs/adr/0013-accepted-intent-vocabulary.md:252. Suite ids are
      data; no ADR ids inside the string (design D3). Verify:
      `grep -rn "read-not-verified" crates models docs` empty outside
      openspec/changes/archive.
- [x] 4.2 Re-freeze `models/intent/traces/fabricAcceptedTrace.itf.json`
      and re-accept `render__dry_run_switched_domain.snap`; flip
      docs/baseline/dpsw.md:246-247 DPSW-I1/I2 cells from bare
      "candidate" to the I3-style verified wording naming
      V-DPSW-1/V-DPSW-4. Verify: replay + `cargo test -p dpaa2-tools
      --test render` green; `grep -n "| DPSW-I1 "
      docs/baseline/dpsw.md` names the suites; no other trace file
      diffs.

## 5. Dry-run switch preview (bead E — M4; after group 1)

- [x] 5.1 Factor `converge_switches`' pre-dispatch mutation-free
      verdict loop (engine.rs:878-903) into a `plan_switches` /
      `render_switches` pair (the plan_links/render_links idiom); the
      DryRun arm (main.rs:208-252) composes it like every other pass.
      Preview composition only — D6 census and #15 untouched. Verify:
      dry-run over a `vdpsw5_intents`-shaped config renders the
      switch section with the Disruptive headline;
      `render__dry_run_switched_domain.snap` regenerated; ensure-path
      engine tests green unchanged.

## 6. Capture-tap diagnostic attribution (bead F — M16)

- [x] 6.1 In crates/dpaa2-verify/src/board/capture.rs:423-426,
      surface the restool spawn error and the capture-file read error
      in the preamble refusal string (two `map_err`s) instead of
      `let _ =` + `unwrap_or_default()` folding every cause into
      "dead tap"; add a `ponytail:` marker on the fixed 1s flush
      sleep naming its upgrade path. Verify: `grep -n
      unwrap_or_default crates/dpaa2-verify/src/board/capture.rs`
      empty; dead-tap tests green; new assertion pins the
      missing-restool message naming the spawn cause.

## 7. Pre-archive doc/spec staleness sweep (bead G — M5, M7–M14; gates archive)

One sweep, one commit under the `Change: dpsw-typestate` trailer
(design D4 — the deltas are live until archive):

- [ ] 7.1 Code-comment and render wording (M5): compiled.rs:508-513 +
      render.rs:337-345 — "never actuated" / "families the reconciler
      does not execute" is false for dpsw since `converge_switches`;
      use the pass-3 wording ("no port-facet Transition; actuated by
      the ensure engine passes, reported here, never drift");
      regenerate the dry-run snapshot. mc.rs:88 (M14): the per-if
      face is display + the pinned has_endpoint-law twin, not a
      census input; `port_unconnected` stays the typed law twin.
- [ ] 7.2 dpsw-typestate change-doc fixes: specs/mc-backend/
      spec.md:48-66 (M7) — bare bind always ENODEVs; name
      `driver_override`-then-bind as the shipped path (kernel.rs:
      146-180), keep ENODEV-as-deferral for the fresh-node race;
      specs/mbt-harness/spec.md:11-17 (M9) — drop the vacuous
      `lastVerbs` claim, name `ioctl_policy.rs:91` as the whitelist
      gate; proposal.md:102 (M10) — "four" → "three" dpsw verbs;
      design.md:176-179 (M11) — annotate the FDB-split open question
      "answered — dpsw.md unknown 6 (driver convention)".
- [ ] 7.3 Record sweeps: docs/adr/0013:594-596 (M8) — fabric now has
      an executor (only crypto/dpseci stays plan-only) and the ledger
      records DPSW-I1/I2 modeled with board faces; models/
      COVERAGE.md:153 (M13) — drop "(pending task 2.2)", point at
      `FabricNotKernelForwarded`; models/board/V-DPSW-5/V-DPSW-5.sh
      header (M12) — one line: traffic leg deferred (#15 trigger,
      bead dbf.19), verdict structural 8/8. Verify: every per-row
      grep from synthesis §1 comes back clean; ledger lint green.

## 8. Rule outcomes (synthesis §5)

- [ ] 8.1 R1 (accepted, reshaped): the board-suite protocol gains the
      named offline pre-check step — render the suite and run its
      intents against the fake backend end to end in the pre-run
      record; any typed refusal kills the sitting before it starts.
      Land it where the protocol lives (the board-suite bd memory +
      `models/board/README.md` protocol section). R2: the close-out
      doc-bank checklist gains the annotate-at-bank sweep — sealed
      task text naming an overturned design decision gets a one-line
      bracketed supersession note; annotate, never rewrite. Verify:
      both records name this change as their source.

## Workflow follow-up

- `bd note` on dpaa2-controlplane-94k naming
  `models/board/V-DPSW-2/corpus/`, `models/board/V-DPSW-5/corpus/`,
  and `capture.rs` `Corpus::parse` as #10's banked fixture input
  (M17, self-contained-carriers rule) — lands on that bead directly,
  not in this change.
- After group 7 seals: `/opsx:archive dpsw-typestate` (review
  directory included), then this change implements to completion and
  archives in its own turn.
