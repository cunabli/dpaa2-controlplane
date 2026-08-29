// dpaiop family parameters (dpaiop.md, Tier C). Not creatable on this
// platform: the MC's create handler refuses on AIOP-less silicon
// regardless of permissions (DPAIOP-I1/I2) — the "permission bit is not
// the last gate" exemplar. Its aiop_container_id is a create-time
// reference to an AIOP-flagged dprc, not a connect edge (DPAIOP-I4).
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPAIOP-I1 families/dpaiop.qnt creatable=false + main.qnt DPAIOP_I1Test;
//   DPAIOP-I2 main.qnt DPAIOP_I1Test (OBJ_CREATE present, refused anyway).
// Apalache-marked subset: none.
module dpaiop {
  import types.* from "../core/types"

  pure val dpaiopParams: FamilyParams = {
    creatable: false,             // platform-refused (DPAIOP-I1)
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: false,
    hotBindAvailable: false,      // driver-less
    endpointPorts: 0,             // no endpoint surface (DPAIOP-I4)
    createTriggersRescan: false,
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}
