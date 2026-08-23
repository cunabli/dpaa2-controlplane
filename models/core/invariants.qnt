// Phase-2 invariant encodings (design D3): the baseline's invariant
// candidates as named predicates over CoreState, names = baseline ids
// (traceability rule — the ledger, the docs, and the model never drift).
//
// Only candidates the machine actually enforces live here as state
// invariants — they must be green under random simulation. Candidates
// that are action-guard properties or Breaking absence-of-assumption
// witnesses are directed runs in models/main.qnt under the same ids.
// Candidates not expressible over this state (cfg contents, exit
// status, counters, command versions) are carried structurally or are
// harness-owned; models/COVERAGE.md (task 2.4) records every
// disposition.
//
// The six cross-cutting laws (object-model.md §6) map as:
//   LAW 1 (cfg is the type parameter)  — structural: the state carries
//          no cfg block, so no action can mutate one; identity
//          immutability witnessed by DPRC_I10Test.
//   LAW 2 (read-back is the only convergence observable) — harness-
//          owned (phase 4); the model exhibits the state divergences
//          only read-back can see (DPNI_I8Test, DPBP_I3Test).
//   LAW 3 (tri-state visibility) — LAW3_triState below, plus
//          DPRC_I6Test (existence without bus visibility).
//   LAW 4 (two id spaces) — LAW4_twoIdSpaces below, plus
//          DPBP_I5Test / DPCON_I2Test (hwId ≠ object id witnesses).
//   LAW 5 (ownership exclusivity is an assumption) — structural: the
//          single-valued bind/allocatedBy fields ARE ADR-0006's
//          single-initiating-writer assumption; the board catches the
//          MC allowing the silent fight (DPSECI-I7, DPRTC-I2).
//   LAW 6 (version is per-action state) — not modeled in state; the
//          phase-4 adapter carries emitted command versions per action
//          (DPNI-I11, DPDMAI-I2).
//
// Named invariants (state, simulator rung): LAW3_triState,
//   LAW4_twoIdSpaces, DPRC_I1, DPRC_I2, DPRC_I4, DPNI_I9, DPMAC_I1,
//   DPBP_I2, DPIO_I1.
// Apalache-marked subset (task 2.3): all nine, at bounded depth 1 —
//   init plus every single-step successor class, symbolically. Depth 1
//   is the empirical ceiling: the transition relation (lineage
//   unrolling, map rebuilds) makes each further symbolic step cost
//   tens of minutes at quint 0.32.0 / Apalache 0.56.1, while the
//   simulator covers depth 40 randomly. Empirical notes: Apalache
//   0.62.1 is client-incompatible with quint 0.32.0 (server starts,
//   check never runs) — the pin is 0.56.1; the verify-path flattener
//   loses sum-type constructors whose only value use is in the main
//   module (hence families/params.qnt hosts ALL_PARAMS); Apalache
//   demands constant integer ranges (machine.MAX_ENDPOINT_PORTS).
// TLA+ escalation: none.
module invariants {
  import types.* from "./types"
  import containment.* from "./containment"
  import connect.* from "./connect"

  // LAW 3 — visibility is tri-state per object: MC existence (objs
  // membership), bus visibility, driver binding — advanced by different
  // actions, never inferred from each other (DPRC-I6, DPCI-I3,
  // DPRC-I2). The inferable direction is downward only: a bound driver
  // implies a visible bus device implies MC existence. The converse
  // holds nowhere — DPRC_I6Test exhibits existence without visibility.
  // Also view-1 well-formedness: one mc.global, every parent a live
  // dprc, container state exactly on the dprc population.
  pure def LAW3_triState(s: CoreState): bool = and {
    // exactly one parentless object, and it is a dprc (mc.global)
    s.objs.keys().filter(o => s.objs.get(o).parent == None).size() == 1,
    s.objs.keys().forall(o => or {
      s.objs.get(o).parent != None,
      o.fam == Dprc,
    }),
    // every parent edge points at a live container
    s.objs.keys().forall(o =>
      match s.objs.get(o).parent {
        | Some(p) => s.dprcs.keys().contains(p)
        | None => true
      }),
    // ContainerState exists exactly for the dprc population
    s.dprcs.keys() == s.objs.keys().filter(o => o.fam == Dprc),
    // driver bound ⇒ bus-visible (existence is implicit: o ∈ objs)
    s.objs.keys().forall(o =>
      (s.objs.get(o).bind != Unbound) implies s.objs.get(o).busVisible),
  }

