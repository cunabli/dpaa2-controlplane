// dpni family parameters (dpni.md). The created-then-bound consumer the
// canonical lifecycle order was board-validated on (C1 [verified]).
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dpni {
  import types.* from "../core/types"

  pure val dpniParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: true,            // bind resets the object (DPNI-I2, breaking)
    hasKernelDriver: true,        // fsl_dpaa2_eth binds on plug
    hotBindAvailable: true,       // board-anchored (V-LIFE-DPNI-1)
    endpointPorts: 1,             // dpni↔dpmac/dpni/dpsw/dpdmux edges (§2)
    createTriggersRescan: false,  // most creates trigger no rescan (DPCI-I3)
    // fsl_dpaa2_eth probe census (§3): 1 dpmcp, 1 dpbp,
    // min(affine-dpio CPUs, num_queues) dpcons — abstracted to 1 at
    // model scope (DPCON-I1: one per polled queue); refine via a model
    // const if a phase-2 invariant needs the coupling. dpio is borrowed
    // from the shared service, never drawn.
    draw: { dpmcp: 1, dpbp: 1, dpcon: 1 },
  }
}
