// The dpni↔dpni wire typestate (cross-dprc-links design D2/D3/D7). The
// isomorphic twin (ADR-0002 §3, quint-is-the-spec) of the Rust link
// typestates parcel 1b grows, on the dpseci_lifecycle / dpmac_lifecycle
// reference idiom: a focused module carrying the spec-level phase as a
// proper sum, its guards expressed by the shared connection operators of
// core/connect.qnt — never duplicated here.
//
// The wire moves through four phases (cross-dprc-links design D7): ends-exist → connected
// → disconnected → end-destroy-legal. dpni↔dpni is root-issued connect
// with disconnect-only teardown — no driver handback by construction, so
// connect.edgeDemandsSeveredWitness stays false for it and no
// severed-witness machinery lives here (cross-dprc-links design D3). The connect is issued
// at a common ancestor (root today, the CONNECT_ANCESTOR of cross-dprc-links design D2);
// core/connect's canConnect carries no ancestor gate (always satisfiable
// via the root), so the wire is container-agnostic: root↔root, root↔child
// and child↔child are all representable, nothing pre-forbidden beyond what
// the core operators refuse.
//
// Parcel 1b also grows the container interplay of cross-dprc-links design D4/D5: the
// populate → connect → bind partial order (connect legal on the unplugged
// container, extending ADR-0017 decision 3; post-bind connect additionally
// legal for already-visible endpoints), the eager DeferredVisibility obligation
// a post-bind create must carry, the consented rebind cycle as the sole
// discharge (declined consent → a typed standing residue), and the lazy
// stale-node mirror residue a post-bind destroy leaves. No reboot transition
// exists on this surface (cross-dprc-links design D5).
//
// Everything below traces to docs/baseline/object-model.md §2 (the
// connect edge, cardinality one, disconnect-before-reconnect), ADR-0019
// (the edge facet), and cross-dprc-links design D2/D3/D4/D5/D7.
//
// Named invariants — the LINK-I* set (cross-dprc-links design D7; dispositions
// in models/COVERAGE.md, ledger rows land with cross-dprc-links task 1.5):
//   LINK_I1 disconnect-before-destroy — state face below (no edge outlives its
//     endpoints), guard witness destroyConnectedEndRefusedTest;
//   LINK_I2 no post-bind create without an attached obligation / planned
//     discharge — state face below, witness postBindCreateObligatedTest;
//   LINK_I3 cardinality one / disconnect-before-reconnect — state face below
//     (DPRC-I5 promoted from candidate), guard witnesses
//     connectAlreadyConnectedRefusedTest / reconnectAfterDisconnectTest;
//   LINK_I4 MC-refused patterns surface as refusals — action-guard property
//     only (the .fail() runs), no state face, so no stateInvariants conjunct.
// Apalache-marked subset: the stateInvariants conjunction (LINK_I1/I2/I3) —
//   all three are state-expressible over World or finite-quantified endpoints,
//   and verify at bounded depth 1 (mirrors dpmac_lifecycle / dpni_lifecycle).
module link_lifecycle {
  // The shared types and the connection surface this module consumes: the
  // dpni↔dpni edge legality (legalPair), the connect gate (canConnect), the
  // cardinality-one endpoint predicate (connected) — the core operators are
  // the guards, never re-stated here (cross-dprc-links design D3/D7).
  import types.* from "../core/types"
  import connect.* from "../core/connect"
  // canConnect reads the per-family endpoint-port count; ALL_PARAMS is the
  // corpus-wide table the rest of the model already uses (object-model.md §2).
  import params.ALL_PARAMS from "./params"

  // ---- the wire typestate (cross-dprc-links design D7) ----

  // The four phases a dpni↔dpni wire moves through, as a proper sum — the
  // phase is the specification, no boolean/sentinel encoding (ADR-0002 §3).
  type WirePhase =
    | EndsExist        // both dpni ends exist in their containers, no edge
    | Connected        // the dprc-connect edge stands (canConnect was met)
    | Disconnected     // the edge was torn; both ends still exist
    | EndDestroyLegal  // a disconnected end was destroyed — teardown terminal

