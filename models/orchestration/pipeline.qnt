// The ensure pipeline as a guarded-action convergence model (ADR-0023; the
// resource-aware-orchestration change design D1). Actions are the engine's
// passes plus an observation action and an adversarial environment action; the
// state is the census-schema projection (ADR-0023 design D11), NOT the core
// object machine — per-construct status, the switch-port edge, dpmac driver
// occupancy, the two board-proven budget counts (container pools, dpsw FDB), and
// the pending plan multisets, parametric over the family table shared with
// core/types.qnt (ADR-0023 design D10/D11: a new family is a table row, never
// rework). Family internals are trusted atomic verdicts (assume-guarantee
// layering): a construct's convergence is one status transition.
//
// Guard discipline (ADR-0023 design D9): guards are census-read by default —
// answered from the loop's snapshot, with the firmware/MC re-validating at
// dispatch so a stale guard costs one refused verb. Exactly one guard is
// live-read — the dpmac driver occupancy gating the switch-port wire — because
// its observing authority (the fsl-mc bus) differs from the verb's executing
// authority (the MC), so the TOCTOU window is real damage (the phylink handover
// use-after-free, docs/upstream/phylink-dpni-churn-crash.md), not a wasted verb.
// `wireSwitchPort` reads live `occ`; `censusOcc` is the snapshot a census-read
// would use and deliberately does not — the seam the handover pair (ADR-0023
// resource-aware-orchestration task 2.2) flips.
//
// Bounded-convergence claim (ADR-0023 design D7; ORCH-I4): admission is
// check-without-reservation and the environment is a modeled adversary, so the
// correctness claim is conditional — from any reachable state the loop reaches
// its fixed point ONLY under an eventual-environment-quiescence fairness
// assumption (the env action eventually stutters). `convergeTest` witnesses it;
// the safety invariants (ORCH-I1/I2/I3/I5) hold against interference unconditionally.
//
// Named invariants (Apalache-marked via `stateInvariants`; dispositions in
// models/COVERAGE.md, "Orchestration invariants" section):
//   ORCH-I1 completeness/no-dead-family; ORCH-I2 order-as-guards (the live-read
//   handover example); ORCH-I3 admission soundness; ORCH-I5 quiescence/no-churn;
//   ORCH-I7 containment order — a child-homed family (dpni/dpbp/dpio/dpcon/dpmcp)
//   homes inside the child dprc, so it is converged-or-drifted only while its
//   container dprc is converged (ADR-0004 containment; the dprc's instance of
//   ADR-0023 design D4 "companions ≺ object ≺ edges; reverse on teardown").
//   ORCH-I4 (bounded convergence) is liveness — stated above, witnessed by
//   `convergeTest`, not a max-steps=1 safety conjunct. ORCH-I6 (confluence) is
//   stated here and checked by resource-aware-orchestration task 7.1 (bounded
//   pairwise, simulator-first).
module orchestration {
  import types.* from "../core/types"

  // ---- census-schema projection (ADR-0023 design D11) ----

  // Per-construct status: the four dispositions the engine can observe.
  type Status = Missing | Converged | Drifted | Orphan

  // dpmac driver occupancy read back off the fsl-mc bus (ADR-0023 design D2 bus
  // source; ADR-0023 D9 live-read): driver-free, held by the standalone mac
  // driver, or taken by the switch driver.
  type Occupancy = Free | MacHeld | SwitchHeld

  // The resource a family's create draws, keyed into the two budgets
  // (ADR-0023 design D8).
  type ResKind = PoolDraw | FdbDraw | PlainCreate | NoCreate

  // A family-table row (ADR-0023 design D10): `Execute(k)` names the pass that
  // consumes the family and its draw; `Unimplemented(trigger)` is the typed
  // whole-intent refusal naming its retirement trigger (dpseci is first).
  type Disposition = Execute(ResKind) | Unimplemented(str)

  // The admission verdict — an enum, so "admitted and refused at once" is
  // type-unrepresentable (ADR-0023 design D5).
  type Verdict = Unjudged | Admitted | AdmissionRefused

