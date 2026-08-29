// dpci family parameters (dpci.md, Tier C). Created + dprc connect; no
// kernel driver — userspace/DPDK consumers only, so a restool-only dpci
// is created-but-never-enabled and may never leave link-down (DPCI-I5).
// Its Tx priority capacity is the peer's count, not its own (DPCI-I1,
// asymmetry) — a phase-2 invariant concern.
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPCI-I1 core/connect.qnt legalPair + main.qnt DPCI_I1Test/DPCI_I1CrossFamilyTest;
//   DPCI-I2 structural (no dpci options state);
//   DPCI-I3 main.qnt DPCI_I3Test + families/dpci.qnt createTriggersRescan=false;
//   DPCI-I5 main.qnt DPCI_I5Test.
// Apalache-marked subset: none.
module dpci {
  import types.* from "../core/types"

  pure val dpciParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: false,       // no kernel driver (§3)
    hotBindAvailable: false,      // driver-less
    endpointPorts: 1,             // dpci↔dpci, the only same-family-only edge (§2)
    createTriggersRescan: false,  // dpci creates trigger no rescan (DPCI-I3)
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}
