// View 4 — allocation pools: sizing couplings (object-model.md §4).
// The derivation direction: consumer sizing triggers the companion math,
// the pool families carry it, and the arrows bottom out at dpmcp
// (DPMCP-I1) — with the twist that dpio, itself unpooled (DPIO-I1),
// draws a dpmcp at kernel probe: the dpio→dpmcp ordering edge inside the
// pool-companion phase. Per-family draw counts live in FamilyParams.draw
// (family modules, tasks 1.2/1.3); this module gives the census
// predicates phase-2 invariants and the family sizing derivations lean
// on.
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module pools {
  import types.* from "./types"
  import create_allocate.* from "./create_allocate"

  // Whether container c can satisfy fam's kernel-probe draw census right
  // now. Exhaustion is silent (-EPROBE_DEFER retry loop; DPNI-I5): in
  // the machine an unsatisfiable draw simply leaves the bind action
  // disabled — the C1 failure class made executable.
  pure def drawSatisfiable(s: CoreState, params: Family -> FamilyParams,
                           c: ObjId, fam: Family): bool = and {
    freePool(s, params, c, Dpmcp).size() >= params.get(fam).draw.dpmcp,
    freePool(s, params, c, Dpbp).size() >= params.get(fam).draw.dpbp,
    freePool(s, params, c, Dpcon).size() >= params.get(fam).draw.dpcon,
  }
}