  // The family table: parametric over core `Family`, no fork. It carries the full
  // derived vocabulary (`core/types.qnt` `DERIVED_FAMILIES`: dprc, dpni, dpbp,
  // dpio, dpcon, dpmcp, dpseci, dpsw) plus the DPC-born dpmac — so no family the
  // intent can derive surprises the resource-aware-orchestration task 3.2 twin,
  // which keys the Rust dispatch table by exactly that vocabulary (dpmac the
  // twin's only exemption: no runtime create, its occupancy is the handover face).
  // The pool companions (dpbp/dpio/dpcon/dpmcp) share the `PoolDraw` row; the
  // switch draws FDB; dprc and dpni are plain creates; dpseci is the first
  // Unimplemented row. #12 dpdmux joins as an Execute row.
  pure val TABLE: Family -> Disposition = Map(
    Dprc   -> Execute(PlainCreate),
    Dpni   -> Execute(PlainCreate),
    Dpbp   -> Execute(PoolDraw),
    Dpio   -> Execute(PoolDraw),
    Dpcon  -> Execute(PoolDraw),
    Dpmcp  -> Execute(PoolDraw),
    Dpseci -> Unimplemented("crypto-provisioning change (ADR-0023 D10)"),
    Dpsw   -> Execute(FdbDraw),
    Dpmac  -> Execute(NoCreate)
  )
  pure val TABLE_FAMS: Set[Family] = TABLE.keys()

  // The families whose plan entries home inside the child dprc (ADR-0004
  // containment): their object waits on the container, and the container drains
  // only once they are gone. The dprc's instance of ADR-0023 design D4 ordering.
  pure val CHILD_HOMED: Set[Family] = Set(Dpni, Dpbp, Dpio, Dpcon, Dpmcp)

  // The compiled plan wants the switch port wired (the I2 edge). A plan flag, not
  // a construct row — the wire is a cross-family edge pass (ADR-0023 design D4).
  pure val WANT_PORT: bool = true

  // ---- state ----

  var status: Family -> Status        // per-construct status (one construct per family)
  var pending: Family -> int          // pending plan multiset per family
  var occ: Occupancy                  // live dpmac occupancy (ADR-0023 design D9 live-read)
  var censusOcc: Occupancy            // last-observed occupancy (the census-read snapshot)
  var portWired: bool                 // the switch-port edge status
  var poolFree: int                   // container-pool budget, free count (ADR-0011)
  var fdbFree: int                    // dpsw FDB budget, free count (V-DPSW-2)
  var verdict: Verdict                // admission verdict for the compiled plan
  var refused: Family -> bool         // typed whole-intent refusal recorded (ADR-0023 design D5/D10)

  // ---- family-table helpers (ADR-0023 design D10/D11) ----

  pure def execKind(f: Family): Option[ResKind] =
    match TABLE.get(f) { | Execute(k) => Some(k) | Unimplemented(_) => None }

  // A create pass exists (Execute, drawing) — dpmac's NoCreate row has none.
  pure def isCreate(f: Family): bool =
    match execKind(f) { | None => false | Some(k) => k != NoCreate }

  // The family carries no consuming pass (the Unimplemented disposition).
  pure def isUnimplemented(f: Family): bool = execKind(f) == None

  // The firmware enforces the real budget at dispatch, so this guard is
  // census-read (ADR-0023 design D9): a stale read costs one refused verb.
  def budgetOk(f: Family): bool =
    match execKind(f) {
      | None => true
      | Some(k) => match k {
        | PoolDraw => poolFree > 0
        | FdbDraw => fdbFree > 0
        | PlainCreate => true
        | NoCreate => true
      }
    }

  // Companions precede their object (ADR-0023 design D4 ordering-as-guards): the
  // dpni object waits on its dpbp pool companion.
  pure def companionsReady(f: Family, st: Family -> Status): bool =
    if (f == Dpni) st.get(Dpbp) == Converged else true

  // Containment order (ADR-0023 design D4; ADR-0004): a child-homed family's
  // object waits on its container dprc converging first. Census-read (same MC
  // authority, ADR-0023 design D9) — no live-read.
  pure def containmentReady(f: Family, st: Family -> Status): bool =
    CHILD_HOMED.contains(f) implies st.get(Dprc) == Converged

