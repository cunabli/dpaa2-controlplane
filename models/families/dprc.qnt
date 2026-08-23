// dprc family parameters (dprc.md). The container family itself: child
// containers are created under SPAWN and bound by fsl_mc_dprc
// automatically; identity (id, icid, portal) is pool-assigned at create.
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dprc {
  import types.* from "../core/types"

  pure val dprcParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // fsl_mc_dprc binds automatically (§3)
    endpointPorts: 0,             // dprc owns connections but has no endpoints (§2)
    createTriggersRescan: true,   // dprc create does trigger a rescan (§5 step 6)
    // Probe draws nothing; the /dev/dprc.N-opener dpmcp holds are the
    // post-probe allocate path (§3 census, last row), not a probe draw.
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}