  // The D4 fresh partial order on the container typestate (cross-dprc-links design D4):
  // populate (ends exist, container unplugged — the connect face lives here,
  // extending ADR-0017 decision 3) → bind (container plugged, ends kernel-
  // visible). Post-bind connect/disconnect of already-visible endpoints stays
  // legal, so the order is partial, not total. A proper sum — the typestate is
  // the specification (ADR-0002 §3).
  type ContainerBind =
    | Populated   // ends exist, container unplugged: connect is legal here
    | Bound       // container plugged, ends kernel-visible

  // The eager ADR-0017 drift obligation the create side carries (cross-dprc-links design D5):
  // a post-bind create is MC-accepted but kernel-invisible until a scan re-
  // observes it. Minted at create, never optional — the only post-bind create
  // action attaches it, so an unobligated post-bind create is unrepresentable,
  // not merely refused.
  type DeferredVisibility = { endpoint: ObjId }

  // Explicit consent on the rebind discharge (cross-dprc-links design D5; the ADR-0015
  // consent machinery is parcel 3's Rust concern — here consent is an action
  // parameter). Granted runs the Disruptive-class rebind; Declined leaves a
  // standing residue.
  type Consent = Granted | Declined

  // The typed standing residues this surface may carry (cross-dprc-links design D5,
  // pool-objects honest-residue idiom). Neither blocks anything; both are
  // observed, stand indefinitely, and are never a teardown. No reboot residue
  // lives here — reboot residue stays exclusive to ADR-0020 pool shrink.
  type WireResidue =
    | StaleNode(ObjId)           // destroy side: a kernel node lingering after MC destroy (lazy mirror)
    | DeclinedVisibility(ObjId)  // create side: a DeferredVisibility whose rebind consent was declined

  // The substrate the core/connect operators read: the live CoreState
  // carrying the two dpni ends and the edge set, alongside the spec-level
  // phase the typestate advances. The operators are pure CoreState readers,
  // so the machine threads a real CoreState rather than re-deriving them.
  // The container interplay rides alongside (cross-dprc-links design D4/D5): the
  // bind facet of the partial order and the two obligation/residue carriers.
  type World = {
    core: CoreState,
    phase: WirePhase,
    containerBind: ContainerBind,
    obligations: Set[DeferredVisibility],
    residues: Set[WireResidue],
  }

  var world: World

  // ---- the two dpni ends and the containers they may sit in ----

  // The root container is the common ancestor connect is issued at today
  // (cross-dprc-links design D2, CONNECT_ANCESTOR); the two child containers let the directed
  // runs place the ends across containers (root↔child, child↔child).
  pure val rootContainer: ObjId = { fam: Dprc, num: 1 }
  pure val childA: ObjId = { fam: Dprc, num: 2 }
  pure val childB: ObjId = { fam: Dprc, num: 3 }

  pure val dpniLeft: ObjId = { fam: Dpni, num: 0 }
  pure val dpniRight: ObjId = { fam: Dpni, num: 1 }
  // A third end the post-bind create mints (cross-dprc-links design D5): the endpoint
  // the obligation names, created after the container is already Bound.
  pure val dpniThird: ObjId = { fam: Dpni, num: 2 }

  // A dpni faces its single endpoint port 0 (dpniParams.endpointPorts == 1,
  // object-model.md §2 type.id.port).
  pure def portZero(o: ObjId): Endpoint = { obj: o, port: 0 }

  // ---- the minimal CoreState the connect operators read ----

  // A plain existing dpni end in a container: the minimal ObjState
  // validEndpoint needs (its key, a valid container) and nothing else. The
  // bootObj shape of core/machine.qnt, built locally — it is not a connect
  // operator, so it is not imported (and core/ is untouched, cross-dprc-links design D7).
  pure def existingEnd(container: ObjId): ObjState = {
    parent: Some(container), owner: Some(container),
    plugged: true, busVisible: true, bind: Unbound,
    enabled: false, pristine: true, linkUp: false,
    allocatedBy: None, hwId: 0,
  }

