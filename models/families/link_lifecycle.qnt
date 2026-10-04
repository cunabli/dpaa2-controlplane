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
// Everything below traces to docs/baseline/object-model.md §2 (the
// connect edge, cardinality one, disconnect-before-reconnect), ADR-0019
// (the edge facet), and cross-dprc-links design D2/D3/D5/D7.
//
// Named invariants: none here — the LINK-I* invariants, the stateInvariants
// conjunction and the Apalache marks land with parcel 1c (cross-dprc-links design D7); this
// parcel is the typestate machine and its directed canonical-trace runs.
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

  // The substrate the core/connect operators read: the live CoreState
  // carrying the two dpni ends and the edge set, alongside the spec-level
  // phase the typestate advances. The operators are pure CoreState readers,
  // so the machine threads a real CoreState rather than re-deriving them.
  type World = { core: CoreState, phase: WirePhase }

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

  // ---- init ----

  // Start with both ends existing in the given containers; the canonical
  // init places them both in the root (root↔root).
  action initAt(leftContainer: ObjId, rightContainer: ObjId): bool =
    world' = { core: bothEndsExist(leftContainer, rightContainer),
               phase: EndsExist }

  action init = initAt(rootContainer, rootContainer)

  // ---- transitions (guards via core/connect operators) ----

  // connect: the dprc connect issued at the common ancestor. Admitted
  // exactly when core/connect.canConnect is met — a legal dpni↔dpni pair,
  // both ends valid and currently disconnected. Cardinality one lives in
  // canConnect (its not(connected) clauses), so connecting an already-
  // connected end is refused by the core operator, not a local guard
  // (object-model.md §2, DPNI-I9).
  action connectWireAt(a: Endpoint, b: Endpoint): bool = all {
    canConnect(world.core, ALL_PARAMS, a, b),
    world' = {
      core: world.core.with("conns", world.core.conns.union(Set((a, b)))),
      phase: Connected,
    },
  }

  // disconnect: tear the edge — the only teardown verb dpni↔dpni carries
  // (disconnect-only, cross-dprc-links design D3). Guarded on both ends actually being
  // connected (core/connect.connected). The ends remain; the phase becomes
  // Disconnected, so a reconnect is legal again (disconnect-before-reconnect).
  action disconnectWireAt(a: Endpoint, b: Endpoint): bool = all {
    connected(world.core, a),
    connected(world.core, b),
    world' = {
      core: world.core.with("conns", world.core.conns.exclude(Set((a, b)))),
      phase: Disconnected,
    },
  }

  // end-destroy: destroy one dpni end. Legal only once the end is no longer
  // connected — disconnect-before-destroy (cross-dprc-links design D5, finding 34
  // generalized), expressed by the core/connect.connected guard: destroying
  // a still-connected end is refused. Removing the end advances the wire to
  // its teardown-terminal phase.
  action destroyEndAt(o: ObjId): bool = all {
    world.core.objs.keys().contains(o),
    not(connected(world.core, portZero(o))),
    world' = {
      core: world.core.with("objs",
        world.core.objs.keys().exclude(Set(o))
          .mapBy(k => world.core.objs.get(k))),
      phase: EndDestroyLegal,
    },
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
}
