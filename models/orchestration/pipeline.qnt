// The ensure pipeline as a guarded-action convergence model (ADR-0023; the
// resource-aware-orchestration change design D1). Actions are the engine's
// passes plus an observation action, an adversarial environment action, and a
// rollback action over a run-journal projection (the executed creates of the
// current loop, in execution order; plan-rollback design D8). The
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

  // The per-run mid-run failure policy (plan-rollback design D6): `Rollback` (the
  // default) unwinds the run journal; `Abort` stops and leaves the executed prefix
  // for the next loop to re-judge. `Continue` is the named deferred variant and is
  // deliberately NOT a model value.
  type Policy = Rollback | Abort

  // A run-journal entry (plan-rollback design D8): a faithful witness of an executed
  // actuation, not only its budget draw. `JCreate(f)` is a family create (draws, so undo
  // returns the draw); `JWire` is the switch-port wire (draws no FDB unit, V-DPSW-2, so undo
  // returns nothing and only clears the edge). The Rust twin (group 5) journals wires/binds
  // too, so the two must not diverge on what counts as an actuation.
  type JEntry = JCreate(Family) | JWire

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
  var poolFree: int                   // container-pool census budget, free count (ADR-0011)
  var globalPoolFree: int             // MC-global pool ledger, free count — the hardware row the census cannot see (ADR-0011; plan-rollback design D2/D9)
  var fdbFree: int                    // dpsw FDB budget, free count (V-DPSW-2)
  var verdict: Verdict                // admission verdict for the compiled plan
  var refused: Family -> bool         // typed whole-intent refusal recorded (ADR-0023 design D5/D10)
  // The run-journal projection (plan-rollback design D8): the current loop's executed
  // actuations (creates and the switch-port wire) in execution order — a faithful witness,
  // not just the budget-drawing subset. Reset at each loop boundary (`observe`); unwound by `doRollback`.
  var journal: List[JEntry]
  var policy: Policy                  // the per-run mid-run failure policy (plan-rollback design D6)
  var residue: int                    // typed irreversible residue: spent dpio seats (ADR-0003 §7)
  // Set by a rollback, cleared by the re-admission (`observe`/`admit`): forbids any
  // re-dispatch of a rolled-back run before it is re-judged (ORCH-I3 restated, no oscillation).
  var reverted: bool

  // The nine vars bundled as one record (ADR-0023 design D11): the pure pass
  // twins and the ORCH-I6 pairwise diamond read and transform `State`, and `cur`
  // lifts the live vars into it, so the actions and the confluence check share
  // one transition definition (resource-aware-orchestration task 7.1).
  type State = {
    status: Family -> Status, pending: Family -> int,
    occ: Occupancy, censusOcc: Occupancy, portWired: bool,
    poolFree: int, globalPoolFree: int, fdbFree: int, verdict: Verdict, refused: Family -> bool,
    journal: List[JEntry], policy: Policy, residue: int, reverted: bool,
  }
  val cur: State = {
    status: status, pending: pending, occ: occ, censusOcc: censusOcc,
    portWired: portWired, poolFree: poolFree, globalPoolFree: globalPoolFree, fdbFree: fdbFree,
    verdict: verdict, refused: refused,
    journal: journal, policy: policy, residue: residue, reverted: reverted,
  }

  // ---- family-table helpers (ADR-0023 design D10/D11) ----

  pure def execKind(f: Family): Option[ResKind] =
    match TABLE.get(f) { | Execute(k) => Some(k) | Unimplemented(_) => None }

  // A create pass exists (Execute, drawing) — dpmac's NoCreate row has none.
  pure def isCreate(f: Family): bool =
    match execKind(f) { | None => false | Some(k) => k != NoCreate }

  // The family carries no consuming pass (the Unimplemented disposition).
  pure def isUnimplemented(f: Family): bool = execKind(f) == None

  // The families with a proven MC-global pool row (ADR-0011; plan-rollback design D2): bp (dpbp)
  // — V-CEIL-1, V-DPRC-7 rev 2. The Rust face also carries the proven dpcon `swpch.2wq` row
  // (V-DPCON-2), but the single `globalPoolFree` scalar cannot carry two independent rows, so
  // this set stays bp-only until a dpcon-drain transition is worth freezing (after the
  // per-family replay pricing, bead dpaa2-controlplane-97v). dpmcp stays out. A create of
  // such a family draws the global ledger of its family.
  pure val GLOBAL_ROWS: Set[Family] = Set(Dpbp)
  pure def drawsGlobal(f: Family): bool = GLOBAL_ROWS.contains(f)

  // The firmware enforces the real budget at dispatch, so this guard is
  // census-read (ADR-0023 design D9): a stale read costs one refused verb.
  pure def budgetOkOn(s: State, f: Family): bool = and {
    match execKind(f) {
      | None => true
      | Some(k) => match k {
        | PoolDraw => s.poolFree > 0
        | FdbDraw => s.fdbFree > 0
        | PlainCreate => true
        | NoCreate => true
      }
    },
    // The MC-global pool row of the family (plan-rollback design D2/D9): a hardware-side
    // drain refuses the create even when the census row shows headroom (ADR-0011).
    drawsGlobal(f) implies s.globalPoolFree > 0,
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
  // The MC-global pool draw: the pending creates of the families with a proven global
  // row (bp now; plan-rollback design D2/D9). Admission judges it against `globalPoolFree`,
  // so a pending bp create refuses at planning time when the global row cannot cover it even
  // with census headroom — closing the rev-3 blindness (ADR-0011; the census-blind admission).
  pure def grossGlobalDraw(pend: Family -> int): int =
    TABLE_FAMS.filter(f => drawsGlobal(f)).fold(0, (a, f) => a + pend.get(f))

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

  // Overcommit (plan-rollback design D1): a create whose every guard holds EXCEPT the
  // budget draw — the lockless admission plus a concurrent writer jointly exceeded a
  // budget after admission, discovered at dispatch. This is the mid-run guard refusal
  // the rollback/abort dispositions answer, distinct from an admission-time headroom refusal.
  pure def overcommitAt(s: State, f: Family): bool = and {
    s.verdict == Admitted,
    isCreate(f),
    s.status.get(f) == Missing,
    s.pending.get(f) > 0,
    not(s.refused.get(f)),
    companionsReady(f, s.status),
    containmentReady(f, s.status),
    noUnimplementedPendingOn(s),
    not(budgetOkOn(s, f)),
  }
  pure def anyOvercommitOn(s: State): bool = TABLE_FAMS.exists(f => overcommitAt(s, f))
  def anyOvercommit: bool = anyOvercommitOn(cur)

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
    // The MC-global bp pool starts with its own free count (plan-rollback design D2/D9);
    // the hardware drain moves it apart from the census row (ADR-0011).
    globalPoolFree' = 4,
    fdbFree' = 2,
    verdict' = Unjudged,
    refused' = TABLE_FAMS.mapBy(f => false),
    // Empty journal, the Default policy (plan-rollback design D6), no residue, no pending revert.
    journal' = [],
    policy' = Rollback,
    residue' = 0,
    reverted' = false,
  }

  // ---- observation (the loop boundary) ----

  // Re-observe: snapshot occupancy into the census-read value and re-open the
  // admission judgement (level-triggered, no persisted verdict). Recorded
  // Unimplemented refusals persist until the intent changes.
  action observe = all {
    censusOcc' = occ,
    verdict' = Unjudged,
    status' = status, pending' = pending, occ' = occ, portWired' = portWired,
    poolFree' = poolFree, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree, refused' = refused,
    // A new loop is a new run: empty journal, revert flag cleared. Residue is grow-only
    // across loops (plan-rollback design D5); the policy persists until re-chosen.
    journal' = [], reverted' = false, policy' = policy, residue' = residue,
  }

  // ---- admission (ADR-0023 design D5/D8: gross draw vs observed headroom) ----

  action admit = {
    // Admission reads both ledgers (plan-rollback design D2/D9): the census rows (pool, FDB)
    // and the MC-global pool row of each family that proves one (bp). A pending bp create is
    // refused at planning time when the global row cannot cover it even with census headroom.
    val fits = grossPoolDraw(pending) <= poolFree and grossFdbDraw(pending) <= fdbFree
               and grossGlobalDraw(pending) <= globalPoolFree
    all {
      verdict == Unjudged,
      verdict' = if (fits) Admitted else AdmissionRefused,
      // An over-budget intent refuses whole and typed (ADR-0023 design D5): the
      // drawing families with pending entries are marked refused, never silently kept.
      refused' = if (fits) refused
                 else TABLE_FAMS.fold(refused, (acc, f) =>
                        if (isCreate(f) and pending.get(f) > 0) acc.set(f, true) else acc),
      status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
      portWired' = portWired, poolFree' = poolFree, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree,
      // Admission is the re-judgement: it clears the revert flag (the drained budget is
      // re-judged here), carrying the journal/policy/residue (plan-rollback design D8).
      journal' = journal, policy' = policy, residue' = residue, reverted' = false,
    }
  }

  // ---- engine passes ----

  // Each engine pass's state update as a pure `State -> State` twin, an
  // ADR-0023 design D11 census transform. The actions below commit the twin's
  // result and the ORCH-I6 pairwise diamond applies the same twins to states,
  // so there is one transition definition and no hand-copy that can drift
  // (resource-aware-orchestration task 7.1).

  // Create/converge a construct, draining one plan entry and its budget draw, and
  // recording the actuation in the run journal (plan-rollback design D8). The journal
  // witnesses executed actuations, so a create is recorded here and the wire in `applyWire`;
  // reconcile/prune/refuse are not forward actuations of the run and stay unrecorded.
  pure def applyCreate(s: State, f: Family): State = { ...s,
    status: s.status.set(f, Converged),
    pending: s.pending.set(f, s.pending.get(f) - 1),
    poolFree: if (execKind(f) == Some(PoolDraw) and f != Dpio) s.poolFree - 1 else s.poolFree,
    // A root create lowers both the census row and the MC-global row of its family
    // (plan-rollback design D2/D9): the two ledgers move together on a root draw.
    globalPoolFree: if (drawsGlobal(f)) s.globalPoolFree - 1 else s.globalPoolFree,
    fdbFree: if (execKind(f) == Some(FdbDraw)) s.fdbFree - 1 else s.fdbFree,
    journal: s.journal.append(JCreate(f)),
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
    // Teardown returns the MC-global row symmetrically with the census row (bp moves
    // symmetrically to the object, V-DPRC-7 rev 2; plan-rollback design D2/D9).
    globalPoolFree: if (drawsGlobal(f)) s.globalPoolFree + 1 else s.globalPoolFree,
    fdbFree: if (execKind(f) == Some(FdbDraw)) s.fdbFree + 1 else s.fdbFree,
  }

  // The typed Unimplemented refusal (ADR-0023 design D10): draining the plan
  // entries as refused — never left silently pending.
  pure def applyRefuse(s: State, f: Family): State = { ...s,
    refused: s.refused.set(f, true),
    pending: s.pending.set(f, 0),
  }

  // Wire the switch port (ADR-0023 design D9); the SwitchHeld move is the
  // kernel's, so `occ` is untouched here — the env effects it. The wire is an executed
  // actuation, so it is journaled (plan-rollback design D8) and unwound on rollback.
  pure def applyWire(s: State): State = { ...s,
    portWired: true,
    journal: s.journal.append(JWire),
  }

  // ---- rollback / abort dispositions (plan-rollback design D6/D8) ----

  // Undo one journaled actuation. The reversal table (plan-rollback design D5) is total:
  //   - JWire returns nothing (a wire draws no FDB unit, V-DPSW-2) and only clears the edge;
  //   - a reversible JCreate destroys the construct, restores its plan entry, and returns its
  //     budget draw (the forward ordering guards reversed collapse to one atomic status step
  //     in the census projection);
  //   - the irreversible JCreate(Dpio) is spent headroom that never returns (ADR-0003 §7), so
  //     it contributes typed residue instead of undoing (its object and drawn-down plan stand);
  //   - a container JCreate(Dprc) is left unreversed while a resident still stands (ORCH-I7; the
  //     MC refuses a dprc destroy with live residents — plan-rollback design D5 ResidentsDrained
  //     reversed-ordering plus plan-rollback design D6 unreversed-and-continue), so an irreversible
  //     dpio seat strands its dprc Converged too: containment holds, a resident never outlives its dprc.
  pure def undoOne(s: State, e: JEntry): State =
    match e {
      | JWire => { ...s, portWired: false }
      | JCreate(f) =>
        if (f == Dpio)
          { ...s, residue: s.residue + 1 }
        else if (f == Dprc and childLive(s.status))
          // The container's teardown is blocked while a resident stands: dprc stays Converged with
          // its create unreversed — no status/plan/budget return — mirroring `pruneEnabled(Dprc)`'s
          // all-children-Missing guard (ORCH-I7; plan-rollback design D5/D6).
          s
        else
          { ...s,
            status: s.status.set(f, Missing),
            pending: s.pending.set(f, s.pending.get(f) + 1),
            poolFree: if (execKind(f) == Some(PoolDraw)) s.poolFree + 1 else s.poolFree,
            // The reverse journal returns the MC-global draw too (plan-rollback design D2/D9).
            globalPoolFree: if (drawsGlobal(f)) s.globalPoolFree + 1 else s.globalPoolFree,
            fdbFree: if (execKind(f) == Some(FdbDraw)) s.fdbFree + 1 else s.fdbFree,
          }
    }

  // The journal in reverse (the foldl-prepend idiom, models/intent/main.qnt): the fixed
  // unwind order plan-rollback design D8 mandates, so ORCH-I6 gains no scheduler choice.
  pure def reversed(l: List[JEntry]): List[JEntry] = l.foldl([], (acc, e) => [e].concat(acc))

  // Rollback (plan-rollback design D8): fold the reverse journal onto the census, then end
  // the run — journal emptied, verdict reset to Unjudged (forcing re-admission), and the
  // revert flag raised so no re-dispatch slips through before the next admission re-judges.
  pure def applyRollback(s: State): State =
    val undone = reversed(s.journal).foldl(s, (acc, e) => undoOne(acc, e))
    { ...undone, journal: [], verdict: Unjudged, reverted: true }

  // Abort (plan-rollback design D6): stop, keep the executed prefix on the board, end the
  // run so the next loop re-judges. No undo, no residue — the prefix is left standing.
  pure def applyAbort(s: State): State = { ...s,
    journal: [], verdict: Unjudged, reverted: false,
  }

  // Commit a twin's resulting `State` onto the model vars (ADR-0023 design D11).
  action commitState(ns: State): bool = all {
    status' = ns.status,
    pending' = ns.pending,
    occ' = ns.occ,
    censusOcc' = ns.censusOcc,
    portWired' = ns.portWired,
    poolFree' = ns.poolFree,
    globalPoolFree' = ns.globalPoolFree,
    fdbFree' = ns.fdbFree,
    verdict' = ns.verdict,
    refused' = ns.refused,
    journal' = ns.journal,
    policy' = ns.policy,
    residue' = ns.residue,
    reverted' = ns.reverted,
  }

  action doCreate(f: Family): bool = all { execEnabled(f), commitState(applyCreate(cur, f)) }
  action doReconcile(f: Family): bool = all { reconcileEnabled(f), commitState(applyReconcile(cur, f)) }
  action doPrune(f: Family): bool = all { pruneEnabled(f), commitState(applyPrune(cur, f)) }
  action doRefuse(f: Family): bool = all { refuseEnabled(f), commitState(applyRefuse(cur, f)) }
  action doWire = all { wireEnabled, commitState(applyWire(cur)) }

  // The rollback disposition: enabled on a mid-run overcommit under the rollback policy
  // (plan-rollback design D8). Unwinds the reverse journal via the shared `applyRollback`
  // twin, so no hand-copied effect can drift from the retro re-apply variant.
  action doRollback = all { anyOvercommit, policy == Rollback, commitState(applyRollback(cur)) }

  // The abort disposition: enabled on a mid-run overcommit under the abort policy; stops
  // and leaves the executed prefix for the next loop (plan-rollback design D6).
  action doAbort = all { anyOvercommit, policy == Abort, commitState(applyAbort(cur)) }

  // The caller chooses the per-run failure policy at planning time, i.e. at the loop
  // boundary before admission (plan-rollback design D6). Modeled as a free choice over the
  // two values; `Continue` is deferred and has no action.
  action setPolicy(p: Policy): bool = all {
    verdict == Unjudged,
    policy' = p,
    status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree,
    verdict' = verdict, refused' = refused,
    journal' = journal, residue' = residue, reverted' = reverted,
  }

  // ---- environment adversary (ADR-0023 design D7) ----

  // The standalone mac driver grabs a free dpmac — only while the switch has not
  // wired the port (a switch-held port is beyond the mac driver's reach; this is
  // what keeps ORCH-I2 safe against interference).
  action envOccMac = all {
    occ == Free, not(portWired),
    occ' = MacHeld,
    status' = status, pending' = pending, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree,
    verdict' = verdict, refused' = refused,
    journal' = journal, policy' = policy, residue' = residue, reverted' = reverted,
  }

  // A driver releases the dpmac back to free.
  action envOccFree = all {
    occ != Free,
    occ' = Free,
    status' = status, pending' = pending, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree,
    verdict' = verdict, refused' = refused,
    journal' = journal, policy' = policy, residue' = residue, reverted' = reverted,
  }

  // The switch driver takes over the dpmac once the port is wired (the handover
  // completes in the safe direction).
  action envOccSwitch = all {
    occ == Free, portWired,
    occ' = SwitchHeld,
    status' = status, pending' = pending, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree,
    verdict' = verdict, refused' = refused,
    journal' = journal, policy' = policy, residue' = residue, reverted' = reverted,
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
    poolFree' = poolFree, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree, verdict' = verdict, refused' = refused,
    journal' = journal, policy' = policy, residue' = residue, reverted' = reverted,
  }

  // The environment withdraws an intent's claim on a converged construct, orphaning it
  // (ADR-0023 design D7) so the prune lane fires; the dprc never orphans under a live resident.
  action envOrphan(f: Family): bool = all {
    isCreate(f), status.get(f) == Converged,
    (f == Dprc) implies not(childLive(status)),
    status' = status.set(f, Orphan),
    pending' = pending, occ' = occ, censusOcc' = censusOcc, portWired' = portWired,
    poolFree' = poolFree, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree, verdict' = verdict, refused' = refused,
    journal' = journal, policy' = policy, residue' = residue, reverted' = reverted,
  }

  // A concurrent writer eats free budget headroom (never below zero — the board
  // is the only ledger; a post-admission squeeze resolves at the next loop,
  // ADR-0023 design D7).
  action envEatPool = all {
    poolFree > 0, poolFree' = poolFree - 1,
    status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
    portWired' = portWired, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree, verdict' = verdict, refused' = refused,
    journal' = journal, policy' = policy, residue' = residue, reverted' = reverted,
  }
  action envEatFdb = all {
    fdbFree > 0, fdbFree' = fdbFree - 1,
    status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, globalPoolFree' = globalPoolFree, verdict' = verdict, refused' = refused,
    journal' = journal, policy' = policy, residue' = residue, reverted' = reverted,
  }

  // A create in another container draws the MC-global pool without moving the root
  // census (plan-rollback design D2/D9): the hardware-side drain the census cannot
  // see, so admission's census rows stay blind to it (ADR-0011; rev-3 finding).
  action envEatGlobalPool = all {
    globalPoolFree > 0, globalPoolFree' = globalPoolFree - 1,
    status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, fdbFree' = fdbFree, verdict' = verdict, refused' = refused,
    journal' = journal, policy' = policy, residue' = residue, reverted' = reverted,
  }

  action stutter = all {
    status' = status, pending' = pending, occ' = occ, censusOcc' = censusOcc,
    portWired' = portWired, poolFree' = poolFree, globalPoolFree' = globalPoolFree, fdbFree' = fdbFree,
    verdict' = verdict, refused' = refused,
    journal' = journal, policy' = policy, residue' = residue, reverted' = reverted,
  }

  action doEnv = any {
    envOccMac, envOccFree, envOccSwitch,
    nondet f = TABLE_FAMS.oneOf() envDrift(f),
    nondet f = TABLE_FAMS.oneOf() envOrphan(f),
    envEatPool, envEatFdb, envEatGlobalPool,
  }

  action step = any {
    observe,
    admit,
    nondet f = TABLE_FAMS.oneOf() doCreate(f),
    nondet f = TABLE_FAMS.oneOf() doReconcile(f),
    nondet f = TABLE_FAMS.oneOf() doPrune(f),
    nondet f = TABLE_FAMS.oneOf() doRefuse(f),
    doWire,
    doRollback,
    doAbort,
    nondet p = Set(Rollback, Abort).oneOf() setPolicy(p),
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

  // ORCH-I3 admission soundness, restated under rollback (plan-rollback design D8/D9): dispatch
  // never overdraws a budget — neither the census rows (pool, FDB) NOR the MC-global pool row
  // (`globalPoolFree >= 0`, the hardware vector) — the verdict enum making admitted-and-refused
  // unrepresentable. AND a rolled-back run cannot re-dispatch before it is re-judged: `reverted`
  // (raised by a rollback) forbids any verdict but Unjudged, so the next admission re-judges the
  // drained budget — census or global — and refuses at planning time, no apply/rollback oscillation.
  // Two retro variants turn this red: the re-apply-without-re-admission variant (keeps the verdict
  // Admitted) and the masked-global-row variant (`doRollbackGlobalBlind`: census-blind re-admission
  // re-admits the global-drained plan, the rev-3 oscillation); the shipped rollback resets to
  // Unjudged and reads the global row, so both stay green. Net identity holds up to the typed
  // residue (`residue`) and the container a standing resident blocks from teardown (an irreversible
  // dpio seat strands its dprc Converged, ORCH-I7) — the effects a rolled-back run leaves;
  // `overcommitRollbackTest` (the global-ledger drain) and `overcommitAbortTest` (the census drain)
  // witness the dispositions.
  val ORCH_I3: bool =
    poolFree >= 0 and fdbFree >= 0 and globalPoolFree >= 0 and (reverted implies verdict == Unjudged)

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
  // (`observe`/`admit`/`setPolicy`), the mid-run dispositions (`doRollback`/`doAbort`, whose
  // unwind order is the fixed reverse journal — no scheduler choice, plan-rollback design D8)
  // and `stutter` are not scheduler-ordered engine passes, so they are out of the alphabet.
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

  // ORCH-I3 restated — the overcommit-rollback lane on the HARDWARE vector (plan-rollback design D2/D9;
  // the spec scenario "the rollback action unwinds the journal projection"): admission passes,
  // the prefix dprc/dpio/dpsw plus the wire executes and journals, then a concurrent create in
  // ANOTHER container drains the MC-global bp row (`envEatGlobalPool`) without moving the root census
  // — the hardware-side drain the census cannot see. A pending dpbp create then overcommits on the
  // global row though its census row has headroom (the mid-run guard bites on the global ledger
  // alone). Under the rollback policy the run unwinds: the reversible dpsw create and the wire net
  // to identity — status back to Missing, plan entry restored, dpsw's FDB draw returned, portWired
  // false — while the irreversible dpio seat stands as typed residue (ADR-0003 §7) and strands its
  // container dprc Converged (ORCH-I7; the MC refuses a dprc destroy with a live resident). The
  // revert flag plus Unjudged verdict force a re-admission, which re-judges the still-drained GLOBAL
  // row and refuses at planning time on it (census still has headroom): no apply/rollback oscillation.
  // NOTE: frozen in models/traces/orchestration/ and re-frozen with the global vector, replayed by
  // orchestration_replay.rs — see plan-rollback design D10.
  run overcommitRollbackTest =
    init
      .then(doRefuse(Dpseci))
      .then(admit)
      .expect(verdict == Admitted)
      .then(doCreate(Dprc))
      .then(doCreate(Dpio))
      .then(doCreate(Dpsw))
      .then(doWire)
      // The journal witnesses every executed actuation — the three creates AND the wire. dpbp is
      // deliberately NOT created: it is the pending create the global drain overcommits.
      .expect(and {
        journal == [JCreate(Dprc), JCreate(Dpio), JCreate(Dpsw), JWire],
        poolFree == 4, fdbFree == 1, globalPoolFree == 4, portWired,
      })
      .then(envEatGlobalPool)
      .then(envEatGlobalPool)
      .then(envEatGlobalPool)
      .then(envEatGlobalPool)
      // The MC-global bp row is drained to zero while the census pool is untouched — the overcommit
      // discovered at dispatch: the pending dpbp create is no longer enabled though its census row
      // shows headroom and every other guard holds (the hardware vector, rev-3).
      .expect(and {
        globalPoolFree == 0, poolFree == 4, fdbFree == 1,
        anyOvercommit, verdict == Admitted,
        overcommitAt(cur, Dpbp), not(execEnabled(Dpbp)),
      })
      .then(doRollback)
      // Net identity up to the dpio residue AND its stranded container: the reversible dpsw create
      // and the wire unwind — portWired false, dpsw's FDB draw returned — while the irreversible
      // dpio seat stands as typed residue. That live resident blocks its container's teardown
      // (ORCH-I7; the MC refuses a dprc destroy with a resident), so dprc stays Converged with its
      // create unreversed — containment holds: dpio Converged implies dprc Converged. The census
      // pool is untouched; the global drain persists.
      .expect(and {
        reverted, verdict == Unjudged, journal == [], residue == 1,
        portWired == false,
        status.get(Dpio) == Converged, status.get(Dprc) == Converged, pending.get(Dprc) == 0,
        status.get(Dpsw) == Missing, pending.get(Dpsw) == 1, pending.get(Dpbp) == 1,
        poolFree == 4, fdbFree == 2, globalPoolFree == 0,
      })
      .then(observe)
      .then(admit)
      // Re-admission refuses on the GLOBAL row while the drain holds: the census rows would admit
      // (gross pool draw fits poolFree), but the global row cannot cover the pending bp create.
      .expect(and {
        verdict == AdmissionRefused, refused.get(Dpbp),
        grossPoolDraw(pending) <= poolFree,
        grossGlobalDraw(pending) > globalPoolFree,
      })

  // ORCH-I3 restated — the abort disposition (plan-rollback design D6): same overcommit, but
  // under the abort policy the run stops and leaves the executed prefix standing (no undo, no
  // residue); the next loop re-judges the drained budget and refuses at planning time.
  run overcommitAbortTest =
    init
      .then(setPolicy(Abort))
      .then(doRefuse(Dpseci))
      .then(admit)
      .then(doCreate(Dprc))
      .then(doCreate(Dpbp))
      .then(doCreate(Dpsw))
      .then(doWire)
      .expect(portWired)
      .then(envEatPool)
      .then(envEatPool)
      .then(envEatPool)
      .expect(poolFree == 0 and anyOvercommit and policy == Abort and verdict == Admitted)
      .then(doAbort)
      // Abort leaves the executed prefix standing — no undo, so the wired port stays wired.
      .expect(and {
        verdict == Unjudged, journal == [], residue == 0, reverted == false,
        status.get(Dprc) == Converged, status.get(Dpbp) == Converged,
        status.get(Dpsw) == Converged, portWired,
      })
      .then(observe)
      .then(admit)
      .expect(verdict == AdmissionRefused and refused.get(Dpcon))

  // Witness (plan-rollback design D2/D9; the rev-3 finding, ADR-0011): a hardware-side drain
  // empties the MC-global bp row while the per-container census pool row still shows headroom —
  // the blindness state is representable. A pending dpbp create then refuses on the global row
  // though its census row has room (the mid-run guard bites on the global ledger alone). Scope
  // note: re-framing the overcommit trace and ORCH-I3's re-proof are plan-rollback task 8.2, not here.
  run globalDrainBlindTest =
    init
      .then(doRefuse(Dpseci))
      .then(admit)
      .expect(verdict == Admitted)
      .then(doCreate(Dprc))
      .then(envEatGlobalPool)
      .then(envEatGlobalPool)
      .then(envEatGlobalPool)
      .then(envEatGlobalPool)
      .expect(and {
        globalPoolFree == 0, poolFree == 4,
        overcommitAt(cur, Dpbp), not(budgetOk(Dpbp)),
      })
}