  // A populated-but-unbound end (cross-dprc-links design D4): the object exists in
  // its container, unplugged and kernel-invisible — the populate face the fresh
  // partial order's connect lives on, before bind plugs it.
  pure def populatedEnd(container: ObjId): ObjState =
    existingEnd(container).with("plugged", false).with("busVisible", false)

  // A post-bind create's end (cross-dprc-links design D5): MC-accepted (plugged at the
  // MC) but kernel-invisible until a scan — the DeferredVisibility state.
  pure def mcAcceptedInvisibleEnd(container: ObjId): ObjState =
    existingEnd(container).with("busVisible", false)

  pure def containerState(id: int): ContainerState =
    { icid: id, perms: Set(Spawn, Alloc, ObjCreate, IrqCfg, TopologyChanges),
      locked: false }

  // Both ends existing, each in its given container, no edge yet — the
  // ends-exist starting substrate. dprcs/nextNum/nextHw are populated to a
  // complete, valid CoreState; the connect operators read only objs/conns.
  pure def bothEndsExist(leftContainer: ObjId, rightContainer: ObjId): CoreState = {
    objs: Map(dpniLeft -> existingEnd(leftContainer),
              dpniRight -> existingEnd(rightContainer)),
    dprcs: Set(rootContainer, childA, childB).mapBy(c => containerState(c.num)),
    conns: Set(),
    nextNum: FAMILIES.mapBy(fam => 0),
    nextHw: 0,
  }

  // Both ends present but unbound — the populate face of the fresh partial order
  // (cross-dprc-links design D4). Same substrate as bothEndsExist but the ends unplugged
  // and kernel-invisible until bind.
  pure def bothEndsPopulated(leftContainer: ObjId, rightContainer: ObjId): CoreState =
    bothEndsExist(leftContainer, rightContainer)
      .with("objs", Map(dpniLeft -> populatedEnd(leftContainer),
                        dpniRight -> populatedEnd(rightContainer)))

  // ---- init ----

