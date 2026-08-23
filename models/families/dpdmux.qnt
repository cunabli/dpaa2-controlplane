// dpdmux family parameters (dpdmux.md, Tier B). Staging dpaa2-evb
// binds, or DPDK claims via VFIO. In the reset-on-bind class
// (DPDMUX-I3, breaking) — uniquely, a prior owner may store skip-reset
// policy (set_resetable) that a later reset honors: a phase-2 invariant
// concern, not a parameter.
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dpdmux {
  import types.* from "../core/types"

  pure val dpdmuxParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: true,            // DPDMUX-I3, breaking
    hasKernelDriver: true,        // staging dpaa2-evb (§3)
    hotBindAvailable: true,       // driver present; unsat (batch 3)
    // dpdmux.N.M (§2): port 0 is the uplink (dpmac-only since MC 10.37,
    // enforced in core connect.legalPorts); 1 downlink at model scope.
    endpointPorts: 2,
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // §3 census
  }
}
