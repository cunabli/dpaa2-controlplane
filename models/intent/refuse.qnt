// The total function's other half (openspec change `intent-layer`, design
// D5; ADR-0005 §2): `compile(intent, inventory)` is total — every intent is
// either a complete plan or the complete, non-empty refusal set. `refusals`
// runs every rule unconditionally and returns their union, never the first
// violation: the compiler idiom, so the operator fixes a file in one pass
// ("All violations at once", spec.md). `derive` is total in Quint (no partial
// ops), so computing feasibility on an otherwise-refused intent is safe; the
// derivation of a refused intent is defined, not wrong (derive.qnt).
//
// Overrides follow the request/limit idiom (design D5): a derived count is a
// request, a per-family limit raises it, a limit below the request refuses.
// Ceilings follow ADR-0011's three values (design D2): feasibility refuses
// against Counted and Observed and warns on Unknown, never inventing a number.
//
// DEVIATION (Quint QNT101): the spec names the reserved/foreign-anchor
// refusals `Reserved` and `Foreign`, but those collide with the imported
// `Availability` constructors of the same name (intent/types.qnt), which this
// module must match on in the anchor rule. The two Refusal constructors are
// spelled `ReservedAnchor` / `ForeignAnchor` here; the Rust enum (task 3.x)
// keeps the spec names `Reserved` / `Foreign`.
//
// Named invariants: none here (intent/invariants.qnt, task 1.4).
// Apalache-marked subset: none.
module intent_refuse {
  import types.* from "../core/types"           // Family, ObjId
  import companions.* from "../core/companions" // Regime = Kernel | PollMode
  import intent_types.* from "./types"           // Intent, Inventory, Availability, DERIVED_FAMILIES
  import intent_derive.* from "./derive"         // derive, sizeConsumer, ownedPorts, threadCount, …

  // The rule an intent broke, naming the offending construct. One variant per
  // rule (design D5, spec.md "The compiler refuses by name").
  type Refusal =
    | ConsumerAbsent({ construct: str, consumer: str })         // a construct names a consumer not declared (DPDCEI-I1 generalised)
    | MemberUnresolved({ fabric: str, member: Member })         // a fabric member names a port/consumer/fabric not declared
    | SelfMember({ fabric: str, member: Member })               // a fabric member resolves to the fabric's own owner
    | Unanchored({ port: str, dpmac: int })                     // dpmac not in the inventory
    | ReservedAnchor({ port: str, dpmac: int, why: str })       // ADR-0003 §3 matrix, from Availability (spec name: Reserved)
    | ForeignAnchor({ port: str, dpmac: int, owner: str })      // ADR-0001 §4, from Availability (spec name: Foreign)
    | DoubleClaimed({ dpmac: int, constructs: Set[str] })       // two ports on one dpmac, or one port in two fabrics
    | OverRate({ port: str, rate: int, maxRate: int })          // rate > dpmac maxRate
    | FabricNotKernelSteered({ fabric: str, owner: str })       // Hardware fabric whose owner != KERNEL (dpsw.md)
    | PortOwnerMismatch({ fabric: str, port: str, owner: str }) // member port whose owner != fabric owner (types.qnt Port comment)
    | UnsupportedEdge({ fabric: str, member: str })             // Hardware fabric listing a Hardware fabric (object-model.md §2; derive.qnt (d))
    | UnknownRateClass({ consumer: str, rates: List[int] })     // poll-mode consumer with no seeded (rate, count) row (design D3; derive.qnt (b))
    | CoreBudgetExceeded({ consumer: str, t: int, maxCores: int }) // poll-mode T > maxCores (design D3; ADR-0012 never rations)
    | LimitBelowRequest({ consumer: str, family: Family, request: int, limit: int }) // design D5 (D5's OverrideBelowFloor)
    | LimitNotDerived({ consumer: str, family: Family })        // a limit on a family whose count is fixed by constructs (design D7)
    | Infeasible({ family: Family, needed: int, available: int }) // sum of derived objs of a family > a Counted/Observed ceiling (ADR-0011; design D2)

  // A non-fatal note: the ceiling is Unknown, so feasibility could not check
  // it — warn, never invent a number (ADR-0011).
  type Warning = UnknownCeiling({ family: Family, needed: int })

  // The compile result: a plan with its warnings, or the complete refusal set.
  type Compiled = Ok({ plan: Plan, warnings: Set[Warning] }) | Refused(Set[Refusal])

  // ---- shared lookups ----

  pure def consumerNames(intent: Intent): Set[str] =
    intent.consumers.foldl(Set(), (acc, c) => acc.union(Set(c.name)))

  pure def consumerByName(intent: Intent, nm: str): Option[Consumer] =
    intent.consumers.foldl(None, (acc, c) => if (c.name == nm) Some(c) else acc)

