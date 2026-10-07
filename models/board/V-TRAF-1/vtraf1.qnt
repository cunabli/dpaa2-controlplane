// Scenario module for board suite V-TRAF-1 (cross-dprc-links task 1.4,
// cross-dprc-links design D10): the dpni↔dpni wire's six faces.
//
// This file carries two forms (models/README.md two-forms rule):
//   - module vtraf1: directed runs, one per cross-dprc-links design D10 face,
//     driving the link_lifecycle machine (cross-dprc-links design D7) at the
//     World level — the byte-stable record models/COVERAGE.md cites. These are
//     unchanged: core/ and the family machine stay untouched.
//   - modules vtraf1face1 … vtraf1face6: the MBT half the suite generator
//     consumes — one per-face scenario over the CORE machine (core/machine),
//     V-TRAF-0 style, each freezing one canonical --mbt trace. generate.rs
//     reads a `--mbt` core-machine trace via parse_mbt_trace
//     (crates/dpaa2-verify/src/board/adapter.rs), whose decoder knows only the
//     core action vocabulary and the machine's nondet binder names; a World-
//     level link_lifecycle trace does not parse (its state var is `world`, not
//     a bare CoreState, and its actions are scenario-local). So the frozen
//     traces are core-machine traces, and suite render of cross-dprc-links task
//     6.1 is pure rendering (.sh/.plan.json), not a fresh model run.
//
// The MBT determinism device (cross-dprc-links task 1.4): each per-face module
// gates its wrapper actions on the core state m::s (and, where a face revisits
// an identical core state, on m::lastVerbs — which parse_mbt_trace deliberately
// ignores), so EXACTLY ONE action is enabled per state and
// `quint run --main=vtraf1face<k> --mbt` freezes one canonical trace regardless
// of seed. No module carries its own state var — the guards are the device, not
// a cursor. Each trace ends when no action is enabled; --max-steps is set to the
// exact face length so the run stops there (V-TRAF-0's end-of-trace handling).
// The frozen traces are generated with seed 0x1:
//
//   pnpm exec quint run models/board/V-TRAF-1/vtraf1.qnt --main=vtraf1face1 \
//     --mbt --max-samples=1 --seed=0x1 --max-steps=3 \
//     --out-itf=models/board/V-TRAF-1/vtraf1.face1.itf.json
//   pnpm exec quint run models/board/V-TRAF-1/vtraf1.qnt --main=vtraf1face2 \
//     --mbt --max-samples=1 --seed=0x1 --max-steps=9 \
//     --out-itf=models/board/V-TRAF-1/vtraf1.face2.itf.json
//   pnpm exec quint run models/board/V-TRAF-1/vtraf1.qnt --main=vtraf1face3 \
//     --mbt --max-samples=1 --seed=0x1 --max-steps=5 \
//     --out-itf=models/board/V-TRAF-1/vtraf1.face3.itf.json
//   pnpm exec quint run models/board/V-TRAF-1/vtraf1.qnt --main=vtraf1face4 \
//     --mbt --max-samples=1 --seed=0x1 --max-steps=14 \
//     --out-itf=models/board/V-TRAF-1/vtraf1.face4.itf.json
//   pnpm exec quint run models/board/V-TRAF-1/vtraf1.qnt --main=vtraf1face5 \
//     --mbt --max-samples=1 --seed=0x1 --max-steps=4 \
//     --out-itf=models/board/V-TRAF-1/vtraf1.face5.itf.json
//   pnpm exec quint run models/board/V-TRAF-1/vtraf1.qnt --main=vtraf1face6 \
//     --mbt --max-samples=1 --seed=0x1 --max-steps=4 \
//     --out-itf=models/board/V-TRAF-1/vtraf1.face6.itf.json
//
// Each face is renderable board work: scratch dpnis are created in-trace via
// createObject (the board script mints them — magic init populations cannot be
// rendered), child containers via createContainer, and boot population comes
// from the machine's init action (the reference boot: dprc.0/dprc.1/dprtc.0 and
// the DPC-born dpmacs, REF_DPMACS below). No PHY dpmac participates in any face
// edge (cross-dprc-links design D10, finding 49): every connect is dpni↔dpni
// (legal per core/connect.qnt legalPair).
//
// Refusal faces cannot ride a forward trace — a disabled (.fail()) action
// cannot step, so a refusal can only be a run's terminal step. They stay the
// directed runs below and reach the suite as directed-run probes / hook steps
// for cross-dprc-links task 6.1, the V-TRAF-0 convention (generate.rs: "A hook
// carries the steps a trace cannot express"). The two trace-inexpressible
// refusals are disconnect-before-destroy (LINK-I1) and the DPRC-I5 double-
// connect (LINK-I3).
//
// Rev-3 banked disposition (cross-dprc-links task 7.2, suite ledger): on-board
// (2026-10-05 revs 1-2, fixture asserted) the MC ACCEPTED destroy of a still-
// connected dpni end, falsifying LINK-I1; DPRC-I5 is unresolvable without that
// same destroy; both sittings lost the standing management dpni from the bus in
// the probe window, so the hook quarantines the probe. The runs below are
// unchanged — this is the model record of a board finding, not a model change.
//
// The faces, in cross-dprc-links design D10 order:
//   1 root↔root converge   — the frame witness and saturation smoke are
//                            board-side hooks (6.1); the trace carries the
//                            converge steps and the face marker for them.
//   2 cross-container      — populate → connect → bind, then a post-bind
//                            connect (the kernel-end ENDPOINT_CHANGED dmesg
//                            law, recorded board-side, cross-dprc-links D4).
//   3 child↔child          — the same connect, nothing privileging the root.
//   4 create-side heal     — post-bind create's DeferredVisibility obligation
//                            discharged by a consented rebind cycle (unbind →
//                            kernelBind → rescan), cross-dprc-links design D5.
//   5 destroy-mirror       — a visible, container-bound, disconnected end is
//                            destroyed, leaving the lazy StaleNode residue
//                            (cross-dprc-links design D5).
//   6 teardown laws        — the positive half: connect then disconnect of a
//                            live wire. The two refusals ride directed runs.
// The child-issued-connect refusal face is NOT walked here: the suite replays
// the banked V-DPCI-1 witness for it (6.1's concern), not re-run.
//
// Named invariants: none — the machine's guards carry the model gate and the
// runs assert each face's law directly. TLA+ escalation: none.
module vtraf1 {
  // The wire machine this scenario drives (cross-dprc-links design D7). It is
  // a plain module (no instance params, unlike core/machine), so it is
  // imported unqualified: its actions, phases, residues and end/container
  // constants all become available here.
  import link_lifecycle.* from "../../families/link_lifecycle"

