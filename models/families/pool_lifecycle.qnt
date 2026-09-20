// The P3 allocator-custody substrate (ADR-0019 pattern P3; pool-objects design
// D7 as amended 2026-09-20, ADR-0019). ONE const-parameterized machine over a single
// pooled FAMILY — the common P3 mechanics every allocator family shares, with no
// family particulars. The trio family files (dpbp/dpmcp/dpcon) each carry a thin
// `<fam>_lifecycle` module that instantiates this one contextualized to their
// family (`import pool_lifecycle(FAMILY = Dpbp, CEILING = ...).* `); dpio is
// unpooled and owns its own `dpio_lifecycle` (dpio.qnt), reusing the shared core
// transforms rather than this pooled machine. Re-deriving these mechanics in a
// family file is the review defect the amended ADR-0019 D7 names — a thin
// instantiation is the desired shape.
//
// Scope (pool-objects task 1.1 + 1.2 + 1.3 — phase 1 complete): the custody
// cycle — create → free-pool membership → draw → return, DPBP-I3's dirty return,
// DPBP-I7's census-floor refusal — AND the count-convergence/prune laws
// (pool-objects task 1.3; object-model.md §4, pool-objects design D3):
// deficit→grow, surplus→free-only shrink, ShrinkBelowDraw refusal, prune of
// undeclared non-DPL-born free objects. dpio's seat faces live in dpio_lifecycle.
//
// The state is a CoreState so the core census/membership predicates apply
// unchanged (create_allocate.freePool/inPool/canCreate, lifecycle.createT/plugT/
// markAllocated/destroyT) — no re-derivation of pool mechanics, and the shape
// stays isomorphic (ADR-0002 §3) to the Rust P3 surface phase 2 builds. Params
// come from families/params.qnt ALL_PARAMS (only ALL_PARAMS.get(FAMILY) is ever
// queried); hosting the records there keeps every Family constructor used outside
// the instantiating main module, the QNT404 flattener rule the verify rung needs.
//
// Named invariants (Apalache-marked via `stateInvariants`; dispositions in
// models/COVERAGE.md, "Pool convergence laws" section for the POOL_* laws):
//   POOL_CENSUS (DPBP-I7 floor; ADR-0011) — population never exceeds CEILING;
//   POOL_CUSTODY (object-model.md §3; DPBP-I2) — a drawn individual stays
//     plugged and allocator-held;
//   POOL_DPL_SURVIVES (pool-objects design D3; roadmap #14) — the DPL-born boot object is
//     never destroyed;
//   POOL_MANAGED_LIVE — the managed ghost never drifts off the live pool;
//   POOL_IDEMPOTENT (pool-objects design D3; formal-models req 2) — a converged state enables
//     no grow, shrink, or prune.
// Directed runs: custodyCycleTest, dirtyReturnTest (DPBP-I3), censusRefusesAtCeilingTest
//   (DPBP-I7), convergenceGrowTest, freeOnlyShrinkTest, drawnNeverShrunkTest,
//   shrinkBelowDrawRefusedTest, prunePreservesDplBornTest, idempotentReconvergeTest.
// DPBP-I3, the free-only/refusal/prune runs are directed (simulate-only), the
// split main.qnt applies to its own Breaking candidates.
module pool_lifecycle {
  import types.* from "../core/types"
  import create_allocate.* from "../core/create_allocate"
  import lifecycle.* from "../core/lifecycle"
  import params.* from "./params"

  // The pooled family this instance manages, bound at instantiation by the
  // family's <fam>_lifecycle module. The P3 pattern is one machine, contextualized.
  const FAMILY: Family
  // The pool ceiling for FAMILY — the board's `mc.global --resources` free count
  // (ADR-0011 §3; DPBP-I7's board-settled floor).
  const CEILING: int

  // The container the pool lives in (dprc.1, the Linux root) and its parent
  // mc.global (dprc.0), the minimal tree canCreate/lockedAnywhere walk.
  pure val dprc0: ObjId = { fam: Dprc, num: 0 }
  pure val poolContainer: ObjId = { fam: Dprc, num: 1 }

