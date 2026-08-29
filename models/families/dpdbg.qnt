// dpdbg family parameters (dpdbg.md, Tier C). Runtime-created singleton
// that exists only in the root container — firmware-refused elsewhere
// (DPDBG-I1) — and driver-less by design. Its entire SET surface is
// write-only (DPDBG-I2, breaking): an observation-layer law encoded in
// phase 2, not a parameter.
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPDBG-I1 families/dpdbg.qnt RootOnly+singleton + main.qnt DPDBG_I1Test/DPDBG_I1SingletonTest;
//   DPDBG-I2 structural (no debug-state observable); DPDBG-I4 main.qnt DPDBG_I4Test.
// Apalache-marked subset: none.
module dpdbg {
  import types.* from "../core/types"

  pure val dpdbgParams: FamilyParams = {
    creatable: true,
    placement: RootOnly,          // DPDBG-I1
    singleton: true,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: false,       // driver-less by design (§3)
    hotBindAvailable: false,      // driver-less
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}