  // ---- face 1: root↔root converge (cross-dprc-links design D10 face 1) ----

  // Both ends in the root, converge the wire to Connected. The frame witness
  // (netns exact-count dpni-statistics oracle) and the saturation smoke are
  // the suite hooks beside the rendered .sh (6.1); this run carries only the
  // converge the hooks stand on.
  run face1RootToRootConvergeTest =
    initAt(rootContainer, rootContainer)
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .expect(and {
        world.phase == Connected,
        world.core.conns.size() == 1,
      })

  // ---- face 2: cross-container populate→connect→bind + post-bind connect ----

  // The fresh partial order on a cross-container pair (root↔child,
  // cross-dprc-links design D4): connect on the unplugged (Populated) container,
  // then bind both ends visible. The disconnect re-exposes the pair so the
  // final connect runs post-bind on already-visible endpoints — the face that
  // records the ENDPOINT_CHANGED → -EPERM-discarded dmesg law board-side.
  run face2CrossContainerTest =
    initPopulatedAt(rootContainer, childA)
      .then(check(world.containerBind == Populated))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(check(world.phase == Connected))
      .then(bindContainerAt)
      .then(check(and {
        world.containerBind == Bound,
        world.core.objs.get(dpniLeft).busVisible,
      }))
      .then(disconnectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .expect(and {
        world.phase == Connected,
        world.containerBind == Bound,
      })

  // ---- face 3: child↔child generality (cross-dprc-links design D10 face 3) ----

  // Two ends in distinct child containers connect just as a root pair does;
  // the connect carries no container gate (cross-dprc-links design D2).
  run face3ChildToChildTest =
    initAt(childA, childB)
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .expect(world.phase == Connected)

  // ---- face 4: create-side heal (cross-dprc-links design D5) ----

  // A post-bind create mints its eager DeferredVisibility obligation; the sole
  // discharge is a consented (Granted) rebind cycle, which makes the node
  // kernel-visible and clears the obligation. The board oracle is the resident
  // reappearing in its container's IOMMU group (the V-DPRC-8 oracle); the model
  // carries the obligation and its consented discharge.
  run face4CreateSideHealTest =
    initPopulatedAt(rootContainer, rootContainer)
      .then(bindContainerAt)
      .then(createEndpointPostBindAt(dpniThird, rootContainer))
      .then(check(world.obligations.contains({ endpoint: dpniThird })))
      .then(rebindDischargeAt(dpniThird, Granted))
      .expect(and {
        not(world.obligations.contains({ endpoint: dpniThird })),
        world.core.objs.get(dpniThird).busVisible,
      })

  // ---- face 5: destroy-mirror (cross-dprc-links design D5) ----

  // Destroying a kernel-visible (bound) end leaves the lazy StaleNode mirror
  // residue — the board face that answers whether a stale node lingers. The
  // end is disconnected at init, so disconnect-before-destroy is satisfied.
  run face5DestroyMirrorTest =
    init
      .then(destroyEndAt(dpniLeft))
      .expect(and {
        world.phase == EndDestroyLegal,
        world.residues.contains(StaleNode(dpniLeft)),
      })

  // ---- face 6: teardown laws (cross-dprc-links design D5/D7) ----

  // Face 6 carries three teardown laws. A Quint .fail() step does not advance
  // state (world' stays unset), so a refusal can only be a run's terminal
  // step — each refusal law therefore rides its own run, the idiom
  // link_lifecycle's own connectAlreadyConnectedRefusedTest already uses.

  // The sanctioned teardown verb: disconnect of the live wire (the only
  // dpni↔dpni teardown, disconnect-only — cross-dprc-links design D3).
  run face6DisconnectLiveTest =
    init
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(disconnectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .expect(world.phase == Disconnected)

  // Disconnect-before-destroy (LINK-I1, cross-dprc-links design D5): destroying
  // a still-connected end is refused — a terminal .fail().
  run face6DisconnectBeforeDestroyRefusedTest =
    init
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(destroyEndAt(dpniLeft).fail())

  // DPRC-I5 double-connect refusal (LINK-I3 cardinality one): connecting an
  // already-connected end is refused — a terminal .fail(). It is a refusal,
  // not a pre-forbidden move (LINK-I4, cross-dprc-links design D2/D7).
  run face6DoubleConnectRefusedTest =
    init
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)).fail())

  // ---- the six faces, end to end (cross-dprc-links design D10) ----

  // One directed walk through the positive transition of each face in
  // cross-dprc-links design D10 order, re-initializing at each face boundary
  // (the machine fixes end containers at init, so the walk re-inits to change
  // topology — the face marker V-TRAF-0 renders as a phase comment). A forward
  // walk cannot contain a disabled action (a .fail() step would be terminal),
  // so face 6's two refusal laws stay on their own runs above; this walk ends
  // on face 6's disconnect-live teardown.
  run vtraf1WalkTest =
    // 1 root↔root converge
    initAt(rootContainer, rootContainer)
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(check(world.phase == Connected))
    // 2 cross-container populate→connect→bind, then post-bind connect
      .then(initPopulatedAt(rootContainer, childA))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(bindContainerAt)
      .then(disconnectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(check(and { world.phase == Connected, world.containerBind == Bound }))
    // 3 child↔child
      .then(initAt(childA, childB))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(check(world.phase == Connected))
    // 4 create-side heal
      .then(initPopulatedAt(rootContainer, rootContainer))
      .then(bindContainerAt)
      .then(createEndpointPostBindAt(dpniThird, rootContainer))
      .then(rebindDischargeAt(dpniThird, Granted))
      .then(check(world.core.objs.get(dpniThird).busVisible))
    // 5 destroy-mirror
      .then(init)
      .then(destroyEndAt(dpniLeft))
      .then(check(world.residues.contains(StaleNode(dpniLeft))))
    // 6 teardown laws (disconnect-live; the two refusals ride their own runs)
      .then(init)
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(disconnectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .expect(world.phase == Disconnected)
}

// ---- the MBT half: per-face frozen --mbt traces (cross-dprc-links task 1.4) ----
//
// One module per cross-dprc-links design D10 face, each a V-TRAF-0-style
// scenario over the core machine (core/machine). The boot population and the
// reference dpmac set are V-TRAF-0's (REF_DPMACS). Each module names its
// wrapper actions EXACTLY as the parse_mbt_trace vocabulary
// (crates/dpaa2-verify/src/board/adapter.rs action()) — createContainer,
// createObject, connectEdge, plug, rescan, kernelBind, unbind, disconnectEdge,
// destroy — with nondet binders of the matching names (parent, c, fam, a, b, o,
// e), so the decoder reads each taken pick. One action is enabled per state
// (guards over m::s, and m::lastVerbs where a core state is revisited), so the
// --mbt run is seed-independent.

// face 1: root↔root converge (cross-dprc-links design D10 face 1), 3 steps.
// Mint two root dpnis, then connect them (connect carries no plug/visibility
// gate — core/connect.canConnect — so the bare converge is legal).
module vtraf1face1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val dpni0: ObjId = { fam: Dpni, num: 0 }
  pure val dpni1: ObjId = { fam: Dpni, num: 1 }
  pure def ep(o: ObjId): Endpoint = { obj: o, port: 0 }

  action init = m::init

  // The two scratch ends, both in the root container, serialized by the dpni
  // counter so dpni.0 then dpni.1 mint in order.
  action createObject = {
    nondet fam = Set(Dpni).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dpni) < 2,
      m::createObjectAt(fam, c),
    }
  }

  // The converge: connect the pair once both exist. Disabled afterwards (an
  // endpoint carries at most one peer), so the trace ends at the connect.
  action connectEdge = {
    nondet a = Set(ep(dpni0)).oneOf()
    nondet b = Set(ep(dpni1)).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 2,
      m::s.conns.size() == 0,
      m::connectEdgeAt(a, b),
    }
  }

  action step = any { createObject, connectEdge }
}

