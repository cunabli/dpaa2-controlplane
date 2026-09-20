// The P3 allocator-custody substrate (ADR-0019 pattern P3; pool-objects design
// D7 as amended 2026-09-20, ADR-0019). ONE const-parameterized machine over a
// single pooled FAMILY — the common P3 mechanics every allocator family shares,
// with no family particulars. The trio family files (dpbp/dpmcp/dpcon) each carry
// a thin `<fam>_lifecycle` module that instantiates this one contextualized to
// their family (`import pool_lifecycle(FAMILY = Dpbp, CEILING = ...).* `); dpio is
// unpooled and owns its own `dpio_lifecycle` (dpio.qnt), reusing the shared core
// transforms rather than this pooled machine. Re-deriving these mechanics in a
// family file is the review defect the amended ADR-0019 D7 names — a thin
// instantiation is the desired shape.
//
// Scope (pool-objects task 1.1 + 1.2): the custody cycle — create → free-pool
// membership → draw → return, DPBP-I3's dirty return, DPBP-I7's census-floor
// refusal. dpio's seat faces live in dpio_lifecycle. Count convergence and prune
// (pool-objects task 1.3) land in a follow-on layer here.
//
// The state is a CoreState so the core census/membership predicates apply
// unchanged (create_allocate.freePool/inPool/canCreate, lifecycle.createT/plugT/
// markAllocated) — no re-derivation of pool mechanics, and the shape stays
// isomorphic (ADR-0002 §3) to the Rust P3 surface phase 2 builds. Params come
// from families/params.qnt ALL_PARAMS (only ALL_PARAMS.get(FAMILY) is ever
// queried); hosting the records there keeps every Family constructor used outside
// the instantiating main module, the QNT404 flattener rule the verify rung needs.
//
// Named invariants (Apalache-marked via `stateInvariants`; dispositions in
// models/COVERAGE.md):
//   POOL_CENSUS (DPBP-I7 floor; ADR-0011) — population never exceeds CEILING;
//   POOL_CUSTODY (object-model.md §3; DPBP-I2) — a drawn individual stays
//     plugged and allocator-held.
// Directed runs: custodyCycleTest, dirtyReturnTest (DPBP-I3), censusRefusesAtCeilingTest
//   (DPBP-I7). DPBP-I3 is a directed run (simulate-only), the split main.qnt
//   applies to its own Breaking candidates.
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

  var s: CoreState

  // ---- census (ADR-0011 census-first; DPBP-I7 floor) ----

  pure def poolPopulation(st: CoreState): int =
    st.objs.keys().filter(o => o.fam == FAMILY and st.objs.get(o).parent == Some(poolContainer)).size()

  // A create is admitted only below the ceiling — the refusal is a disabled
  // guard, never a recorded over-ceiling state (DPBP-I7; ADR-0011).
  pure def censusAdmitsCreate(st: CoreState): bool = poolPopulation(st) < CEILING

  // ---- init ----

  pure def container(parent: Option[ObjId], hw: int): ObjState = {
    parent: parent, owner: parent, plugged: true, busVisible: true,
    bind: Unbound, enabled: false, pristine: true, linkUp: false,
    allocatedBy: None, hwId: hw,
  }

  pure val initialPool: CoreState = {
    objs: Map(dprc0 -> container(None, 0), poolContainer -> container(Some(dprc0), 1)),
    dprcs: Map(
      dprc0 -> { icid: 0, perms: Set(Spawn, Alloc, ObjCreate, IrqCfg, TopologyChanges), locked: false },
      poolContainer -> { icid: 1, perms: Set(Spawn, Alloc, ObjCreate, IrqCfg), locked: false }
    ),
    conns: Set(),
    nextNum: FAMILIES.mapBy(f => if (f == Dprc) 2 else 0),
    nextHw: 100,
  }

  action init = s' = initialPool

  // ---- custody transitions (pool-objects task 1.1) ----

  // A create — census/ceiling gated (DPBP-I7). Anonymous — companions wear no
  // name of their own (pool-objects design D2; ADR-0015).
  action createPooledAt: bool = all {
    canCreate(s, ALL_PARAMS, FAMILY, poolContainer),
    censusAdmitsCreate(s),
    s' = createT(s, ALL_PARAMS, FAMILY, poolContainer),
  }

  // Plug it: a pooled family thereby enters its container's free pool (inPool is
  // derived — plugged, allocator-held, undrawn; create_allocate.inPool).
  action plugPooledAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    o.fam == FAMILY,
    not(s.objs.get(o).plugged),
    s' = plugT(s, o),
  }

  // Draw: the allocator claims a free individual for a consumer. markAllocated
  // dirties it (pristine false) — a drawn object is programmed by its consumer.
  action drawAt(consumer: ObjId, p: ObjId): bool = all {
    p.fam == FAMILY,
    freePool(s, ALL_PARAMS, poolContainer, FAMILY).contains(p),
    s' = markAllocated(s, consumer, Set(p)),
  }

  // Return to the free pool. No MC reset on the way out (DPBP-I3, DPMCP-I3,
  // DPCON-I5, each doc's "Kernel-side behavior"): pristine keeps whatever it was
  // — the drain is the kernel's, observed not driven (pool-objects design D3).
  action returnAt(p: ObjId): bool = all {
    s.objs.keys().contains(p),
    p.fam == FAMILY,
    s.objs.get(p).allocatedBy != None,
    s' = setObj(s, p, s.objs.get(p).with("allocatedBy", None)),
  }

  action stutter = s' = s

  // ---- nondet wrappers ----

  action doCreatePooled = createPooledAt
  action doPlugPooled = { nondet o = s.objs.keys().oneOf() plugPooledAt(o) }
  action doDraw = { nondet p = s.objs.keys().oneOf() drawAt(CONSUMER, p) }
  action doReturn = { nondet p = s.objs.keys().oneOf() returnAt(p) }

  action step = any {
    doCreatePooled,
    doPlugPooled,
    doDraw,
    doReturn,
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

  val stateInvariants: bool = and {
    POOL_CENSUS,
    POOL_CUSTODY,
  }

  // ---- directed runs (generic over FAMILY; inherited by each instantiation) ----

  pure val first: ObjId = { fam: FAMILY, num: 0 }

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
  // disabled by the census — no over-ceiling state exists (CEILING creates hit it).
  run censusRefusesAtCeilingTest =
    init
      .then(createPooledAt)
      .then(createPooledAt)
      .then(createPooledAt)
      .expect(poolPopulation(s) == CEILING)
      .then(createPooledAt.fail())
}
