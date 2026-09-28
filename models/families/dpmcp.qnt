// dpmcp family parameters (dpmcp.md). Pooled: created + plugged →
// enters FSL_MC_POOL_DPMCP. Claimed only via fsl_mc_portal_allocate —
// the object-allocate path refuses dpmcp (§3); the machine's single
// allocate action models both paths, the distinction is a phase-2
// invariant concern. Never reset, on any path (DPMCP-I3, breaking).
// The dependency arrows of §4 bottom out here (DPMCP-I1).
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPMCP-I1 main.qnt DPMCP_I1Test; DPMCP-I2 main.qnt DPMCP_I2Test/DPMCP_I2ReturnTest;
//   DPMCP-I3 main.qnt DPMCP_I3Test + dpmcp_lifecycle dirtyReturnTest
//     (the never-reset custody return, shared with DPBP-I3, inherited from
//     pool_lifecycle);
//   DPMCP-I4 core/invariants.qnt DPRC_I1 (placement face).
// Apalache-marked subset: DPMCP-I4.
module dpmcp {
  import types.* from "../core/types"

  pure val dpmcpParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: true,
    resetOnBind: false,
    hasKernelDriver: false,       // allocator custody = pool membership in the model
    hotBindAvailable: false,      // allocator custody, not a bind
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}

// The dpmcp custody+convergence machine: a thin instantiation of the shared P3
// substrate contextualized to dpmcp (pool-objects design D7 as amended, ADR-0019).
module dpmcp_lifecycle {
  import types.* from "../core/types"
  import pool_lifecycle(FAMILY = Dpmcp, CEILING = 3).* from "./pool_lifecycle"
}
