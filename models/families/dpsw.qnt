// dpsw family parameters (dpsw.md, Tier B). Kernel-only consumer:
// dpaa2-switch binds; DPDK discards dpsw (DPSW-I6). In the
// reset-on-bind class — binding additionally dismantles MC-default
// state (DPSW-I4, breaking).
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module dpsw {
  import types.* from "../core/types"

  pure val dpswParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: true,            // DPSW-I4, breaking
    hasKernelDriver: true,        // dpaa2-switch (kernel-only, DPSW-I6)
    // Only for a create in the configuration the driver accepts: it
    // refuses a dpsw whose control interface is off or whose flooding
    // or broadcast domain is not per FDB, which is what restool's
    // defaults produce (dpaa2-switch.h, supports_cpu_traffic). The
    // refusal is logged, not silent. V-DPSW-1 is the board anchor.
    hotBindAvailable: true,
    // Switch ports address as dpsw.N.M (§2); the port count is
    // create-config — fixed to 2 at model scope, refine per-scenario.
    endpointPorts: 2,
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 1, dpcon: 0 },  // §3 census: dpbp for ctrl-if; dpio borrowed
  }
}
