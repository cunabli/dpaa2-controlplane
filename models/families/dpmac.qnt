// dpmac family parameters (dpmac.md). DPC-born in the root at boot and
// pinned there: the runtime lifecycle is connect/disconnect, not
// create/move (DPMAC-I1) — creatable=false plus RootOnly encode it.
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dpmac {
  import types.* from "../core/types"

  pure val dpmacParams: FamilyParams = {
    creatable: false,             // boot-born from the DPC (DPMAC-I1)
    placement: RootOnly,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // standalone driver binds the leftovers (§3)
    endpointPorts: 1,             // faces dpni / dpsw uplink / dpdmux.N.0 (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // standalone dpmac: 1 dpmcp (§3 census)
  }
}
