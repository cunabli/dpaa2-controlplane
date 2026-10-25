// Retro-oracle for the ensure pipeline (ADR-0023; resource-aware-orchestration
// tasks 2.1 + 2.2). It pins the change's origin — the dpsw-typestate/dpseci gap
// class (bead dpaa2-controlplane-dbf.19: a compiled family with no executor,
// silently dropped) — as a directed expect-red check, and earns the live-read
// discipline (ADR-0023 design D9) by showing the handover window becomes
// reachable the moment the wire guard reads the census snapshot, not live occ.
//
// The shipped model is NEVER weakened: this module imports `orchestration`
// (models/orchestration/pipeline.qnt) read-only and reuses its `init`/`step`
// unchanged. The two variants live here, comment-anchored to the pipeline
// definitions they mirror:
//   - the mask family: a per-family `orchI1(masked)` restates ORCH-I1
//     completeness (pipeline `ORCH_I1`, ADR-0023 design D3/D10) with one
//     family's servicing transition forced dead; the shipped `step` drives it.
//   - the census seam: `wireEnabledCensus`/`doWireCensus` mirror pipeline
//     `wireEnabled`/`doWire` with the one live-read of `occ`
//     replaced by the dormant `censusOcc` snapshot — the seam pipeline's header
//     calls out — and `stepCensus` swaps `doWire` for it.
//   - the re-apply seam (plan-rollback design D6): `doRollbackReapply` mirrors pipeline
//     `doRollback` over the shared `applyRollback` twin but leaves the verdict Admitted —
//     re-applying the drained plan without re-admission — and `stepReapply` swaps
//     `doRollback` for it, turning pipeline `ORCH_I3` (ORCH-I3 restated) red.
//
// The oracle driver is models/helpers/orch-retro-oracle.py: it runs every check
// and asserts the baseline (unmasked / live-read-in-place) stays green while
// each variant goes red, so expect-red plumbing can never hide a genuinely red
// model. Wired into the ladder as `pnpm model:retro-orch` (in model:validation).
//
// Named invariants: none of its own — it checks pipeline's ORCH-I1/I2 from the
// two adversarial angles. Apalache-marked subset: none (directed expect-red at
// the simulate bound, the precedent the intent/retro expect-red runs set).
module retro_oracle {
  import orchestration.* from "./pipeline"
  import types.* from "../core/types"

  // ---- task 2.1: the mask-a-family retro-oracle (ADR-0023 design D3/D10) ----

  // When ORCH-I1 completeness obliges a family to carry a live servicing
  // transition: a create whose guards are all met (pipeline `createGuards`), a
  // pending Unimplemented family (pipeline `refuseEnabled` antecedent), or an
  // already-converged table family (DPC-born dpmac lives here). The obligation
  // is gated exactly as pipeline `ORCH_I1`, so a pre-admission or budget-starved
  // create is waiting, not dead, and the unmasked baseline stays green.
  def servicingObligation(f: Family): bool = or {
    createGuards(f),
    isUnimplemented(f) and pending.get(f) > 0 and not(refused.get(f)),
    status.get(f) == Converged,
  }

  // The family's servicing transition is live (create executor, typed refusal,
  // or already serviced), UNLESS it is masked — the gap class: a family emitted
  // into the plan with its executor forced dead (bead dpaa2-controlplane-dbf.19).
  def servicedExcept(masked: Set[Family], f: Family): bool =
    (or {
      status.get(f) == Converged,
      refused.get(f),
      execEnabled(f),
      refuseEnabled(f),
    }) and not(masked.contains(f))

  // ORCH-I1 completeness, maskable: no obliged family is dead. Empty mask is the
  // shipped law (green on `step`); any singleton mask kills that family's only
  // servicing transition and turns ORCH-I1 red within bound.
  def orchI1(masked: Set[Family]): bool =
    TABLE_FAMS.forall(f => servicingObligation(f) implies servicedExcept(masked, f))

