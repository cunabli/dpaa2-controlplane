// dpmac family parameters (dpmac.md). DPC-born in the root at boot and
// pinned there: the runtime lifecycle is connect/disconnect, not
// create/move (DPMAC-I1) — creatable=false plus RootOnly encode it.
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPMAC-I1 core/invariants.qnt DPMAC_I1; DPMAC-I5 main.qnt DPMAC_I5Test;
//   DPMAC-I8 main.qnt DPSECI_I8Test (class witness) + ph.4 adapter law.
// Apalache-marked subset: DPMAC-I1.
module dpmac {
  import types.* from "../core/types"

  pure val dpmacParams: FamilyParams = {
    creatable: false,             // boot-born from the DPC (DPMAC-I1)
    placement: RootOnly,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // standalone driver binds the leftovers (§3)
    hotBindAvailable: true,       // boot-born; never hot-created
    endpointPorts: 1,             // faces dpni / dpsw uplink / dpdmux.N.0 (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // standalone dpmac: 1 dpmcp (§3 census)
  }
}
