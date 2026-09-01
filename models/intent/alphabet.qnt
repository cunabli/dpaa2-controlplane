// The finite alphabet random simulation and Apalache draw intents from
// (openspec change `intent-layer`, design Risks): the inventory's dpmacs,
// two rate classes, tenants <= 3, cores <= 16 — built incrementally one
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
//
// Coverage (task 2.4). `pnpm model:coverage` runs the alphabet under every
// invariant with the named witnesses below; the counted run is
//   seed=20260831, --max-steps=12, --max-samples=3000, 2026-09-01,
// deterministic and reproducible. No invariant violation was found. Two
// alphabet widenings this counting drove are stated where they land (DPMACS
// 99 for Unanchored, RATES 40000 for UnknownRateClass) — never silent. The
// counts were re-recorded after the 2.6a vocabulary rename moved the FLOWS
// draw from addTenant to addCrypto (crypto owns its flows): the trace stream
// shifts, so the numbers differ from the 2026-08-31 run while every recorded
// unknown (below) stays at zero.
//
// Witness counts (traces of 3000 in which the witness held):
//   wAccepted 3000, wRefused 2997, wUnknownCeiling 3000,
//   wLimitBelowRequest 1891, wUnpricedDataplane 2053, wOverRate 1656,
//   wMemberUnresolved 1592, wReservedAnchor 1548, wLimitNotDerived 950,
//   wUnanchored 854, wSelfMember 657, wDoubleClaimed 715,
//   wPortTenantMismatch 674, wMixedRates 451,
//   wFabricNotKernelForwarded 386, wUnknownRateClass 318,
//   wCoreBudgetExceeded 269, wUnsupportedEdge 173;
//   structure wCryptoPresent 2882, wLimitPresent 2838, wFabricPresent 2786,
//   wThreeTenants 2391, wLinkPresent 2309, wInterruptDrawn 2053.
//
// Recorded unknowns / alphabet-unreachable (0 traces — where each is covered):
//   wTenantAbsent — unreachable BY CONSTRUCTION: addPort/addLink/addFabric/
//     addCrypto/addLimit all draw tenant names from the declared set, so no
//     construct can name an undeclared tenant. Covered by the directed
//     tenantAbsentTest (intent/main.qnt).
//   wForeignAnchor — unreachable: REF_INVENTORY marks only dpni.0 Foreign, no
//     dpmac; the alphabet never invents a foreign dpmac. Covered by the
//     directed unanchoredForeignTest (intent/main.qnt, invWithForeignDpmac7).
//   wInfeasible — unreachable against REF_INVENTORY: intents of <=3 tenants,
//     <=3 ports, one link and one fabric never sum past a checkable ceiling
//     (Dpni Observed 18, Dpbp Counted 63, Dpmcp Observed 203). The ceiling
//     refuses, not the shape: covered by the vfabric overdrawn-pool twin
//     (scenarios/vfabric.qnt twinInfeasibleTest, Counted(5)) and the directed
//     infeasibleTest (intent/main.qnt).
//   wUnmeasuredCombination — reachable but not hit in 3000 samples: the
//     warning needs an *accepted* poll tenant terminating both seeded classes,
//     and the sole Free 25G dpmac in the alphabet is 4, so a clean cross-class
//     mix is a narrow draw. The shape precursor wMixedRates is counted (454);
//     the warning itself is covered by the directed mixedRateClassWarnsTest
//     (intent/main.qnt).
module intent_alphabet {
  import types.* from "../core/types"           // Family, ObjId
  import intent_types.* from "./types"           // Intent, Tenant, Dataplane, …, kernelTenant, SWITCHINGS
  import intent_inventory.* from "./inventory"   // REF_INVENTORY
  import intent_derive.* from "./derive"         // derive, Plan
  import intent_refuse.* from "./refuse"         // compile, Compiled, tenantNames
  import intent_invariants.* from "./invariants" // planInvariants, compileInvariants

  var intent: Intent

  // ---- the finite alphabet (design Risks) ----

  // The two non-kernel tenant names; the kernel is seeded by init. A third
  // tenant is refused, keeping tenants <= 3.
  pure val TENANT_NAMES: List[str] = ["c1", "c2"]
  pure val PORT_NAMES: List[str] = ["p1", "p2", "p3"]
  pure val LINK_NAMES: List[str] = ["l1"]
  pure val FABRIC_NAMES: List[str] = ["f1"]

