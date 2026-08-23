// dpdbg family parameters (dpdbg.md, Tier C). Runtime-created singleton
// that exists only in the root container — firmware-refused elsewhere
// (DPDBG-I1) — and driver-less by design. Its entire SET surface is
// write-only (DPDBG-I2, breaking): an observation-layer law encoded in
// phase 2, not a parameter.
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dpdbg {
  import types.* from "../core/types"

  pure val dpdbgParams: FamilyParams = {
    creatable: true,
    placement: RootOnly,          // DPDBG-I1
    singleton: true,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: false,       // driver-less by design (§3)
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}