  // Start with both ends existing in the given containers; the canonical
  // init places them both in the root (root↔root).
  // The ends already exist bound (plugged, kernel-visible): the post-bind face,
  // where connect of already-visible endpoints is the legal starting move
  // (cross-dprc-links design D4). No obligation or residue stands at init.
  action initAt(leftContainer: ObjId, rightContainer: ObjId): bool =
    world' = { core: bothEndsExist(leftContainer, rightContainer),
               phase: EndsExist, containerBind: Bound,
               obligations: Set(), residues: Set() }

  action init = initAt(rootContainer, rootContainer)

  // The populate face (cross-dprc-links design D4): ends exist unbound, the container
  // unplugged — the fresh-convergence starting point that connect lives on
  // before bind.
  action initPopulatedAt(leftContainer: ObjId, rightContainer: ObjId): bool =
    world' = { core: bothEndsPopulated(leftContainer, rightContainer),
               phase: EndsExist, containerBind: Populated,
               obligations: Set(), residues: Set() }

  // ---- transitions (guards via core/connect operators) ----

  // connect: the dprc connect issued at the common ancestor. Admitted
  // exactly when core/connect.canConnect is met — a legal dpni↔dpni pair,
  // both ends valid and currently disconnected. Cardinality one lives in
  // canConnect (its not(connected) clauses), so connecting an already-
  // connected end is refused by the core operator, not a local guard
  // (object-model.md §2, DPNI-I9).
  action connectWireAt(a: Endpoint, b: Endpoint): bool = all {
    canConnect(world.core, ALL_PARAMS, a, b),
    world' = world
      .with("core", world.core.with("conns", world.core.conns.union(Set((a, b)))))
      .with("phase", Connected),
  }

  // disconnect: tear the edge — the only teardown verb dpni↔dpni carries
  // (disconnect-only, cross-dprc-links design D3). Guarded on both ends actually being
  // connected (core/connect.connected). The ends remain; the phase becomes
  // Disconnected, so a reconnect is legal again (disconnect-before-reconnect).
  action disconnectWireAt(a: Endpoint, b: Endpoint): bool = all {
    connected(world.core, a),
    connected(world.core, b),
    world' = world
      .with("core", world.core.with("conns", world.core.conns.exclude(Set((a, b)))))
      .with("phase", Disconnected),
  }

  // end-destroy: destroy one dpni end. Legal only once the end is no longer
  // connected — disconnect-before-destroy (cross-dprc-links design D5, finding 34
  // generalized), expressed by the core/connect.connected guard: destroying
  // a still-connected end is refused. Removing the end advances the wire to
  // its teardown-terminal phase.
  // A destroy of a kernel-visible (bound) end leaves a lazy stale-node mirror
  // residue (cross-dprc-links design D5): the kernel may still list the node. It blocks
  // nothing and stands indefinitely — no action discharges it. Destroying a
  // still-populated (invisible) end mints none.
  action destroyEndAt(o: ObjId): bool = all {
    world.core.objs.keys().contains(o),
    not(connected(world.core, portZero(o))),
    world' = world
      .with("core", world.core.with("objs",
        world.core.objs.keys().exclude(Set(o))
          .mapBy(k => world.core.objs.get(k))))
      .with("phase", EndDestroyLegal)
      .with("residues",
        if (world.core.objs.get(o).busVisible)
          world.residues.union(Set(StaleNode(o)))
        else world.residues),
  }

  // bind: plug the container, so both ends become kernel-visible — the third
  // step of the fresh partial order (cross-dprc-links design D4). Legal from the Populated
  // face; connect may already have run (connect-then-bind) or not (bind-then-
  // connect of the now-visible ends is the additionally-legal post-bind face).
  action bindContainerAt: bool = all {
    world.containerBind == Populated,
    world' = world
      .with("containerBind", Bound)
      .with("core", world.core.with("objs",
        world.core.objs.keys().mapBy(k =>
          world.core.objs.get(k).with("plugged", true).with("busVisible", true)))),
  }

  // post-bind create: an end created after the container is Bound is MC-accepted
  // but kernel-invisible, and ALWAYS carries an eager DeferredVisibility
  // obligation (cross-dprc-links design D5). This is the only action that creates an end
  // post-bind, and it mints the obligation by construction — an unobligated
  // post-bind create is unrepresentable.
  action createEndpointPostBindAt(o: ObjId, container: ObjId): bool = all {
    world.containerBind == Bound,
    not(world.core.objs.keys().contains(o)),
    world' = world
      .with("core", world.core.with("objs",
        world.core.objs.put(o, mcAcceptedInvisibleEnd(container))))
      .with("obligations", world.obligations.union(Set({ endpoint: o }))),
  }

  // rebind discharge: the only way a DeferredVisibility obligation clears
  // (cross-dprc-links design D5) — a consented rebind cycle (unbind → bind → re-observe),
  // a Disruptive-class transition. Granted makes the node visible and drops the
  // obligation. Declined performs no rebind (no silent rebind): the obligation
  // becomes a typed DeclinedVisibility residue that stands, the node staying
  // kernel-invisible (pool-objects honest-residue idiom).
  action rebindDischargeAt(o: ObjId, consent: Consent): bool = all {
    world.containerBind == Bound,
    world.obligations.contains({ endpoint: o }),
    world.core.objs.keys().contains(o),
    if (consent == Granted)
      world' = world
        .with("obligations", world.obligations.exclude(Set({ endpoint: o })))
        .with("core", world.core.with("objs",
          world.core.objs.put(o, world.core.objs.get(o).with("busVisible", true))))
    else
      world' = world
        .with("obligations", world.obligations.exclude(Set({ endpoint: o })))
        .with("residues", world.residues.union(Set(DeclinedVisibility(o)))),
  }

  // ---- nondet wrappers (random simulation) ----

  // The finite end alphabet the random sweep draws from: the two boot ends and
  // the post-bind-create third. Every action's guard rejects an ineligible draw
  // (a missing end, an already-connected pair), so the step never errors — the
  // same guards the directed runs pin (models/README.md two-forms rule).
  pure val ENDS: Set[ObjId] = Set(dpniLeft, dpniRight, dpniThird)

  action doConnect = {
    nondet a = ENDS.oneOf()
    nondet b = ENDS.oneOf()
    connectWireAt(portZero(a), portZero(b))
  }
  action doDisconnect = {
    nondet a = ENDS.oneOf()
    nondet b = ENDS.oneOf()
    disconnectWireAt(portZero(a), portZero(b))
  }
  action doDestroy = {
    nondet o = ENDS.oneOf()
    destroyEndAt(o)
  }
  action doCreatePostBind = {
    nondet o = ENDS.oneOf()
    createEndpointPostBindAt(o, rootContainer)
  }
  action doRebindDischarge = {
    nondet o = ENDS.oneOf()
    nondet c = Set(Granted, Declined).oneOf()
    rebindDischargeAt(o, c)
  }

  action stutter = world' = world

  // The canonical init is Bound root↔root, so bindContainerAt's Populated guard
  // never fires from here — the populate face is swept by the directed runs
  // (freshPartialOrderTest, …); random simulation drives the Bound surface.
  action step = any {
    doConnect,
    doDisconnect,
    doDestroy,
    doCreatePostBind,
    doRebindDischarge,
    bindContainerAt,
    stutter,
  }

  // ---- named invariants (cross-dprc-links design D7): the LINK-I* set ----
  //
  // Four laws, each on its honest rung (COVERAGE.md ledger rows land with
  // cross-dprc-links task 1.5, not here):
  //   LINK_I1 state invariant below + guard witness destroyConnectedEndRefusedTest
  //   LINK_I2 state invariant below + witness postBindCreateObligatedTest
  //   LINK_I3 state invariant below (the DPRC-I5 state face) + guard witnesses
  //           connectAlreadyConnectedRefusedTest / reconnectAfterDisconnectTest
  //   LINK_I4 action-guard property only — the refusal runs; no World predicate,
  //           so it is not a stateInvariants conjunct.

  // LINK_I1 — disconnect-before-destroy (cross-dprc-links design D5, finding 34
  // generalized). State face: no edge outlives its endpoints — every connected
  // endpoint's object is still present. The bad combination (an edge to a
  // destroyed end) is representable but unreachable, since destroyEndAt's
  // not(connected) guard forbids destroying a still-connected end — the
  // ordering-as-state-invariant idiom of dpmac's DPMAC_SeverOrder. The guard
  // itself is witnessed by destroyConnectedEndRefusedTest.
  // Board (V-TRAF-1, 2026-10-05): the refusal is FALSIFIED at the guard level —
  // the MC accepts dpni destroy of a still-connected dpni<->dpni root end. This
  // state face is unaffected (the edge's fate on endpoint destroy was
  // unobserved, the probe quarantined), so LINK_I1 stays carried, and
  // disconnect-before-destroy stays the engine's typestate policy, stricter
  // than hardware. Model weakening lands at cross-dprc-links task 7.5.
  val LINK_I1: bool =
    world.core.conns.forall(c =>
      world.core.objs.keys().contains(c._1.obj)
        and world.core.objs.keys().contains(c._2.obj))

  // LINK_I2 — no post-bind create without an attached obligation / planned
  // discharge (cross-dprc-links design D5). State face: a Bound end that is
  // kernel-invisible is always either named by a standing DeferredVisibility
  // obligation or carried as a DeclinedVisibility residue — an unobligated
  // invisible bound end is unreachable, because createEndpointPostBindAt mints
  // the obligation by construction and the rebind discharge either makes the
  // end visible (Granted) or leaves the typed residue (Declined). Witnessed by
  // postBindCreateObligatedTest.
  val LINK_I2: bool =
    world.core.objs.keys().forall(o =>
      (world.containerBind == Bound and not(world.core.objs.get(o).busVisible))
        implies (world.obligations.contains({ endpoint: o })
                 or world.residues.contains(DeclinedVisibility(o))))

  // LINK_I3 — cardinality one (object-model.md §2, DPNI-I9; DPRC-I5 promoted
  // from candidate to a named invariant by this change). State face: every
  // endpoint sits in at most one edge, so no two peers claim the same end.
  // canConnect's not(connected) clauses are what hold it, so the
  // disconnect-before-reconnect ordering follows — a reconnect is legal only
  // after a disconnect. The guard faces are witnessed by
  // connectAlreadyConnectedRefusedTest (the DPRC-I5 double-connect refusal) and
  // reconnectAfterDisconnectTest.
  val LINK_I3: bool =
    endpointsOf(world.core).forall(e =>
      world.core.conns.filter(c => c._1 == e or c._2 == e).size() <= 1)

  // LINK_I4 — MC-refused patterns surface as typed refusals, never pre-forbidden
  // (cross-dprc-links design D2/D7). This is an action-guard property, not a
  // World predicate: the machine admits the refusable moves and lets the
  // core/connect guards reject them, witnessed by the .fail() runs
  // (connectAlreadyConnectedRefusedTest, destroyConnectedEndRefusedTest). It
  // carries no stateInvariants conjunct — there is no reachable state to
  // quantify over.

  // The simulator/Apalache conjunction (pnpm model:simulate passes
  // --invariant=stateInvariants): the three state-expressible invariants.
  // LINK_I4 is guard-only and stays a directed run, not a conjunct.
  val stateInvariants: bool = and {
    LINK_I1,
    LINK_I2,
    LINK_I3,
  }

  // A stutter-check step for directed runs (holds the state and checks p).
  action check(p: bool): bool = all { p, world' = world }

  // ---- directed runs (rung 2a): the canonical lifecycle and the refusals ----

  // dpni↔dpni is the legal pair the module consumes, and it carries no
  // severed-witness demand (cross-dprc-links design D3) — unlike dpni↔dpmac.
  run dpniPairLegalTest =
    init.then(check(and {
      legalPair(Dpni, Dpni),
      not(edgeDemandsSeveredWitness(Dpni, Dpni)),
    }))

  // The canonical wire lifecycle (root↔root): ends-exist → connect →
  // connected → disconnect → disconnected → destroy an end → end-destroy-legal.
  run wireLifecycleTest =
    init
      .then(check(world.phase == EndsExist))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(check(and {
        world.phase == Connected,
        connected(world.core, portZero(dpniLeft)),
        connected(world.core, portZero(dpniRight)),
      }))
      .then(disconnectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(check(and {
        world.phase == Disconnected,
        not(connected(world.core, portZero(dpniLeft))),
        not(connected(world.core, portZero(dpniRight))),
      }))
      .then(destroyEndAt(dpniLeft))
      .expect(and {
        world.phase == EndDestroyLegal,
        not(world.core.objs.keys().contains(dpniLeft)),
      })

  // Cardinality one: connecting an already-connected end is refused — the
  // refusal is canConnect's not(connected) clause, not a local guard
  // (object-model.md §2, DPNI-I9).
  run connectAlreadyConnectedRefusedTest =
    init
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)).fail())

  // disconnect-before-reconnect: once the edge is torn, canConnect is met
  // again and the same ends reconnect.
  run reconnectAfterDisconnectTest =
    init
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(disconnectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .expect(world.phase == Connected)

  // disconnect-before-destroy (cross-dprc-links design D5): destroying a still-connected end
  // is refused — end-destroy-legal is reachable only from a disconnected end.
  // Board (V-TRAF-1, 2026-10-05): this guard is board-falsified — the MC accepts
  // the still-connected destroy; the run stays as the engine's POLICY witness,
  // stricter than hardware. Model weakening lands at cross-dprc-links task 7.5.
  run destroyConnectedEndRefusedTest =
    init
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(destroyEndAt(dpniLeft).fail())

  // ---- container-agnostic representability (cross-dprc-links design D2) ----

  // root↔child: a root end wired to a child-container end connects — the
  // connect carries no container gate (canConnect, cross-container verified).
  run crossContainerWireTest =
    initAt(rootContainer, childA)
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .expect(world.phase == Connected)

  // child↔child: two ends in different child containers connect just as well
  // — nothing privileges the root but the ancestor the connect is issued at.
  run childToChildWireTest =
    initAt(childA, childB)
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .expect(world.phase == Connected)

  // ---- the container interplay (cross-dprc-links design D4/D5) ----

  // The fresh partial order populate → connect → bind (cross-dprc-links design D4): connect
  // is legal on the unplugged (Populated) container — the ends stay kernel-
  // invisible when the edge stands — and bind plugs them visible afterwards.
  run freshPartialOrderTest =
    initPopulatedAt(rootContainer, rootContainer)
      .then(check(and {
        world.containerBind == Populated,
        not(world.core.objs.get(dpniLeft).busVisible),
      }))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .then(check(and {
        world.phase == Connected,
        world.containerBind == Populated,   // connected while still unplugged
        not(world.core.objs.get(dpniLeft).busVisible),
      }))
      .then(bindContainerAt)
      .expect(and {
        world.containerBind == Bound,
        world.core.objs.get(dpniLeft).busVisible,
        world.core.objs.get(dpniRight).busVisible,
      })

  // Bind-before-connect stays legal for already-visible endpoints post-bind
  // (cross-dprc-links design D4): the order is partial, not total — binding first then
  // connecting the now-visible ends is admitted.
  run bindBeforeConnectTest =
    initPopulatedAt(rootContainer, rootContainer)
      .then(bindContainerAt)
      .then(check(world.containerBind == Bound))
      .then(connectWireAt(portZero(dpniLeft), portZero(dpniRight)))
      .expect(and {
        world.phase == Connected,
        world.core.objs.get(dpniLeft).busVisible,
      })

  // A post-bind create carries its DeferredVisibility obligation by construction
  // (cross-dprc-links design D5): the created end is MC-present but kernel-invisible, and
  // the obligation naming it stands — no post-bind create exists without one.
  run postBindCreateObligatedTest =
    initPopulatedAt(rootContainer, rootContainer)
      .then(bindContainerAt)
      .then(createEndpointPostBindAt(dpniThird, rootContainer))
      .expect(and {
        world.obligations.contains({ endpoint: dpniThird }),
        world.core.objs.keys().contains(dpniThird),
        not(world.core.objs.get(dpniThird).busVisible),
      })

  // The consented rebind is the sole discharge (cross-dprc-links design D5): granted consent
  // runs the Disruptive-class unbind → bind → re-observe cycle, clearing the
  // obligation and making the node visible, with no residue left standing.
  run consentedRebindDischargesTest =
    initPopulatedAt(rootContainer, rootContainer)
      .then(bindContainerAt)
      .then(createEndpointPostBindAt(dpniThird, rootContainer))
      .then(rebindDischargeAt(dpniThird, Granted))
      .expect(and {
        not(world.obligations.contains({ endpoint: dpniThird })),
        world.core.objs.get(dpniThird).busVisible,
        world.residues == Set(),
      })

  // Declined consent leaves a typed standing residue (cross-dprc-links design D5): no silent
  // rebind — the obligation becomes a DeclinedVisibility that stands indefinitely,
  // the node staying kernel-invisible, and no step discharges it.
  run declinedConsentResidueTest =
    initPopulatedAt(rootContainer, rootContainer)
      .then(bindContainerAt)
      .then(createEndpointPostBindAt(dpniThird, rootContainer))
      .then(rebindDischargeAt(dpniThird, Declined))
      .expect(and {
        not(world.obligations.contains({ endpoint: dpniThird })),
        world.residues.contains(DeclinedVisibility(dpniThird)),
        not(world.core.objs.get(dpniThird).busVisible),
      })
      .then(check(world.residues.contains(DeclinedVisibility(dpniThird))))

  // A post-bind destroy leaves the lazy stale-node mirror residue (cross-dprc-links design D5):
  // destroying a kernel-visible end mints a StaleNode that blocks nothing — a
  // fresh post-bind create still proceeds afterwards — and needs no discharge.
  run postBindDestroyStaleNodeTest =
    init
      .then(destroyEndAt(dpniLeft))
      .expect(and {
        world.phase == EndDestroyLegal,
        not(world.core.objs.keys().contains(dpniLeft)),
        world.residues.contains(StaleNode(dpniLeft)),
      })
      .then(createEndpointPostBindAt(dpniThird, rootContainer))
      .expect(and {
        world.obligations.contains({ endpoint: dpniThird }),
        world.residues.contains(StaleNode(dpniLeft)),   // the stale node still stands
      })
}
