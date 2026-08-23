// View 2 — connect edges (object-model.md §2).
// Connections are owned by the dprc family: restool is type-agnostic,
// the MC validates pair legality. Pure helpers; actions in machine.qnt.
//
// Named invariants: none yet (phase 2). Apalache-marked subset: none.
module connect {
  import types.* from "./types"

  // Edge inventory (§2 table), unordered — both orders checked.
  pure def legalPair(a: Family, b: Family): bool =
    Set(
      (Dpni, Dpmac),    // physical port; MAC inherited dpmac→dpni (DPNI-I3)
      (Dpni, Dpni),     // point-to-point incl. cross-container, and loopback (DPNI-I9)
      (Dpni, Dpsw),     // switch port membership
      (Dpni, Dpdmux),   // demux downlink
      (Dpsw, Dpmac),    // switch uplink
      (Dpdmux, Dpmac),  // demux uplink — port 0 only since MC 10.37
      (Dpci, Dpci)      // inter-partition link; the only same-family-only edge (DPCI-I1)
    ).exists(p => p == (a, b) or p == (b, a))

  // dpdmux uplink restriction: only dpdmux.N.0 may face a dpmac
  // (dpdmux.md; MC ≥ 10.37 refuses non-dpmac uplinks older firmware
  // accepted).
  pure def legalPorts(a: Endpoint, b: Endpoint): bool = and {
    if (a.obj.fam == Dpdmux and b.obj.fam == Dpmac) a.port == 0 else true,
    if (b.obj.fam == Dpdmux and a.obj.fam == Dpmac) b.port == 0 else true,
  }

  pure def endpointsOf(s: CoreState): Set[Endpoint] =
    s.conns.map(c => Set(c._1, c._2)).flatten()

  // Cardinality one: an endpoint has at most one peer; reconnect
  // requires disconnect first (DPNI-I9, DPCI-I1).
  pure def connected(s: CoreState, e: Endpoint): bool =
    endpointsOf(s).contains(e)

  pure def objConnected(s: CoreState, o: ObjId): bool =
    endpointsOf(s).exists(e => e.obj == o)

  pure def validEndpoint(s: CoreState, params: Family -> FamilyParams,
                         e: Endpoint): bool = and {
    s.objs.keys().contains(e.obj),
    e.port >= 0,
    e.port < params.get(e.obj.fam).endpointPorts,
  }

  // dprc connect is issued on a common ancestor of both endpoints
  // (always satisfiable via the root — no extra gate here), both
  // endpoints currently disconnected, endpoints allowed to live in
  // different containers (DPRC-I5, DPNI-I9 verified cross-container).
  // No plug-state guard: connect-then-plug and plug-then-connect are
  // both legal — only the dpni side gets ENDPOINT_CHANGED and the late
  // side absorbs it (§2).
  pure def canConnect(s: CoreState, params: Family -> FamilyParams,
                      a: Endpoint, b: Endpoint): bool = and {
    validEndpoint(s, params, a),
    validEndpoint(s, params, b),
    legalPair(a.obj.fam, b.obj.fam),
    legalPorts(a, b),
    not(connected(s, a)),
    not(connected(s, b)),
  }
}
