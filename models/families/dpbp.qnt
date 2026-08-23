// dpbp family parameters (dpbp.md). Pooled: created + plugged → enters
// FSL_MC_POOL_DPBP; consumers allocate. Allocator custody is modeled as
// pool membership, not driver binding, so hasKernelDriver is false here.
// Pool free does no MC reset — membership never implies clean state
// (DPBP-I3, breaking).
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dpbp {
  import types.* from "../core/types"

  pure val dpbpParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: true,
    resetOnBind: false,
    hasKernelDriver: false,
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}