  // A live resident still homes in the dprc (ORCH-I7): a child-homed family that
  // is converged or drifted. The container cannot drift or drain under one.
  pure def childLive(st: Family -> Status): bool =
    CHILD_HOMED.exists(c => st.get(c) == Converged or st.get(c) == Drifted)

  // Gross draw (ADR-0023 design D8): the plan's full derived draw, planned frees
  // uncounted.
  pure def grossPoolDraw(pend: Family -> int): int =
    TABLE_FAMS.filter(f => execKind(f) == Some(PoolDraw)).fold(0, (a, f) => a + pend.get(f))
  pure def grossFdbDraw(pend: Family -> int): int =
    TABLE_FAMS.filter(f => execKind(f) == Some(FdbDraw)).fold(0, (a, f) => a + pend.get(f))
      + (if (WANT_PORT) 1 else 0)

  // ---- pass enablement ----

  // The create preconditions. ORCH-I1 asserts the executor is enabled whenever
  // these hold — masking (ADR-0023 resource-aware-orchestration task 2.1) adds
  // `and not masked.contains(f)` to `execEnabled` alone, so a masked family
  // satisfies the guards yet is dead.
  def createGuards(f: Family): bool = and {
    verdict == Admitted,
    isCreate(f),
    status.get(f) == Missing,
    pending.get(f) > 0,
    not(refused.get(f)),
    companionsReady(f, status),
    containmentReady(f, status),
    budgetOk(f),
  }
  def execEnabled(f: Family): bool = createGuards(f)

  def reconcileEnabled(f: Family): bool = status.get(f) == Drifted
  // Teardown reverses create (ADR-0023 design D4; ORCH-I7): the container dprc
  // drains only once every child-homed family is gone.
  def pruneEnabled(f: Family): bool = and {
    status.get(f) == Orphan,
    (f == Dprc) implies CHILD_HOMED.forall(c => status.get(c) == Missing),
  }

  // The Unimplemented refusal pass (ADR-0023 design D10): refusable while entries pend.
  def refuseEnabled(f: Family): bool = and {
    isUnimplemented(f),
    pending.get(f) > 0,
    not(refused.get(f)),
  }

  // The switch-port wire — the one live-read guard (ADR-0023 design D9): it reads
  // the live `occ`, never `censusOcc`, so the port is wired only while the dpmac
  // reads back driver-free (ORCH-I2; the phylink handover window).
  def wireEnabled: bool = and {
    verdict == Admitted,
    WANT_PORT,
    not(portWired),
    status.get(Dpsw) == Converged,
    occ == Free,
    fdbFree > 0,
  }

  def anyPassEnabled: bool =
    TABLE_FAMS.exists(f => or {
      execEnabled(f), reconcileEnabled(f), pruneEnabled(f), refuseEnabled(f)
    }) or wireEnabled

  // Observable drift still outstanding (ORCH-I5): a family needs a pass, or the
  // wanted edge is not yet wired.
  def constructWork(f: Family): bool = or {
    pending.get(f) > 0 and not(refused.get(f)),
    status.get(f) == Drifted,
    status.get(f) == Orphan,
  }
  def hasWork: bool =
    TABLE_FAMS.exists(f => constructWork(f)) or (WANT_PORT and not(portWired))

  // ---- init ----

  // The family-keyed maps are derived from `TABLE_FAMS`, never hardcoded — the
  // ADR-0023 design D11 promise (a new family is a table row, never model rework)
  // the old literal maps broke. The DPC-born dpmac starts converged with nothing
  // pending (DPMAC-I1); every other family starts missing with one plan entry out.
  action init = all {
    status' = TABLE_FAMS.mapBy(f => if (f == Dpmac) Converged else Missing),
    pending' = TABLE_FAMS.mapBy(f => if (f == Dpmac) 0 else 1),
    occ' = Free,
    censusOcc' = Free,
    portWired' = false,
    // The four pool companions (dpbp/dpio/dpcon/dpmcp) each draw 1, so the gross
    // pool draw is 4 — the headroom an admitted intent needs (ADR-0023 design D8).
    poolFree' = 4,
    fdbFree' = 2,
    verdict' = Unjudged,
    refused' = TABLE_FAMS.mapBy(f => false),
  }

  // ---- observation (the loop boundary) ----

