// dpio family parameters (dpio.md). NOT pooled — there is no
// FSL_MC_POOL_DPIO (DPIO-I1); the kernel binds each dpio to the dpio
// driver as a per-CPU shared service, and DPDK claims them exclusively
// per thread. The dpio driver itself draws a dpmcp at probe — the
// dpio→dpmcp ordering edge inside the pool-companion phase (§4).
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPIO-I1 core/invariants.qnt DPIO_I1 + main.qnt DPMCP_I1Test;
//   DPIO-I4 structural (state carries no dpio-to-CPU pairing).
// Apalache-marked subset: DPIO-I1.
module dpio {
  import types.* from "../core/types"

  pure val dpioParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,                // regime-typed, never pooled (DPIO-I1/I2)
    resetOnBind: false,
    hasKernelDriver: true,
    hotBindAvailable: false,      // every per-CPU seat taken at boot
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // the forgotten draw (§3 census)
  }
}
