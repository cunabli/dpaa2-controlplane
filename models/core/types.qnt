// Shared state of the core object machine (design D4: "the shared state").
// Executable rendering of docs/baseline/object-model.md §§1–5; each field
// cites the view or law it carries. Pure types only — the machine that
// animates them is machine.qnt.
//
// Named invariants: none here — the phase-2 encodings live in
// core/invariants.qnt and models/main.qnt under baseline ids.

module types {
  // The 16 MC object families of the corpus (object-model.md §3 table).
  type Family =
    | Dprc | Dpni | Dpmac | Dpbp | Dpio | Dpcon | Dpmcp | Dpseci
    | Dpsw | Dpdmux
    | Dpaiop | Dpci | Dpdcei | Dpdmai | Dprtc | Dpdbg

  pure val FAMILIES: Set[Family] = Set(
    Dprc, Dpni, Dpmac, Dpbp, Dpio, Dpcon, Dpmcp, Dpseci,
    Dpsw, Dpdmux, Dpaiop, Dpci, Dpdcei, Dpdmai, Dprtc, Dpdbg
  )

  type Option[a] = Some(a) | None

  // restool object id space (type.id) — distinct from the MC hardware-id
  // space (bpid, qbman_ch_id, …): two id spaces, law §6.4.
  type ObjId = { fam: Family, num: int }

  // Multi-port objects address connect endpoints as type.id.port (§2).
  type Endpoint = { obj: ObjId, port: int }

  // Driver binding is one of the three visibility predicates (§1, §6.3)
  // and is regime-typed: kernel binding draws from pools, VFIO draws
  // nothing (§3).
  type BindState = Unbound | BoundKernel | BoundVfio

  // DPRC permission bits (§1). Which bit gates which operation is an open
  // matrix (dprc.md unknown #3); the machine uses ObjCreate for object
  // create/destroy and Spawn for child containers, commented at use.
  type Perm = Spawn | Alloc | ObjCreate | IrqCfg | TopologyChanges

  type ObjState = {
    // §1 containment: every object lives in exactly one DPRC at a time;
    // None only for the MC's own container (dprc.0 / mc.global).
    parent: Option[ObjId],
    // §5 step 4: plug is the kernel's bind trigger (DPRC-I2) and the
    // mobility gate (DPRC-I3).
    plugged: bool,
    // §1/§6.3 tri-state visibility: MC existence is presence in the objs
    // map; bus visibility and driver binding are these two fields —
    // advanced by different actions, never inferred from each other.
    busVisible: bool,
    bind: BindState,
    // §5 step 7: enable is always consumer-side; restool enables nothing.
    enabled: bool,
    // true iff mutable state equals post-create defaults. Bind-side reset
    // (DPNI-I2 class) restores it; pool free never does (DPBP-I3,
    // DPMCP-I3): membership never implies clean state.
    pristine: bool,
    // §2 connection ≠ link: PHY/consumer link state is distinct from the
    // dprc connection edge (DPMAC-I5).
    linkUp: bool,
    // §3 allocation edge: Some(consumer) while drawn from the pool.
    allocatedBy: Option[ObjId],
    // MC-assigned hardware id — the second id space (§6.4). Unique
    // within a family, not across families.
    hwId: int,
  }

  // dprc-only state (§1): identity and permissions fixed at create.
  type ContainerState = {
    icid: int,        // IOMMU isolation context — VFIO groups by container
    perms: Set[Perm],
    locked: bool,     // set-locked strips ops from the sub-hierarchy (DPRC-I11)
  }

  // §5 create-time placement gates orthogonal to permissions. RootOnly
  // also pins the object there — it is never assign-movable (dpdbg
  // DPDBG-I1; dpmac DPMAC-I1).
  type Placement = AnyContainer | RootOnly

  // Kernel-probe pool draw census row (§3): what a binding consumer pulls
  // from its own container's pools. dpio borrows are not draws.
  type Draw = { dpmcp: int, dpbp: int, dpcon: int }

  // The per-family parameters that instantiate the one generic machine
  // (design D2). Family modules (tasks 1.2/1.3) provide the values.
  type FamilyParams = {
    // runtime-creatable here: dpaiop is platform-refused (DPAIOP-I1),
    // dpmac is DPC-born at boot (DPMAC-I1)
    creatable: bool,
    placement: Placement,
    // firmware-enforced system-wide cardinality 1 (dprtc, DPRTC-I1)
    singleton: bool,
    // enters its kernel pool on plug — exactly dpbp/dpcon/dpmcp;
    // dpio is NOT pooled (DPIO-I1)
    pooled: bool,
    // bind resets mutable state to defaults; only the immutable cfg
    // block survives (DPNI-I2, DPDMUX-I3, DPSW-I4)
    resetOnBind: bool,
    // a kernel driver binds on plug (dpci/dpdbg/dpdcei have none)
    hasKernelDriver: bool,
    // number of connect endpoint ports; 0 = no endpoint surface (§2)
    endpointPorts: int,
    // dprc create triggers a bus rescan; most creates do not (DPCI-I3)
    createTriggersRescan: bool,
    // kernel-probe census row (§3 consumer draw table)
    draw: Draw,
  }

  // Whole-machine state as one record so the view modules stay pure
  // CoreState -> CoreState transforms.
  type CoreState = {
    objs: ObjId -> ObjState,
    dprcs: ObjId -> ContainerState,
    conns: Set[(Endpoint, Endpoint)],
    nextNum: Family -> int,  // restool id space, per family
    nextHw: int,             // MC hardware-id space (§6.4)
  }

  // Evidence status carried per transition (design D2): a failed board
  // step must distinguish "model wrong" from "claim never verified".
  type Evidence =
    | BoardExercised  // driven on the board by prior work (anchors in object-model.md)
    | Read            // stands on source/restool/manual reading; awaits phase 5
}
