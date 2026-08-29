// dpni family parameters (dpni.md). The created-then-bound consumer the
// canonical lifecycle order was board-validated on (C1 [verified]).
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPNI-I1 structural (LAW 1, no cfg block in state); DPNI-I2 main.qnt DPNI_I2Test;
//   DPNI-I3 retro/reconciler.qnt association runs (itf-replay);
//   DPNI-I4 machine.qnt kernelBind census guard + main.qnt DPNI_I4Test;
//   DPNI-I8 main.qnt DPNI_I8Test;
//   DPNI-I9 core/invariants.qnt DPNI_I9 + main.qnt DPNI_I9Test;
//   DPNI-I11 main.qnt IOCTL_OK/DPNI_I11Test + core/ioctl_policy.qnt.
// Apalache-marked subset: DPNI-I9, DPNI-I11.
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