// face 2: cross-container populate→connect→bind + post-bind connect
//   (cross-dprc-links design D10 face 2, cross-dprc-links design D4), 8 steps.
// Child container, a root (kernel) end and a child end, connect on the
// unplugged container (populate→connect), then bind the kernel end visible
// (plug + root rescan), then disconnect and reconnect post-bind. The reconnect
// returns the core state to the post-bind-pre-disconnect one (same objs, same
// conns), so the disconnect arm is distinguished by m::lastVerbs (the rescan's
// "dprc sync") — which the parser ignores.
module vtraf1face2 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val childA: ObjId = { fam: Dprc, num: 2 }
  pure val dpni0: ObjId = { fam: Dpni, num: 0 }   // the root/kernel end
  pure val dpni1: ObjId = { fam: Dpni, num: 1 }   // the child end
  pure def ep(o: ObjId): Endpoint = { obj: o, port: 0 }

  action init = m::init

  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // dpni.0 into the root, dpni.1 into the child — once the child exists.
  action createObject = {
    nondet fam = Set(Dpni).oneOf()
    nondet c = Set(m::dprc1, childA).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 3,
      m::s.nextNum.get(Dpni) < 2,
      if (m::s.nextNum.get(Dpni) == 0) c == m::dprc1 else c == childA,
      m::createObjectAt(fam, c),
    }
  }

  // populate→connect (first, on the unplugged ends) and the post-bind reconnect
  // both run here — both are a legal dpni↔dpni connect of the disconnected pair.
  action connectEdge = {
    nondet a = Set(ep(dpni0)).oneOf()
    nondet b = Set(ep(dpni1)).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 2,
      m::s.conns.size() == 0,
      m::connectEdgeAt(a, b),
    }
  }

  // bind the kernel end: plug it, then a root rescan makes it busVisible.
  // lastVerbs is the first connect's "dprc connect": the final unplug leaves
  // dpni0 unplugged with the wire still up ("dprc assign"), a state this guard
  // must not re-fire on, so the trace ends at the unplug.
  action plug = {
    nondet o = Set(dpni0).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      m::s.conns.size() == 1,
      not(m::s.objs.get(dpni0).plugged),
      m::lastVerbs == Set("dprc connect"),
      m::plugAt(o),
    }
  }

  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      m::s.objs.get(dpni0).plugged,
      not(m::s.objs.get(dpni0).busVisible),
      m::rescanAt(c),
    }
  }

  // tear the wire so the final connect runs post-bind. Enabled only on the
  // first visit to the bound-and-connected state (lastVerbs is the rescan's
  // "dprc sync"); after the reconnect lastVerbs is "dprc connect", so this
  // stays disabled and the trace ends.
  action disconnectEdge = {
    nondet e = Set(ep(dpni0)).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      m::s.objs.get(dpni0).busVisible,
      m::s.conns.size() == 1,
      m::lastVerbs == Set("dprc sync"),
      m::disconnectEdgeAt(e),
    }
  }

  // the face's final step: unplug the bound end. On the board this removes
  // dpni0's bus device, cancelling the deferred kernel probe it would otherwise
  // leave retrying — which rev 1 showed steals face 4's census (bead
  // dpaa2-controlplane-kux.26). Enabled only after the post-bind reconnect
  // (busVisible, wire up, lastVerbs the reconnect's "dprc connect"); unplugT
  // clears plugged so nothing follows.
  action unplug = {
    nondet o = Set(dpni0).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      m::s.objs.get(dpni0).busVisible,
      m::s.conns.size() == 1,
      m::lastVerbs == Set("dprc connect"),
      m::unplugAt(o),
    }
  }

  action step = any {
    createContainer, createObject, connectEdge, plug, rescan, disconnectEdge,
    unplug,
  }
}

