// dpbp family parameters (dpbp.md). Pooled: created + plugged → enters
// FSL_MC_POOL_DPBP; consumers allocate. Allocator custody is modeled as
// pool membership, not driver binding, so hasKernelDriver is false here.
// Pool free does no MC reset — membership never implies clean state
// (DPBP-I3, breaking).
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPBP-I1 main.qnt DPBP_I5Test;
//   DPBP-I2 core/invariants.qnt DPBP_I2 + main.qnt DPBP_I2Test/DPBP_I2PlugTest;
//   DPBP-I3 main.qnt DPBP_I3Test + dpbp_lifecycle dirtyReturnTest
//     (the custody-cycle dirty-return face, inherited from pool_lifecycle);
//   DPBP-I4 main.qnt DPBP_I4Test/DPBP_I4TopUpTest;
//   DPBP-I5 core/invariants.qnt LAW4_twoIdSpaces + main.qnt DPBP_I5Test;
//   DPBP-I7 dpbp_lifecycle POOL_CENSUS + censusRefusesAtCeilingTest
//     (the census-floor refusal; the P3 custody+convergence machine, inherited
//     from families/pool_lifecycle.qnt via the FAMILY=Dpbp instantiation).
// Apalache-marked subset: DPBP-I2, DPBP-I5.
module dpbp {
  import types.* from "../core/types"

  pure val dpbpParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: true,
    resetOnBind: false,
    hasKernelDriver: false,
    hotBindAvailable: false,      // allocator custody, not a bind
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}

// The dpbp custody+convergence machine: a thin instantiation of the shared P3
// substrate contextualized to dpbp (pool-objects design D7 as amended, ADR-0019).
// All laws and directed runs are inherited; nothing dpbp-specific to add.
module dpbp_lifecycle {
  import types.* from "../core/types"
  import pool_lifecycle(FAMILY = Dpbp, CEILING = 3).* from "./pool_lifecycle"
}
