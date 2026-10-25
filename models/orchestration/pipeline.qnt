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
//   stated here and checked by `ORCH_I6_pairwise` — a bounded pairwise diamond
//   over the engine-pass alphabet (`pnpm model:confluence`), simulator-only
//   outside `stateInvariants`; full treatment stays behind the ADR-0023 revisit
//   trigger (resource-aware-orchestration task 7.1).
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

  // The nine vars bundled as one record (ADR-0023 design D11): the pure pass
  // twins and the ORCH-I6 pairwise diamond read and transform `State`, and `cur`
  // lifts the live vars into it, so the actions and the confluence check share
  // one transition definition (resource-aware-orchestration task 7.1).
  type State = {
    status: Family -> Status, pending: Family -> int,
    occ: Occupancy, censusOcc: Occupancy, portWired: bool,
    poolFree: int, fdbFree: int, verdict: Verdict, refused: Family -> bool,
  }
  val cur: State = {
    status: status, pending: pending, occ: occ, censusOcc: censusOcc,
    portWired: portWired, poolFree: poolFree, fdbFree: fdbFree,
    verdict: verdict, refused: refused,
  }

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
  pure def budgetOkOn(s: State, f: Family): bool =
    match execKind(f) {
      | None => true
      | Some(k) => match k {
        | PoolDraw => s.poolFree > 0
        | FdbDraw => s.fdbFree > 0
        | PlainCreate => true
        | NoCreate => true
      }
    }
  def budgetOk(f: Family): bool = budgetOkOn(cur, f)

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

  // Gross draw (ADR-0023 design D8): the pending creates' draw, planned frees
  // uncounted. dpio is exempt from the pool fold — its seats are spent headroom
  // that never refuses (ADR-0003 §7), the Rust `Spent` row's twin. The FDB fold
  // prices dpsw creates only — a switch-port wire draws no FDB unit (V-DPSW-2, the
  // bracket is dpsw-object-granular).
  pure def grossPoolDraw(pend: Family -> int): int =
    TABLE_FAMS.filter(f => execKind(f) == Some(PoolDraw) and f != Dpio).fold(0, (a, f) => a + pend.get(f))
  pure def grossFdbDraw(pend: Family -> int): int =
    TABLE_FAMS.filter(f => execKind(f) == Some(FdbDraw)).fold(0, (a, f) => a + pend.get(f))

  // ---- pass enablement ----

  // The create preconditions. ORCH-I1 asserts the executor is enabled whenever
  // these hold — masking (ADR-0023 resource-aware-orchestration task 2.1) adds
  // `and not masked.contains(f)` to `execEnabled` alone, so a masked family
  // satisfies the guards yet is dead.
  // Enablement is pure over `State` so the actions and the ORCH-I6 pairwise
  // diamond share one guard definition (resource-aware-orchestration task 7.1);
  // the var-reading names below delegate through `cur`.
  // Whole-intent refusal (ADR-0023 design D10; the engine's `reify_guard`): no create
  // fires while an Unimplemented family still has pending entries.
  pure def noUnimplementedPendingOn(s: State): bool =
    TABLE_FAMS.forall(g => isUnimplemented(g) implies s.pending.get(g) == 0)

  pure def createGuardsOn(s: State, f: Family): bool = and {
    s.verdict == Admitted,
    isCreate(f),
    s.status.get(f) == Missing,
    s.pending.get(f) > 0,
    not(s.refused.get(f)),
    companionsReady(f, s.status),
    containmentReady(f, s.status),
    budgetOkOn(s, f),
    noUnimplementedPendingOn(s),
  }
  def createGuards(f: Family): bool = createGuardsOn(cur, f)
  def execEnabled(f: Family): bool = createGuards(f)

  pure def reconcileEnabledOn(s: State, f: Family): bool = s.status.get(f) == Drifted
  def reconcileEnabled(f: Family): bool = reconcileEnabledOn(cur, f)

  // Teardown reverses create (ADR-0023 design D4; ORCH-I7): the container dprc
  // drains only once every child-homed family is gone.
  pure def pruneEnabledOn(s: State, f: Family): bool = and {
    s.status.get(f) == Orphan,
    (f == Dprc) implies CHILD_HOMED.forall(c => s.status.get(c) == Missing),
  }
  def pruneEnabled(f: Family): bool = pruneEnabledOn(cur, f)

  // The Unimplemented refusal pass (ADR-0023 design D10): refusable while entries pend.
  pure def refuseEnabledOn(s: State, f: Family): bool = and {
    isUnimplemented(f),
    s.pending.get(f) > 0,
    not(s.refused.get(f)),
  }
  def refuseEnabled(f: Family): bool = refuseEnabledOn(cur, f)

  // The switch-port wire — the one live-read guard (ADR-0023 design D9): it reads
  // the live `occ`, never `censusOcc`, so the port is wired only while the dpmac
  // reads back driver-free (ORCH-I2; the phylink handover window).
  pure def wireEnabledOn(s: State): bool = and {
    s.verdict == Admitted,
    WANT_PORT,
    not(s.portWired),
    s.status.get(Dpsw) == Converged,
    s.occ == Free,
  }
  def wireEnabled: bool = wireEnabledOn(cur)

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
    // The three reclaimable pool companions (dpbp/dpcon/dpmcp) each draw 1 — the
    // gross pool draw is 3, dpio exempt (spent headroom, ADR-0003 §7). Four free
    // leaves one slot of headroom an eaten-pool squeeze consumes (ADR-0023 design D8).
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

  // Each engine pass's state update as a pure `State -> State` twin, an
  // ADR-0023 design D11 census transform. The actions below commit the twin's
  // result and the ORCH-I6 pairwise diamond applies the same twins to states,
  // so there is one transition definition and no hand-copy that can drift
  // (resource-aware-orchestration task 7.1).

  // Create/converge a construct, draining one plan entry and its budget draw.
  pure def applyCreate(s: State, f: Family): State = { ...s,
    status: s.status.set(f, Converged),
    pending: s.pending.set(f, s.pending.get(f) - 1),
    poolFree: if (execKind(f) == Some(PoolDraw) and f != Dpio) s.poolFree - 1 else s.poolFree,
    fdbFree: if (execKind(f) == Some(FdbDraw)) s.fdbFree - 1 else s.fdbFree,
  }

  // Reconcile a drifted construct back to converged (no draw — it exists).
  pure def applyReconcile(s: State, f: Family): State = { ...s,
    status: s.status.set(f, Converged),
  }

  // Prune an orphan (teardown reverses create; ADR-0023 design D4), returning its
  // draw. dpio is exempt from the pool reclaim, mirroring `grossPoolDraw`/`applyCreate`:
  // its seats are spent headroom that never drew (ADR-0003 §7), so pruning returns none.
  pure def applyPrune(s: State, f: Family): State = { ...s,
    status: s.status.set(f, Missing),
    poolFree: if (execKind(f) == Some(PoolDraw) and f != Dpio) s.poolFree + 1 else s.poolFree,
    fdbFree: if (execKind(f) == Some(FdbDraw)) s.fdbFree + 1 else s.fdbFree,
  }

  // The typed Unimplemented refusal (ADR-0023 design D10): draining the plan
  // entries as refused — never left silently pending.
  pure def applyRefuse(s: State, f: Family): State = { ...s,
    refused: s.refused.set(f, true),
    pending: s.pending.set(f, 0),
  }

  // Wire the switch port (ADR-0023 design D9); the SwitchHeld move is the
  // kernel's, so `occ` is untouched here — the env effects it.
  pure def applyWire(s: State): State = { ...s,
    portWired: true,
  }

  // Commit a twin's resulting `State` onto the model vars (ADR-0023 design D11).
  action commitState(ns: State): bool = all {
    status' = ns.status,
    pending' = ns.pending,
    occ' = ns.occ,
    censusOcc' = ns.censusOcc,
    portWired' = ns.portWired,
    poolFree' = ns.poolFree,
    fdbFree' = ns.fdbFree,
    verdict' = ns.verdict,
    refused' = ns.refused,
  }

  action doCreate(f: Family): bool = all { execEnabled(f), commitState(applyCreate(cur, f)) }
  action doReconcile(f: Family): bool = all { reconcileEnabled(f), commitState(applyReconcile(cur, f)) }
  action doPrune(f: Family): bool = all { pruneEnabled(f), commitState(applyPrune(cur, f)) }
  action doRefuse(f: Family): bool = all { refuseEnabled(f), commitState(applyRefuse(cur, f)) }
  action doWire = all { wireEnabled, commitState(applyWire(cur)) }

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

  // The environment withdraws an intent's claim on a converged construct, orphaning it
  // (ADR-0023 design D7) so the prune lane fires; the dprc never orphans under a live resident.
  action envOrphan(f: Family): bool = all {
    isCreate(f), status.get(f) == Converged,
    (f == Dprc) implies not(childLive(status)),
    status' = status.set(f, Orphan),
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
    nondet f = TABLE_FAMS.oneOf() envOrphan(f),
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
  // is `ORCH_I6_pairwise` below — simulator-only, deliberately NOT a conjunct here.
  val stateInvariants: bool = and { ORCH_I1, ORCH_I2, ORCH_I3, ORCH_I5, ORCH_I7 }

  // ---- ORCH-I6 confluence (bounded pairwise, simulator-first) ----
  // ADR-0023 design "ORCH-I6" / resource-aware-orchestration task 7.1. The
  // engine-pass alphabet: a create/reconcile/prune/refuse per family-table row
  // plus the single switch-port wire. The adversary (`doEnv`), the loop boundary
  // (`observe`/`admit`) and `stutter` are not scheduler-ordered engine passes, so
  // they are out of the alphabet.
  type Pass = PCreate(Family) | PReconcile(Family) | PPrune(Family) | PRefuse(Family) | PWire

  pure val PASS_ALPHABET: Set[Pass] =
    TABLE_FAMS.map(f => PCreate(f))
      .union(TABLE_FAMS.map(f => PReconcile(f)))
      .union(TABLE_FAMS.map(f => PPrune(f)))
      .union(TABLE_FAMS.map(f => PRefuse(f)))
      .union(Set(PWire))

  pure def passEnabled(s: State, p: Pass): bool =
    match p {
      | PCreate(f) => createGuardsOn(s, f)
      | PReconcile(f) => reconcileEnabledOn(s, f)
      | PPrune(f) => pruneEnabledOn(s, f)
      | PRefuse(f) => refuseEnabledOn(s, f)
      | PWire => wireEnabledOn(s)
    }

  pure def passApply(s: State, p: Pass): State =
    match p {
      | PCreate(f) => applyCreate(s, f)
      | PReconcile(f) => applyReconcile(s, f)
      | PPrune(f) => applyPrune(s, f)
      | PRefuse(f) => applyRefuse(s, f)
      | PWire => applyWire(s)
    }

  // The board-observable census (ADR-0023 design D11): per-construct status, the
  // pending multisets, the switch-port edge, the two budgets and the dpmac
  // occupancy. The ADR-0023 design D9 census-read snapshot (`censusOcc`) and the admission
  // scratch (`verdict`) are not board-observable and no engine pass writes them,
  // so they are out of the confluence projection.
  pure def census(s: State): (Family -> Status, Family -> int, bool, int, int, Occupancy, Family -> bool) =
    (s.status, s.pending, s.portWired, s.poolFree, s.fdbFree, s.occ, s.refused)

  // ORCH-I6 confluence (bounded, pairwise): for the CURRENT reachable state, any
  // two distinct engine passes both enabled AND each still enabled after the
  // other (so guards force no order between them) reach the same census in either
  // order. A pass that disables the other is order-forced by guards and exempt —
  // exactly "wherever guards force none". Simulator-only (`pnpm model:confluence`,
  // resource-aware-orchestration task 7.1): NOT in `stateInvariants`; the full
  // treatment stays behind the ADR-0023 revisit trigger.
  val ORCH_I6_pairwise: bool =
    tuples(PASS_ALPHABET, PASS_ALPHABET).forall(((p, q)) =>
      (and {
        p != q,
        passEnabled(cur, p),
        passEnabled(cur, q),
        passEnabled(passApply(cur, p), q),
        passEnabled(passApply(cur, q), p),
      }) implies
        census(passApply(passApply(cur, p), q)) == census(passApply(passApply(cur, q), p)))

  // ---- directed runs (frozen for ITF replay via `pnpm model:freeze-orch`) ----

  // ORCH-I4 bounded convergence under env quiescence: drop the Unimplemented family
  // from the intent up front (whole-intent refusal), admit, run every pass in a
  // dependency-respecting order, wire the port, let the kernel complete the handover
  // — and reach a fixed point where no engine pass is enabled and no work remains.
  run convergeTest =
    init
      .then(doRefuse(Dpseci))
      .then(admit)
      .then(doCreate(Dprc))
      .then(doCreate(Dpbp))
      .then(doCreate(Dpio))
      .then(doCreate(Dpcon))
      .then(doCreate(Dpmcp))
      .then(doCreate(Dpni))
      .then(doCreate(Dpsw))
      .then(doWire)
      .then(envOccSwitch)
      .expect(and { not(anyPassEnabled), not(hasWork), occ == SwitchHeld })

  // ORCH-I2 the handover window: the mac driver grabs the dpmac, the live-read
  // refuses the wire; once it releases and the loop re-observes Free, the wire
  // fires and the kernel take-over completes — ORCH-I2 held throughout.
  run handoverSafeTest =
    init
      .then(doRefuse(Dpseci))
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
      .then(doRefuse(Dpseci))
      .then(admit)
      .then(doCreate(Dprc))
      .then(doCreate(Dpbp))
      .then(doCreate(Dpni))
      .then(envDrift(Dpni))
      .expect(status.get(Dpni) == Drifted)
      .then(doReconcile(Dpni))
      .expect(status.get(Dpni) == Converged)

  // ORCH-I3 the FDB-side refusal lane (V-DPSW-2 board-proven law): the
  // environment eats the dpsw FDB bracket. The first eat drops it to one — the
  // dpsw create still admits, because the wire draws no FDB unit and the create
  // alone costs one (the amended law; the prior over-pricing refused here, so this
  // admit is the standing re-derivation trip wire). The second eat exhausts the
  // bracket, and the next admission refuses the dpsw create whole and typed
  // (ADR-0023 design D5/D8 gross draw vs re-observation).
  run fdbRefusesTest =
    init
      .then(envEatFdb)
      .then(admit)
      .expect(verdict == Admitted and fdbFree == 1)
      .then(observe)
      .then(envEatFdb)
      .then(admit)
      .expect(verdict == AdmissionRefused and refused.get(Dpsw) and fdbFree == 0)

  // ORCH-I7 the teardown half (ADR-0023 design D4 reverse-on-teardown): the environment
  // orphans a converged construct; the prune reclaims a reclaimable pool draw and the
  // container dprc drains only once its resident is gone (the prune lane, no longer vacuous).
  run teardownPruneTest =
    init
      .then(doRefuse(Dpseci))
      .then(admit)
      .then(doCreate(Dprc))
      .then(doCreate(Dpbp))
      .expect(status.get(Dpbp) == Converged and poolFree == 3)
      .then(envOrphan(Dpbp))
      .expect(status.get(Dpbp) == Orphan)
      .then(doPrune(Dpbp))
      .expect(status.get(Dpbp) == Missing and poolFree == 4)
      .then(envOrphan(Dprc))
      .expect(status.get(Dprc) == Orphan)
      .then(doPrune(Dprc))
      .expect(status.get(Dprc) == Missing)
}
