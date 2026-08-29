// dpsw family parameters (dpsw.md, Tier B). Kernel-only consumer:
// dpaa2-switch binds; DPDK discards dpsw (DPSW-I6). In the
// reset-on-bind class — binding additionally dismantles MC-default
// state (DPSW-I4, breaking).
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPSW-I3 main.qnt DPSW_I3Test; DPSW-I4 main.qnt DPSW_I4Test;
//   DPSW-I5 core/invariants.qnt LAW4_twoIdSpaces (DPBP_I5Test witness).
// Apalache-marked subset: DPSW-I5.
module dpsw {
  import types.* from "../core/types"

  pure val dpswParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: true,            // DPSW-I4, breaking
    hasKernelDriver: true,        // dpaa2-switch (kernel-only, DPSW-I6)
    // Board-anchored 2026-08-23 (V-DPSW-1), with a condition: only a
    // create in the configuration the driver accepts binds. It refuses
    // a dpsw whose control interface is off or whose flooding or
    // broadcast domain is not per FDB, which is exactly what restool's
    // defaults produce (dpaa2-switch.h, supports_cpu_traffic). The
    // refusal is logged, not silent.
    hotBindAvailable: true,
    // Switch ports address as dpsw.N.M (§2); the port count is
    // create-config — fixed to 2 at model scope, refine per-scenario.
    endpointPorts: 2,
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 1, dpcon: 0 },  // §3 census: dpbp for ctrl-if; dpio borrowed
  }
}
