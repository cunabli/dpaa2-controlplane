// dpseci family parameters (dpseci.md). Created-then-bound: dpaa2_caam
// or VFIO/PMD binds. Not in the reset-on-bind class; unbind-side resets
// are version-gated instead (≤ 5.3 leaves armed state behind,
// DPSECI-I5) — the core's unbind transform already models "no
// guarantee".
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dpseci {
  import types.* from "../core/types"

  pure val dpseciParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // dpaa2_caam (§3)
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // §3 census; dpio borrowed
  }
}
