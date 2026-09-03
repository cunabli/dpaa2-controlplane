// The total function's other half (openspec change `intent-layer`, design
// D5; ADR-0005 §2): `compile(intent, inventory)` is total — every intent is
// either a complete plan or the complete, non-empty refusal set. `refusals`
// runs every rule unconditionally and returns their union, never the first
// violation: the compiler idiom, so the operator fixes a file in one pass
// ("All violations at once", spec.md). `derive` is total in Quint (no partial
// ops), so computing feasibility on an otherwise-refused intent is safe; the
// derivation of a refused intent is defined, not wrong (derive.qnt).
//
// Extras follow the request/extra idiom (design D5): a derived count is a
// request, a per-(tenant, family) extra adds its count on top (raise-only); an
// extra on a non-companion family, or one whose count is below 1, refuses.
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
  import intent_types.* from "./types"           // Intent, Inventory, Availability, Dataplane, DERIVED_FAMILIES
  import intent_derive.* from "./derive"         // derive, sizeTenant, terminatedPorts, threadCount, hasPricing, …
  import dpseci.* from "../families/dpseci"      // DPSECI_MAX_QUEUE_NUM (the one-device queue-pair ceiling)

  // The rule an intent broke, naming the offending construct. One variant per
  // rule (design D5, spec.md "The compiler refuses by name").
  type Refusal =
    | TenantAbsent({ construct: str, tenant: str })         // a construct names a tenant not declared (DPDCEI-I1 generalised)
    | MemberUnresolved({ fabric: str, member: Member })         // a fabric member names a port/tenant/fabric not declared
    | SelfMember({ fabric: str, member: Member })               // a fabric member resolves to the fabric's own owner
    | Unanchored({ port: str, dpmac: int })                     // dpmac not in the inventory
    | ReservedAnchor({ port: str, dpmac: int, why: str })       // ADR-0003 §3 matrix, from Availability (spec name: Reserved)
    | ForeignAnchor({ port: str, dpmac: int, owner: str })      // ADR-0001 §4, from Availability (spec name: Foreign)
    | DoubleClaimed({ dpmac: int, constructs: Set[str] })       // two ports on one dpmac, or one port in two fabrics
    | OverRate({ port: str, rate: int, maxRate: int })          // rate > dpmac maxRate
    | FabricNotKernelForwarded({ fabric: str, forwardedBy: str })       // Hardware fabric whose forwarding tenant != KERNEL (dpsw.md)
    | PortTenantMismatch({ fabric: str, port: str, tenant: str }) // member port whose tenant != the fabric's forwarding tenant (types.qnt Port comment)
    | UnsupportedEdge({ fabric: str, member: str })             // Hardware fabric listing a Hardware fabric (object-model.md §2; derive.qnt (d))
    | UnknownRateClass({ tenant: str, rates: List[int] })     // userspace-poll tenant terminating a rate class with no seeded worker row (design D3; derive.qnt (b))
    | CoreBudgetExceeded({ tenant: str, t: int, maxCores: int }) // userspace-poll T > maxCores (design D3; ADR-0012 never rations)
    | ExtraNotCompanion({ tenant: str, family: Family })      // an extra on a family that is not one of the four companions (design D5)
    | ExtraNotPositive({ tenant: str, family: Family, count: int }) // an extra whose count is below 1 (design D5)
    | CryptoFlowsNotPositive({ tenant: str, ordinal: int, flows: int }) // a crypto block whose flows are below 1 (design D1; dpseci.md); ordinal = the block's 1-based declaration position (task 2.6e)
    | CryptoFlowsOverDevice({ tenant: str, ordinal: int, flows: int, maxFlows: int }) // a crypto block whose flows exceed one dpseci's DPSECI_MAX_QUEUE_NUM queue pairs — one block is one device, so this cannot be realized; split into blocks (task 2.6e; dpseci.h)
    | Infeasible({ family: Family, needed: int, available: int }) // sum of derived objs of a family > a Counted/Observed ceiling (ADR-0011; design D2)
    | UnpricedDataplane({ tenant: str, dataplane: Dataplane }) // a tenant whose dataplane ADR-0012 does not price (today UserspaceEvent; design D3)
    | PoolWithoutRestricted({ tenant: str, pool: str })       // a pool named on a tenant that is not Restricted — a contradiction (task 2.6c)
    | RestrictedWithoutPool({ tenant: str })                  // a Restricted tenant that names no pool holder (task 2.6c)
    | HolderNotPublic({ tenant: str, holder: str })           // a Restricted tenant's pool holder is not Public (task 2.6c)
    | PoolChain({ tenant: str, holder: str })                 // a Restricted tenant's pool holder itself has a pool — no chains (task 2.6c)
    | PoolDataplaneMismatch({ tenant: str, drawer: Dataplane, holder: Dataplane }) // a Restricted tenant's dataplane differs from its holder's (task 2.6c; reserved kernel counts as kernel-netlink)

  // A non-fatal note attached to an accepted compile (the review's
  // escape-hatch-warns rule). UnknownCeiling: the ceiling is Unknown, so
  // feasibility could not check the family — warn, never invent a number
  // (ADR-0011; design D2). UnmeasuredCombination: a userspace-poll tenant
  // terminates more than one seeded rate class, so the worker formula prices
  // its T over an unmeasured cross-class mix (design D3) — flagged, never
  // silent, naming the tenant and the classes mixed.
  type Warning =
    | UnknownCeiling({ family: Family, needed: int })
    | UnmeasuredCombination({ tenant: str, rates: Set[int] })

  // The compile result: a plan with its warnings, or the complete refusal set.
  type Compiled = Ok({ plan: Plan, warnings: Set[Warning] }) | Refused(Set[Refusal])

  // ---- shared lookups ----

  pure def tenantNames(intent: Intent): Set[str] =
    intent.tenants.foldl(Set(), (acc, c) => acc.union(Set(c.name)))

  pure def tenantByName(intent: Intent, n: str): Option[Tenant] =
    intent.tenants.foldl(None, (acc, c) => if (c.name == n) Some(c) else acc)

  // ---- rule 1: a construct names an undeclared tenant (design D5) ----
  // Generalises DPDCEI-I1's tenant-absence rule to every construct that
  // names a tenant: port owner, link ends, fabric owner, crypto owner, extra
  // tenant. crypto and extra carry no name, so the construct field reads the
  // literal "crypto" / "extra".
  pure def tenantAbsentRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    val names = tenantNames(intent)
    val fromPorts = intent.ports.foldl(Set(), (acc, p) =>
      if (names.contains(p.tenant)) acc
      else acc.union(Set(TenantAbsent({ construct: p.name, tenant: p.tenant }))))
    // A link end may name the reserved kernel without declaring it (task 2.6c):
    // the reserved tenant is always available as a pseudo-wire end, so KERNEL is
    // never absent here (the derivation materialises its dpni, derive.qnt).
    val fromLinks = intent.links.foldl(fromPorts, (acc, l) =>
      val a1 = if (names.contains(l.interfaceA) or l.interfaceA == KERNEL) acc else acc.union(Set(TenantAbsent({ construct: l.name, tenant: l.interfaceA })))
      if (names.contains(l.interfaceB) or l.interfaceB == KERNEL) a1 else a1.union(Set(TenantAbsent({ construct: l.name, tenant: l.interfaceB }))))
    val fromFabrics = intent.fabrics.foldl(fromLinks, (acc, f) =>
      if (names.contains(f.forwardedBy)) acc
      else acc.union(Set(TenantAbsent({ construct: f.name, tenant: f.forwardedBy }))))
    val fromCrypto = intent.crypto.foldl(fromFabrics, (acc, k) =>
      if (names.contains(k.tenant)) acc
      else acc.union(Set(TenantAbsent({ construct: "crypto", tenant: k.tenant }))))
    intent.extras.fold(fromCrypto, (acc, e) =>
      if (names.contains(e.tenant)) acc
      else acc.union(Set(TenantAbsent({ construct: "extra", tenant: e.tenant }))))

  // ---- rule 2: a fabric member names something not declared ----
  pure def memberUnresolvedRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    val names = tenantNames(intent)
    intent.fabrics.foldl(Set(), (acc, f) =>
      f.members.foldl(acc, (a2, m) =>
        match m {
          | MPort(p) => if (portByName(intent, p) == None) a2.union(Set(MemberUnresolved({ fabric: f.name, member: m }))) else a2
          | MTenant(c) => if (not(names.contains(c))) a2.union(Set(MemberUnresolved({ fabric: f.name, member: m }))) else a2
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
      | Some(g) => f.switching == Software and g.switching == Software and g.forwardedBy == f.forwardedBy
      | None => false
    }

  pure def selfMemberRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.fabrics.foldl(Set(), (acc, f) =>
      f.members.foldl(acc, (a2, m) =>
        match m {
          | MTenant(c) =>
              if (f.switching == Software and c == f.forwardedBy)
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

  // ---- rule 6: fabric forwarding, port tenancy, and unsupported edges (design D4/D5) ----
  // The two member checks hoisted out of their match arms (same
  // translator limit as selfViaChain above).
  pure def portOwnerMismatchOf(intent: Intent, f: Fabric, pn: str): Set[Refusal] =
    match portByName(intent, pn) {
      | Some(port) => if (port.tenant != f.forwardedBy)
                        Set(PortTenantMismatch({ fabric: f.name, port: pn, tenant: port.tenant }))
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
      val notKernel = if (f.switching == Hardware and f.forwardedBy != KERNEL)
                        Set(FabricNotKernelForwarded({ fabric: f.name, forwardedBy: f.forwardedBy }))
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

  // ---- rule 7: userspace-poll thread-count sizing (design D3; ADR-0012) ----
  // KernelNetlink tenants: none. Under the online-CPU reading of open question 2
  // (KERNEL_BUDGET_IS_DPIO_COUNT = false), maxCores is not a bound on the
  // kernel dataplane, so no CoreBudgetExceeded fires for a kernel tenant.
  pure def sizingRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.tenants.foldl(Set(), (acc, c) =>
      if (c.dataplane == UserspacePoll)
        val ports = terminatedPorts(intent, c.name)
        match threadCount(ports) {
          | None => acc.union(Set(UnknownRateClass({ tenant: c.name, rates: ports.foldl([], (a, p) => a.append(p.rate)) })))
          | Some(t) => if (t > c.maxCores) acc.union(Set(CoreBudgetExceeded({ tenant: c.name, t: t, maxCores: c.maxCores }))) else acc
        }
      else acc)

  // ---- rule 8: additive extras (design D5) ----
  // An extra on a non-companion family is never silently ignored; an extra
  // whose count is below 1 (a no-op or a lowering) refuses. Both checks are
  // independent — an extra can trip both. Extras are additive, matched by
  // (tenant, family), so there is no request comparison. An absent tenant is
  // skipped — TenantAbsent fires for it in rule 1.
  pure def extraRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.extras.fold(Set(), (acc, e) =>
      val notCompanion = if (not(Set(Dpio, Dpbp, Dpmcp, Dpcon).contains(e.family)))
                           Set(ExtraNotCompanion({ tenant: e.tenant, family: e.family }))
                         else Set()
      val notPositive = if (e.count < 1)
                          Set(ExtraNotPositive({ tenant: e.tenant, family: e.family, count: e.count }))
                        else Set()
      acc.union(notCompanion).union(notPositive))

  // ---- rule 11: a crypto block's flows out of the one-device range (design D1; dpseci.md) ----
  // A [[crypto]] block sizes ONE dpseci to its flows, so its flows must land in
  // the device's realizable range 1..DPSECI_MAX_QUEUE_NUM: flows < 1 would size
  // a zero-queue accelerator (CryptoFlowsNotPositive); flows above the ceiling
  // demands more queue pairs than one dpseci carries (CryptoFlowsOverDevice) —
  // not clamped, refused, since one block is one device (task 2.6e), and the
  // operator's remedy is splitting the demand into blocks, which per-block
  // sizing now makes expressible. maxFlows rides the payload so the message is
  // self-standing. An absent tenant is skipped — TenantAbsent fires for it in
  // rule 1. Both refusals name the block's 1-based declaration ordinal (task
  // 2.6e): two bad blocks of one tenant that share a flows value would
  // otherwise collapse in the Set to a single refusal, hiding one — the ordinal
  // keeps each block's refusal distinct (spec.md "all violations at once").
  pure def cryptoRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.crypto.foldl({ i: 0, set: Set() }, (acc, k) =>
      { i: acc.i + 1,
        set: if (k.flows < 1)
               acc.set.union(Set(CryptoFlowsNotPositive({ tenant: k.tenant, ordinal: acc.i + 1, flows: k.flows })))
             else if (k.flows > DPSECI_MAX_QUEUE_NUM)
               acc.set.union(Set(CryptoFlowsOverDevice({ tenant: k.tenant, ordinal: acc.i + 1, flows: k.flows, maxFlows: DPSECI_MAX_QUEUE_NUM })))
             else acc.set }).set

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

  // ---- rule 10: a tenant whose dataplane has no companion pricing (design D3; ADR-0012) ----
  // UserspaceEvent has no ADR-0012 draw, so the derivation cannot size it
  // (intent/derive.qnt hasPricing skips it); the refusal is the answer.
  pure def unpricedDataplaneRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.tenants.foldl(Set(), (acc, c) =>
      if (hasPricing(c)) acc
      else acc.union(Set(UnpricedDataplane({ tenant: c.name, dataplane: c.dataplane }))))

  // ---- rule 12: tenant isolation and pooling (task 2.6c) ----
  // The private-VLAN shape the MC container tree enforces, stated as a
  // complete list of contradictions. A `pool` on a non-Restricted tenant is a
  // contradiction; a Restricted tenant with no pool names no holder; a named
  // holder must exist (TenantAbsent, construct "pool"), must be Public, and
  // must not itself have a pool (no chains); and the drawer's dataplane must
  // equal the holder's — the reserved kernel counting as kernel-netlink, so a
  // kernel-netlink drawer must pool the kernel and a userspace-poll drawer
  // pooling the kernel is a mismatch. UnpricedDataplane (rule 10) is untouched.
  pure def poolRefusals(intent: Intent, inv: Inventory): Set[Refusal] =
    intent.tenants.foldl(Set(), (acc, c) =>
      if (c.isolation != Restricted)
        if (c.pool != "") acc.union(Set(PoolWithoutRestricted({ tenant: c.name, pool: c.pool }))) else acc
      else if (c.pool == "")
        acc.union(Set(RestrictedWithoutPool({ tenant: c.name })))
      else if (c.pool == KERNEL)
        if (c.dataplane != KernelNetlink)
          acc.union(Set(PoolDataplaneMismatch({ tenant: c.name, drawer: c.dataplane, holder: KernelNetlink })))
        else acc
      else match tenantByName(intent, c.pool) {
        | None => acc.union(Set(TenantAbsent({ construct: "pool", tenant: c.pool })))
        | Some(h) =>
            (if (h.isolation != Public) Set(HolderNotPublic({ tenant: c.name, holder: c.pool })) else Set())
              .union(if (h.pool != "") Set(PoolChain({ tenant: c.name, holder: c.pool })) else Set())
              .union(if (c.dataplane != h.dataplane) Set(PoolDataplaneMismatch({ tenant: c.name, drawer: c.dataplane, holder: h.dataplane })) else Set())
              .union(acc)
      })

  // Every rule runs unconditionally; the refusal set is their union.
  pure def refusals(intent: Intent, inv: Inventory): Set[Refusal] =
    tenantAbsentRefusals(intent, inv)
      .union(memberUnresolvedRefusals(intent, inv))
      .union(selfMemberRefusals(intent, inv))
      .union(anchorRefusals(intent, inv))
      .union(doubleClaimedRefusals(intent, inv))
      .union(fabricRulesRefusals(intent, inv))
      .union(sizingRefusals(intent, inv))
      .union(extraRefusals(intent, inv))
      .union(cryptoRefusals(intent, inv))
      .union(feasibilityRefusals(intent, inv))
      .union(unpricedDataplaneRefusals(intent, inv))
      .union(poolRefusals(intent, inv))

  // A warning per derived family whose ceiling is Unknown and whose count is
  // non-zero (ADR-0011: the cap ended, or the family was never measured).
  pure def warnings(intent: Intent, inv: Inventory): Set[Warning] =
    val plan = derive(intent, inv)
    val ceilingWarnings = DERIVED_FAMILIES.fold(Set(), (acc, fam) =>
      val needed = plan.objs.filter(o => o.key.family == fam).size()
      match inv.ceilings.get(fam) {
        | Unknown => if (needed > 0) acc.union(Set(UnknownCeiling({ family: fam, needed: needed }))) else acc
        | _ => acc
      })
    // one UnmeasuredCombination per userspace-poll tenant whose terminated
    // ports span more than one seeded rate class (design D3): the formula
    // prices the mix, but cross-class pricing is unmeasured — flag it, never
    // pass silently.
    intent.tenants.foldl(ceilingWarnings, (acc, c) =>
      if (c.dataplane == UserspacePoll and seededRateClasses(intent, c.name).size() > 1)
        acc.union(Set(UnmeasuredCombination({ tenant: c.name, rates: seededRateClasses(intent, c.name) })))
      else acc)

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
    TenantAbsent({ construct: "c", tenant: "x" }),
    MemberUnresolved({ fabric: "f", member: MPort("p") }),
    SelfMember({ fabric: "f", member: MTenant("c") }),
    Unanchored({ port: "p", dpmac: 0 }),
    ReservedAnchor({ port: "p", dpmac: 0, why: "w" }),
    ForeignAnchor({ port: "p", dpmac: 0, owner: "o" }),
    DoubleClaimed({ dpmac: 0, constructs: Set() }),
    OverRate({ port: "p", rate: 0, maxRate: 0 }),
    FabricNotKernelForwarded({ fabric: "f", forwardedBy: "o" }),
    PortTenantMismatch({ fabric: "f", port: "p", tenant: "o" }),
    UnsupportedEdge({ fabric: "f", member: "m" }),
    UnknownRateClass({ tenant: "c", rates: [] }),
    CoreBudgetExceeded({ tenant: "c", t: 0, maxCores: 0 }),
    ExtraNotCompanion({ tenant: "c", family: Dpni }),
    ExtraNotPositive({ tenant: "c", family: Dpio, count: 0 }),
    CryptoFlowsNotPositive({ tenant: "c", ordinal: 0, flows: 0 }),
    CryptoFlowsOverDevice({ tenant: "c", ordinal: 0, flows: 0, maxFlows: 0 }),
    Infeasible({ family: Dpbp, needed: 0, available: 0 }),
    UnpricedDataplane({ tenant: "c", dataplane: UserspaceEvent }),
    PoolWithoutRestricted({ tenant: "c", pool: "h" }),
    RestrictedWithoutPool({ tenant: "c" }),
    HolderNotPublic({ tenant: "c", holder: "h" }),
    PoolChain({ tenant: "c", holder: "h" }),
    PoolDataplaneMismatch({ tenant: "c", drawer: UserspacePoll, holder: KernelNetlink }))
  pure val WARNINGS_USE: Set[Warning] = Set(
    UnknownCeiling({ family: Dpio, needed: 0 }),
    UnmeasuredCombination({ tenant: "c", rates: Set() }))
  pure val COMPILED_USE: Set[Compiled] = Set(
    Ok({ plan: { objs: Set(), edges: Set(), order: [], provenance: Map() }, warnings: Set() }),
    Refused(Set()))
}
