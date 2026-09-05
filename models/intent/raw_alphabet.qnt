// The dirty raw alphabet (openspec change `intent-layer`, task 3.3e, bead
// gqf.48; intent/alphabet.qnt is the accepted-intent counterpart). The finite
// alphabet random simulation draws `RawIntent`s from, grown one construct per
// step from the empty document — deliberately producing the near-misses the
// gqf.39 hole hid: duplicate port/link names, dangling references, the reserved
// `kernel` declared, zero/negative counts, self-loop links, unresolved fabric
// members, and non-companion/unknown extra families. Validity is NOT encoded in
// the generator — that is exactly how gqf.39 stayed invisible; the draw sets
// mix clean and dirty values freely, and `parse` (intent/intent_raw.qnt) is what
// separates them.
//
// The rung (rawInvariants) is the three named laws over every reachable raw:
// parseOkWellFormed (the surface-refusal law — an accepted parse is well-formed),
// acceptedSurvives (the no-surprise law — accepted declarations survive
// verbatim), and nearMissRefusedByName (every near-miss present is refused by its
// own name). The witnesses below evidence that the near-miss states are actually
// visited (`pnpm model:coverage` counts them), so a law that held only because a
// shape was never reached is caught.
//
// Named invariants: rawInvariants (parseOkWellFormed, acceptedSurvives,
//   nearMissRefusedByName, over the alphabet's reachable raws).
// Apalache-marked subset: none.
module intent_raw_alphabet {
  import types.* from "../core/types"            // Family
  import intent_types.* from "./types"           // Dataplane, Isolation, Switching, ISOLATIONS, SWITCHINGS, KERNEL, …
  import intent_raw.* from "./intent_raw"        // RawIntent, parse, parseRefusals, the laws, the is*/has* predicates

  var raw: RawIntent

  // ---- the finite dirty alphabet ----

  // Two non-kernel tenant names; the reserved kernel is drawable only through
  // addKernelTenant (the reserved-name near-miss).
  pure val R_TENANTS: Set[str] = Set("c1", "c2")
  // A small port-name set so a duplicate is a frequent draw (the DuplicateName
  // near-miss) — the generator never guards uniqueness.
  pure val R_PORT_NAMES: Set[str] = Set("p1", "p2")
  pure val R_LINK_NAMES: Set[str] = Set("l1", "l2")
  pure val R_FABRIC_NAMES: Set[str] = Set("f1", "f2")
  // Tenant references: two declared, the reserved kernel, "" (a kernel-defaulting
  // port owner), and "ghost" — undeclared, the dangling-reference near-miss.
  pure val R_TENANT_REFS: Set[str] = Set("c1", "c2", "kernel", "", "ghost")
  // Counts: negative and zero are representable (parse carries them; compile
  // refuses ExtraNotPositive) — the zero/negative near-miss.
  pure val R_COUNTS: Set[int] = Set(-1, 0, 1, 2)
  pure val R_RATES: Set[int] = Set(-1, 0, 10000)
  pure val R_DPMACS: Set[int] = Set(7, 8, 99)
  // Family names: a companion, a non-companion real family, and an unknown one
  // (the UnknownExtraFamily near-miss). ExtraNotCompanion is compile's, not
  // parse's — "dpni" parses, so it is not a raw refusal here.
  pure val R_FAMILIES: Set[str] = Set("dpio", "dpni", "dpwidget")
  // Fabric member draws: a port, a tenant, the kernel, a declared fabric, and
  // "ghost" — the unresolved-member near-miss.
  pure val R_MEMBERS: Set[str] = Set("p1", "c1", "kernel", "f1", "ghost")
  // Pool draws: "" and a name, so both isolation/pool contradictions arise.
  pure val R_POOLS: Set[str] = Set("", "c1")

  action init = raw' = emptyRaw