  // A stand-in consumer the allocator draws on behalf of (object-model.md §3).
  pure val CONSUMER: ObjId = { fam: Dpni, num: 0 }

  // A boot-baseline pooled object of FAMILY: foreign to the reconciler,
  // structurally exempt from prune (roadmap #14 standing rule). Seeded free below.
  pure val born: ObjId = { fam: FAMILY, num: 0 }

  // ---- convergence state (pool-objects task 1.3; pool-objects design D3) ----

  // derivedReq is the compiled per-family target (ADR-0012 counts, consumed as
  // state the environment retargets); managed is the ghost set of companions the
  // reconciler grew (its own to shrink); refusal is the observable ShrinkBelowDraw
  // flag (pool-objects design D3: a requirement below draw surfaces, never a teardown).
  type Conv = { derivedReq: int, managed: Set[ObjId], refusal: bool }

  var s: CoreState
  var conv: Conv

  // ---- census + convergence predicates (ADR-0011; object-model.md §4) ----

  pure def poolPopulation(st: CoreState): int =
    st.objs.keys().filter(o => o.fam == FAMILY and st.objs.get(o).parent == Some(poolContainer)).size()

  // A create is admitted only below the ceiling — the refusal is a disabled
  // guard, never a recorded over-ceiling state (DPBP-I7; ADR-0011).
  pure def censusAdmitsCreate(st: CoreState): bool = poolPopulation(st) < CEILING

  // The reconciler converges the objects it owns: the managed companion count
  // against the derived requirement (all managed are FAMILY objects here).
  pure def managedCount(cv: Conv): int = cv.managed.size()

  pure def drawnManaged(cv: Conv, st: CoreState): int =
    cv.managed.filter(o => st.objs.get(o).allocatedBy != None).size()

  // The prune target set (pool-objects design D3): a pool object undeclared (not managed),
  // not DPL-born, and free.
  pure def prunable(st: CoreState, cv: Conv, o: ObjId): bool = and {
    o.fam == FAMILY,
    st.objs.keys().contains(o),
    st.objs.get(o).parent == Some(poolContainer),
    o != born,
    not(cv.managed.contains(o)),
    st.objs.get(o).allocatedBy == None,
  }

  // Enablement predicates, shared by the guards and the idempotence law.
  pure def growEnabled(st: CoreState, cv: Conv): bool =
    managedCount(cv) < cv.derivedReq and censusAdmitsCreate(st)
  pure def shrinkEnabled(st: CoreState, cv: Conv): bool =
    managedCount(cv) > cv.derivedReq
      and cv.managed.exists(o => st.objs.get(o).allocatedBy == None)
  pure def anyPrunable(st: CoreState, cv: Conv): bool =
    st.objs.keys().exists(o => prunable(st, cv, o))

  // Converged: the managed count meets the requirement and no prune candidate
  // remains (pool-objects design D3; formal-models req 2).
  pure def isConverged(st: CoreState, cv: Conv): bool =
    managedCount(cv) == cv.derivedReq and not(anyPrunable(st, cv))

  // ---- init ----

  pure def container(parent: Option[ObjId], hw: int): ObjState = {
    parent: parent, owner: parent, plugged: true, busVisible: true,
    bind: Unbound, enabled: false, pristine: true, linkUp: false,
    allocatedBy: None, hwId: hw,
  }

  // A boot pool object: plugged and free, so it sits in the free pool from init.
  pure def bootPoolObj(hw: int): ObjState =
    container(Some(poolContainer), hw)

