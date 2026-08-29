// dpdmai family parameters (dpdmai.md, Tier C). fsl-dpaa2-qdma binds
// after a rescan (the create itself triggers none — same class as dpci,
// DPCI-I3), or DPDK claims it as a dmadev. Firmware behavior swings
// inside the "additive" API window (DPL-vs-CREATE divergence
// pre-10.38.1, DPDMAI-I2) — per-action version state, a phase-2 law.
//
// Named invariants: none modeled — see models/COVERAGE.md for the family's dispositions. Apalache-marked subset: none.
module dpdmai {
  import types.* from "../core/types"

  pure val dpdmaiParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // fsl-dpaa2-qdma (§3)
    hotBindAvailable: false,      // no qdma driver on this kernel
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // §3 census; needs dpios present
  }
}
