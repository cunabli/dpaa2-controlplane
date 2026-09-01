// dpseci family parameters (dpseci.md). Created-then-bound: dpaa2_caam
// or VFIO/PMD binds. Not in the reset-on-bind class; unbind-side resets
// are version-gated instead (≤ 5.3 leaves armed state behind,
// DPSECI-I5) — the core's unbind transform already models "no
// guarantee".
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPSECI-I5 main.qnt DPSECI_I5Test; DPSECI-I7 structural (LAW 5);
//   DPSECI-I8 main.qnt DPSECI_I8Test.
// Apalache-marked subset: none.
module dpseci {
  import types.* from "../core/types"

  // The queue-pair ceiling of one dpseci device: a single object carries at
  // most 16 Tx/Rx queue pairs, so a crypto block demanding more flows than
  // this cannot be realized by one device (intent/refuse.qnt refuses it; the
  // remedy is splitting the demand across blocks). Verified in the NXP trees:
  //   DPSECI_MAX_QUEUE_NUM == 16 —
  //   .build/src/linux/drivers/crypto/caam/dpseci.h:25 (kernel),
  //   .build/src/dpdk/drivers/crypto/dpaa2_sec/mc/fsl_dpseci.h:25 (DPDK MC).
  pure val DPSECI_MAX_QUEUE_NUM: int = 16

  pure val dpseciParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // dpaa2_caam (§3)
    hotBindAvailable: false,      // crypto names claimed for the boot
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // §3 census; dpio borrowed
  }
}