  val ORCH_I1_maskNone:   bool = orchI1(Set())
  val ORCH_I1_maskDprc:   bool = orchI1(Set(Dprc))
  val ORCH_I1_maskDpni:   bool = orchI1(Set(Dpni))
  val ORCH_I1_maskDpbp:   bool = orchI1(Set(Dpbp))
  val ORCH_I1_maskDpio:   bool = orchI1(Set(Dpio))
  val ORCH_I1_maskDpcon:  bool = orchI1(Set(Dpcon))
  val ORCH_I1_maskDpmcp:  bool = orchI1(Set(Dpmcp))
  val ORCH_I1_maskDpsw:   bool = orchI1(Set(Dpsw))
  val ORCH_I1_maskDpmac:  bool = orchI1(Set(Dpmac))
  val ORCH_I1_maskDpseci: bool = orchI1(Set(Dpseci))

  // ---- task 2.2: the handover-window live-read pair (ADR-0023 design D9) ----

  // The violating state ORCH-I2 forbids: the switch port wired while the
  // standalone mac driver still holds the dpmac (the phylink use-after-free,
  // docs/upstream/phylink-dpni-churn-crash.md). Unreachable on the shipped
  // `step` (live-read in place), reachable on `stepCensus` (live-read removed).
  val handoverSafe: bool = not(portWired and occ == MacHeld)

  // Mirrors pipeline `wireEnabled` with the ONE live-read of `occ`
  // replaced by the dormant `censusOcc` snapshot — the removed-live-read variant.
  val wireEnabledCensus: bool = and {
    verdict == Admitted,
    WANT_PORT,
    not(portWired),
    status.get(Dpsw) == Converged,
    censusOcc == Free,
    fdbFree > 0,
  }

  // Mirrors pipeline `doWire` over the census-read guard: the census guard swapped
  // in, but the wire effect itself is pipeline's own `applyWire` committed through
  // `commitState`, so no hand-copied effect can drift from the shipped twin.
  action doWireCensus = all {
    wireEnabledCensus,
    commitState(applyWire(cur)),
  }

  // Mirrors pipeline `step` with `doWire` swapped for `doWireCensus`.
  action stepCensus = any {
    observe,
    admit,
    nondet f = TABLE_FAMS.oneOf() doCreate(f),
    nondet f = TABLE_FAMS.oneOf() doReconcile(f),
    nondet f = TABLE_FAMS.oneOf() doPrune(f),
    nondet f = TABLE_FAMS.oneOf() doRefuse(f),
    doWireCensus,
    doEnv,
    stutter,
  }

  // ---- plan-rollback: the re-apply-without-re-admission pair (plan-rollback design D6) ----

  // Mirrors pipeline `doRollback` over the SHARED `applyRollback` twin, but relicenses the
  // drained plan to re-dispatch by leaving the verdict Admitted instead of forcing
  // re-admission (Unjudged) — the "re-apply without re-admission" mutation. The revert flag
  // `applyRollback` raised still stands, so pipeline `ORCH_I3` (its reverted-implies-Unjudged
  // conjunct, ORCH-I3 restated) is violated the instant this fires: a rolled-back run proceeds
  // without re-judging the drained budget, the apply/rollback oscillation the restatement forbids.
  action doRollbackReapply = all {
    anyOvercommit,
    policy == Rollback,
    commitState({ ...applyRollback(cur), verdict: Admitted }),
  }

  // Mirrors pipeline `step` with `doRollback` swapped for `doRollbackReapply`.
  action stepReapply = any {
    observe,
    admit,
    nondet f = TABLE_FAMS.oneOf() doCreate(f),
    nondet f = TABLE_FAMS.oneOf() doReconcile(f),
    nondet f = TABLE_FAMS.oneOf() doPrune(f),
    nondet f = TABLE_FAMS.oneOf() doRefuse(f),
    doWire,
    doRollbackReapply,
    doAbort,
    nondet p = Set(Rollback, Abort).oneOf() setPolicy(p),
    doEnv,
    stutter,
  }
}
