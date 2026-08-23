// dpmcp family parameters (dpmcp.md). Pooled: created + plugged →
// enters FSL_MC_POOL_DPMCP. Claimed only via fsl_mc_portal_allocate —
// the object-allocate path refuses dpmcp (§3); the machine's single
// allocate action models both paths, the distinction is a phase-2
// invariant concern. Never reset, on any path (DPMCP-I3, breaking).
// The dependency arrows of §4 bottom out here (DPMCP-I1).
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dpmcp {
  import types.* from "../core/types"

  pure val dpmcpParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: true,
    resetOnBind: false,
    hasKernelDriver: false,       // allocator custody = pool membership in the model
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}
