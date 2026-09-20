// dpcon family parameters (dpcon.md). Pooled: created + plugged →
// enters FSL_MC_POOL_DPCON; consumers allocate one per polled queue
// (DPCON-I1). Shortage after the first degrades the consumer instead of
// deferring it (DPNI-I5, breaking) — a phase-2 invariant concern.
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPCON-I2 core/invariants.qnt LAW4_twoIdSpaces + main.qnt DPCON_I2Test;
//   DPCON-I4 families/dpcon.qnt NotificationEdge type (the mutable dpcon→dpio
//     notification edge; the retarget dynamics land in a later #6 phase;
//     docs/baseline/dpcon.md "Attribute mutability");
//   DPCON-I5 main.qnt DPCON_I5Test + dpcon_lifecycle dirtyReturnTest
//     (the never-reset custody return, shared with DPBP-I3, inherited from
//     pool_lifecycle).
// Apalache-marked subset: DPCON-I2.
module dpcon {
  import types.* from "../core/types"

  // DPCON-I4: the one mutable dpcon edge — set_notification retargets the
  // channel at a dpio at runtime (dpcon.md "Attribute mutability"). The address
  // is the qbman channel id, a second id space distinct from the object id
  // (DPCON-I2). Type only here; the retarget dynamics are a later #6 phase.
  type NotificationEdge = {
    qbmanChId: int,        // the dequeue/CDAN address, not the object id (DPCON-I2)
    target: Option[int],   // notified dpio id; None == DPCON_INVALID_DPIO_ID (-1), disabled
    priority: int,         // priority in the DPIO's ceiling, not the dpcon's own
  }

  pure val dpconParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: true,
    resetOnBind: false,
    hasKernelDriver: false,       // allocator custody = pool membership in the model
    hotBindAvailable: false,      // allocator custody, not a bind
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}

// The dpcon custody+convergence machine: a thin instantiation of the shared P3
// substrate contextualized to dpcon (pool-objects design D7 as amended, ADR-0019).
module dpcon_lifecycle {
  import types.* from "../core/types"
  import pool_lifecycle(FAMILY = Dpcon, CEILING = 3).* from "./pool_lifecycle"
}
