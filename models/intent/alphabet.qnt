// The finite alphabet random simulation and Apalache draw intents from
// (openspec change `intent-layer`, design Risks): the inventory's dpmacs,
// two rate classes, consumers <= 3, cores <= 16 — built incrementally one
// construct per step, never a powerset draw (core/machine.qnt's Apalache
// lesson: powerset nondet made the checker intractable). Task 2.4 counts
// coverage over this alphabet; task 1.5 only makes it green: random
// simulation under `intentInvariants`.
//
// A finite intent is grown from the kernel-only init by seven actions, each
// guarded so its bound holds and each a nondet wrapper over the finite
// constant sets; `stutter` keeps `step` total once every bound is hit.
//
// Named invariants: intentInvariants (simulator rung: compileInvariants plus
//   planInvariants of every accepted plan).
// Apalache-marked subset: compileLaws — feasibleAgainstCeilings and
//   companionCountsByRegime (INTENT_I7/I8) — defined but simulator-only.
//   The quint→TLA translation succeeds (refuse.qnt hoists the match shapes
//   the 0.56.1 translator dies on with quint exiting 0 — a silent
//   false-green; derive.qnt bounds every sizing range by MAX_DERIVED_COUNT),
//   but Apalache's InlinePass inlines several full copies of the
//   compile/derive call tree and exhausts the heap: OOM at 4G and 24G, and a
//   40G server ran >12 min to ~50G RSS without reaching the checker.
//   Accepted 2026-08-30 for excessive cost (bead dpaa2-controlplane-gqf.26):
//   compileLaws runs at the simulator rung; revisit on a quint/Apalache
//   upgrade or a restructure that binds one compile per law.
module intent_alphabet {
  import types.* from "../core/types"           // Family, ObjId
  import companions.* from "../core/companions" // Regime = Kernel | PollMode
  import intent_types.* from "./types"           // Intent, Consumer, …, kernelConsumer, SWITCHINGS
  import intent_inventory.* from "./inventory"   // REF_INVENTORY
  import intent_derive.* from "./derive"         // derive, Plan
  import intent_refuse.* from "./refuse"         // compile, Compiled, consumerNames
  import intent_invariants.* from "./invariants" // planInvariants, compileInvariants

  var intent: Intent

  // ---- the finite alphabet (design Risks) ----

  // The two non-kernel consumer names; the kernel is seeded by init. A third
  // consumer is refused, keeping consumers <= 3.
  pure val CONSUMER_NAMES: List[str] = ["c1", "c2"]
  pure val PORT_NAMES: List[str] = ["p1", "p2", "p3"]
  pure val LINK_NAMES: List[str] = ["l1"]
  pure val FABRIC_NAMES: List[str] = ["f1"]

  // A slice of REF_INVENTORY's dpmacs: one reserved 25G (3), one free 25G
  // (4), three free 10G (7, 8, 9), the reserved 1G management nic (17) — so
  // the anchor rules (Free / Reserved / OverRate) all have a witness.
  pure val DPMACS: Set[int] = Set(3, 4, 7, 8, 9, 17)
  pure val RATES: Set[int] = Set(10000, 25000)  // the two rate classes
  pure val CORES: Set[int] = Set(1, 4, 5, 8, 16)
  pure val FLOWS: Set[int] = Set(0, 1, 4)        // crypto_flows demand
  pure val LIMIT_VALUES: Set[int] = Set(1, 2, 10, 12)
  pure val LIMIT_FAMILIES: Set[Family] = Set(Dpio, Dpbp, Dpmcp, Dpcon, Dpni)

  // A hand-enumerated slice of the member-list space: ports, consumers, a
  // self-member fabric (MFabric("f1"), the only fabric), a member resolving
  // to the kernel, and the empty list — small, so a step never draws a huge
  // fabric, but wide enough to reach every fabric refusal and origin.
  pure val MEMBER_LISTS: Set[List[Member]] = Set(
    [MPort("p1")],
    [MPort("p1"), MPort("p2")],
    [MPort("p1"), MConsumer("c1")],
    [MConsumer("c1"), MConsumer("c2")],
    [MPort("p2"), MFabric("f1")],
    [MConsumer("kernel")],
    [MPort("p3"), MConsumer("kernel")],
    [])