// face 3: child↔child generality (cross-dprc-links design D10 face 3), 5 steps.
// Two child containers, one dpni each, connected — nothing privileges the root
// (cross-dprc-links design D2).
module vtraf1face3 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val childA: ObjId = { fam: Dprc, num: 2 }
  pure val childB: ObjId = { fam: Dprc, num: 3 }
  pure val dpni0: ObjId = { fam: Dpni, num: 0 }
  pure val dpni1: ObjId = { fam: Dpni, num: 1 }
  pure def ep(o: ObjId): Endpoint = { obj: o, port: 0 }

  action init = m::init

  // Two children of the root, in counter order (dprc.2 then dprc.3).
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) < 4,
      m::createContainerAt(parent),
    }
  }

  // One dpni into each child.
  action createObject = {
    nondet fam = Set(Dpni).oneOf()
    nondet c = Set(childA, childB).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 4,
      m::s.nextNum.get(Dpni) < 2,
      if (m::s.nextNum.get(Dpni) == 0) c == childA else c == childB,
      m::createObjectAt(fam, c),
    }
  }

  action connectEdge = {
    nondet a = Set(ep(dpni0)).oneOf()
    nondet b = Set(ep(dpni1)).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 2,
      m::s.conns.size() == 0,
      m::connectEdgeAt(a, b),
    }
  }

  action step = any { createContainer, createObject, connectEdge }
}

