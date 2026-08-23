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
// Phase 2 adds the invariant encodings (design D3). State invariants
// live in core/invariants.qnt and are checked in random simulation via
//
//   pnpm exec quint run models/main.qnt --main=main \
//     --invariant=stateInvariants
//
// Action-guard and Breaking absence-of-assumption candidates are the
// directed `<ID>Test` runs below, named by baseline id; dispositions
// for every candidate are tabulated in models/COVERAGE.md (task 2.4).
//
// Named invariants: stateInvariants (see core/invariants.qnt for the
// per-id list). Apalache-marked subset: all nine state invariants at
// bounded depth 1 (rationale and empirical notes in
// core/invariants.qnt):
//
//   pnpm exec quint verify models/main.qnt --main=main \
//     --invariant=stateInvariants --max-steps=1
module main {
  import types.* from "./core/types"
  import invariants as inv from "./core/invariants"
  import params.* from "./families/params"

  // DPC-born dpmacs of the reference environment: 3–10 and 17
  // (reference-environment.md). The board safety envelope (dpmac.3/17
  // total-deny) is a harness law (phase 4), not a model gate — model
  // traces referencing them never reach a board script unfiltered.
  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ).* from "./core/machine"

  // Each state invariant bound to the machine state under its baseline
  // id, individually selectable by the CLI (`--invariants A B …`).
  val LAW3_triState = inv::LAW3_triState(s)
  val LAW4_twoIdSpaces = inv::LAW4_twoIdSpaces(s)
  val DPRC_I1 = inv::DPRC_I1(s)
  val DPRC_I2 = inv::DPRC_I2(s, ALL_PARAMS)
  val DPRC_I4 = inv::DPRC_I4(s)
  val DPNI_I9 = inv::DPNI_I9(s, ALL_PARAMS)
  val DPMAC_I1 = inv::DPMAC_I1(s, REF_DPMACS)
  val DPBP_I2 = inv::DPBP_I2(s)
  val DPIO_I1 = inv::DPIO_I1(s, ALL_PARAMS)

  // The simulator-rung conjunction (design D5): every state invariant
  // the machine must preserve, under their baseline ids.
  val stateInvariants: bool = and {
    LAW3_triState,
    LAW4_twoIdSpaces,
    DPRC_I1,
    DPRC_I2,
    DPRC_I4,
    DPNI_I9,
    DPMAC_I1,
    DPBP_I2,
    DPIO_I1,
  }

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
  pure val dpseci0: ObjId = { fam: Dpseci, num: 0 }
  pure val dpsw0: ObjId = { fam: Dpsw, num: 0 }
  pure val dpdmux0: ObjId = { fam: Dpdmux, num: 0 }
  pure val dpci0: ObjId = { fam: Dpci, num: 0 }
  pure val dpci1: ObjId = { fam: Dpci, num: 1 }
  pure val dpdbg0: ObjId = { fam: Dpdbg, num: 0 }
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

  // DPAIOP-I1: platform-refused on this silicon. Also DPAIOP-I2
  // (Breaking): dprc.1 carries OBJ_CREATE, and the create is refused
  // anyway — DPRC permission is not create permission.
  run DPAIOP_I1Test =
    init.then(createObjectAt(Dpaiop, dprc1).fail())

  // DPRTC-I1 (board-pending): system-wide singleton — dprtc.0 exists.
  run DPRTC_I1Test =
    init.then(createObjectAt(Dprtc, dprc1).fail())

  // DPDBG-I1: root-only placement…
  run DPDBG_I1Test =
    init
      .then(createContainerAt(dprc1))                      // dprc.2
      .then(createObjectAt(Dpdbg, dprc2).fail())

  // …and singleton: a second create is refused by firmware.
  run DPDBG_I1SingletonTest =
    init
      .then(createObjectAt(Dpdbg, dprc1))
      .then(createObjectAt(Dpdbg, dprc1).fail())

  // The spec scenario's own example: assigning a plugged object across
  // containers is refused (DPRC-I3, move precondition).
  run DPRC_I3Test =
    init
      .then(createContainerAt(dprc1))
      .then(createObjectAt(Dpni, dprc1))
      .then(plugAt(dpni0))
      .then(assignChildAt(dpni0, dprc2).fail())

  run refusesIllegalPairTest =
    init.then(connectEdgeAt(ep(mac7), ep(mac9)).fail())    // no dpmac↔dpmac edge (§2)

  // Cardinality one: reconnect requires disconnect first (DPNI-I9; the
  // both-endpoints-disconnected face of DPRC-I5).
  run DPNI_I9Test =
    init
      .then(createObjectAt(Dpni, dprc1))
      .then(connectEdgeAt(ep(dpni0), ep(mac7)))
      .then(connectEdgeAt(ep(dpni0), ep(mac8)).fail())

  // --- phase-2 directed encodings (design D3): action-guard and
  // --- Breaking absence-of-assumption candidates, named by baseline id

  // DPRC-I6 (Breaking): sync is not visibility. A root rescan reaches
  // its own residents only; a child-container object stays existence-
  // without-bus-visibility, and rescan of a child dprc is refused — the
  // model exposes no observable equating the two. (A failed action
  // leaves the state unset in Quint, so refusals terminate their run —
  // hence the split second run.)
  run DPRC_I6Test =
    init
      .then(createContainerAt(dprc1))              // dprc.2
      .then(createObjectAt(Dpni, dprc2))           // dpni.0 in the child
      .then(rescanAt(dprc1))                       // reaches dprc.1 residents only
      .then(check(not(s.objs.get(dpni0).busVisible)))

  run DPRC_I6RescanRefusedTest =
    init
      .then(createContainerAt(dprc1))
      .then(rescanAt(dprc2).fail())                // rescan is root-only

  // DPRC-I7 (Breaking): Linux-side release does not destroy MC objects
  // — after unbind the object is still on the bus, still plugged.
  run DPRC_I7Test =
    init
      .then(createObjectAt(Dpbp, dprc1))
      .then(plugAt(dpbp0))
      .then(rescanAt(dprc1))
      .then(vfioBindAt(dpbp0))
      .then(unbindAt(dpbp0))
      .then(check(s.objs.keys().contains(dpbp0) and s.objs.get(dpbp0).plugged))

  // DPRC-I10: container identity (icid, perm mask) never changes across
  // any post-create action sequence — the LAW-1 witness for dprc.
  run DPRC_I10Test =
    init
      .then(createContainerAt(dprc1))              // dprc.2, icid pool-assigned = 100
      .then(check(s.dprcs.get(dprc2).icid == 100))
      .then(createObjectAt(Dpni, dprc2))
      .then(setLockedAt(dprc2, true))
      .then(setLockedAt(dprc2, false))
      .then(check(and {
        s.dprcs.get(dprc2).icid == 100,
        s.dprcs.get(dprc2).perms == Set(Spawn, Alloc, ObjCreate, IrqCfg),
      }))

  // DPRC-I11 (board-pending): set-locked strips create/destroy/assign
  // from the sub-hierarchy; unlock restores it. One run per refusal
  // (refusals are terminal), one for the restore.
  run lockedChildPrefix =
    init
      .then(createContainerAt(dprc1))              // dprc.2
      .then(setLockedAt(dprc2, true))

  run DPRC_I11Test =
    lockedChildPrefix
      .then(createObjectAt(Dpni, dprc2).fail())

  run DPRC_I11SpawnTest =
    lockedChildPrefix
      .then(createContainerAt(dprc2).fail())

  run DPRC_I11UnlockTest =
    lockedChildPrefix
      .then(setLockedAt(dprc2, false))
      .then(createObjectAt(Dpni, dprc2))

  // DPNI-I2 (Breaking): MC-side state set before kernel bind does not
  // survive the bind — the pre-plug mutation is futile against a
  // reset-on-bind consumer; only the cfg block survives.
  run DPNI_I2Test =
    init
      .then(createObjectAt(Dpmcp, dprc1))
      .then(createObjectAt(Dpbp, dprc1))
      .then(createObjectAt(Dpcon, dprc1))
      .then(createObjectAt(Dpni, dprc1))
      .then(preplugMutateAt(dpni0))
      .then(check(not(s.objs.get(dpni0).pristine)))
      .then(plugAt(dpmcp0))
      .then(plugAt(dpbp0))
      .then(plugAt(dpcon0))
      .then(plugAt(dpni0))
      .then(rescanAt(dprc1))
      .then(kernelBindAt(dpni0, Set(dpmcp0), Set(dpbp0), Set(dpcon0)))
      .then(check(s.objs.get(dpni0).pristine))

  // DPNI-I4 (verified, ADR-0001 C1): kernel bind completes only with
  // the census draw satisfiable; a bare dpni stays unbound (the silent
  // -EPROBE_DEFER loop as refusal).
  run DPNI_I4Test =
    init
      .then(createObjectAt(Dpni, dprc1))
      .then(plugAt(dpni0))
      .then(rescanAt(dprc1))
      .then(kernelBindAt(dpni0, Set(), Set(), Set()).fail())

  // DPNI-I8 (board-pending): unbind-side reset is best-effort — the
  // model grants no guarantee: state stays dirty across unbind, and
  // convergence is established only by read-back (LAW 2 witness).
  run DPNI_I8Test =
    DPNI_I2Test
      .then(enableAt(dpni0))
      .then(check(not(s.objs.get(dpni0).pristine)))
      .then(unbindAt(dpni0))
      .then(check(and {
        s.objs.get(dpni0).bind == Unbound,
        not(s.objs.get(dpni0).pristine),
      }))

  // DPMAC-I5 (Breaking): connection state is not link state — the two
  // vary independently; no observable derives one from the other.
  run DPMAC_I5Test =
    init
      .then(createObjectAt(Dpni, dprc1))
      .then(connectEdgeAt(ep(dpni0), ep(mac7)))
      .then(check(and {
        s.conns.exists(c => c._1.obj == mac7 or c._2.obj == mac7),
        not(s.objs.get(mac7).linkUp),
      }))
      .then(linkChangeAt(mac7))
      .then(disconnectEdgeAt(ep(mac7)))
      .then(check(and {
        s.objs.get(mac7).linkUp,
        not(s.conns.exists(c => c._1.obj == mac7 or c._2.obj == mac7)),
      }))

  // DPBP-I5 (Breaking) / DPBP-I1: the MC-assigned hwId (bpid) is not
  // the object id — the model carries both, and they diverge.
  run DPBP_I5Test =
    init
      .then(createObjectAt(Dpbp, dprc1))
      .then(check(s.objs.get(dpbp0).hwId == 100 and dpbp0.num == 0))

  // DPCON-I2: same two-id-space law for the dpcon channel id
  // (qbman_ch_id ≠ object id).
  run DPCON_I2Test =
    init
      .then(createObjectAt(Dpcon, dprc1))
      .then(check(s.objs.get(dpcon0).hwId == 100 and dpcon0.num == 0))

  // DPMCP-I1 / DPIO-I1's arrow: the dpio kernel probe itself draws a
  // dpmcp — the dependency bottoms out at dpmcp, and the draw dirties
  // (membership never implies clean state).
  run DPMCP_I1Test =
    init
      .then(createObjectAt(Dpmcp, dprc1))          // dpmcp.0
      .then(createObjectAt(Dpio, dprc1))           // dpio.0
      .then(plugAt(dpmcp0))
      .then(plugAt(dpio0))
      .then(rescanAt(dprc1))
      .then(kernelBindAt(dpio0, Set(dpmcp0), Set(), Set()))
      .then(check(and {
        s.objs.get(dpmcp0).allocatedBy == Some(dpio0),
        not(s.objs.get(dpmcp0).pristine),
      }))

  // DPMCP-I2: concurrent openers each hold a pool dpmcp; a drawn portal
  // cannot be drawn again until closed (freed).
  run allocatedPortalPrefix =
    DPMCP_I1Test
      .then(createObjectAt(Dpmcp, dprc1))          // dpmcp.1
      .then(plugAt(dpmcp1))
      .then(allocateAt(dpio0, dpmcp1))

  run DPMCP_I2Test =
    allocatedPortalPrefix
      .then(allocateAt(dpio0, dpmcp1).fail())      // exhausted

  run DPMCP_I2ReturnTest =
    allocatedPortalPrefix
      .then(freeAt(dpmcp1))                        // close returns it
      .then(allocateAt(dpio0, dpmcp1))

  // DPMCP-I3 (Breaking): no reset anywhere in the lifecycle — a freed
  // portal returns to the pool dirty.
  run DPMCP_I3Test =
    allocatedPortalPrefix
      .then(freeAt(dpmcp1))
      .then(check(and {
        s.objs.get(dpmcp1).allocatedBy == None,
        not(s.objs.get(dpmcp1).pristine),
      }))

  // DPBP-I2: pool entry ⟺ plugged — an unplugged dpbp is dprc-visible
  // but allocator-invisible (the refusal run); plugging admits it.
  run DPBP_I2Test =
    DPMCP_I1Test                                   // dpio.0 kernel-bound
      .then(createObjectAt(Dpbp, dprc1))           // dpbp.0, unplugged
      .then(allocateAt(dpio0, dpbp0).fail())

  run DPBP_I2PlugTest =
    DPMCP_I1Test
      .then(createObjectAt(Dpbp, dprc1))
      .then(plugAt(dpbp0))
      .then(allocateAt(dpio0, dpbp0))

  // DPBP-I3 (Breaking): free does no MC reset — the pool returns the
  // object dirty; cleanliness is the next allocator's job, or nobody's.
  run DPBP_I3Test =
    DPBP_I2PlugTest
      .then(freeAt(dpbp0))
      .then(check(and {
        s.objs.get(dpbp0).allocatedBy == None,
        not(s.objs.get(dpbp0).pristine),
      }))

  // DPCON-I5 (Breaking): shared with DPBP-I3 — no reset on free.
  run DPCON_I5Test =
    DPMCP_I1Test
      .then(createObjectAt(Dpcon, dprc1))          // dpcon.0
      .then(plugAt(dpcon0))
      .then(allocateAt(dpio0, dpcon0))
      .then(freeAt(dpcon0))
      .then(check(and {
        s.objs.get(dpcon0).allocatedBy == None,
        not(s.objs.get(dpcon0).pristine),
      }))

  // DPBP-I4 (verified, C1 class): exhaustion defers the bind (refusal
  // run); creating and plugging one dpbp unblocks it (top-up run).
  run noDpbpPrefix =
    init
      .then(createObjectAt(Dpmcp, dprc1))
      .then(createObjectAt(Dpcon, dprc1))
      .then(createObjectAt(Dpni, dprc1))
      .then(plugAt(dpmcp0))
      .then(plugAt(dpcon0))
      .then(plugAt(dpni0))
      .then(rescanAt(dprc1))

  run DPBP_I4Test =
    noDpbpPrefix
      .then(kernelBindAt(dpni0, Set(dpmcp0), Set(), Set(dpcon0)).fail())

  run DPBP_I4TopUpTest =
    noDpbpPrefix
      .then(createObjectAt(Dpbp, dprc1))           // top-up
      .then(plugAt(dpbp0))
      .then(kernelBindAt(dpni0, Set(dpmcp0), Set(dpbp0), Set(dpcon0)))

  // DPSECI-I5 (Breaking, board-pending): unbind does not imply clean MC
  // state — the version-gated reset is modeled as no guarantee.
  run DPSECI_I5Test =
    init
      .then(createObjectAt(Dpseci, dprc1))
      .then(plugAt(dpseci0))
      .then(rescanAt(dprc1))
      .then(vfioBindAt(dpseci0))
      .then(enableAt(dpseci0))
      .then(unbindAt(dpseci0))
      .then(check(and {
        s.objs.get(dpseci0).bind == Unbound,
        not(s.objs.get(dpseci0).pristine),
      }))

  // DPSECI-I8 / DPMAC-I8 class: restool's in_use check reads
  // root-container sysfs only — a VFIO-bound object passes it and the
  // destroy goes through. The model refuses only what restool refuses;
  // no observable derives destruction from an exit status (LAW 2).
  run DPSECI_I8Test =
    init
      .then(createObjectAt(Dpseci, dprc1))
      .then(plugAt(dpseci0))
      .then(rescanAt(dprc1))
      .then(vfioBindAt(dpseci0))
      .then(destroyAt(dpseci0))
      .then(check(not(s.objs.keys().contains(dpseci0))))

  // --- Tier B (dpsw, dpdmux), then Tier C (task 2.2). Candidates not
  // --- expressible over this state (cfg predicates, regime matrices,
  // --- DPL round-trips, exit-code laws) are ledgered in COVERAGE.md.

  // DPSW-I3: a kernel-bound dpsw draws exactly 1 dpmcp + 1 dpbp and no
  // dpcon from its own container (dpio is the shared service).
  run DPSW_I3Test =
    init
      .then(createObjectAt(Dpmcp, dprc1))
      .then(createObjectAt(Dpbp, dprc1))
      .then(createObjectAt(Dpsw, dprc1))
      .then(plugAt(dpmcp0))
      .then(plugAt(dpbp0))
      .then(plugAt(dpsw0))
      .then(rescanAt(dprc1))
      .then(kernelBindAt(dpsw0, Set(dpmcp0), Set(dpbp0), Set()))
      .then(check(and {
        s.objs.get(dpmcp0).allocatedBy == Some(dpsw0),
        s.objs.get(dpbp0).allocatedBy == Some(dpsw0),
      }))

  // DPSW-I4: bind-resets-object, strong form — no pre-bind MC-side
  // switch config survives the probe.
  run DPSW_I4Test =
    init
      .then(createObjectAt(Dpmcp, dprc1))
      .then(createObjectAt(Dpbp, dprc1))
      .then(createObjectAt(Dpsw, dprc1))
      .then(preplugMutateAt(dpsw0))
      .then(check(not(s.objs.get(dpsw0).pristine)))
      .then(plugAt(dpmcp0))
      .then(plugAt(dpbp0))
      .then(plugAt(dpsw0))
      .then(rescanAt(dprc1))
      .then(kernelBindAt(dpsw0, Set(dpmcp0), Set(dpbp0), Set()))
      .then(check(s.objs.get(dpsw0).pristine))

  // DPDMUX-I3: bind-resets-object (dpni-class); the mutable resetable
  // mask that can skip parts of the reset is board-pending, ledgered.
  run DPDMUX_I3Test =
    init
      .then(createObjectAt(Dpmcp, dprc1))
      .then(createObjectAt(Dpdmux, dprc1))
      .then(preplugMutateAt(dpdmux0))
      .then(check(not(s.objs.get(dpdmux0).pristine)))
      .then(plugAt(dpmcp0))
      .then(plugAt(dpdmux0))
      .then(rescanAt(dprc1))
      .then(kernelBindAt(dpdmux0, Set(dpmcp0), Set(), Set()))
      .then(check(s.objs.get(dpdmux0).pristine))

  // DPDMUX-I5: kernel-bound dpdmux draws exactly 1 dpmcp, nothing else.
  run DPDMUX_I5Test =
    init
      .then(createObjectAt(Dpmcp, dprc1))
      .then(createObjectAt(Dpdmux, dprc1))
      .then(plugAt(dpmcp0))
      .then(plugAt(dpdmux0))
      .then(rescanAt(dprc1))
      .then(kernelBindAt(dpdmux0, Set(dpmcp0), Set(), Set()))
      .then(check(s.objs.get(dpmcp0).allocatedBy == Some(dpdmux0)))

  // DPDMUX-I8: interface 0 is the uplink — only dpdmux.N.0 may face a
  // dpmac (connect.legalPorts, MC ≥ 10.37); a downlink port is refused.
  run DPDMUX_I8Test =
    init
      .then(createObjectAt(Dpdmux, dprc1))
      .then(connectEdgeAt({ obj: dpdmux0, port: 0 }, ep(mac8)))

  run DPDMUX_I8NonZeroUplinkTest =
    init
      .then(createObjectAt(Dpdmux, dprc1))
      .then(connectEdgeAt({ obj: dpdmux0, port: 1 }, ep(mac8)).fail())

  // DPCI-I1: pair symmetry — dpci↔dpci is the only legal edge for the
  // family (the one same-family-only edge of §2)…
  run DPCI_I1Test =
    init
      .then(createObjectAt(Dpci, dprc1))
      .then(createObjectAt(Dpci, dprc1))
      .then(connectEdgeAt(ep(dpci0), ep(dpci1)))

  // …and a cross-family dpci edge is refused.
  run DPCI_I1CrossFamilyTest =
    init
      .then(createObjectAt(Dpci, dprc1))
      .then(createObjectAt(Dpni, dprc1))
      .then(connectEdgeAt(ep(dpci0), ep(dpni0)).fail())

  // DPCI-I3 (Breaking): create performs no rescan — MC existence and
  // bus visibility diverge until an explicit rescan.
  run DPCI_I3Test =
    init
      .then(createObjectAt(Dpci, dprc1))
      .then(check(not(s.objs.get(dpci0).busVisible)))
      .then(rescanAt(dprc1))
      .then(check(s.objs.get(dpci0).busVisible))

  // DPCI-I5 (board-pending): link state reflects consumer enable, not
  // mere connection — a restool-only pair stays link-down.
  run DPCI_I5Test =
    init
      .then(createObjectAt(Dpci, dprc1))
      .then(createObjectAt(Dpci, dprc1))
      .then(connectEdgeAt(ep(dpci0), ep(dpci1)))
      .then(check(and {
        not(s.objs.get(dpci0).linkUp),
        not(s.objs.get(dpci1).linkUp),
      }))

  // DPDBG-I4 (board-pending): a created dpdbg enumerates on the bus
  // driver-less — visible after rescan, and never kernel-bindable.
  run DPDBG_I4Test =
    init
      .then(createObjectAt(Dpdbg, dprc1))
      .then(plugAt(dpdbg0))
      .then(rescanAt(dprc1))
      .then(check(and {
        s.objs.get(dpdbg0).busVisible,
        s.objs.get(dpdbg0).bind == Unbound,
      }))
      .then(kernelBindAt(dpdbg0, Set(), Set(), Set()).fail())
}
