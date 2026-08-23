// dprtc family parameters (dprtc.md, Tier C). Firmware-enforced
// system-wide singleton (DPRTC-I1): dprtc.0 is DPL-born in the machine's
// initial state, so a second create is refused by the singleton gate.
// Root residency is DPL convention, not a modeled placement law —
// placement stays AnyContainer. The kernel-MMIO vs DPDK-MC split
// (DPRTC-I2) is the ownership-exclusivity law, phase 2.
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dprtc {
  import types.* from "../core/types"

  pure val dprtcParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: true,              // second create refused (DPRTC-I1)
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // fsl-dpaa2-ptp (§3)
    hotBindAvailable: true,       // driver present; boot-born
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // §3 census
  }
}