// face 4: create-side heal (cross-dprc-links design D10 face 4,
//   cross-dprc-links design D5), 14 steps.
// Bind a kernel end's container surface (its census companions, then plug +
// rescan + kernelBind), createObject a third dpni post-bind (kernel-invisible),
// then the consented rebind cycle as explicit core verbs — unbind → kernelBind
// → rescan — which re-observes the third end busVisible (the Granted discharge).
// kernelBind draws one dpmcp/dpbp/dpcon from the container (dpni census,
// families/dpni.qnt), so the companions are minted and plugged first.
// The rebind revisits the "dpni.0 bound, third still invisible" core state
// (once right after the create, once after the rebind); m::lastVerbs tells them
// apart (the create's "dpni create" vs the rebind kernelBind's empty verb set).
module vtraf1face4 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val dpmcp0: ObjId = { fam: Dpmcp, num: 0 }
  pure val dpbp0: ObjId = { fam: Dpbp, num: 0 }
  pure val dpcon0: ObjId = { fam: Dpcon, num: 0 }
  pure val dpni0: ObjId = { fam: Dpni, num: 0 }   // the bound container surface
  pure val dpni1: ObjId = { fam: Dpni, num: 1 }   // the post-bind third end

  action init = m::init

  // The census companions, then the kernel end — all in the root, serialized by
  // the per-family counters (V-TRAF-0's create chaining). dpni.0 mints after the
  // three companions; dpni.1 (the third end) mints only post-bind (dpni.0 bound).
  action createObject = {
    nondet fam = Set(Dpmcp, Dpbp, Dpcon, Dpni).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      if (fam == Dpmcp) m::s.nextNum.get(Dpmcp) == 0
      else if (fam == Dpbp) and {
        m::s.nextNum.get(Dpmcp) == 1,
        m::s.nextNum.get(Dpbp) == 0,
      } else if (fam == Dpcon) and {
        m::s.nextNum.get(Dpbp) == 1,
        m::s.nextNum.get(Dpcon) == 0,
      } else if (m::s.nextNum.get(Dpni) == 0) m::s.nextNum.get(Dpcon) == 1
      else and {
        m::s.nextNum.get(Dpni) == 1,
        m::s.objs.keys().contains(dpni0),
        m::s.objs.get(dpni0).bind == BoundKernel,
      },
      m::createObjectAt(fam, c),
    }
  }

  // plug the companions (so they enter the pool) then the kernel end, in
  // creation order.
  action plug = {
    nondet o = Set(dpmcp0, dpbp0, dpcon0, dpni0).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      not(m::s.objs.get(o).plugged),
      if (o == dpmcp0) true
      else if (o == dpbp0) m::s.objs.get(dpmcp0).plugged
      else if (o == dpcon0) m::s.objs.get(dpbp0).plugged
      else m::s.objs.get(dpcon0).plugged,
      m::plugAt(o),
    }
  }

  // the root rescan: first to make the kernel end busVisible (so kernelBind may
  // attempt it), last to re-observe the third end busVisible after the rebind.
  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      or {
        and {
          m::s.objs.keys().contains(dpni0),
          m::s.objs.get(dpni0).plugged,
          not(m::s.objs.get(dpni0).busVisible),
        },
        and {
          m::s.objs.keys().contains(dpni1),
          not(m::s.objs.get(dpni1).busVisible),
          m::s.objs.get(dpni0).bind == BoundKernel,
          m::lastVerbs == Set(),
        },
      },
      m::rescanAt(c),
    }
  }

  // the kernel probe: binds the end and draws its 1/1/1 census. Runs twice — to
  // bind the surface, and again as the rebind cycle's re-bind.
  action kernelBind = {
    nondet o = Set(dpni0).oneOf()
    nondet mcp = Set(dpmcp0).oneOf()
    nondet bp = Set(dpbp0).oneOf()
    nondet con = Set(dpcon0).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      m::s.objs.get(dpni0).busVisible,
      m::s.objs.get(dpni0).bind == Unbound,
      m::kernelBindAt(o, Set(mcp), Set(bp), Set(con)),
    }
  }

  // the rebind cycle's unbind — only on the first visit to the bound surface
  // with the third end present (lastVerbs is the create's "dpni create"); after
  // the re-bind lastVerbs is empty, so this stays disabled.
  action unbind = {
    nondet o = Set(dpni0).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      m::s.objs.get(dpni0).bind == BoundKernel,
      m::s.nextNum.get(Dpni) == 2,
      m::lastVerbs == Set("dpni create"),
      m::unbindAt(o),
    }
  }

  action step = any { createObject, plug, rescan, kernelBind, unbind }
}