  // A userspace tenant with an isolation/pool combination drawn freely (both
  // contradictions reachable). put overwrites, so tenants stay <= 2 non-kernel.
  action addTenant = {
    nondet name = R_TENANTS.oneOf()
    nondet iso = ISOLATIONS.oneOf()
    nondet pool = R_POOLS.oneOf()
    all {
      raw.tenants.keys().size() < 3,
      raw' = raw.with("tenants",
        raw.tenants.put(name, { dataplane: UserspacePoll, maxCores: 8, isolation: iso, pool: pool })),
    }
  }

  // The reserved kernel declared as a tenant — the ReservedKernel near-miss.
  action addKernelTenant = all {
    not(raw.tenants.keys().contains(KERNEL)),
    raw' = raw.with("tenants",
      raw.tenants.put(KERNEL, { dataplane: KernelNetlink, maxCores: 16, isolation: Isolated, pool: "" })),
  }

  // A port appended (never dedup-guarded): a repeated name is the DuplicateName
  // near-miss, a "ghost"/"" owner the dangling/kernel case, a <= 0 rate the
  // zero/negative case.
  action addPort = {
    nondet nm = R_PORT_NAMES.oneOf()
    nondet dp = R_DPMACS.oneOf()
    nondet rt = R_RATES.oneOf()
    nondet tn = R_TENANT_REFS.oneOf()
    all {
      raw.ports.length() < 3,
      raw' = raw.with("ports", raw.ports.append({ name: nm, dpmac: dp, rate: rt, tenant: tn })),
    }
  }

  // A link with ends drawn freely: equal ends are the LinkSelfLoop near-miss, a
  // "ghost" end the dangling case.
  action addLink = {
    nondet nm = R_LINK_NAMES.oneOf()
    nondet a = R_TENANT_REFS.oneOf()
    nondet b = R_TENANT_REFS.oneOf()
    all {
      raw.links.length() < 2,
      raw' = raw.with("links", raw.links.append({ name: nm, interfaceA: a, interfaceB: b })),
    }
  }

  // A fabric with a single member drawn freely: a "ghost" member is the
  // RawMemberUnresolved near-miss, a "ghost" forwarder the dangling case.
  action addFabric = {
    nondet fn = R_FABRIC_NAMES.oneOf()
    nondet fb = R_TENANT_REFS.oneOf()
    nondet m = R_MEMBERS.oneOf()
    nondet sw = SWITCHINGS.oneOf()
    all {
      raw.fabrics.keys().size() < 2,
      raw' = raw.with("fabrics", raw.fabrics.put(fn, { switching: sw, forwardedBy: fb, members: [m] })),
    }
  }

  // One extra family per tenant table (put overwrites the tenant's inner map):
  // an unknown family is the UnknownExtraFamily near-miss, a "ghost" tenant the
  // dangling case, a <= 0 count the zero/negative case.
  action addExtra = {
    nondet t = R_TENANT_REFS.oneOf()
    nondet fam = R_FAMILIES.oneOf()
    nondet c = R_COUNTS.oneOf()
    all {
      raw.extras.keys().size() < 3,
      raw' = raw.with("extras", raw.extras.put(t, Map(fam -> c))),
    }
  }

  action stutter = raw' = raw

  action step = any {
    addTenant, addKernelTenant, addPort, addLink, addFabric, addExtra, stutter,
  }

  // ---- the rung (intent/intent_raw.qnt laws) ----

  val rawInvariants = and {
    parseOkWellFormed(raw),
    acceptedSurvives(raw),
    nearMissRefusedByName(raw),
  }

  // ---- coverage witnesses (task 3.3e) ----
  // One named predicate per near-miss dimension, counted by `pnpm model:coverage`
  // (--witnesses): the seeded run reports for how many traces each held. A
  // witness at 0 traces is a recorded unknown, never a silent gap — the honesty
  // mechanism intent/alphabet.qnt applies to the accepted-intent side.

  val wRawAccepted = match parse(raw) { | ParseOk(_) => true | ParseRefused(_) => false }
  val wRawRefused = parseRefusals(raw) != Set()
  val wReservedKernel = refusesWith(raw, isReservedKernel)
  val wDuplicateName = refusesWith(raw, isDuplicateName)
  val wLinkSelfLoop = refusesWith(raw, isLinkSelfLoop)
  val wRawMemberUnresolved = refusesWith(raw, isRawMemberUnresolved)
  val wUnknownExtraFamily = refusesWith(raw, isUnknownExtraFamily)
  val wTenantAbsent = refusesWith(raw, isTenantAbsent)
  val wPoolWithoutRestricted = refusesWith(raw, isPoolWithoutRestricted)
  val wRestrictedWithoutPool = refusesWith(raw, isRestrictedWithoutPool)
  // The zero/negative-count near-miss is representable in the surface (parse
  // carries it through; compile refuses it) — witnessed on the raw shape.
  val wNonPositiveExtra =
    raw.extras.keys().exists(t => raw.extras.get(t).keys().exists(f => raw.extras.get(t).get(f) <= 0))
  val wNonPositiveRate = raw.ports.foldl(false, (a, p) => a or p.rate <= 0)
  // Structure: the shapes a trace must reach to exercise the clauses.
  val wKernelDeclared = raw.tenants.keys().contains(KERNEL)
  val wRawPortPresent = raw.ports.length() > 0
  val wRawLinkPresent = raw.links.length() > 0
  val wRawFabricPresent = raw.fabrics.keys().size() > 0
  val wRawExtraPresent = raw.extras.keys().size() > 0
}
