// View 1 — containment: the DPRC tree (object-model.md §1).
// Pure helpers over CoreState; the machine (machine.qnt) wires them into
// actions.
//
// Named invariants: none here — the phase-2 encodings live in
// core/invariants.qnt and models/main.qnt under baseline ids.
module containment {
  import types.* from "./types"

  // Quint has no recursion; ancestor walks unroll to a bounded depth.
  // The reference tree is 3 deep (dprc.0 → dprc.1 → dprc.2); scratch
  // DPRCs nest below.
  pure val MAX_DEPTH = 8

  pure def parentSet(s: CoreState, o: ObjId): Set[ObjId] =
    if (s.objs.keys().contains(o))
      match s.objs.get(o).parent {
        | Some(p) => Set(p)
        | None => Set()
      }
    else Set()

  // Ancestors of o, including o itself.
  pure def lineage(s: CoreState, o: ObjId): Set[ObjId] =
    1.to(MAX_DEPTH).fold(Set(o), (acc, _) =>
      acc.union(acc.map(x => parentSet(s, x)).flatten()))

  // mc.global: the unique dprc with no parent (dprc.0).
  pure def isMcGlobal(s: CoreState, c: ObjId): bool = and {
    c.fam == Dprc,
    s.objs.keys().contains(c),
    s.objs.get(c).parent == None,
  }

  // A "root container" as the kernel bus sees it: a dprc whose parent is
  // mc.global (dprc.1 on the reference tree). Bus rescan reaches only
  // these (DPRC-I6).
  pure def isLinuxRoot(s: CoreState, c: ObjId): bool = and {
    s.objs.keys().contains(c),
    match s.objs.get(c).parent {
      | Some(p) => isMcGlobal(s, p)
      | None => false
    },
  }

  // set-locked 1 strips create/destroy/assign/lock from the entire
  // sub-hierarchy (DPRC-I11, board-pending): an operation on c is
  // refused if any dprc on c's lineage is locked.
  pure def lockedAnywhere(s: CoreState, c: ObjId): bool =
    lineage(s, c).exists(a =>
      s.dprcs.keys().contains(a) and s.dprcs.get(a).locked)

  pure def residents(s: CoreState, c: ObjId): Set[ObjId] =
    s.objs.keys().filter(o => s.objs.get(o).parent == Some(c))

  // dprc assign --child moves an object parent↔child only while
  // unplugged (DPRC-I3); allocation and driver binding never cross the
  // container boundary (DPRC-I1), so a drawn or bound object cannot move
  // either. RootOnly families are pinned (DPMAC-I1, DPDBG-I1). Moving a
  // dprc itself is not in the corpus — refused until a family doc adds
  // it.
  pure def canMove(s: CoreState, params: Family -> FamilyParams,
                   o: ObjId, dst: ObjId): bool = and {
    s.objs.keys().contains(o),
    o.fam != Dprc,
    params.get(o.fam).placement == AnyContainer,
    dst.fam == Dprc,
    s.dprcs.keys().contains(dst),
    not(s.objs.get(o).plugged),
    s.objs.get(o).bind == Unbound,
    s.objs.get(o).allocatedBy == None,
    not(lockedAnywhere(s, dst)),
    match s.objs.get(o).parent {
      | Some(p) => not(lockedAnywhere(s, p))
      | None => false
    },
  }
}