  pure val initialPool: CoreState = {
    objs: Map(
      dprc0 -> container(None, 0),
      poolContainer -> container(Some(dprc0), 1),
      born -> bootPoolObj(50)
    ),
    dprcs: Map(
      dprc0 -> { icid: 0, perms: Set(Spawn, Alloc, ObjCreate, IrqCfg, TopologyChanges), locked: false },
      poolContainer -> { icid: 1, perms: Set(Spawn, Alloc, ObjCreate, IrqCfg), locked: false }
    ),
    conns: Set(),
    // FAMILY's id space skips the boot object so runtime creates never remint it.
    nextNum: FAMILIES.mapBy(f => if (f == Dprc) 2 else if (f == FAMILY) 1 else 0),
    nextHw: 100,
  }

  action init = all {
    s' = initialPool,
    conv' = { derivedReq: 0, managed: Set(), refusal: false },
  }

  // ---- custody transitions (pool-objects task 1.1) ----

  // A bare/undeclared create — census/ceiling gated (DPBP-I7). Not added to
  // managed, so prune can later reclaim it (an out-of-band companion).
  action createPooledAt: bool = all {
    canCreate(s, ALL_PARAMS, FAMILY, poolContainer),
    censusAdmitsCreate(s),
    s' = createT(s, ALL_PARAMS, FAMILY, poolContainer),
    conv' = conv,
  }

