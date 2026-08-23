// Retro-model of the reconciler's dpni↔dpmac association flow (task
// 3.1, design D2): the one flow already encoded and board-validated in
// Rust (ADR-0001, crates/dpaa2-api reconcile.rs + crates/dpaa2-mc
// restool.rs), rendered as directed runs over the core machine — no new
// actions, no family fork. The reconciler is a modeling subject, read
// only.
//
// Granularity (the D2 risk, settled here): one reconciler plan step is
// COARSER than one machine action. The expansion mirrors RestoolMc:
//
//   Create{port}   = ensure_dpio (dpio + its dpmcp, plugged) +
//                    dep chain (dpbp, dpmcp, dpcon, plugged) +
//                    dpni create (unplugged)
//   Connect{port}  = plug dpni + dprc connect + sync (rescan)
//   Bind{port}     = wait-to-observe; the KERNEL fires kernelBind
//   Unbind{dpni}   = unbindAt        (kernel unbind)
//   Disconnect{dpni} = disconnectEdgeAt
//   Destroy{dpni}  = destroyAt
//
// The reconciler is level-triggered: it observes, plans, applies the
// whole plan, re-observes. Observation points therefore segment each
// run into epochs; the epoch boundaries are recorded per frozen trace
// in the replayer's table (crates/dpaa2-verify), which projects each
// observation state to an ObservedTopology, runs reconcile(), and
// diffs the plan against the epoch's state deltas. Runs contain no
// check() stutter steps so state index == action index + 1.
//
// Out of retro scope (covered by reconcile.rs unit tests, noted for
// honesty): MAC assert/actuate (the model carries no MAC values),
// fixed-link ports (no link_type in CoreState), foreign-object
// preservation, immutable-attribute drift.
//
// Named invariants: none — conformance is the replayer's diff (3.2).
// Apalache-marked subset: none. TLA+ escalation: none.
module retro {
  import types.* from "../core/types"
  import params.* from "../families/params"

  // Same reference environment as models/main.qnt.
  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ).* from "../core/machine"

  // Ids deterministic from the per-family counters, in creation order.
  pure val dpio0: ObjId = { fam: Dpio, num: 0 }
  pure val dpmcp0: ObjId = { fam: Dpmcp, num: 0 }   // dpio's companion
  pure val dpbp0: ObjId = { fam: Dpbp, num: 0 }
  pure val dpmcp1: ObjId = { fam: Dpmcp, num: 1 }   // dpni's draw
  pure val dpcon0: ObjId = { fam: Dpcon, num: 0 }
  pure val dpni0: ObjId = { fam: Dpni, num: 0 }
  pure val mac7: ObjId = { fam: Dpmac, num: 7 }
  pure def ep(o: ObjId): Endpoint = { obj: o, port: 0 }

  // Forward association: desired = one present port anchored at dpmac.7.
  //
  // Epoch 0 (state 0, init): observe -> plan [Create, Connect, Bind].
  //   states 1..11: Create expansion (RestoolMc::create_dpni order:
  //   ensure_dpio, then dpbp/dpmcp/dpcon deps, then dpni, unplugged),
  //   states 12..14: Connect expansion (plug, connect, sync), and the
  //   sync-triggered dpio kernel probe (state 15) — restool creates
  //   trigger no rescan (DPCI-I3), so dpio binds only after the sync.
  // Epoch 1 (state 15): observe -> plan [Bind] (wait-to-observe);
  //   state 16: the kernel fires the dpni probe (census draw).
  // Epoch 2 (state 16, final): observe -> converged.
  run retroAssociationTest =
    init
      // Create{port} — ensure_dpio: one dpio with its own dpmcp
      .then(createObjectAt(Dpio, dprc1))            // 1
      .then(plugAt(dpio0))                          // 2
      .then(createObjectAt(Dpmcp, dprc1))           // 3
      .then(plugAt(dpmcp0))                         // 4
      // Create{port} — the dpni dep chain, each create-and-plugged
      .then(createObjectAt(Dpbp, dprc1))            // 5
      .then(plugAt(dpbp0))                          // 6
      .then(createObjectAt(Dpmcp, dprc1))           // 7
      .then(plugAt(dpmcp1))                         // 8
      .then(createObjectAt(Dpcon, dprc1))           // 9
      .then(plugAt(dpcon0))                         // 10
      // Create{port} — the dpni itself, unplugged (plug is deferred to
      // Connect so actuate-mode set_mac would run pre-plug)
      .then(createObjectAt(Dpni, dprc1))            // 11
      // Connect{port} — plug + connect + sync
      .then(plugAt(dpni0))                          // 12
      .then(connectEdgeAt(ep(dpni0), ep(mac7)))     // 13
      .then(rescanAt(dprc1))                        // 14
      // the sync made the bus current; the dpio service probes first
      .then(kernelBindAt(dpio0, Set(dpmcp0), Set(), Set()))  // 15, epoch 1 boundary
      // Bind{port} — the kernel fires the dpni probe (wait-to-observe)
      .then(kernelBindAt(dpni0, Set(dpmcp1), Set(dpbp0), Set(dpcon0))) // 16, epoch 2

  // Teardown under prune: desired = the same port, presence Absent,
  // options.prune = true, starting from the converged forward state.
  //
  // Epoch 0 (state 16): observe -> plan [Unbind, Disconnect, Destroy].
  // Epoch 1 (state 19, final): observe -> converged (port gone).
  run retroTeardownTest =
    retroAssociationTest
      .then(unbindAt(dpni0))                        // 17
      .then(disconnectEdgeAt(ep(dpni0)))            // 18
      .then(destroyAt(dpni0))                       // 19
}
