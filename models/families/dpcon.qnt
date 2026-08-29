// dpcon family parameters (dpcon.md). Pooled: created + plugged →
// enters FSL_MC_POOL_DPCON; consumers allocate one per polled queue
// (DPCON-I1). Shortage after the first degrades the consumer instead of
// deferring it (DPNI-I5, breaking) — a phase-2 invariant concern.
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPCON-I2 core/invariants.qnt LAW4_twoIdSpaces + main.qnt DPCON_I2Test;
//   DPCON-I5 main.qnt DPCON_I5Test.
// Apalache-marked subset: DPCON-I2.
module dpcon {
  import types.* from "../core/types"

  pure val dpconParams: FamilyParams = {
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