  // Plug it: a pooled family thereby enters its container's free pool (inPool is
  // derived — plugged, allocator-held, undrawn; create_allocate.inPool).
  action plugPooledAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    o.fam == FAMILY,
    not(s.objs.get(o).plugged),
    s' = plugT(s, o),
    conv' = conv,
  }

  // Draw: the allocator claims a free individual for a consumer. markAllocated
  // dirties it (pristine false) — a drawn object is programmed by its consumer.
  action drawAt(consumer: ObjId, p: ObjId): bool = all {
    p.fam == FAMILY,
    freePool(s, ALL_PARAMS, poolContainer, FAMILY).contains(p),
    s' = markAllocated(s, consumer, Set(p)),
    conv' = conv,
  }

  // Return to the free pool. No MC reset on the way out (DPBP-I3, DPMCP-I3,
  // DPCON-I5, each doc's "Kernel-side behavior"): pristine keeps whatever it was
  // — the drain is the kernel's, observed not driven (pool-objects design D3).
  action returnAt(p: ObjId): bool = all {
    s.objs.keys().contains(p),
    p.fam == FAMILY,
    s.objs.get(p).allocatedBy != None,
    s' = setObj(s, p, s.objs.get(p).with("allocatedBy", None)),
    conv' = conv,
  }

  // ---- convergence transitions (pool-objects task 1.3; pool-objects design D3) ----

  // Deficit → a managed grow toward the derived count, still census/ceiling gated.
  action growCreateAt: bool =
    val o = { fam: FAMILY, num: s.nextNum.get(FAMILY) }
    all {
      managedCount(conv) < conv.derivedReq,
      canCreate(s, ALL_PARAMS, FAMILY, poolContainer),
      censusAdmitsCreate(s),
      s' = createT(s, ALL_PARAMS, FAMILY, poolContainer),
      conv' = conv.with("managed", conv.managed.union(Set(o))),
    }

  // Surplus → destroy a FREE managed individual, never a drawn one (pool-objects design D3);
  // the victim is chosen nondeterministically — no law distinguishes free ones.
  action shrinkDestroyAt(victim: ObjId): bool = all {
    victim.fam == FAMILY,
    conv.managed.contains(victim),
    managedCount(conv) > conv.derivedReq,
    s.objs.get(victim).allocatedBy == None,
    s' = destroyT(s, victim),
    conv' = conv.with("managed", conv.managed.exclude(Set(victim))),
  }

  // Prune → an undeclared, non-DPL-born, free pool object is deleted (pool-objects design D3;
  // DPL-born boot objects are structurally exempt, roadmap #14).
  action pruneAt(o: ObjId): bool = all {
    prunable(s, conv, o),
    s' = destroyT(s, o),
    conv' = conv,
  }

  // ShrinkBelowDraw: a requirement below the drawn count cannot be met by a
  // free-only shrink, so it surfaces a refusal (the observable flag), never a
  // teardown of a live consumer (pool-objects design D3). s unchanged.
  action shrinkBelowDrawAt: bool = all {
    conv.derivedReq < drawnManaged(conv, s),
    s' = s,
    conv' = conv.with("refusal", true),
  }

  // The environment retargets the derived requirement (compiled counts arrive as
  // state, so requirement-drop and idempotence scenarios are drivable).
  action retargetAt(n: int): bool = all {
    n >= 0 and n <= CEILING,
    s' = s,
    conv' = conv.with("derivedReq", n),
  }

  action stutter = all { s' = s, conv' = conv }

  // ---- nondet wrappers ----

  action doCreatePooled = createPooledAt
  action doPlugPooled = { nondet o = s.objs.keys().oneOf() plugPooledAt(o) }
  action doDraw = { nondet p = s.objs.keys().oneOf() drawAt(CONSUMER, p) }
  action doReturn = { nondet p = s.objs.keys().oneOf() returnAt(p) }
  action doGrow = growCreateAt
  action doShrink = { nondet v = s.objs.keys().oneOf() shrinkDestroyAt(v) }
  action doPrune = { nondet o = s.objs.keys().oneOf() pruneAt(o) }
  action doShrinkBelowDraw = shrinkBelowDrawAt
  action doRetarget = { nondet n = 0.to(CEILING).oneOf() retargetAt(n) }

  action step = any {
    doCreatePooled,
    doPlugPooled,
    doDraw,
    doReturn,
    doGrow,
    doShrink,
    doPrune,
    doShrinkBelowDraw,
    doRetarget,
    stutter,
  }

  // ---- named state invariants ----

  // DPBP-I7 (ADR-0011 census-first): population never exceeds the ceiling.
  val POOL_CENSUS: bool = poolPopulation(s) <= CEILING

  // The allocation-edge custody law (object-model.md §3; DPBP-I2): a drawn
  // individual stays plugged and allocator-held (bind Unbound).
  val POOL_CUSTODY: bool =
    s.objs.keys().forall(o => and {
      o.fam == FAMILY,
      s.objs.get(o).allocatedBy != None,
    } implies and {
      s.objs.get(o).plugged,
      s.objs.get(o).bind == Unbound,
    })

  // DPL-born survive (pool-objects design D3; roadmap #14; object-model.md §4): a boot-baseline
  // object is never destroyed — prune exempts it, shrink touches only managed.
  val POOL_DPL_SURVIVES: bool = s.objs.keys().contains(born)

  // Ghost honesty: every managed companion is a live FAMILY pool object here.
  val POOL_MANAGED_LIVE: bool =
    conv.managed.forall(o => and {
      s.objs.keys().contains(o),
      o.fam == FAMILY,
      s.objs.get(o).parent == Some(poolContainer),
    })

  // Idempotent + level-triggered (pool-objects design D3; formal-models req 2): a converged
  // state enables no create, destroy, or prune — the second pass is a no-op.
  val POOL_IDEMPOTENT: bool =
    isConverged(s, conv) implies and {
      not(growEnabled(s, conv)),
      not(shrinkEnabled(s, conv)),
      not(anyPrunable(s, conv)),
    }

  val stateInvariants: bool = and {
    POOL_CENSUS,
    POOL_CUSTODY,
    POOL_DPL_SURVIVES,
    POOL_MANAGED_LIVE,
    POOL_IDEMPOTENT,
  }

  // ---- directed runs (generic over FAMILY; inherited by each instantiation) ----

  pure val first: ObjId = { fam: FAMILY, num: 1 }
  pure val second: ObjId = { fam: FAMILY, num: 2 }

  // The custody cycle: create → free-pool membership → draw → return.
  run custodyCycleTest =
    init
      .then(createPooledAt)
      .then(plugPooledAt(first))
      .expect(freePool(s, ALL_PARAMS, poolContainer, FAMILY).contains(first))
      .then(drawAt(CONSUMER, first))
      .expect(and {
        s.objs.get(first).allocatedBy == Some(CONSUMER),
        not(freePool(s, ALL_PARAMS, poolContainer, FAMILY).contains(first)),
      })
      .then(returnAt(first))
      .expect(freePool(s, ALL_PARAMS, poolContainer, FAMILY).contains(first))

  // DPBP-I3 (Breaking): a return restores membership without a reset — membership
  // never implies clean state (docs/baseline/{dpbp,dpmcp,dpcon}.md).
  run dirtyReturnTest =
    init
      .then(createPooledAt)
      .then(plugPooledAt(first))
      .then(drawAt(CONSUMER, first))
      .then(returnAt(first))
      .expect(and {
        freePool(s, ALL_PARAMS, poolContainer, FAMILY).contains(first),
        not(s.objs.get(first).pristine),
      })

  // DPBP-I7 (ADR-0011): create-until-refused. At the ceiling the create is
  // disabled by the census — no over-ceiling state exists (born + 2 == CEILING).
  run censusRefusesAtCeilingTest =
    init
      .then(createPooledAt)
      .then(createPooledAt)
      .expect(poolPopulation(s) == CEILING)
      .then(createPooledAt.fail())

  // Convergence deficit → grow to the derived count; at the count, grow is off.
  run convergenceGrowTest =
    init
      .then(retargetAt(2))
      .then(growCreateAt)
      .then(growCreateAt)
      .expect(managedCount(conv) == 2)
      .then(growCreateAt.fail())

  // Surplus → free-only shrink: the free managed individual is the victim, the
  // drawn one survives (pool-objects design D3).
  run freeOnlyShrinkTest =
    init
      .then(retargetAt(2))
      .then(growCreateAt)
      .then(growCreateAt)
      .then(plugPooledAt(first))
      .then(drawAt(CONSUMER, first))
      .then(retargetAt(1))
      .then(shrinkDestroyAt(second))
      .expect(and {
        not(s.objs.keys().contains(second)),
        s.objs.keys().contains(first),
        s.objs.get(first).allocatedBy == Some(CONSUMER),
      })

  // A drawn individual is never a shrink victim, even under surplus (pool-objects design D3).
  run drawnNeverShrunkTest =
    init
      .then(retargetAt(2))
      .then(growCreateAt)
      .then(growCreateAt)
      .then(plugPooledAt(first))
      .then(drawAt(CONSUMER, first))
      .then(retargetAt(1))
      .then(shrinkDestroyAt(first).fail())

  // ShrinkBelowDraw: a requirement below the drawn count surfaces a refusal, the
  // consumer intact (pool-objects design D3; formal-models req 2 "refusal, not a teardown").
  run shrinkBelowDrawRefusedTest =
    init
      .then(retargetAt(1))
      .then(growCreateAt)
      .then(plugPooledAt(first))
      .then(drawAt(CONSUMER, first))
      .then(retargetAt(0))
      .then(shrinkBelowDrawAt)
      .expect(and {
        conv.refusal,
        s.objs.get(first).allocatedBy == Some(CONSUMER),
        s.objs.keys().contains(first),
      })

  // Prune removes an undeclared free runtime object; the DPL-born survives, and
  // is itself never prunable (pool-objects design D3; roadmap #14).
  run prunePreservesDplBornTest =
    init
      .then(createPooledAt)
      .then(plugPooledAt(first))
      .expect(prunable(s, conv, first))
      .then(pruneAt(first))
      .expect(and {
        not(s.objs.keys().contains(first)),
        s.objs.keys().contains(born),
      })
      .then(pruneAt(born).fail())

  // Idempotence: a converged state enables no grow, shrink, or prune — a second
  // pass is stutter-only (formal-models req 2 "idempotent and level-triggered").
  run idempotentReconvergeTest =
    init
      .then(retargetAt(1))
      .then(growCreateAt)
      .expect(and {
        isConverged(s, conv),
        not(growEnabled(s, conv)),
        not(shrinkEnabled(s, conv)),
        not(anyPrunable(s, conv)),
      })
}