// face 5: destroy-mirror (cross-dprc-links design D10 face 5,
//   cross-dprc-links design D5), 4 steps.
// A root dpni minted, plugged and made busVisible (the container-bound surface
// the World typestate calls Bound — plugged + visible, bind untouched), never
// connected, is destroyed. busVisible is what mints the lazy StaleNode mirror
// at the World level; destroy of an unbound, undrawn, disconnected end in the
// root is accepted (core/machine.destroyAt). No PHY dpmac, no edge.
module vtraf1face5 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val dpni0: ObjId = { fam: Dpni, num: 0 }

  action init = m::init

  action createObject = {
    nondet fam = Set(Dpni).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 0,
      m::createObjectAt(fam, c),
    }
  }

  action plug = {
    nondet o = Set(dpni0).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      not(m::s.objs.get(dpni0).plugged),
      m::plugAt(o),
    }
  }

  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      m::s.objs.get(dpni0).plugged,
      not(m::s.objs.get(dpni0).busVisible),
      m::rescanAt(c),
    }
  }

  action destroy = {
    nondet o = Set(dpni0).oneOf()
    all {
      m::s.objs.keys().contains(dpni0),
      m::s.objs.get(dpni0).busVisible,
      m::destroyAt(o),
    }
  }

  action step = any { createObject, plug, rescan, destroy }
}