  // ---- init: the kernel consumer alone (design D1) ----

  action init = intent' = {
    consumers: [kernelConsumer(16)], ports: [], links: [], fabrics: [],
    crypto: Set(), limits: Set(),
  }

  // ---- one construct per step (never a powerset draw) ----

  // A poll-mode consumer named by position among the non-kernel names.
  action addConsumer = {
    nondet maxCores = CORES.oneOf()
    nondet cryptoFlows = FLOWS.oneOf()
    all {
      intent.consumers.length() < 3,
      intent' = intent.with("consumers", intent.consumers.append({
        name: CONSUMER_NAMES.nth(intent.consumers.length() - 1),
        regime: PollMode, maxCores: maxCores, cryptoFlows: cryptoFlows })),
    }
  }

  // A port anchored on an alphabet dpmac at an alphabet rate, owned by a
  // declared consumer (an undeclared owner is unreachable here).
  action addPort = {
    nondet dpmac = DPMACS.oneOf()
    nondet rate = RATES.oneOf()
    nondet owner = consumerNames(intent).oneOf()
    all {
      intent.ports.length() < 3,
      intent' = intent.with("ports", intent.ports.append({
        name: PORT_NAMES.nth(intent.ports.length()),
        dpmac: dpmac, rate: rate, owner: owner })),
    }
  }

  // The single link, between two consumer names (may be equal — a self-link).
  action addLink = {
    nondet a = consumerNames(intent).oneOf()
    nondet b = consumerNames(intent).oneOf()
    all {
      intent.links.length() == 0,
      intent.consumers.length() >= 2,
      intent' = intent.with("links", intent.links.append({ name: "l1", a: a, b: b })),
    }
  }

  // The single fabric, over a member list from the alphabet.
  action addFabric = {
    nondet switching = SWITCHINGS.oneOf()
    nondet owner = consumerNames(intent).oneOf()
    nondet members = MEMBER_LISTS.oneOf()
    all {
      intent.fabrics.length() == 0,
      intent' = intent.with("fabrics", intent.fabrics.append({
        name: "f1", switching: switching, owner: owner, members: members })),
    }
  }

  // One accelerator per consumer at most: an owner already in crypto is a
  // no-op pick (the guard disables the action, stutter keeps step total).
  action addCrypto = {
    nondet owner = consumerNames(intent).oneOf()
    all {
      not(intent.crypto.map(k => k.owner).contains(owner)),
      intent' = intent.with("crypto", intent.crypto.union(Set({ owner: owner }))),
    }
  }

  // A per-family limit override, at most two.
  action addLimit = {
    nondet consumer = consumerNames(intent).oneOf()
    nondet family = LIMIT_FAMILIES.oneOf()
    nondet limit = LIMIT_VALUES.oneOf()
    all {
      intent.limits.size() < 2,
      intent' = intent.with("limits",
        intent.limits.union(Set({ consumer: consumer, family: family, limit: limit }))),
    }
  }

  action stutter = intent' = intent

  action step = any {
    addConsumer, addPort, addLink, addFabric, addCrypto, addLimit, stutter,
  }

  // ---- the two rungs (design Risks; intent/invariants.qnt) ----

  // The Apalache-marked subset: feasibleAgainstCeilings and
  // companionCountsByRegime (INTENT_I7/I8) over (intent, REF_INVENTORY).
  val compileLaws = compileInvariants(intent, REF_INVENTORY)

  // The plan relationships (containmentByConsumer … provenanceClosed,
  // INTENT_I1..I6) of every accepted plan; a refused
  // intent has no plan to check.
  val acceptedPlanLaws =
    match compile(intent, REF_INVENTORY) {
      | Ok(r) => planInvariants(r.plan)
      | Refused(_) => true
    }

  // The simulator rung: both, over the alphabet's reachable intents.
  val intentInvariants = and { compileLaws, acceptedPlanLaws }
}