  // Re-observe: snapshot occupancy into the census-read value and re-open the
  // admission judgement (level-triggered, no persisted verdict). Recorded
  // Unimplemented refusals persist until the intent changes.
  action observe = all {
    censusOcc' = occ,
    verdict' = Unjudged,
    status' = status, pending' = pending, occ' = occ, portWired' = portWired,
    poolFree' = poolFree, fdbFree' = fdbFree, refused' = refused,
  }

  // ---- admission (ADR-0023 design D5/D8: gross draw vs observed headroom) ----

  action admit = {
    val fits = grossPoolDraw(pending) <= poolFree and grossFdbDraw(pending) <= fdbFree
    all {
      verdict == Unjudged,
      verdict' = if (fits) Admitted else AdmissionRefused,
      // An over-budget intent refuses whole and typed (ADR-0023 design D5): the
      // drawing families with pending entries are marked refused, never silently kept.
      refused' = if (fits) refused
                 else TABLE_FAMS.fold(refused, (acc, f) =>
                        if (isCreate(f) and pending.get(f) > 0) acc.set(f, true) else acc),
      status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
      portWired' = portWired, poolFree' = poolFree, fdbFree' = fdbFree,
    }
  }

  // ---- engine passes ----

  // Create/converge a construct, draining one plan entry and its budget draw.
  action doCreate(f: Family): bool = all {
    execEnabled(f),
    status' = status.set(f, Converged),
    pending' = pending.set(f, pending.get(f) - 1),
    poolFree' = if (execKind(f) == Some(PoolDraw)) poolFree - 1 else poolFree,
    fdbFree' = if (execKind(f) == Some(FdbDraw)) fdbFree - 1 else fdbFree,
    occ' = occ, censusOcc' = censusOcc, portWired' = portWired,
    verdict' = verdict, refused' = refused,
  }

  // Reconcile a drifted construct back to converged (no draw — it exists).
  action doReconcile(f: Family): bool = all {
    reconcileEnabled(f),
    status' = status.set(f, Converged),
    pending' = pending, poolFree' = poolFree, fdbFree' = fdbFree,
    occ' = occ, censusOcc' = censusOcc, portWired' = portWired,
    verdict' = verdict, refused' = refused,
  }

  // Prune an orphan (teardown reverses create; ADR-0023 design D4), returning its draw.
  action doPrune(f: Family): bool = all {
    pruneEnabled(f),
    status' = status.set(f, Missing),
    poolFree' = if (execKind(f) == Some(PoolDraw)) poolFree + 1 else poolFree,
    fdbFree' = if (execKind(f) == Some(FdbDraw)) fdbFree + 1 else fdbFree,
    pending' = pending, occ' = occ, censusOcc' = censusOcc, portWired' = portWired,
    verdict' = verdict, refused' = refused,
  }

  // The typed Unimplemented refusal (ADR-0023 design D10): dispatched before any
  // verb, draining the plan entries as refused — never left silently pending.
  action doRefuse(f: Family): bool = all {
    refuseEnabled(f),
    refused' = refused.set(f, true),
    pending' = pending.set(f, 0),
    status' = status, poolFree' = poolFree, fdbFree' = fdbFree,
    occ' = occ, censusOcc' = censusOcc, portWired' = portWired, verdict' = verdict,
  }

  // Wire the switch port over the live-read driver-free dpmac (ADR-0023 D9 the
  // one live-read guard). The SwitchHeld move is the kernel's — the env effects it.
  action doWire = all {
    wireEnabled,
    portWired' = true,
    fdbFree' = fdbFree - 1,
    status' = status, pending' = pending, poolFree' = poolFree,
    occ' = occ, censusOcc' = censusOcc, verdict' = verdict, refused' = refused,
  }

  // ---- environment adversary (ADR-0023 design D7) ----

  // The standalone mac driver grabs a free dpmac — only while the switch has not
  // wired the port (a switch-held port is beyond the mac driver's reach; this is
  // what keeps ORCH-I2 safe against interference).
  action envOccMac = all {
    occ == Free, not(portWired),
    occ' = MacHeld,
    status' = status, pending' = pending, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, fdbFree' = fdbFree,
    verdict' = verdict, refused' = refused,
  }

