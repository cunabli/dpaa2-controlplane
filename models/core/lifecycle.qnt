// View 5 — lifecycle ordering (object-model.md §5).
// Pure CoreState -> CoreState transforms for the canonical forward order
// (pool companions → create → pre-plug mutation → plug/bind → connect →
// rescan → enable) and the weaker-guaranteed teardown mirror. Guards
// live beside the actions in machine.qnt.
//
// Named invariants: none here — the phase-2 encodings live in
// core/invariants.qnt and models/main.qnt under baseline ids.
module lifecycle {
  import types.* from "./types"
  import containment.* from "./containment"
  import create_allocate.* from "./create_allocate"

  pure def setObj(s: CoreState, o: ObjId, st: ObjState): CoreState =
    s.with("objs", s.objs.put(o, st))

  pure def dropKey(m: a -> b, k: a): a -> b =
    m.keys().exclude(Set(k)).mapBy(x => m.get(x))

  pure def freshId(s: CoreState, fam: Family): ObjId =
    { fam: fam, num: s.nextNum.get(fam) }

  pure def bump(s: CoreState, fam: Family): CoreState =
    s.with("nextNum", s.nextNum.set(fam, s.nextNum.get(fam) + 1))
     .with("nextHw", s.nextHw + 1)

  // §5 step 6 rescan: sync is not visibility (DPRC-I6). A rescan reaches
  // root containers only and discards errors; child containers refresh
  // only via their own IRQ path.
  pure def rescanContainerT(s: CoreState, c: ObjId): CoreState =
    s.with("objs", s.objs.keys().mapBy(o =>
      if (s.objs.get(o).parent == Some(c))
        s.objs.get(o).with("busVisible", true)
      else s.objs.get(o)))

  pure def rescanAllRootsT(s: CoreState): CoreState =
    s.dprcs.keys().filter(c => isLinuxRoot(s, c))
      .fold(s, (acc, c) => rescanContainerT(acc, c))

  // §5 step 2 create. Object id from the per-family restool id space,
  // hwId (and icid for containers) from the MC-assigned space — two
  // counters, two id spaces (§6.4). dprc create triggers a bus rescan;
  // most creates do not (DPCI-I3).
  pure def createT(s: CoreState, params: Family -> FamilyParams,
                   fam: Family, c: ObjId): CoreState = {
    val o = freshId(s, fam)
    val s1 = bump(setObj(s, o, newObj(c, s.nextHw)), fam)
    val s2 =
      if (fam == Dprc)
        // Child-container identity is pool-assigned at create; the perm
        // mask is the restool default (DPRC-I4).
        s1.with("dprcs", s1.dprcs.put(o,
          { icid: s.nextHw, perms: Set(Spawn, Alloc, ObjCreate, IrqCfg),
            locked: false }))
      else s1
    if (params.get(fam).createTriggersRescan) rescanAllRootsT(s2) else s2
  }

  // §5 step 4 plug: pooled families thereby enter their pool (inPool is
  // derived, not stored).
  pure def plugT(s: CoreState, o: ObjId): CoreState =
    setObj(s, o, s.objs.get(o).with("plugged", true))

  // Drawn pool objects are programmed by their consumer — membership
  // never implies clean state (DPBP-I3, DPMCP-I3), so a draw dirties.
  pure def markAllocated(s: CoreState, consumer: ObjId,
                         drawn: Set[ObjId]): CoreState =
    drawn.fold(s, (acc, p) => setObj(acc, p,
      acc.objs.get(p).with("allocatedBy", Some(consumer))
                     .with("pristine", false)))

  // Kernel bind (triggered by plug, DPRC-I2), drawing the census set
  // from the consumer's own container. Binding resets the object for the
  // reset-on-bind class: only the immutable cfg block survives into a
  // bound consumer (DPNI-I2, DPDMUX-I3, DPSW-I4) — which is what makes
  // the pre-plug mutation window futile against a kernel consumer.
  pure def kernelBindT(s: CoreState, params: Family -> FamilyParams,
                       o: ObjId, drawn: Set[ObjId]): CoreState = {
    val st = s.objs.get(o).with("bind", BoundKernel)
    val st1 =
      if (params.get(o.fam).resetOnBind)
        st.with("pristine", true).with("enabled", false)
      else st
    markAllocated(setObj(s, o, st1), o, drawn)
  }