  // ---- rule 1: a construct names an undeclared consumer (design D5) ----
  // Generalises DPDCEI-I1's consumer-absence rule to every construct that
  // names a consumer: port owner, link ends, fabric owner, crypto owner, limit
  // consumer. crypto and limit carry no name, so the construct field reads the
  // literal "crypto" / "limit".
  pure def consumerAbsentRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    val names = consumerNames(intent)
    val fromPorts = intent.ports.foldl(Set(), (acc, p) =>
      if (names.contains(p.owner)) acc
      else acc.union(Set(ConsumerAbsent({ construct: p.name, consumer: p.owner }))))
    val fromLinks = intent.links.foldl(fromPorts, (acc, l) =>
      val a1 = if (names.contains(l.a)) acc else acc.union(Set(ConsumerAbsent({ construct: l.name, consumer: l.a })))
      if (names.contains(l.b)) a1 else a1.union(Set(ConsumerAbsent({ construct: l.name, consumer: l.b }))))
    val fromFabrics = intent.fabrics.foldl(fromLinks, (acc, f) =>
      if (names.contains(f.owner)) acc
      else acc.union(Set(ConsumerAbsent({ construct: f.name, consumer: f.owner }))))
    val fromCrypto = intent.crypto.fold(fromFabrics, (acc, k) =>
      if (names.contains(k.owner)) acc
      else acc.union(Set(ConsumerAbsent({ construct: "crypto", consumer: k.owner }))))
    intent.limits.fold(fromCrypto, (acc, l) =>
      if (names.contains(l.consumer)) acc
      else acc.union(Set(ConsumerAbsent({ construct: "limit", consumer: l.consumer }))))

  // ---- rule 2: a fabric member names something not declared ----
  pure def memberUnresolvedRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    val names = consumerNames(intent)
    intent.fabrics.foldl(Set(), (acc, f) =>
      f.members.foldl(acc, (a2, m) =>
        match m {
          | MPort(p) => if (portByName(intent, p) == None) a2.union(Set(MemberUnresolved({ fabric: f.name, member: m }))) else a2
          | MConsumer(c) => if (not(names.contains(c))) a2.union(Set(MemberUnresolved({ fabric: f.name, member: m }))) else a2
          | MFabric(h) => if (fabricByName(intent, h) == None) a2.union(Set(MemberUnresolved({ fabric: f.name, member: m }))) else a2
        }))

  // ---- rule 3: a fabric member resolves to the fabric's own owner ----
  // A hardware fabric may list its own owner (the kernel's dpni on its own
  // dpsw, figure 5); a software fabric wiring itself is a refusal.
  // A software fabric listing a same-owner software fabric wires the owner
  // to itself through the chain. Hoisted out of the match arm: the
  // quint→Apalache translator (0.56.1) dies without a message on a match
  // nested inside a match arm next to a constructor-wildcard arm.
  pure def selfViaChain(intent: Intent, f: Fabric, h: str): bool =
    match fabricByName(intent, h) {
      | Some(g) => f.switching == Software and g.switching == Software and g.owner == f.owner
      | None => false
    }

  pure def selfMemberRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.fabrics.foldl(Set(), (acc, f) =>
      f.members.foldl(acc, (a2, m) =>
        match m {
          | MConsumer(c) =>
              if (f.switching == Software and c == f.owner)
                a2.union(Set(SelfMember({ fabric: f.name, member: m })))
              else a2
          | MFabric(h) =>
              if (h == f.name or selfViaChain(intent, f, h))
                a2.union(Set(SelfMember({ fabric: f.name, member: m })))
              else a2
          | _ => a2
        }))

  // ---- rule 4: the port's dpmac anchor (design D2; ADR-0003 §3; ADR-0001 §4) ----
  pure def anchorRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.ports.foldl(Set(), (acc, p) =>
      if (not(inv.dpmacs.keys().contains(p.dpmac)))
        acc.union(Set(Unanchored({ port: p.name, dpmac: p.dpmac })))
      else match availabilityOf(inv, { fam: Dpmac, num: p.dpmac }) {
        | Reserved(why) => acc.union(Set(ReservedAnchor({ port: p.name, dpmac: p.dpmac, why: why })))
        | Foreign(o) => acc.union(Set(ForeignAnchor({ port: p.name, dpmac: p.dpmac, owner: o })))
        | Free => if (p.rate > inv.dpmacs.get(p.dpmac).maxRate)
                    acc.union(Set(OverRate({ port: p.name, rate: p.rate, maxRate: inv.dpmacs.get(p.dpmac).maxRate })))
                  else acc
      })

