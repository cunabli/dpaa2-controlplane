# Tasks

## 1. Orchestration model (ADR-0002: Quint first)

- [x] 1.1 Create `models/orchestration/` on the census-schema projection
      (design D11): per-construct status, edge status, dpmac occupancy,
      the two budget counts, pending plan multisets; parametric over the
      family table shared with `core/types.qnt`. Actions: observe, one
      pass per family-table row, admission, and the adversarial
      environment action (design D7). Guards carry the census-read/
      live-read discipline (design D9). Verify: ladder typecheck +
      simulate green; the family table imports `Family` from core, no
      fork.
- [x] 1.2 State and check ORCH-I1–I5 (design "Named invariants");
      Apalache-mark ORCH-I1–I3 and ORCH-I5 within ladder bounds;
      bounded convergence under the eventual-environment-quiescence
      fairness assumption stated in the model header. Freeze the ITF
      traces for the scheduling behaviors. Verify: marked-Apalache
      green; traces committed beside the model; `models/COVERAGE.md`
      rows with named dispositions.

## 2. Retro-oracle (the origin pinned as a test)

- [x] 2.1 Script the mask-a-family check: for each family-table row,
      force its pass guard false with plan entries pending and expect
      ORCH-I1 red within bound (the dpsw-typestate/dpseci gap class,
      mechanized). Wire into the ladder beside the existing model
      gates. Verify: the oracle fails on every masked family and passes
      unmasked; expect-red plumbing does not mask a genuinely red
      model.
- [x] 2.2 Check the handover-window pair (orchestration-model delta
      scenario): live-read in place → violating state unreachable;
      live-read removed → reachable. Verify: both halves run in the
      ladder; the reachable half is an expect-red variant, not a
      shipped weakening.

## 3. Total dispatch (reconciler delta; design D10)

- [x] 3.1 Restructure plan-entry consumption through one compile-checked
      total dispatch over the construct families (visitor-shaped trait
      without defaults or exhaustive match — first parcel decides
      against ADR-0018's trait seams; runtime maps refused). Rows:
      `Execute` per delivered family, `Unimplemented` for dpseci naming
      its retirement trigger. Absorb the interim `reify_guard`
      (main.rs; dpsw-typestate 6.4) without duplicating it. Verify:
      quality floor; removing any family's row fails `cargo build`; the
      family-iteration unit test asserts every `Unimplemented` row
      names its trigger; dpseci intents refuse whole with unchanged
      voice.
- [x] 3.2 Land the `dpaa2-verify` registry twin: the dispatch's row set
      checked against the model's transition set; frozen orchestration
      traces replay against the engine's pass schedule. Verify: twin
      red when a pass exists without a model action or the reverse;
      existing family traces replay unchanged.

## 4. Capacity face + admission (capacity-census delta)

- [x] 4.1 Add the typed capacity projection to the census in
      `dpaa2-api`: per-container pool rows + the dpsw FDB budget,
      re-derived each observation from the parses the pool passes
      already own; dpio seats render as spent headroom (ADR-0003 §7).
      Verify: unit tests from banked census fixtures; no state carried
      across two derivations.
- [x] 4.2 Add the gross draw derivation (pure, over the compiled plan;
      creates counted, planned frees not — design D8) and the admission
      verdict in the shared read → complete → compile pipeline, with
      ADR-0013 refusal rows (budget, draw, headroom, data-only).
      Verify: the admission oracle offline — committed over-budget
      intents (pool draw and FDB cases) refuse typed against banked
      fixtures; the rebuild-on-full-pool fixture refuses, then admits
      on the post-teardown census; refusal idempotent across repeated
      runs.

## 5. CLI rendering (provisioning-cli delta)

- [x] 5.1 Render admission verdicts in `dry-run` (budget/draw/headroom/
      verdict beside the plan) and the `ensure` pre-dispatch refusal;
      `Unimplemented` refusals name their retirement pointer. Verify:
      rendered-output tests in the established voice; `ensure` dispatch
      refused before any verb on an over-budget fixture.

## 6. Board witness

- [x] 6.1 Add one board suite row confirming the admission refusal live
      (an over-budget intent against the reference board's observed
      pools), riding the next sitting that runs anyway — no dedicated
      sitting. Verify: suite row banked with the sitting's record.

## 7. Confluence follow-on (ORCH-I6)

- [x] 7.1 Bounded pairwise confluence check over passes,
      simulator-first: where guards force no order, either pass order
      reaches the same final census. Full treatment stays behind the
      ADR-0023 revisit trigger. Verify: pairwise check green in the
      ladder at bounded depth; cost recorded in COVERAGE.
