// The one generic object machine (design D2): the five views composed
// into guarded actions over the shared state. Family modules (tasks
// 1.2/1.3) provide PARAMS; models/main.qnt instantiates and runs it.
//
// Each transition comes in two forms sharing one set of guards: a
// parameterized `<name>At(args)` action (used by directed runs and,
// later, the ITF replayer) and a nondet wrapper for random simulation.
//
// Every transition carries its evidence status in EVIDENCE (design D2):
// BoardExercised means prior work drove this transition class on the
// board; Read means it stands on source/restool/manual reading and
// awaits phase 5. A failed board step against a Read transition means
// "claim was never verified", not "model wrong".
//
// Named invariants: none here — the phase-2 encodings live in
// core/invariants.qnt and models/main.qnt under baseline ids.
// TLA+ escalation: none.
module machine {
  import types.* from "./types"
  import containment.* from "./containment"
  import connect.* from "./connect"
  import create_allocate.* from "./create_allocate"
  import lifecycle.* from "./lifecycle"

  // Per-family parameters (design D2); values come from the family
  // modules when they instantiate this machine.
  const PARAMS: Family -> FamilyParams

  // dpmacs are DPC-born in the root and stay there (DPMAC-I1): their
  // runtime lifecycle is connect/disconnect, not create/move. Must be
  // non-empty — endpoint-picking actions assume at least one endpoint
  // object exists (true of the reference environment).
  const BOOT_DPMACS: Set[int]

  var s: CoreState

  pure val dprc0: ObjId = { fam: Dprc, num: 0 }
  pure val dprc1: ObjId = { fam: Dprc, num: 1 }
  pure val dprtc0: ObjId = { fam: Dprtc, num: 0 }

  // Boot-born objects are on the bus already; hwId reuses the object
  // number (unique within a family — the only uniqueness the second id
  // space promises, §6.4). Runtime creates draw from nextHw = 100 up.
  pure def bootObj(parent: Option[ObjId], hw: int): ObjState = {
    parent: parent, owner: parent,
    plugged: true, busVisible: true, bind: Unbound,
    enabled: false, pristine: true, linkUp: false, allocatedBy: None,
    hwId: hw,
  }

  pure val bootIds: Set[ObjId] =
    Set(dprc0, dprc1, dprtc0)
      .union(BOOT_DPMACS.map(n => { fam: Dpmac, num: n }))

  pure def startNum(f: Family): int =
    if (f == Dprc) 2 else if (f == Dprtc) 1 else 0

  pure val rootPerms: Set[Perm] =
    Set(Spawn, Alloc, ObjCreate, IrqCfg, TopologyChanges)

  // Reference-environment init (dprc.md [verified]): dprc.0 (mc.global)
  // → dprc.1 (the root container Linux sees) holding the DPC-born
  // dpmacs and the DPL-born dprtc.0. The root has the default mask plus
  // TOPOLOGY_CHANGES (DPRC-I4).
  pure val initialState: CoreState = {
    objs: bootIds.mapBy(o =>
      if (o == dprc0) bootObj(None, o.num)
      else if (o == dprc1) bootObj(Some(dprc0), o.num)
      else bootObj(Some(dprc1), o.num)),
    dprcs: Map(
      dprc0 -> { icid: 0, perms: rootPerms, locked: false },
      dprc1 -> { icid: 1, perms: rootPerms, locked: false }
    ),
    conns: Set(),
    nextNum: FAMILIES.mapBy(f => startNum(f)),
    nextHw: 100,
  }

  action init = s' = initialState

  // Highest endpointPorts across the family modules (dpsw/dpdmux at
  // model scope). Apalache demands constant integer ranges, so the port
  // range is constant-then-filtered rather than sized per family.
  pure val MAX_ENDPOINT_PORTS = 2

  // Connect endpoints existing right now, one per port (§2 type.id.port).
  def candidateEndpoints: Set[Endpoint] =
    s.objs.keys().map(o =>
      0.to(MAX_ENDPOINT_PORTS - 1)
        .filter(p => p < PARAMS.get(o.fam).endpointPorts)
        .map(p => { obj: o, port: p })
    ).flatten()

  // --- forward order (§5) ---

  // Child containers are created under SPAWN (§1); identity (id, icid,
  // portal) is pool-assigned — restool cannot pin them, only a DPL can.
  action createContainerAt(parent: ObjId): bool = all {
    s.dprcs.keys().contains(parent),
    s.dprcs.get(parent).perms.contains(Spawn),
    not(lockedAnywhere(s, parent)),
    s' = createT(s, PARAMS, Dprc, parent),
  }