  // ---- rule 5: a dpmac or a port claimed twice (design D5) ----
  pure def portsOnDpmac(intent: Intent, d: int): Set[str] =
    intent.ports.foldl(Set(), (acc, p) => if (p.dpmac == d) acc.union(Set(p.name)) else acc)

  pure def claimedDpmacs(intent: Intent): Set[int] =
    intent.ports.foldl(Set(), (acc, p) => acc.union(Set(p.dpmac)))

  pure def fabricsNamingPort(intent: Intent, pn: str): Set[str] =
    intent.fabrics.foldl(Set(), (acc, f) =>
      if (f.members.select(m => memberIsPort(m, pn)).length() > 0) acc.union(Set(f.name)) else acc)

  pure def memberPortNames(intent: Intent): Set[str] =
    intent.fabrics.foldl(Set(), (acc, f) =>
      f.members.foldl(acc, (a2, m) => match m { | MPort(p) => a2.union(Set(p)) | _ => a2 }))

  pure def dpmacOfPortName(intent: Intent, pn: str): int =
    match portByName(intent, pn) { | Some(p) => p.dpmac | None => 0 }

  pure def doubleClaimedRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    val byDpmac = claimedDpmacs(intent)
      .filter(d => portsOnDpmac(intent, d).size() >= 2)
      .map(d => DoubleClaimed({ dpmac: d, constructs: portsOnDpmac(intent, d) }))
    val byFabric = memberPortNames(intent)
      .filter(pn => fabricsNamingPort(intent, pn).size() >= 2)
      .map(pn => DoubleClaimed({ dpmac: dpmacOfPortName(intent, pn), constructs: fabricsNamingPort(intent, pn) }))
    byDpmac.union(byFabric)

  // ---- rule 6: fabric steering, port ownership, and unsupported edges (design D4/D5) ----
  // The two member checks hoisted out of their match arms (same
  // translator limit as selfViaChain above).
  pure def portOwnerMismatchOf(intent: Intent, f: Fabric, pn: str): Set[Refusal] =
    match portByName(intent, pn) {
      | Some(port) => if (port.owner != f.owner)
                        Set(PortOwnerMismatch({ fabric: f.name, port: pn, owner: port.owner }))
                      else Set()
      | None => Set()
    }

  pure def memberIsHwFabric(intent: Intent, h: str): bool =
    match fabricByName(intent, h) {
      | Some(g) => g.switching == Hardware
      | None => false
    }

  pure def fabricRulesRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.fabrics.foldl(Set(), (acc, f) =>
      val notKernel = if (f.switching == Hardware and f.owner != KERNEL)
                        Set(FabricNotKernelSteered({ fabric: f.name, owner: f.owner }))
                      else Set()
      val mismatches = f.members.foldl(Set(), (a2, m) =>
        match m {
          | MPort(p) => a2.union(portOwnerMismatchOf(intent, f, p))
          | _ => a2
        })
      val unsupported = if (f.switching == Hardware)
        f.members.foldl(Set(), (a2, m) =>
          match m {
            | MFabric(h) => if (memberIsHwFabric(intent, h))
                              a2.union(Set(UnsupportedEdge({ fabric: f.name, member: h })))
                            else a2
            | _ => a2
          })
        else Set()
      acc.union(notKernel).union(mismatches).union(unsupported))

  // ---- rule 7: poll-mode thread-count sizing (design D3; ADR-0012) ----
  // Kernel consumers: none. Under the online-CPU reading of open question 2
  // (KERNEL_BUDGET_IS_DPIO_COUNT = false), maxCores is not a bound on the
  // kernel regime, so no CoreBudgetExceeded fires for a kernel consumer.
  pure def sizingRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.consumers.foldl(Set(), (acc, c) =>
      if (c.regime == PollMode)
        val ports = ownedPorts(intent, c.name)
        match threadCount(ports) {
          | None => acc.union(Set(UnknownRateClass({ consumer: c.name, rates: ports.foldl([], (a, p) => a.append(p.rate)) })))
          | Some(t) => if (t > c.maxCores) acc.union(Set(CoreBudgetExceeded({ consumer: c.name, t: t, maxCores: c.maxCores }))) else acc
        }
      else acc)

  // ---- rule 8: request/limit overrides (design D5/D7) ----
  // A limit on a non-count family is never silently ignored; a limit below its
  // request refuses. An absent consumer is skipped — ConsumerAbsent fires for
  // it in rule 1.
  pure def limitRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.limits.fold(Set(), (acc, l) =>
      if (not(Set(Dpio, Dpbp, Dpmcp, Dpcon).contains(l.family)))
        acc.union(Set(LimitNotDerived({ consumer: l.consumer, family: l.family })))
      else match consumerByName(intent, l.consumer) {
        | None => acc
        | Some(c) =>
            val s = sizeConsumer(intent, inv, c)
            val request = if (l.family == Dpio) s.effDpio.request
                          else if (l.family == Dpbp) s.effDpbp.request
                          else if (l.family == Dpmcp) s.effDpmcp.request
                          else s.effDpcon.request
            if (l.limit < request)
              acc.union(Set(LimitBelowRequest({ consumer: l.consumer, family: l.family, request: request, limit: l.limit })))
            else acc
      })

