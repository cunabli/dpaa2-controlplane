// dprc family parameters (dprc.md). The container family itself: child
// containers are created under SPAWN and bound by fsl_mc_dprc
// automatically; identity (id, icid, portal) is pool-assigned at create.
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPRC-I1 core/invariants.qnt DPRC_I1; DPRC-I2 core/invariants.qnt DPRC_I2;
//   DPRC-I3 main.qnt DPRC_I3Test; DPRC-I4 core/invariants.qnt DPRC_I4;
//   DPRC-I5 core/connect.qnt canConnect + main.qnt DPNI_I9Test;
//   DPRC-I6 main.qnt DPRC_I6Test/DPRC_I6RescanRefusedTest;
//   DPRC-I7 main.qnt DPRC_I7Test; DPRC-I10 main.qnt DPRC_I10Test;
//   DPRC-I11 main.qnt DPRC_I11Test + DPRC_I11SpawnTest/DPRC_I11UnlockTest.
// Apalache-marked subset: DPRC-I1, DPRC-I2, DPRC-I4.
module dprc {
  import types.* from "../core/types"

  pure val dprcParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // fsl_mc_dprc binds automatically (§3)
    hotBindAvailable: true,       // fsl_mc_dprc takes new children
    endpointPorts: 0,             // dprc owns connections but has no endpoints (§2)
    createTriggersRescan: true,   // dprc create does trigger a rescan (§5 step 6)
    // Probe draws nothing; the /dev/dprc.N-opener dpmcp holds are the
    // post-probe allocate path (§3 census, last row), not a probe draw.
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}