  action createContainer = {
    nondet parent = s.dprcs.keys().oneOf()
    createContainerAt(parent)
  }

  action createObjectAt(fam: Family, c: ObjId): bool = all {
    fam != Dprc,
    canCreate(s, PARAMS, fam, c),
    s' = createT(s, PARAMS, fam, c),
  }

  action createObject = {
    nondet fam = FAMILIES.exclude(Set(Dprc)).oneOf()
    nondet c = s.dprcs.keys().oneOf()
    createObjectAt(fam, c)
  }

  // §5 step 3, the pre-plug mutation window (restool-expressible: dpni
  // update --mac-addr). Futile against a reset-on-bind consumer —
  // kernelBind restores pristine, exhibiting DPNI-I2.
  action preplugMutateAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    o.fam != Dprc,
    not(s.objs.get(o).plugged),
    s' = setObj(s, o, s.objs.get(o).with("pristine", false)),
  }

  action preplugMutate = {
    nondet o = s.objs.keys().oneOf()
    preplugMutateAt(o)
  }

  action assignChildAt(o: ObjId, dst: ObjId): bool = all {
    canMove(s, PARAMS, o, dst),
    s' = assignT(s, o, dst),
  }

  action assignChild = {
    nondet o = s.objs.keys().oneOf()
    nondet dst = s.dprcs.keys().oneOf()
    assignChildAt(o, dst)
  }

  // plug is an assign-class operation, so the lock strip gates it too.
  action plugAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    not(s.objs.get(o).plugged),
    match s.objs.get(o).parent {
      | Some(p) => not(lockedAnywhere(s, p))
      | None => false
    },
    s' = plugT(s, o),
  }

  action plug = {
    nondet o = s.objs.keys().oneOf()
    plugAt(o)
  }

  action unplugAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    s.objs.get(o).plugged,
    // Unplugging a drawn pool object out from under its consumer is
    // not in the corpus — refused until a family doc settles it.
    s.objs.get(o).allocatedBy == None,
    match s.objs.get(o).parent {
      | Some(p) => not(lockedAnywhere(s, p))
      | None => false
    },
    s' = unplugT(s, o),
  }

  action unplug = {
    nondet o = s.objs.keys().oneOf()
    unplugAt(o)
  }

  // Kernel probe: bind + census draw from the consumer's own container
  // (DPRC-I1). An unsatisfiable draw leaves this action disabled — the
  // silent -EPROBE_DEFER loop (DPNI-I5) as refusal. Pooled families are
  // held by the allocator, not consumer drivers.
  action kernelBindAt(o: ObjId, mcps: Set[ObjId], bps: Set[ObjId],
                      cons: Set[ObjId]): bool = all {
    s.objs.keys().contains(o),
    s.objs.get(o).plugged,
    s.objs.get(o).busVisible,
    s.objs.get(o).bind == Unbound,
    PARAMS.get(o.fam).hasKernelDriver,
    not(PARAMS.get(o.fam).pooled),
    match s.objs.get(o).parent {
      | Some(c) => and {
          mcps.subseteq(freePool(s, PARAMS, c, Dpmcp)),
          bps.subseteq(freePool(s, PARAMS, c, Dpbp)),
          cons.subseteq(freePool(s, PARAMS, c, Dpcon)),
        }
      | None => false
    },
    mcps.size() == PARAMS.get(o.fam).draw.dpmcp,
    bps.size() == PARAMS.get(o.fam).draw.dpbp,
    cons.size() == PARAMS.get(o.fam).draw.dpcon,
    s' = kernelBindT(s, PARAMS, o, mcps.union(bps).union(cons)),
  }

  // Draw sets are built from single picks, not powersets: every family
  // census draws at most 1 per pool family (FamilyParams.draw), and
  // powerset nondet is what made Apalache intractable. Extend to
  // multi-picks only when a family module actually draws more.
  action kernelBind = {
    nondet o = s.objs.keys().oneOf()
    nondet mcp = s.objs.keys().oneOf()
    nondet bp = s.objs.keys().oneOf()
    nondet con = s.objs.keys().oneOf()
    kernelBindAt(o,
      if (PARAMS.get(o.fam).draw.dpmcp == 0) Set() else Set(mcp),
      if (PARAMS.get(o.fam).draw.dpbp == 0) Set() else Set(bp),
      if (PARAMS.get(o.fam).draw.dpcon == 0) Set() else Set(con))
  }

  // VFIO claim: any family in the container, pooled ones included —
  // the DPDK regime draws zero from kernel pools (§3).
  action vfioBindAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    o.fam != Dprc,
    s.objs.get(o).plugged,
    s.objs.get(o).busVisible,
    s.objs.get(o).bind == Unbound,
    s.objs.get(o).allocatedBy == None,
    s' = vfioBindT(s, o),
  }

  action vfioBind = {
    nondet o = s.objs.keys().oneOf()
    vfioBindAt(o)
  }

  action unbindAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    s.objs.get(o).bind != Unbound,
    s' = unbindT(s, o),
  }

  action unbind = {
    nondet o = s.objs.keys().oneOf()
    unbindAt(o)
  }

  action connectEdgeAt(a: Endpoint, b: Endpoint): bool = all {
    canConnect(s, PARAMS, a, b),
    s' = connectT(s, a, b),
  }

  action connectEdge = {
    nondet a = candidateEndpoints.oneOf()
    nondet b = candidateEndpoints.oneOf()
    connectEdgeAt(a, b)
  }

  action disconnectEdgeAt(e: Endpoint): bool = all {
    connected(s, e),
    s' = disconnectT(s, e),
  }

  action disconnectEdge = {
    nondet e = candidateEndpoints.oneOf()
    disconnectEdgeAt(e)
  }

  // Bus rescan reaches root containers only and discards errors
  // (DPRC-I6, breaking): sync is not visibility.
  action rescanAt(c: ObjId): bool = all {
    isLinuxRoot(s, c),
    s' = rescanContainerT(s, c),
  }

  action rescan = {
    nondet c = s.dprcs.keys().oneOf()
    rescanAt(c)
  }

  // Child containers refresh only via their own IRQ path when
  // autorescan is on (dprc.md unknown #12) — modeled as available; the
  // unknown is ledgered in phase 2.
  action childIrqRefreshAt(c: ObjId): bool = all {
    s.dprcs.keys().contains(c),
    not(isLinuxRoot(s, c)),
    not(isMcGlobal(s, c)),
    s' = rescanContainerT(s, c),
  }

  action childIrqRefresh = {
    nondet c = s.dprcs.keys().oneOf()
    childIrqRefreshAt(c)
  }

  // Post-probe pool draws (§3 census, last row): e.g. concurrent
  // /dev/dprc.N openers each hold a dpmcp, returned on close.
  action allocateAt(consumer: ObjId, p: ObjId): bool = all {
    s.objs.keys().contains(consumer),
    s.objs.get(consumer).bind == BoundKernel,
    match s.objs.get(consumer).parent {
      | Some(c) => freePool(s, PARAMS, c, p.fam).contains(p)
      | None => false
    },
    s' = markAllocated(s, consumer, Set(p)),
  }

  action allocate = {
    nondet consumer = s.objs.keys().oneOf()
    nondet p = s.objs.keys().oneOf()
    allocateAt(consumer, p)
  }

  action freeAt(p: ObjId): bool = all {
    s.objs.keys().contains(p),
    s.objs.get(p).allocatedBy != None,
    // No MC reset on free — the object returns to the pool dirty
    // (DPBP-I3, DPMCP-I3): pristine keeps whatever it was.
    s' = setObj(s, p, s.objs.get(p).with("allocatedBy", None)),
  }

  action free = {
    nondet p = s.objs.keys().oneOf()
    freeAt(p)
  }

  // §5 step 7: enable is always consumer-side — restool enables nothing
  // in any family; a restool-only object is created-but-never-enabled.
  action enableAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    s.objs.get(o).bind != Unbound,
    not(s.objs.get(o).enabled),
    s' = setObj(s, o, s.objs.get(o).with("enabled", true)
                                   .with("pristine", false)),
  }

  action enable = {
    nondet o = s.objs.keys().oneOf()
    enableAt(o)
  }

  action disableAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    s.objs.get(o).enabled,
    s' = setObj(s, o, s.objs.get(o).with("enabled", false)),
  }

  action disable = {
    nondet o = s.objs.keys().oneOf()
    disableAt(o)
  }

  // set-locked strips ops from the subtree below; toggling c itself is
  // done from above, so only ancestors strictly above c gate it
  // (DPRC-I11, board-pending).
  action setLockedAt(c: ObjId, v: bool): bool = all {
    s.dprcs.keys().contains(c),
    not(isMcGlobal(s, c)),
    match s.objs.get(c).parent {
      | Some(p) => not(lockedAnywhere(s, p))
      | None => false
    },
    s' = s.with("dprcs", s.dprcs.set(c, s.dprcs.get(c).with("locked", v))),
  }

  action setLocked = {
    nondet c = s.dprcs.keys().oneOf()
    nondet v = Set(true, false).oneOf()
    setLockedAt(c, v)
  }

  // Environment: PHY reality flows up, peer requests flow down — link
  // state is a different variable from connection state (DPMAC-I4/I5).
  action linkChangeAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    PARAMS.get(o.fam).endpointPorts > 0,
    s' = setObj(s, o, s.objs.get(o).with("linkUp",
                        not(s.objs.get(o).linkUp))),
  }

  action linkChange = {
    nondet o = s.objs.keys().oneOf()
    linkChangeAt(o)
  }

  // Teardown destroy (§5). restool refuses driver-bound objects, but
  // the in_use check reads root-container sysfs only — child-container
  // and VFIO-bound objects pass it (dpdmux.md); MC-side destroy under a
  // bound driver is the unguarded race dpni.md records. The model
  // refuses only what restool actually refuses. Connected endpoints are
  // not checked (dpci.md unknown #2): edges vanish with the object.
  action destroyAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    not(isMcGlobal(s, o)),
    match s.objs.get(o).parent {
      | Some(p) => and {
          not(lockedAnywhere(s, p)),
          not(isLinuxRoot(s, p) and or {
            s.objs.get(o).bind == BoundKernel,
            s.objs.get(o).allocatedBy != None,
          }),
        }
      | None => false
    },
    // Destroy authority is creator-bound (ADR-0007 §2): the manual's
    // *_DESTROY sections demand the token of the container that created
    // the object, and the board refused `dpni destroy` on an object
    // moved away from its (already destroyed) creator — with restool
    // exiting 0 over the MC "No privilege" error, so only read-back saw
    // it (DPMAC-I8). Repatriating restores authority.
    s.objs.get(o).owner == s.objs.get(o).parent,
    // A container destroy handles residents by ownership (V-DPRC-1 both
    // sittings): residents the container created are released with it;
    // foreign residents are evicted one hop up, not destroyed. Only the
    // observed resident shape is admitted either way — unplugged,
    // unbound, undrawn, not itself a dprc. NXP's own teardown tooling
    // empties containers first, so wider shapes stay refused until a
    // probe proves them (ADR-0007).
    if (o.fam == Dprc)
      residents(s, o).forall(r => and {
        r.fam != Dprc,
        not(s.objs.get(r).plugged),
        s.objs.get(r).bind == Unbound,
        s.objs.get(r).allocatedBy == None,
      })
    else true,
    s' = destroyT(s, o),
  }

  action destroy = {
    nondet o = s.objs.keys().oneOf()
    destroyAt(o)
  }

  action step = any {
    createContainer, createObject, preplugMutate, assignChild, plug,
    unplug, kernelBind, vfioBind, unbind, connectEdge, disconnectEdge,
    rescan, childIrqRefresh, allocate, free, enable, disable, setLocked,
    linkChange, destroy,
  }

  // Per-transition evidence (design D2). Anchors are the object-model.md
  // claims each transition renders; [verified] means board-exercised by
  // prior work.
  pure val EVIDENCE: str -> Evidence = Map(
    "createContainer" -> BoardExercised, // dprc.2 provisioning (dprc.md [verified])
    "createObject" -> BoardExercised,    // dpni case C1 [verified]
    "preplugMutate" -> Read,             // update --mac-addr futility (DPNI-I2 [read])
    "assignChild" -> BoardExercised,     // parent↔child moves in C1 (DPRC-I3)
    "plug" -> BoardExercised,            // bind trigger (DPRC-I2 [verified])
    "unplug" -> BoardExercised,          // C1 teardown [verified]
    "kernelBind" -> BoardExercised,      // fsl_dpaa2_eth probe (C1 [verified])
    "vfioBind" -> BoardExercised,        // VPP child container [verified]
    "unbind" -> Read,                    // unbind-side resets best-effort/version-gated
    "connectEdge" -> BoardExercised,     // dpni↔dpmac, cross-container dpni↔dpni (DPNI-I3/I9)
    "disconnectEdge" -> BoardExercised,  // dpmac runtime lifecycle (DPMAC-I1)
    "rescan" -> BoardExercised,          // root-only reach (DPRC-I6)
    "childIrqRefresh" -> Read,           // autorescan IRQ path (dprc.md unknown #12)
    "allocate" -> Read,                  // pool claim paths from kernel source
    "free" -> Read,                      // dirty return (DPBP-I3, DPMCP-I3)
    "enable" -> BoardExercised,          // consumer enable via kernel/VPP [verified]
    "disable" -> Read,
    "setLocked" -> Read,                 // DPRC-I11 board-pending
    "linkChange" -> BoardExercised,      // link work on flagged ports (DPMAC-I4/I5)
    "destroy" -> BoardExercised          // C1 teardown [verified]
  )
}