  // ---- rule 9: cross-plan feasibility against the ceilings (ADR-0011; design D2) ----
  // available = the ceiling's n; whether foreign/DPL objects already on the
  // board should be subtracted is the fit check's question (task 2.5). Unknown
  // ceilings produce a warning (below), never a refusal.
  pure def feasibilityRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    val plan = derive(intent, inv)
    DERIVED_FAMILIES.fold(Set(), (acc, fam) =>
      val needed = plan.objs.filter(o => o.key.family == fam).size()
      match inv.ceilings.get(fam) {
        | Counted(n) => if (needed > n) acc.union(Set(Infeasible({ family: fam, needed: needed, available: n }))) else acc
        | Observed(o) => if (needed > o.n) acc.union(Set(Infeasible({ family: fam, needed: needed, available: o.n }))) else acc
        | Unknown => acc
      })

  // Every rule runs unconditionally; the refusal set is their union.
  pure def refusals(intent: Intent, inv: Inventory): Set[Refusal] =
    consumerAbsentRefusals(intent, inv)
      .union(memberUnresolvedRefusals(intent, inv))
      .union(selfMemberRefusals(intent, inv))
      .union(anchorRefusals(intent, inv))
      .union(doubleClaimedRefusals(intent, inv))
      .union(fabricRulesRefusals(intent, inv))
      .union(sizingRefusals(intent, inv))
      .union(limitRefusals(intent, inv))
      .union(feasibilityRefusals(intent, inv))

  // A warning per derived family whose ceiling is Unknown and whose count is
  // non-zero (ADR-0011: the cap ended, or the family was never measured).
  pure def warnings(intent: Intent, inv: Inventory): Set[Warning] =
    val plan = derive(intent, inv)
    DERIVED_FAMILIES.fold(Set(), (acc, fam) =>
      val needed = plan.objs.filter(o => o.key.family == fam).size()
      match inv.ceilings.get(fam) {
        | Unknown => if (needed > 0) acc.union(Set(UnknownCeiling({ family: fam, needed: needed }))) else acc
        | _ => acc
      })

  // The total function: an empty refusal set yields the plan and its warnings,
  // else the complete refusal set.
  pure def compile(intent: Intent, inv: Inventory): Compiled =
    val rs = refusals(intent, inv)
    if (rs == Set()) Ok({ plan: derive(intent, inv), warnings: warnings(intent, inv) })
    else Refused(rs)

  // A value use for every sum-type constructor inside the module: the
  // verify-path flattener drops a constructor whose only value use sits in
  // another module (QNT404; see intent/types.qnt ETH_IFS block).
  pure val REFUSALS_USE: Set[Refusal] = Set(
    ConsumerAbsent({ construct: "c", consumer: "x" }),
    MemberUnresolved({ fabric: "f", member: MPort("p") }),
    SelfMember({ fabric: "f", member: MConsumer("c") }),
    Unanchored({ port: "p", dpmac: 0 }),
    ReservedAnchor({ port: "p", dpmac: 0, why: "w" }),
    ForeignAnchor({ port: "p", dpmac: 0, owner: "o" }),
    DoubleClaimed({ dpmac: 0, constructs: Set() }),
    OverRate({ port: "p", rate: 0, maxRate: 0 }),
    FabricNotKernelSteered({ fabric: "f", owner: "o" }),
    PortOwnerMismatch({ fabric: "f", port: "p", owner: "o" }),
    UnsupportedEdge({ fabric: "f", member: "m" }),
    UnknownRateClass({ consumer: "c", rates: [] }),
    CoreBudgetExceeded({ consumer: "c", t: 0, maxCores: 0 }),
    LimitBelowRequest({ consumer: "c", family: Dpio, request: 0, limit: 0 }),
    LimitNotDerived({ consumer: "c", family: Dpni }),
    Infeasible({ family: Dpbp, needed: 0, available: 0 }))
  pure val WARNINGS_USE: Set[Warning] = Set(UnknownCeiling({ family: Dpio, needed: 0 }))
  pure val COMPILED_USE: Set[Compiled] = Set(
    Ok({ plan: { objs: Set(), edges: Set(), order: [], prov: Map() }, warnings: Set() }),
    Refused(Set()))
}