  // A slice of REF_INVENTORY's dpmacs: one reserved 25G (3), one free 25G
  // (4), three free 10G (7, 8, 9), the reserved 1G management nic (17) — so
  // the anchor rules (Free / Reserved / OverRate) all have a witness. 99 is
  // absent from REF_INVENTORY: the Unanchored witness (task 2.4 widening —
  // the alphabet reached no unanchored port until a dpmac with no offer was
  // drawable).
  pure val DPMACS: Set[int] = Set(3, 4, 7, 8, 9, 17, 99)
  // The two seeded rate classes, plus 40000 — the unseeded class, no worker
  // row (task 2.4 widening for the UnknownRateClass witness). It usually
  // co-fires with OverRate (no 40G dpmac exists in the alphabet), but the
  // complete-refusal-list semantics still witness UnknownRateClass in the set.
  pure val RATES: Set[int] = Set(10000, 25000, 40000)
  pure val CORES: Set[int] = Set(1, 4, 5, 8, 16)
  pure val FLOWS: Set[int] = Set(0, 1, 4)        // the crypto construct's flows demand
  // The userspace dataplanes a non-kernel tenant may run: UserspacePoll is
  // priced (ADR-0012), UserspaceEvent is not — the latter is a legal
  // draw whose compile is refused UnpricedDataplane (a refusal is a legal
  // outcome, so the rung stays green).
  pure val USERSPACE_DATAPLANES: Set[Dataplane] = Set(UserspacePoll, UserspaceEvent)
  pure val LIMIT_VALUES: Set[int] = Set(1, 2, 10, 12)
  pure val LIMIT_FAMILIES: Set[Family] = Set(Dpio, Dpbp, Dpmcp, Dpcon, Dpni)

  // A hand-enumerated slice of the member-list space: ports, tenants, a
  // self-member fabric (MFabric("f1"), the only fabric), a member resolving
  // to the kernel, and the empty list — small, so a step never draws a huge
  // fabric, but wide enough to reach every fabric refusal and origin.
  pure val MEMBER_LISTS: Set[List[Member]] = Set(
    [MPort("p1")],
    [MPort("p1"), MPort("p2")],
    [MPort("p1"), MTenant("c1")],
    [MTenant("c1"), MTenant("c2")],
    [MPort("p2"), MFabric("f1")],
    [MTenant("kernel")],
    [MPort("p3"), MTenant("kernel")],
    [])

  // ---- init: the kernel tenant alone (design D1) ----