  // A driver releases the dpmac back to free.
  action envOccFree = all {
    occ != Free,
    occ' = Free,
    status' = status, pending' = pending, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, fdbFree' = fdbFree,
    verdict' = verdict, refused' = refused,
  }

  // The switch driver takes over the dpmac once the port is wired (the handover
  // completes in the safe direction).
  action envOccSwitch = all {
    occ == Free, portWired,
    occ' = SwitchHeld,
    status' = status, pending' = pending, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, fdbFree' = fdbFree,
    verdict' = verdict, refused' = refused,
  }

  // A converged construct drifts under the environment (spec scenario: the
  // environment interferes between passes; the next observation re-judges).
  action envDrift(f: Family): bool = all {
    isCreate(f), status.get(f) == Converged,
    // Containment (ADR-0023 design D4 reverse-on-teardown; ORCH-I7): the container
    // dprc never drifts out from under a live resident.
    (f == Dprc) implies not(childLive(status)),
    status' = status.set(f, Drifted),
    pending' = pending, occ' = occ, censusOcc' = censusOcc, portWired' = portWired,
    poolFree' = poolFree, fdbFree' = fdbFree, verdict' = verdict, refused' = refused,
  }

  // A concurrent writer eats free budget headroom (never below zero — the board
  // is the only ledger; a post-admission squeeze resolves at the next loop,
  // ADR-0023 design D7).
  action envEatPool = all {
    poolFree > 0, poolFree' = poolFree - 1,
    status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
    portWired' = portWired, fdbFree' = fdbFree, verdict' = verdict, refused' = refused,
  }
  action envEatFdb = all {
    fdbFree > 0, fdbFree' = fdbFree - 1,
    status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, verdict' = verdict, refused' = refused,
  }

  action stutter = all {
    status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, fdbFree' = fdbFree,
    verdict' = verdict, refused' = refused,
  }

  action doEnv = any {
    envOccMac, envOccFree, envOccSwitch,
    nondet f = TABLE_FAMS.oneOf() envDrift(f),
    envEatPool, envEatFdb,
  }

  action step = any {
    observe,
    admit,
    nondet f = TABLE_FAMS.oneOf() doCreate(f),
    nondet f = TABLE_FAMS.oneOf() doReconcile(f),
    nondet f = TABLE_FAMS.oneOf() doPrune(f),
    nondet f = TABLE_FAMS.oneOf() doRefuse(f),
    doWire,
    doEnv,
    stutter,
  }

  // ---- named invariants (ADR-0023 "Named invariants") ----

  // ORCH-I1 completeness / no dead family: whenever a family's create
  // preconditions hold, its executor is actually enabled (never masked/dead),
  // and a pending Unimplemented family is refusable. Masking a pass guard
  // (ADR-0023 resource-aware-orchestration task 2.1) breaks the implication
  // within bound — the retro-oracle the gap class (bead dpaa2-controlplane-dbf.19)
  // requires.
  val ORCH_I1: bool =
    TABLE_FAMS.forall(f => and {
      createGuards(f) implies execEnabled(f),
      (isUnimplemented(f) and pending.get(f) > 0 and not(refused.get(f))) implies refuseEnabled(f),
    })

  // ORCH-I2 order as guards: the switch take-over never fires while the
  // standalone mac driver holds the dpmac — unreachable with the live-read in
  // place (ADR-0023 resource-aware-orchestration task 2.2 removes it to show the
  // state becomes reachable).
  val ORCH_I2: bool = (occ == MacHeld) implies not(portWired)

  // ORCH-I3 admission soundness: dispatch never overdraws a budget, so an
  // admitted plan converges without a capacity errno (the verdict enum makes
  // admitted-and-refused unrepresentable; stable refusal is `stableRefusalTest`).
  val ORCH_I3: bool = poolFree >= 0 and fdbFree >= 0

  // ORCH-I5 quiescence / no churn: no engine pass is enabled without observable
  // drift — at the fixed point nothing fires.
  val ORCH_I5: bool = anyPassEnabled implies hasWork

