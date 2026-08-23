// dpdcei family parameters (dpdcei.md, Tier C). Creatable — the
// hardware is present — but no consumer exists in the corpus (§3): a
// created dpdcei stays created-but-never-enabled.
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dpdcei {
  import types.* from "../core/types"

  pure val dpdceiParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: false,       // no consumer in the corpus (§3)
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}
