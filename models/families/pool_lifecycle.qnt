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
// The scope now also carries an environment adversary (pool-objects task 3.8;
// pool-objects design D9): a non-reconciler EXTERNAL consumer draws, and the
// undeclared-cohabitant create stands in for another construct's companion. The
// two board-caught census divergences — the DPL-born drawn folding and the
// other-construct companion folding — were environment gaps (V-POOL-5/V-POOL-6),
// so the model now exercises them offline. The invariants hold under this
// interference (simulate/verify check `stateInvariants`).
//
// The scope also splits the two custody facets the census had collapsed (pool-objects
// task 3.9; pool-objects design D10): plugged (allocatable, in the kernel pool — DPBP-I2:
// allocatable ⟺ plugged ∧ allocator-bound) distinct from drawn (held by a consumer). Grow
// creates-then-plugs, so surplus is drawable; reclaim is the unplug-probe law — an unplug of
// a drawn individual is refused (the refusal is the drawn signal, never a plugged ⇒ drawn
// proxy), a plugged-free individual unplugs then destroys. The twins carry plugged
// explicitly, so a census that infers drawn from plugged fails offline first.
//
// Custody scope rides in Conv as DATA, not a module const (ADR-0020; design D10
// amendment): the Rust twin takes scope as a disposition input and structural
// isomorphism binds the type (ADR-0002 §3). The unplug-probe reclaim is CHILD-scoped
// (ADR-0020 decisions 1/3) — at root every plugged pool object is fsl_mc_allocator-bound
// with no safe drawn-ness signal, so root capacity is grow-only: surplus renders as a
// typed reboot-required residue (poolDisposition, one instance of the grow-only dpio
// residue) and prune keeps only the never-plugged reach (decision 4).
//
// The state is a CoreState so the core census/membership predicates apply
// unchanged (create_allocate.freePool/inPool/canCreate, lifecycle.createT/plugT/unplugT/
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
//   POOL_ROOT_GROW_ONLY (ADR-0020 decisions 1/3; design D10 amendment) — at root scope every
//     managed individual stays plugged, so no destroy of plugged root capacity is reachable
//     and surplus is residue, not teardown.
// Directed runs: custodyCycleTest, dirtyReturnTest (DPBP-I3), censusRefusesAtCeilingTest
//   (DPBP-I7), convergenceGrowTest, freeOnlyShrinkTest (probe-succeeds-on-plugged-free),
//   drawnNeverShrunkTest (probe-refused-on-drawn), shrinkBelowDrawRefusedTest,
//   prunePreservesDplBornTest, idempotentReconvergeTest, managedSurplusReclaimsTest and
//   teardownWalkTest (pool-objects task 3.9; the unplug-probe reclaim and consumer-before-pool
//   ordering, pool-objects design D10),
//   rootSurplusResidueTest (ADR-0020; design D10 amendment — root surplus is a refused
//   residue and prune reaches only the never-plugged foreign),
//   and the environment-adversary trio (pool-objects task 3.8) envBornDrawnNetsTest
//   (V-POOL-6), envForeignDrawnFoldsTest (pool-objects design D2), envCohabitantPrunedTest
//   (pool-objects design D9). The reclaim/refusal runs (freeOnly/drawnNever/shrinkBelowDraw/
//   managedSurplus/teardownWalk and envBornDrawnNets) anchor to the child init variant — those
//   laws are child-scoped (ADR-0020 decision 3); prune runs stay at root under the never-plugged gate.
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

  // Custody scope rides in Conv as DATA, not a module const: the Rust twin takes it as a
  // disposition input and structural isomorphism binds the type (ADR-0002 §3; design D10
  // amendment). Root (dprc.1, the Linux root) is grow-only; child scope runs the
  // unplug-probe reclaim (ADR-0020 decisions 1/3).
  type CustodyScope = RootScope | ChildScope

  // Each constructor gets a value use so the verify-rung flattener keeps them (the QNT404
  // rule; families/params.qnt ALL_PARAMS and dpio.qnt SEAT_REGIMES precedent).
  pure val CUSTODY_SCOPES: Set[CustodyScope] = Set(RootScope, ChildScope)

  // Root surplus is typed reboot-required residue (ADR-0020 decision 2; design D10
  // amendment): managed capacity beyond intent at root cannot be reclaimed, so it renders
  // observed vs required with the family named — one instance of the grow-only dpio residue
  // (dpio.qnt SeatResidue), never a destroy. Converged when the count already meets intent.
  type PoolResidue = { family: Family, observed: int, required: int }
  type PoolDisposition = Converged | RebootRequired(PoolResidue)

  // derivedReq is the compiled per-family target (ADR-0012 counts, consumed as
  // state the environment retargets); managed is the ghost set of companions the
  // reconciler grew (its own to shrink); refusal is the observable ShrinkBelowDraw
  // flag (pool-objects design D3: a requirement below draw surfaces, never a teardown);
  // scope is the custody disposition (root grow-only vs child reclaim, ADR-0020).
  type Conv = { derivedReq: int, managed: Set[ObjId], refusal: bool, scope: CustodyScope }

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

  // The root residue disposition (ADR-0020 decision 2; design D10 amendment): a managed
  // count above intent that root cannot reclaim renders reboot-required, else converged.
  // Twin `PoolFamilyDrift::residue` judges the labeled-plugged census, not `managedCount` — a recorded twin difference, not drift.
  pure def poolDisposition(cv: Conv): PoolDisposition =
    if (managedCount(cv) <= cv.derivedReq) Converged
    else RebootRequired({ family: FAMILY, observed: managedCount(cv), required: cv.derivedReq })

  // The prune target set (pool-objects design D3): a pool object undeclared (not managed),
  // not DPL-born, and free.
  pure def prunable(st: CoreState, cv: Conv, o: ObjId): bool = and {
    o.fam == FAMILY,
    st.objs.keys().contains(o),
    st.objs.get(o).parent == Some(poolContainer),
    o != born,
    not(cv.managed.contains(o)),
    st.objs.get(o).allocatedBy == None,
    // At root a plugged undeclared object is allocator-bound residue (ADR-0020 decision 4);
    // an unplugged one carries no binding and prune reaches it. Child prune is unchanged.
    cv.scope == ChildScope or not(st.objs.get(o).plugged),
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
    conv' = { derivedReq: 0, managed: Set(), refusal: false, scope: RootScope },
  }

  // Child-scoped custody (ADR-0020 decision 3): VFIO owns the residents, no allocator
  // competes, so the unplug-probe reclaim and the ShrinkBelowDraw refusal fire. init keeps
  // root's grow-only meaning; the reclaim/refusal runs anchor here.
  action initChildScoped = all {
    s' = initialPool,
    conv' = { derivedReq: 0, managed: Set(), refusal: false, scope: ChildScope },
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
  // Created-then-plugged (pool-objects design D10; DPBP-I2): the reconciler plugs what it
  // creates so the kernel can draw the capacity — surplus is never left unplugged.
  action growCreateAt: bool =
    val o = { fam: FAMILY, num: s.nextNum.get(FAMILY) }
    all {
      managedCount(conv) < conv.derivedReq,
      canCreate(s, ALL_PARAMS, FAMILY, poolContainer),
      censusAdmitsCreate(s),
      s' = plugT(createT(s, ALL_PARAMS, FAMILY, poolContainer), o),
      conv' = conv.with("managed", conv.managed.union(Set(o))),
    }

  // Surplus reclaim step 1 — the unplug probe (pool-objects design D10; DPBP-I2:
  // allocatable ⟺ plugged ∧ allocator-bound). A managed surplus individual is unplugged
  // first; the MC refuses the unplug of a drawn object (allocatedBy != None disables this
  // guard), and that refusal IS the drawn signal — read from the board, never inferred from
  // a plugged ⇒ drawn proxy. The victim is nondeterministic; no law distinguishes free ones.
  action shrinkUnplugAt(victim: ObjId): bool = all {
    // Child-scoped: root capacity is grow-only, no unplug-probe reaches it (ADR-0020 decision 3).
    conv.scope == ChildScope,
    victim.fam == FAMILY,
    conv.managed.contains(victim),
    managedCount(conv) > conv.derivedReq,
    s.objs.get(victim).allocatedBy == None,
    s.objs.get(victim).plugged,
    s' = unplugT(s, victim),
    conv' = conv,
  }

  // Surplus reclaim step 2 — destroy the probe-cleared individual (pool-objects design D10):
  // only an unplugged (thereby non-allocatable), free managed individual is destroyed and
  // leaves the ghost set. Drawn-ness was discovered by the probe, never inferred here.
  action shrinkDestroyAt(victim: ObjId): bool = all {
    // Child-scoped: the probe cleared it, and only child scope reaches the destroy (ADR-0020 decision 3).
    conv.scope == ChildScope,
    victim.fam == FAMILY,
    conv.managed.contains(victim),
    managedCount(conv) > conv.derivedReq,
    s.objs.get(victim).allocatedBy == None,
    not(s.objs.get(victim).plugged),
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
    // Child-scoped: root drawn-ness is unobservable, so the refusal face cannot fire there
    // (ADR-0020 decision 3).
    conv.scope == ChildScope,
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

  // ---- environment adversary (pool-objects task 3.8; pool-objects design D9) ----

  // A non-reconciler consumer distinct from CONSUMER: the board draws that no
  // construct the reconciler owns can account for. The reconciler is the single
  // provider (pool-objects design D9), so any other draw is the environment's.
  pure val EXTERNAL: ObjId = { fam: Dpni, num: 1 }

  // The environment draws a free individual for its own account — including the
  // DPL-born `born` (free from init), the drawn-DPL-born case V-POOL-6 nets out of
  // the draw guard. `createPooledAt` above IS the environment's undeclared-cohabitant
  // create (the labeled-other object another construct made): pool-objects task 3.7
  // folded the port chain's companions into the derivation, so this cohabitant now
  // stands only for the generic environment adversary — count-level simply foreign,
  // no separate create wrapper is needed here.
  action doEnvDraw = { nondet p = s.objs.keys().oneOf() drawAt(EXTERNAL, p) }

  // ---- nondet wrappers ----

  action doCreatePooled = createPooledAt
  action doPlugPooled = { nondet o = s.objs.keys().oneOf() plugPooledAt(o) }
  action doDraw = { nondet p = s.objs.keys().oneOf() drawAt(CONSUMER, p) }
  action doReturn = { nondet p = s.objs.keys().oneOf() returnAt(p) }
  action doGrow = growCreateAt
  action doShrinkUnplug = { nondet v = s.objs.keys().oneOf() shrinkUnplugAt(v) }
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
    doShrinkUnplug,
    doShrink,
    doPrune,
    doShrinkBelowDraw,
    doRetarget,
    doEnvDraw,
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

  // Root grow-only (ADR-0020 decisions 1/3; design D10 amendment): at root the unplug-probe
  // never fires, so managed capacity stays plugged — no destroy of plugged root capacity is
  // reachable and surplus is the typed residue, not a teardown.
  val POOL_ROOT_GROW_ONLY: bool =
    (conv.scope == RootScope) implies
      conv.managed.forall(o => s.objs.get(o).plugged)

  val stateInvariants: bool = and {
    POOL_CENSUS,
    POOL_CUSTODY,
    POOL_DPL_SURVIVES,
    POOL_MANAGED_LIVE,
    POOL_IDEMPOTENT,
    POOL_ROOT_GROW_ONLY,
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

  // Surplus → free-only shrink via the unplug probe (pool-objects design D10): the free
  // managed individual is probe-cleared then destroyed; the drawn one survives. Grow plugs,
  // so no separate plug step — the probe-succeeds-on-plugged-free admitted direction.
  run freeOnlyShrinkTest =
    initChildScoped
      .then(retargetAt(2))
      .then(growCreateAt)
      .then(growCreateAt)
      .then(drawAt(CONSUMER, first))
      .then(retargetAt(1))
      .then(shrinkUnplugAt(second))
      .then(shrinkDestroyAt(second))
      .expect(and {
        not(s.objs.keys().contains(second)),
        s.objs.keys().contains(first),
        s.objs.get(first).allocatedBy == Some(CONSUMER),
      })

  // A drawn individual is never reclaimed: the unplug probe is refused on it
  // (pool-objects design D10) — the refused guard direction, the board's in-use refusal.
  run drawnNeverShrunkTest =
    initChildScoped
      .then(retargetAt(2))
      .then(growCreateAt)
      .then(growCreateAt)
      .then(drawAt(CONSUMER, first))
      .then(retargetAt(1))
      .then(shrinkUnplugAt(first).fail())

  // ShrinkBelowDraw: a requirement below the drawn count surfaces a refusal, the
  // consumer intact (pool-objects design D3; formal-models req 2 "refusal, not a teardown").
  run shrinkBelowDrawRefusedTest =
    initChildScoped
      .then(retargetAt(1))
      .then(growCreateAt)
      .then(drawAt(CONSUMER, first))
      .then(retargetAt(0))
      .then(shrinkBelowDrawAt)
      .expect(and {
        conv.refusal,
        s.objs.get(first).allocatedBy == Some(CONSUMER),
        s.objs.keys().contains(first),
      })

  // Prune removes an undeclared never-plugged runtime object at root (ADR-0020 decision 4:
  // a never-plugged root object carries no allocator binding, so prune reaches it); the
  // DPL-born survives and is itself never prunable (pool-objects design D3; roadmap #14).
  run prunePreservesDplBornTest =
    init
      .then(createPooledAt)
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

  // Managed surplus reclaims through the probe (pool-objects design D10; formal-models req
  // "Custody carries the plug facet"): a plugged-but-undrawn surplus unplugs then destroys,
  // and the census converges — the reclaim the collapsed proxy could not execute.
  run managedSurplusReclaimsTest =
    initChildScoped
      .then(retargetAt(2))
      .then(growCreateAt)
      .then(growCreateAt)
      .then(retargetAt(1))
      .then(shrinkUnplugAt(second))
      .expect(and {
        not(s.objs.get(second).plugged),
        not(inPool(s, ALL_PARAMS, second)),
      })
      .then(shrinkDestroyAt(second))
      .expect(and {
        not(s.objs.keys().contains(second)),
        managedCount(conv) == 1,
        isConverged(s, conv),
      })

  // Teardown orders consumers before pools (pool-objects design D10; formal-models req
  // "Teardown orders consumers before pools"): the consumer returns its draw first, and only
  // then does the pool shrink probe+destroy the freed individual — the ordering law.
  run teardownWalkTest =
    initChildScoped
      .then(retargetAt(1))
      .then(growCreateAt)
      .then(drawAt(CONSUMER, first))
      .then(retargetAt(0))
      .then(returnAt(first))
      .then(shrinkUnplugAt(first))
      .then(shrinkDestroyAt(first))
      .expect(and {
        not(s.objs.keys().contains(first)),
        managedCount(conv) == 0,
        isConverged(s, conv),
      })

  // ---- environment-adversary directed runs (pool-objects task 3.8) ----

  // The DPL-born nets out of the draw guard: EXTERNAL draws `born`, so the base
  // is drawnManaged (managed drawn only, born excluded) — a requirement of 0 fires
  // no refusal (V-POOL-6; pool-objects design D3). Child-scoped: the refusal face is
  // reachable only in a child (ADR-0020 decision 3), so the netting is what keeps it off.
  run envBornDrawnNetsTest =
    initChildScoped
      .then(drawAt(EXTERNAL, born))
      .expect(and {
        s.objs.get(born).allocatedBy == Some(EXTERNAL),
        s.objs.keys().contains(born),
      })
      .then(retargetAt(0))
      .then(shrinkBelowDrawAt.fail())

  // A drawn foreign is its own census facet (`foreign_drawn`), not a managed one: the
  // managed grow lands `second`, then EXTERNAL draws the cohabitant `first`, which stays
  // drawn and is never a prune candidate (pool-objects design D2). This run stays the
  // isomorphism witness — the count surface's `foreign_drawn` facet nets out of `managed()`
  // exactly as `managedCount` does here, so the two agree with no fold (bead
  // dpaa2-controlplane-960.21).
  run envForeignDrawnFoldsTest =
    init
      .then(createPooledAt)
      .then(plugPooledAt(first))
      .then(retargetAt(1))
      .then(growCreateAt)
      .then(drawAt(EXTERNAL, first))
      .expect(and {
        managedCount(conv) == 1,
        s.objs.get(first).allocatedBy == Some(EXTERNAL),
        not(anyPrunable(s, conv)),
      })

  // The count surface grows atop a drawn foreign (bead dpaa2-controlplane-960.21): EXTERNAL
  // draws the cohabitant `first` BEFORE the managed grow, and the grow still fires `second`.
  // The drawn-foreign facet (`foreign_drawn`) no longer masks the deficit at count level, so
  // draw-then-grow replays through the Rust count surface where the old fold could not
  // (pool-objects design D2).
  run envForeignDrawnGrowsTest =
    init
      .then(createPooledAt)
      .then(plugPooledAt(first))
      .then(drawAt(EXTERNAL, first))
      .then(retargetAt(1))
      .then(growCreateAt)
      .expect(and {
        managedCount(conv) == 1,
        s.objs.get(first).allocatedBy == Some(EXTERNAL),
        conv.managed.contains(second),
      })

  // Convergence under interference (pool-objects design D9; ADR-0020 decision 4): the
  // adversary's never-plugged cohabitant `first` carries no allocator binding and is
  // prune-reachable at root, while the managed `second` and the DPL-born `born` stand —
  // the state converges.
  run envCohabitantPrunedTest =
    init
      .then(createPooledAt)
      .then(retargetAt(1))
      .then(growCreateAt)
      .expect(anyPrunable(s, conv))
      .then(pruneAt(first))
      .expect(and {
        isConverged(s, conv),
        not(s.objs.keys().contains(first)),
        s.objs.keys().contains(born),
        s.objs.keys().contains(second),
      })

  // Root surplus is grow-only residue (ADR-0020; design D10 amendment): a managed surplus at
  // root (plugged, undrawn) reports observed vs required through poolDisposition and no step
  // converges the count — grow is off, prune cannot touch managed, and the unplug probe is
  // refused (decision 3). Prune reaches only the never-plugged foreign (decision 4); the
  // plugged foreign is residue, not a prune candidate. The `.fail()` is last (README rung note).
  run rootSurplusResidueTest =
    init
      .then(retargetAt(1))
      .then(growCreateAt)
      .then(retargetAt(0))
      .expect(and {
        poolDisposition(conv) == RebootRequired({ family: FAMILY, observed: 1, required: 0 }),
        not(isConverged(s, conv)),
        not(growEnabled(s, conv)),
        not(anyPrunable(s, conv)),
      })
      .then(createPooledAt)
      .expect(prunable(s, conv, second))
      .then(plugPooledAt(second))
      .expect(not(prunable(s, conv, second)))
      .then(shrinkUnplugAt(first).fail())
}