  // VFIO bind: the DPDK/VPP regime draws zero from kernel pools — the
  // container's objects are claimed directly (§3).
  pure def vfioBindT(s: CoreState, o: ObjId): CoreState =
    setObj(s, o, s.objs.get(o).with("bind", BoundVfio))

  pure def releaseAllocations(s: CoreState, consumer: ObjId): CoreState =
    s.with("objs", s.objs.keys().mapBy(o => {
      val st = s.objs.get(o)
      // Pool free does no MC reset: the object returns dirty
      // (DPBP-I3, DPMCP-I3) — pristine stays as it was.
      if (st.allocatedBy == Some(consumer)) st.with("allocatedBy", None)
      else st
    }))

  // Teardown step 1 (§5): unbind before destroy. Unbind-side resets are
  // best-effort or version-gated (DPSECI-I5) — the transform leaves
  // pristine unchanged: the weaker guarantee, modeled as no guarantee.
  pure def unbindT(s: CoreState, o: ObjId): CoreState = {
    val s1 = releaseAllocations(s, o)
    setObj(s1, o, s1.objs.get(o).with("bind", Unbound)
                               .with("enabled", false))
  }

  // assign --plugged=0 is the kernel's unbind trigger (DPRC-I2 mirror).
  pure def unplugT(s: CoreState, o: ObjId): CoreState = {
    val s1 = unbindT(s, o)
    setObj(s1, o, s1.objs.get(o).with("plugged", false))
  }

  // §1 membership move: legal only while unplugged/unbound/undrawn
  // (guards in containment.canMove). Crossing the boundary changes the
  // icid domain; bus visibility must be re-established in the
  // destination's own way, so it drops here.
  pure def assignT(s: CoreState, o: ObjId, dst: ObjId): CoreState =
    setObj(s, o, s.objs.get(o).with("parent", Some(dst))
                             .with("busVisible", false))

  pure def connectT(s: CoreState, a: Endpoint, b: Endpoint): CoreState =
    s.with("conns", s.conns.union(Set((a, b))))

  pure def disconnectT(s: CoreState, e: Endpoint): CoreState =
    s.with("conns", s.conns.filter(c => c._1 != e and c._2 != e))

  // Teardown step 2 (§5): destroy. Whether the MC demands disconnect
  // first is open (dpci.md unknown #2) — the transform removes the
  // object's edges with it; the unknown is ledgered in phase 2. Destroy
  // exit codes are untrusted corpus-wide (DPMAC-I8) — an
  // observation-layer law for the harness, not a state transition.
  //
  // Destroying a dprc splits its residents by ownership (ADR-0007;
  // V-DPRC-1 both sittings): residents the container *created* are
  // released with it (board: owned unplugged dpni gone in one command),
  // while foreign residents survive and are evicted one hop up into the
  // container's own parent (board: a moved-in dpni reappeared there,
  // unplugged). Eviction is a move, so bus visibility drops as in
  // assignT. The guard in machine.qnt admits only the observed resident
  // shape — unplugged, unbound, undrawn, non-dprc — so no allocations to
  // release beyond o's own.
  pure def destroyT(s: CoreState, o: ObjId): CoreState = {
    val s1 = releaseAllocations(s, o)
    val up = s1.objs.get(o).parent
    val gone = residents(s1, o)
      .filter(r => s1.objs.get(r).owner == Some(o))
      .union(Set(o))
    val evicted = residents(s1, o).exclude(gone)
    val s2 = evicted.fold(s1, (acc, r) =>
      setObj(acc, r, acc.objs.get(r).with("parent", up)
                                    .with("busVisible", false)))
    s2.with("objs", s2.objs.keys().exclude(gone).mapBy(x => s2.objs.get(x)))
      .with("dprcs", s2.dprcs.keys().exclude(gone).mapBy(x => s2.dprcs.get(x)))
      .with("conns", s2.conns.filter(c =>
        not(gone.contains(c._1.obj)) and not(gone.contains(c._2.obj))))
  }
}