  action init = intent' = {
    tenants: [kernelTenant(16)], ports: [], links: [], fabrics: [],
    crypto: Set(), limits: Set(),
  }

  // ---- one construct per step (never a powerset draw) ----

  // A userspace tenant named by position among the non-kernel names, its
  // dataplane drawn from the userspace alphabet.
  action addTenant = {
    nondet maxCores = CORES.oneOf()
    nondet dataplane = USERSPACE_DATAPLANES.oneOf()
    all {
      intent.tenants.length() < 3,
      intent' = intent.with("tenants", intent.tenants.append({
        name: TENANT_NAMES.nth(intent.tenants.length() - 1),
        dataplane: dataplane, maxCores: maxCores })),
    }
  }

  // A port anchored on an alphabet dpmac at an alphabet rate, owned by a
  // declared tenant (an undeclared owner is unreachable here).
  action addPort = {
    nondet dpmac = DPMACS.oneOf()
    nondet rate = RATES.oneOf()
    nondet owner = tenantNames(intent).oneOf()
    all {
      intent.ports.length() < 3,
      intent' = intent.with("ports", intent.ports.append({
        name: PORT_NAMES.nth(intent.ports.length()),
        dpmac: dpmac, rate: rate, tenant: owner })),
    }
  }

  // The single link, between two tenant names (may be equal — a self-link).
  action addLink = {
    nondet a = tenantNames(intent).oneOf()
    nondet b = tenantNames(intent).oneOf()
    all {
      intent.links.length() == 0,
      intent.tenants.length() >= 2,
      intent' = intent.with("links", intent.links.append({ name: "l1", interfaceA: a, interfaceB: b })),
    }
  }

  // The single fabric, over a member list from the alphabet.
  action addFabric = {
    nondet switching = SWITCHINGS.oneOf()
    nondet owner = tenantNames(intent).oneOf()
    nondet members = MEMBER_LISTS.oneOf()
    all {
      intent.fabrics.length() == 0,
      intent' = intent.with("fabrics", intent.fabrics.append({
        name: "f1", switching: switching, forwardedBy: owner, members: members })),
    }
  }

  // One accelerator per tenant at most: an owner already in crypto is a
  // no-op pick (the guard disables the action, stutter keeps step total). The
  // flows demand is drawn here (design D1: crypto owns its flows).
  action addCrypto = {
    nondet owner = tenantNames(intent).oneOf()
    nondet flows = FLOWS.oneOf()
    all {
      not(intent.crypto.map(k => k.tenant).contains(owner)),
      intent' = intent.with("crypto", intent.crypto.union(Set({ tenant: owner, flows: flows }))),
    }
  }

  // A per-family limit override, at most two.
  action addLimit = {
    nondet tenant = tenantNames(intent).oneOf()
    nondet family = LIMIT_FAMILIES.oneOf()
    nondet limit = LIMIT_VALUES.oneOf()
    all {
      intent.limits.size() < 2,
      intent' = intent.with("limits",
        intent.limits.union(Set({ tenant: tenant, family: family, limit: limit }))),
    }
  }

  action stutter = intent' = intent

  action step = any {
    addTenant, addPort, addLink, addFabric, addCrypto, addLimit, stutter,
  }

  // ---- the two rungs (design Risks; intent/invariants.qnt) ----

  // The Apalache-marked subset: feasibleAgainstCeilings and
  // companionCountsByRegime (INTENT_I7/I8) over (intent, REF_INVENTORY).
  val compileLaws = compileInvariants(intent, REF_INVENTORY)

  // The plan relationships (containmentByTenant … provenanceClosed,
  // INTENT_I1..I6) of every accepted plan; a refused
  // intent has no plan to check.
  val acceptedPlanLaws =
    match compile(intent, REF_INVENTORY) {
      | Ok(r) => planInvariants(r.plan)
      | Refused(_) => true
    }

  // The simulator rung: both, over the alphabet's reachable intents.
  val intentInvariants = and { compileLaws, acceptedPlanLaws }

  // ---- coverage witnesses (task 2.4) ----
  // One named predicate per coverage dimension, counted by `pnpm
  // model:coverage` (--witnesses): the seeded run reports for how many traces
  // each held. The outcome witnesses read `compile(intent, REF_INVENTORY)` by
  // Refusal/Warning variant (design D5 refusals, design D2 warnings); the
  // structure witnesses read the alphabet's shape. A witness at 0 traces is a
  // recorded unknown, never a silent gap (module header, task 2.4).

  // Does some refusal / warning of the compiled intent match the predicate.
  def hasRefusal(pred: (Refusal) => bool): bool =
    match compile(intent, REF_INVENTORY) {
      | Refused(rs) => rs.exists(pred)
      | Ok(_) => false
    }
  def hasWarning(pred: (Warning) => bool): bool =
    match compile(intent, REF_INVENTORY) {
      | Ok(r) => r.warnings.exists(pred)
      | Refused(_) => false
    }

  // Outcome (design D5): accepted, refused, and one per Refusal variant.
  val wAccepted = match compile(intent, REF_INVENTORY) { | Ok(_) => true | Refused(_) => false }
  val wRefused = hasRefusal(_ => true)
  val wTenantAbsent = hasRefusal(r => match r { | TenantAbsent(_) => true | _ => false })
  val wMemberUnresolved = hasRefusal(r => match r { | MemberUnresolved(_) => true | _ => false })
  val wSelfMember = hasRefusal(r => match r { | SelfMember(_) => true | _ => false })
  val wUnanchored = hasRefusal(r => match r { | Unanchored(_) => true | _ => false })
  val wReservedAnchor = hasRefusal(r => match r { | ReservedAnchor(_) => true | _ => false })
  val wForeignAnchor = hasRefusal(r => match r { | ForeignAnchor(_) => true | _ => false })
  val wDoubleClaimed = hasRefusal(r => match r { | DoubleClaimed(_) => true | _ => false })
  val wOverRate = hasRefusal(r => match r { | OverRate(_) => true | _ => false })
  val wFabricNotKernelForwarded = hasRefusal(r => match r { | FabricNotKernelForwarded(_) => true | _ => false })
  val wPortTenantMismatch = hasRefusal(r => match r { | PortTenantMismatch(_) => true | _ => false })
  val wUnsupportedEdge = hasRefusal(r => match r { | UnsupportedEdge(_) => true | _ => false })
  val wUnknownRateClass = hasRefusal(r => match r { | UnknownRateClass(_) => true | _ => false })
  val wCoreBudgetExceeded = hasRefusal(r => match r { | CoreBudgetExceeded(_) => true | _ => false })
  val wLimitBelowRequest = hasRefusal(r => match r { | LimitBelowRequest(_) => true | _ => false })
  val wLimitNotDerived = hasRefusal(r => match r { | LimitNotDerived(_) => true | _ => false })
  val wInfeasible = hasRefusal(r => match r { | Infeasible(_) => true | _ => false })
  val wUnpricedDataplane = hasRefusal(r => match r { | UnpricedDataplane(_) => true | _ => false })

  // Warnings (design D2 ceilings; design D3 cross-class mix).
  val wUnknownCeiling = hasWarning(w => match w { | UnknownCeiling(_) => true | _ => false })
  val wUnmeasuredCombination = hasWarning(w => match w { | UnmeasuredCombination(_) => true | _ => false })

  // Alphabet structure: the shapes a trace must reach to exercise the rules.
  val wThreeTenants = intent.tenants.length() == 3
  val wLinkPresent = intent.links.length() > 0
  val wFabricPresent = intent.fabrics.length() > 0
  val wCryptoPresent = intent.crypto.size() > 0
  val wLimitPresent = intent.limits.size() > 0
  val wInterruptDrawn = intent.tenants.select(c => c.dataplane == UserspaceEvent).length() > 0
  // A tenant terminating both seeded rate classes (10G and 25G): the
  // cross-class mix the worker table prices but flags unmeasured (design D3).
  val wMixedRates = intent.tenants.select(c => seededRateClasses(intent, c.name).size() > 1).length() > 0
}
