// View 3 — create vs allocate (object-model.md §3).
// Two disjoint ways an object reaches a consumer, never conflated:
// creation writes the objs map; allocation only flips allocatedBy on an
// existing pooled object.
//
// Named invariants: none here — the phase-2 encodings live in
// core/invariants.qnt and models/main.qnt under baseline ids.
module create_allocate {
  import types.* from "./types"
  import containment.* from "./containment"

  // Create gates (§5 "create-time gates orthogonal to this order"):
  // permission, placement, platform, cardinality. A permission bit is
  // not the last gate (DPAIOP-I2): creatable=false refuses regardless of
  // perms. ObjCreate as the gating bit is the working assumption of the
  // open permission matrix (dprc.md unknown #3).
  pure def canCreate(s: CoreState, params: Family -> FamilyParams,
                     fam: Family, c: ObjId): bool = and {
    c.fam == Dprc,
    s.dprcs.keys().contains(c),
    s.dprcs.get(c).perms.contains(ObjCreate),
    not(lockedAnywhere(s, c)),
    params.get(fam).creatable,
    if (params.get(fam).placement == RootOnly) isLinuxRoot(s, c) else true,
    if (params.get(fam).singleton)
      s.objs.keys().forall(o => o.fam != fam)
    else true,
  }

  // Post-create defaults: not plugged, not bus-visible (sync is not
  // visibility, DPRC-I6), unbound, disabled, pristine.
  pure def newObj(parent: ObjId, hw: int): ObjState = {
    parent: Some(parent),
    plugged: false,
    busVisible: false,
    bind: Unbound,
    enabled: false,
    pristine: true,
    linkUp: false,
    allocatedBy: None,
    hwId: hw,
  }

  // Kernel pool membership (§3): pooled family + plugged + held by the
  // kernel allocator (not VFIO-owned) + not currently drawn.
  pure def inPool(s: CoreState, params: Family -> FamilyParams,
                  o: ObjId): bool = and {
    params.get(o.fam).pooled,
    s.objs.get(o).plugged,
    s.objs.get(o).bind == Unbound,
    s.objs.get(o).allocatedBy == None,
  }

  // Free pool objects of family fam in container c. Allocation never
  // crosses the container boundary (DPRC-I1).
  pure def freePool(s: CoreState, params: Family -> FamilyParams,
                    c: ObjId, fam: Family): Set[ObjId] =
    s.objs.keys().filter(o => and {
      o.fam == fam,
      s.objs.get(o).parent == Some(c),
      inPool(s, params, o),
    })
}