  // ORCH-I7 containment order: a child-homed family (dpni/dpbp/dpio/dpcon/dpmcp)
  // is converged or drifted only while its container dprc is converged — the
  // dprc's instance of ADR-0023 design D4 "companions ≺ object ≺ edges; reverse on
  // teardown" (ADR-0004 containment). The create guard (`containmentReady`) builds
  // the forward order, the prune/drift guards the reverse.
  val ORCH_I7: bool =
    CHILD_HOMED.forall(f =>
      (status.get(f) == Converged or status.get(f) == Drifted)
        implies status.get(Dprc) == Converged)

  // Apalache-marked subset (ladder bounds): I1, I2, I3, I5, I7. ORCH-I4 (bounded
  // convergence) is liveness, witnessed by `convergeTest`; ORCH-I6 (confluence)
  // is stated here, checked by resource-aware-orchestration task 7.1.
  val stateInvariants: bool = and { ORCH_I1, ORCH_I2, ORCH_I3, ORCH_I5, ORCH_I7 }

  // ---- directed runs (frozen for ITF replay via `pnpm model:freeze-orch`) ----

  // ORCH-I4 bounded convergence under env quiescence: admit, run every pass in a
  // dependency-respecting order, refuse the Unimplemented family, wire the port,
  // let the kernel complete the handover — and reach a fixed point where no
  // engine pass is enabled and no work remains.
  run convergeTest =
    init
      .then(admit)
      .then(doCreate(Dprc))
      .then(doCreate(Dpbp))
      .then(doCreate(Dpio))
      .then(doCreate(Dpcon))
      .then(doCreate(Dpmcp))
      .then(doCreate(Dpni))
      .then(doCreate(Dpsw))
      .then(doRefuse(Dpseci))
      .then(doWire)
      .then(envOccSwitch)
      .expect(and { not(anyPassEnabled), not(hasWork), occ == SwitchHeld })

  // ORCH-I2 the handover window: the mac driver grabs the dpmac, the live-read
  // refuses the wire; once it releases and the loop re-observes Free, the wire
  // fires and the kernel take-over completes — ORCH-I2 held throughout.
  run handoverSafeTest =
    init
      .then(admit)
      .then(doCreate(Dprc))
      .then(doCreate(Dpbp))
      .then(doCreate(Dpni))
      .then(doCreate(Dpsw))
      .then(envOccMac)
      .expect(occ == MacHeld and not(portWired) and not(wireEnabled))
      .then(envOccFree)
      .then(observe)
      .expect(censusOcc == Free and verdict == Unjudged)
      .then(admit)
      .then(doWire)
      .expect(portWired and occ == Free)
      .then(envOccSwitch)
      .expect(occ == SwitchHeld and portWired)

  // ORCH-I3 admission refuses the gross draw: the environment eats the pool
  // headroom before admission, so the over-budget intent refuses whole and typed
  // and no verb dispatches (ADR-0023 design D8 gross draw).
  run admissionRefusesTest =
    init
      .then(envEatPool)
      .then(envEatPool)
      .then(admit)
      .expect(verdict == AdmissionRefused and refused.get(Dpbp))
      .then(doCreate(Dpbp).fail())

  // ORCH-I3 stable refusal: a refused intent re-observed and re-judged against
  // the same census refuses identically — refusals never oscillate
  // (ADR-0023 design D5).
  run stableRefusalTest =
    init
      .then(envEatPool)
      .then(envEatPool)
      .then(admit)
      .expect(verdict == AdmissionRefused)
      .then(observe)
      .then(admit)
      .expect(verdict == AdmissionRefused and refused.get(Dpbp))

  // ORCH-I1 the Unimplemented row (dpseci): a pending family with no consuming
  // pass refuses typed, draining its entries — never left silently pending.
  run unimplementedRefusesTest =
    init
      .then(doRefuse(Dpseci))
      .expect(refused.get(Dpseci) and pending.get(Dpseci) == 0)
      .then(doRefuse(Dpseci).fail())

  // Spec scenario — the environment interferes between passes: a converged
  // construct drifts, and the next pass re-judges and re-converges it
  // (ADR-0023 design D7).
  run envInterferenceTest =
    init
      .then(admit)
      .then(doCreate(Dprc))
      .then(doCreate(Dpbp))
      .then(doCreate(Dpni))
      .then(envDrift(Dpni))
      .expect(status.get(Dpni) == Drifted)
      .then(doReconcile(Dpni))
      .expect(status.get(Dpni) == Converged)
}