// face 6: teardown laws — the positive disconnect-live walk
//   (cross-dprc-links design D10 face 6), 4 steps. The two refusals ride the
// directed runs in module vtraf1. Mint two root ends, connect, then disconnect
// the live wire. The disconnect returns conns to empty, the same as right after
// both ends were minted; m::lastVerbs ("dpni create" before the first connect
// vs "dprc disconnect" after) keeps the connect from re-firing, so the trace
// ends at the disconnect.
module vtraf1face6 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val dpni0: ObjId = { fam: Dpni, num: 0 }
  pure val dpni1: ObjId = { fam: Dpni, num: 1 }
  pure def ep(o: ObjId): Endpoint = { obj: o, port: 0 }

  action init = m::init

  action createObject = {
    nondet fam = Set(Dpni).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dpni) < 2,
      m::createObjectAt(fam, c),
    }
  }

  action connectEdge = {
    nondet a = Set(ep(dpni0)).oneOf()
    nondet b = Set(ep(dpni1)).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 2,
      m::s.conns.size() == 0,
      m::lastVerbs == Set("dpni create"),
      m::connectEdgeAt(a, b),
    }
  }

  action disconnectEdge = {
    nondet e = Set(ep(dpni0)).oneOf()
    all {
      m::s.conns.size() == 1,
      m::disconnectEdgeAt(e),
    }
  }

  action step = any { createObject, connectEdge, disconnectEdge }
}