  // LAW 4 — two id spaces (DPBP-I5, DPSW-I5, DPCON-I2): the MC-assigned
  // hardware id is its own space, unique within a family — the only
  // uniqueness it promises (§6.4). Independence from the restool object
  // id is witnessed by DPBP_I5Test / DPCON_I2Test.
  pure def LAW4_twoIdSpaces(s: CoreState): bool =
    s.objs.keys().forall(a => s.objs.keys().forall(b =>
      (a.fam == b.fam and a != b) implies
        s.objs.get(a).hwId != s.objs.get(b).hwId))

  // DPRC-I1 — allocation never crosses the container boundary:
  // container(consumer) = container(pool object) for every draw, and
  // every draw is held by a live, kernel-bound consumer. Also the
  // checkable face of DPMCP-I4 (placement law, verified class).
  pure def DPRC_I1(s: CoreState): bool =
    s.objs.keys().forall(o =>
      match s.objs.get(o).allocatedBy {
        | Some(c) => and {
            s.objs.keys().contains(c),
            s.objs.get(c).bind == BoundKernel,
            s.objs.get(o).parent == s.objs.get(c).parent,
          }
        | None => true
      })

  // DPRC-I2 — plug gating: a bound driver requires a plugged, visible
  // object; kernel binding additionally requires a matching driver and
  // never claims a pooled family (the allocator holds those). The
  // mirror direction: unplug is the unbind trigger, so an unplugged
  // object is never bound.
  pure def DPRC_I2(s: CoreState, params: Family -> FamilyParams): bool =
    s.objs.keys().forall(o => {
      val st = s.objs.get(o)
      and {
        (st.bind != Unbound) implies (st.plugged and st.busVisible),
        (st.bind == BoundKernel) implies and {
          params.get(o.fam).hasKernelDriver,
          not(params.get(o.fam).pooled),
        },
        not(st.plugged) implies (st.bind == Unbound),
      }
    })

  // DPRC-I4 — restool-created containers carry exactly the default
  // permission mask {SPAWN, ALLOC, OBJ_CREATE, IRQ_CFG}_ALLOWED
  // [verified]; only the boot roots differ (reference tree carries
  // TOPOLOGY_CHANGES there).
  pure def DPRC_I4(s: CoreState): bool =
    s.dprcs.keys().forall(c =>
      (not(isMcGlobal(s, c)) and not(isLinuxRoot(s, c))) implies
        s.dprcs.get(c).perms == Set(Spawn, Alloc, ObjCreate, IrqCfg))

  // DPNI-I9 — endpoint cardinality one and edge legality (verified,
  // kdpni pairs in production): every connection is a legal pair on
  // valid live endpoints (DPRC-I5's precondition face), and no endpoint
  // has two peers (loopback — one edge, both ends — is legal).
  pure def DPNI_I9(s: CoreState, params: Family -> FamilyParams): bool = and {
    s.conns.forall(c => and {
      validEndpoint(s, params, c._1),
      validEndpoint(s, params, c._2),
      legalPair(c._1.obj.fam, c._2.obj.fam),
      legalPorts(c._1, c._2),
    }),
    s.conns.forall(c1 => s.conns.forall(c2 => or {
      c1 == c2,
      Set(c1._1, c1._2).intersect(Set(c2._1, c2._2)) == Set(),
    })),
  }

  // DPMAC-I1 — identity permanence (verified, ADR-0001 §3): dpmacs are
  // DPC-born — the runtime set never grows past the boot inventory —
  // and RootOnly-pinned to the Linux root container. Whether restool
  // destroy of a DPC-born dpmac is accepted at all is off-nominal and
  // board-pending (dpmac.md unknown #1), so the invariant asserts
  // no-additions and no-drift, not constancy.
  pure def DPMAC_I1(s: CoreState, bootMacs: Set[int]): bool =
    s.objs.keys().filter(o => o.fam == Dpmac).forall(m => and {
      bootMacs.contains(m.num),
      match s.objs.get(m).parent {
        | Some(p) => isLinuxRoot(s, p)
        | None => false
      },
    })

  // DPBP-I2 — pool-entry precondition: a drawn allocatable is plugged
  // and allocator-held (never driver-bound); unplugged objects are
  // dprc-visible but allocator-invisible (refusal face in DPBP_I2Test).
  pure def DPBP_I2(s: CoreState): bool =
    s.objs.keys().forall(o =>
      (s.objs.get(o).allocatedBy != None) implies and {
        s.objs.get(o).plugged,
        s.objs.get(o).bind == Unbound,
      })

  // DPIO-I1 — dpio is not pool-allocatable: only pooled families are
  // ever drawn; dpio's own probe *draws a dpmcp* (the dpio→dpmcp
  // dependency arrow), witnessed by DPMCP_I1Test.
  pure def DPIO_I1(s: CoreState, params: Family -> FamilyParams): bool =
    s.objs.keys().forall(o =>
      (s.objs.get(o).allocatedBy != None) implies params.get(o.fam).pooled)
}
