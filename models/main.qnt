// Top-level instantiation of the core machine with all 16 family
// parameter modules (design D2/D4): the corpus-wide simulate target.
//
//   pnpm exec quint test models/main.qnt --main=main   # directed runs below
//   pnpm exec quint run  models/main.qnt --main=main   # random exploration
//
// The directed runs execute the spec scenarios of `formal-models`
// ("A lifecycle trace runs in the simulator"): the canonical forward
// order and teardown of object-model.md §5, and refusal of transitions
// the baseline forbids.
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module main {
  import types.* from "./core/types"
  import dprc.* from "./families/dprc"
  import dpni.* from "./families/dpni"
  import dpmac.* from "./families/dpmac"
  import dpbp.* from "./families/dpbp"
  import dpio.* from "./families/dpio"
  import dpcon.* from "./families/dpcon"
  import dpmcp.* from "./families/dpmcp"
  import dpseci.* from "./families/dpseci"
  import dpsw.* from "./families/dpsw"
  import dpdmux.* from "./families/dpdmux"
  import dpaiop.* from "./families/dpaiop"
  import dpci.* from "./families/dpci"
  import dpdcei.* from "./families/dpdcei"
  import dpdmai.* from "./families/dpdmai"
  import dprtc.* from "./families/dprtc"
  import dpdbg.* from "./families/dpdbg"

  pure val ALL_PARAMS: Family -> FamilyParams = Map(
    Dprc -> dprcParams,
    Dpni -> dpniParams,
    Dpmac -> dpmacParams,
    Dpbp -> dpbpParams,
    Dpio -> dpioParams,
    Dpcon -> dpconParams,
    Dpmcp -> dpmcpParams,
    Dpseci -> dpseciParams,
    Dpsw -> dpswParams,
    Dpdmux -> dpdmuxParams,
    Dpaiop -> dpaiopParams,
    Dpci -> dpciParams,
    Dpdcei -> dpdceiParams,
    Dpdmai -> dpdmaiParams,
    Dprtc -> dprtcParams,
    Dpdbg -> dpdbgParams
  )

  // DPC-born dpmacs of the reference environment: 3–10 and 17
  // (reference-environment.md). The board safety envelope (dpmac.3/17
  // total-deny) is a harness law (phase 4), not a model gate — model
  // traces referencing them never reach a board script unfiltered.
  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)
  ).* from "./core/machine"

  // Assertion step for directed runs: holds the state and checks p.
  action check(p: bool): bool = all { p, s' = s }

  // Ids the runs create, deterministic from the per-family counters.
  pure val dprc2: ObjId = { fam: Dprc, num: 2 }
  pure val dpni0: ObjId = { fam: Dpni, num: 0 }
  pure val dpio0: ObjId = { fam: Dpio, num: 0 }
  pure val dpbp0: ObjId = { fam: Dpbp, num: 0 }
  pure val dpcon0: ObjId = { fam: Dpcon, num: 0 }
  pure val dpmcp0: ObjId = { fam: Dpmcp, num: 0 }
  pure val dpmcp1: ObjId = { fam: Dpmcp, num: 1 }
  pure def ep(o: ObjId): Endpoint = { obj: o, port: 0 }
  pure val mac7: ObjId = { fam: Dpmac, num: 7 }
  pure val mac8: ObjId = { fam: Dpmac, num: 8 }
  pure val mac9: ObjId = { fam: Dpmac, num: 9 }

  // §5 canonical forward order in the root container, then the teardown
  // mirror back to the boot state. Two dpmcps: one is the dpio driver's
  // forgotten draw (DPIO-I1), one is the dpni's.
  run canonicalLifecycleTest =
    init
      // 1. pool companions first (and the unpooled dpio)
      .then(createObjectAt(Dpmcp, dprc1))
      .then(createObjectAt(Dpmcp, dprc1))
      .then(createObjectAt(Dpbp, dprc1))
      .then(createObjectAt(Dpcon, dprc1))
      .then(createObjectAt(Dpio, dprc1))
      .then(plugAt(dpmcp0))
      .then(plugAt(dpmcp1))
      .then(plugAt(dpbp0))
      .then(plugAt(dpcon0))
      .then(plugAt(dpio0))
      .then(rescanAt(dprc1))
      .then(kernelBindAt(dpio0, Set(dpmcp0), Set(), Set()))
      // 2. create the consumer
      .then(createObjectAt(Dpni, dprc1))
      // 3. pre-plug mutation window
      .then(preplugMutateAt(dpni0))
      .then(check(not(s.objs.get(dpni0).pristine)))
      // 4. plug; kernel bind resets the object — the pre-plug mutation
      //    was futile (DPNI-I2)
      .then(plugAt(dpni0))
      .then(rescanAt(dprc1))
      .then(kernelBindAt(dpni0, Set(dpmcp1), Set(dpbp0), Set(dpcon0)))
      .then(check(s.objs.get(dpni0).pristine))
      // 5. connect (legal after plug too — ENDPOINT_CHANGED absorbs)
      .then(connectEdgeAt(ep(dpni0), ep(mac7)))
      // 7. enable, consumer-side
      .then(enableAt(dpni0))
      // --- teardown mirror (§5) ---
      .then(disableAt(dpni0))
      .then(disconnectEdgeAt(ep(dpni0)))
      .then(unplugAt(dpni0))          // kernel unbind trigger; draws released
      .then(destroyAt(dpni0))
      .then(unplugAt(dpio0))
      .then(destroyAt(dpio0))
      .then(unplugAt(dpmcp0))
      .then(unplugAt(dpmcp1))
      .then(unplugAt(dpbp0))
      .then(unplugAt(dpcon0))
      .then(destroyAt(dpmcp0))
      .then(destroyAt(dpmcp1))
      .then(destroyAt(dpbp0))
      .then(destroyAt(dpcon0))
      .then(check(s.objs.keys() == bootIds))

  // Create-time gates orthogonal to the order (§5): platform,
  // cardinality, placement.
  run refusesPlatformGateTest =
    init.then(createObjectAt(Dpaiop, dprc1).fail())        // DPAIOP-I1

  run refusesSingletonTest =
    init.then(createObjectAt(Dprtc, dprc1).fail())         // DPRTC-I1: dprtc.0 exists

  run refusesRootOnlyPlacementTest =
    init
      .then(createContainerAt(dprc1))                      // dprc.2
      .then(createObjectAt(Dpdbg, dprc2).fail())           // DPDBG-I1

  // The spec scenario's own example: assigning a plugged object across
  // containers is refused (DPRC-I3).
  run refusesPluggedMoveTest =
    init
      .then(createContainerAt(dprc1))
      .then(createObjectAt(Dpni, dprc1))
      .then(plugAt(dpni0))
      .then(assignChildAt(dpni0, dprc2).fail())

  run refusesIllegalPairTest =
    init.then(connectEdgeAt(ep(mac7), ep(mac9)).fail())    // no dpmac↔dpmac edge (§2)

  // Cardinality one: reconnect requires disconnect first (DPNI-I9).
  run refusesSecondPeerTest =
    init
      .then(createObjectAt(Dpni, dprc1))
      .then(connectEdgeAt(ep(dpni0), ep(mac7)))
      .then(connectEdgeAt(ep(dpni0), ep(mac8)).fail())
}
