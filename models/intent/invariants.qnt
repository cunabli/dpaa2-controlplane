// The plan relationships design D6 wants unrepresentable in Rust, first
// stated here as named predicates over the derived `Plan` (openspec change
// `intent-layer`, design D6; spec.md "Plan relationships are unrepresentable
// if wrong"). D6 locks edges, resource-container (dprc) memberships, companions, emission
// order, key identity, and provenance closure by construction; the Rust type
// surface (task 3.1) transcribes what these predicates prove, so a
// free-standing companion, a dpmac at a pseudo-wire end, a double-connected
// dpni, or a tenant object in the root dprc is not a value the plan
// type admits. Each `INTENT_In` cites its evidence anchor and names what the
// Rust constructor makes unrepresentable.
//
// The ids `INTENT_I1..I8` are this change's own; the ledger rows that tie
// them to the baseline invariants land at task 5.1.
//
// Named invariants: containmentByTenant … companionCountsByRegime
//   (ids INTENT_I1..I8, below); planInvariants gathers I1–I6 over any Plan,
//   compileInvariants gathers I7–I8 over (intent, inventory).
// Apalache-marked subset: feasibleAgainstCeilings and companionCountsByRegime
//   (INTENT_I7/I8) — collected as intent/alphabet.qnt `compileLaws`, defined
//   for Apalache but simulator-only today: the translation succeeds, the
//   front-end heap does not (intent/alphabet.qnt header; bead
//   dpaa2-controlplane-gqf.26, task 1.5).
module intent_invariants {
  import types.* from "../core/types"           // Family, Option, Perm, ObjId
  import companions as comp from "../core/companions" // comp::Regime, comp::companionDraw (ADR-0012)
  import intent_types.* from "./types"           // Intent, Inventory, KERNEL, DERIVED_FAMILIES
  import intent_derive.* from "./derive"         // Plan, Key, PlanObj, End, Edge, derive, sizeTenant, dprtcKey
  import intent_refuse.* from "./refuse"         // compile, Compiled, Warning

  // ---- helpers ----

  pure def objKeys(p: Plan): Set[Key] = p.objs.map(o => o.key)

  pure def objByKey(p: Plan, k: Key): Option[PlanObj] =
    p.objs.fold(None, (acc, o) => if (o.key == k) Some(o) else acc)

  pure def countOf(p: Plan, tenant: str, fam: Family): int =
    p.objs.filter(o => o.key.tenant == tenant and o.key.family == fam).size()

  pure def ordinalsOf(p: Plan, tenant: str, fam: Family): Set[int] =
    p.objs.filter(o => o.key.tenant == tenant and o.key.family == fam).map(o => o.key.ordinal)

  // Position of a key in the emission order, or -1 if absent (copied from
  // main.qnt to avoid a circular import: main.qnt imports this module).
  pure def keyIndex(order: List[Key], k: Key): int =
    order.foldl({ i: 0, found: -1 }, (acc, x) =>
      { i: acc.i + 1, found: if (acc.found >= 0) acc.found else if (x == k) acc.i else -1 }).found

  pure def endsOf(e: Edge): Set[End] = Set(e.a, e.b)

  // The four companion families D6/ADR-0012 derive; their rule name is the
  // lowercase family name the derivation keys their provenance node by.
  pure val COMPANION_FAMS: Set[Family] = Set(Dpio, Dpbp, Dpmcp, Dpcon)
  pure def companionRule(f: Family): str =
    if (f == Dpio) "dpio" else if (f == Dpbp) "dpbp" else if (f == Dpmcp) "dpmcp" else "dpcon"

  // End shape predicates (object-model.md §2 edge table).
  pure def isMac(en: End): bool = match en { | MacEnd(_) => true | _ => false }
  pure def isFam(en: End, f: Family): bool = match en { | ObjEnd(x) => x.key.family == f | _ => false }
  // An edge matches an unordered pair of end predicates (§2 edges are ↔).
  pure def edgeMatches(e: Edge, pa: (End) => bool, pb: (End) => bool): bool =
    (pa(e.a) and pb(e.b)) or (pa(e.b) and pb(e.a))

  // The multiset of every edge endpoint, and how often a given end occurs.
  pure def allEnds(p: Plan): List[End] =
    p.edges.fold([], (acc, e) => acc.append(e.a).append(e.b))
  pure def endCount(p: Plan, en: End): int =
    allEnds(p).foldl(0, (acc, x) => if (x == en) acc + 1 else acc)
  pure def endsSet(p: Plan): Set[End] =
    p.edges.fold(Set(), (acc, e) => acc.union(endsOf(e)))

  // An ObjEnd is well-formed: its key exists and its family/port surface is
  // a dpni (port 0) or a dpsw interface (port within the switch's num_ifs).
  pure def endWellFormed(p: Plan, en: End): bool =
    match en {
      | MacEnd(_) => true
      | ObjEnd(x) => and {
          objKeys(p).contains(x.key),
          match x.key.family {
            | Dpni => x.port == 0
            | Dpsw => match objByKey(p, x.key) {
                | Some(o) => match o.attrs { | DpswAttrs(a) => x.port >= 0 and x.port < a.numIfs | _ => false }
                | None => false
              }
            | _ => false
          },
        }
    }

  // ---- containmentByTenant (INTENT_I1): one resource container (dprc) per object, root never a tenant ----
  // object-model.md §1 (the DPRC tree); dprc.md (child DPRC per tenant);
  // ADR-0001 §4 (dprc.1 is the kernel's, never a named tenant). The Rust
  // constructor (task 3.1) places every non-kernel, non-Dprc object in its
  // tenant's Child dprc and never renders a dpmac or dpdbg as a
  // derived object.
  pure def containmentByTenant(p: Plan): bool =
    val ks = objKeys(p)
    p.objs.forall(o => and {
      (o.key.tenant != KERNEL and o.key.family != Dprc) implies and {
        o.container == Child(o.key.tenant),
        ks.contains({ tenant: o.key.tenant, family: Dprc, ordinal: 1 }),
      },
      (o.key.tenant == KERNEL) implies o.container == Root,
      (o.key.family == Dprc) implies (o.container == Root and o.key.ordinal == 1),
      o.key.family != Dpmac,
      o.key.family != Dpdbg,
    })

  // ---- edgesTypedAndSingle (INTENT_I2): typed connect ends, no double connect ----
  // object-model.md §2 (edge table + "cardinality one"); DPNI-I9 (the
  // pseudo-wire). The Rust edge constructor takes the deriving construct as a
  // witness, so a dpmac↔dpmac edge, a dpmac at a link/wire end, or a second
  // peer on an already-connected end cannot be built.
  pure def edgesTypedAndSingle(p: Plan): bool = and {
    endsSet(p).forall(en => endCount(p, en) <= 1),
    p.edges.forall(e => and {
      not(isMac(e.a) and isMac(e.b)),
      endWellFormed(p, e.a),
      endWellFormed(p, e.b),
      (e.prov.rule == "link-edge" or e.prov.rule == "fabric-wire")
        implies (isFam(e.a, Dpni) and isFam(e.b, Dpni)),
      (e.prov.rule == "port-edge")
        implies edgeMatches(e, en => isFam(en, Dpni), en => isMac(en)),
      (e.prov.rule == "fabric-edge")
        implies edgeMatches(e, en => isFam(en, Dpsw), en => (isMac(en) or isFam(en, Dpni))),
    }),
  }

  // ---- companionsOnlyDerived (INTENT_I3): companions only as a tenant's derivation ----
  // ADR-0012; design D6. A dpio/dpbp/dpmcp/dpcon exists only as its
  // tenant's count node, never free-standing; the Rust `Tenant`
  // constructor is the only producer of a companion, and its ordinal run is
  // exactly the node's value.
  pure def companionsOnlyDerived(p: Plan): bool =
    p.objs.forall(o =>
      COMPANION_FAMS.contains(o.key.family) implies
        val pk = { tenant: o.key.tenant, rule: companionRule(o.key.family), construct: "" }
        and {
          o.prov == pk,
          p.prov.keys().contains(pk),
          countOf(p, o.key.tenant, o.key.family) == p.prov.get(pk).value,
          ordinalsOf(p, o.key.tenant, o.key.family) == 1.to(p.prov.get(pk).value),
        })

  // ---- emissionOrderLawful (INTENT_I4): emission order ----
  // object-model.md §5 step 1 (pool companions before tenant objects);
  // ADR-0012 (a kernel dpio draws a dpmcp, so dpio precedes dpmcp). Order is
  // a property of the Rust constructors, not a post-hoc sort.
  pure def emissionOrderLawful(p: Plan): bool =
    val ks = objKeys(p)
    and {
      p.order.length() == p.objs.size(),
      p.order.foldl(Set(), (acc, k) => acc.union(Set(k))) == ks,
      p.order.nth(0) == dprtcKey,
      // the child DPRC precedes every other key of its tenant
      ks.forall(k1 => ks.forall(k2 =>
        (k1.family == Dprc and k2.tenant == k1.tenant and k2 != k1)
          implies keyIndex(p.order, k1) < keyIndex(p.order, k2))),
      // every pool companion precedes the tenant objects that draw it
      ks.forall(k1 => ks.forall(k2 =>
        (COMPANION_FAMS.contains(k1.family) and Set(Dpni, Dpseci, Dpsw).contains(k2.family)
         and k2.tenant == k1.tenant)
          implies keyIndex(p.order, k1) < keyIndex(p.order, k2))),
      // in the kernel, every dpio precedes every dpmcp (its forgotten draw)
      ks.forall(k1 => ks.forall(k2 =>
        (k1.tenant == KERNEL and k1.family == Dpio and k2.tenant == KERNEL and k2.family == Dpmcp)
          implies keyIndex(p.order, k1) < keyIndex(p.order, k2))),
    }

  // ---- keysAreIdentities (INTENT_I5): keys are identities, labels a projection ----
  // ADR-0010 §4 (the key is the identity, the label a lossy projection). No
  // two objects share a key, and every ordinal is 1-based; the Rust `Key` is
  // the map key, the rendered label a derived field.
  pure def keysAreIdentities(p: Plan): bool = and {
    p.objs.size() == objKeys(p).size(),
    p.objs.forall(o => o.key.ordinal >= 1),
  }

  // ---- provenanceClosed (INTENT_I6): provenance is closed ----
  // design D6 (the tree reaches the construct and the anchor). Every prov
  // reference resolves, every node carries an anchor, and the only rule that
  // may be Unmeasured today is "T"; a second unmeasured rule must be declared
  // in this predicate before it can appear in a plan. The Rust provenance
  // type is a closed DAG, not a dangling triple.
  pure def provenanceClosed(p: Plan): bool =
    val pk = p.prov.keys()
    and {
      p.objs.forall(o => pk.contains(o.prov)),
      p.edges.forall(e => pk.contains(e.prov)),
      pk.forall(k => p.prov.get(k).inputs.forall(i => pk.contains(i))),
      pk.forall(k => p.prov.get(k).anchor != ""),
      pk.forall(k => (p.prov.get(k).mark == Unmeasured) implies (p.prov.get(k).rule == "T")),
    }

  // ---- feasibleAgainstCeilings (INTENT_I7): feasibility ----
  // ADR-0011 (the three-valued ceiling); design D2. A compile is either a
  // plan within every checkable ceiling (Counted/Observed) with an
  // UnknownCeiling warning for every non-zero Unknown family, or a non-empty
  // refusal set — never an empty refusal. The Rust `compile` returns a total
  // Result whose error branch is never empty.
  pure def feasibleAgainstCeilings(intent: Intent, inv: Inventory): bool =
    match compile(intent, inv) {
      | Ok(r) => DERIVED_FAMILIES.forall(fam =>
          val count = r.plan.objs.filter(o => o.key.family == fam).size()
          match inv.ceilings.get(fam) {
            | Counted(n) => count <= n
            | Observed(o) => count <= o.n
            | Unknown => (count > 0) implies r.warnings.contains(UnknownCeiling({ family: fam, needed: count }))
          })
      | Refused(rs) => rs.size() > 0
    }

  // ---- companionCountsByRegime (INTENT_I8): companion counts by dataplane ----
  // ADR-0012 via companionDraw. On a successful compile every tenant's
  // companion counts are the sizing's effective values, each effective value
  // is exactly the request plus the declared extra (0 when none), the dpio
  // request is exactly companionDraw's field, and every dpni carries at least
  // T (poll-mode) or exactly cpus (kernel) transmit queues. The Rust `Tenant`
  // constructor draws these numbers, never the operator.
  pure def companionCountsByRegime(intent: Intent, inv: Inventory): bool =
    match compile(intent, inv) {
      | Ok(r) => intent.tenants.foldl(true, (acc, c) =>
          val s = sizeTenant(intent, inv, c)
          val reqDraw = if (c.dataplane == KernelNetlink) comp::companionDraw(comp::Kernel, s.cpus, 0, 1, s.dpnis)
                        else comp::companionDraw(comp::PollMode, 0, s.t, 1, s.dpnis)
          acc and and {
            countOf(r.plan, c.name, Dpio) == s.effDpio.value,
            countOf(r.plan, c.name, Dpbp) == s.effDpbp.value,
            countOf(r.plan, c.name, Dpmcp) == s.effDpmcp.value,
            countOf(r.plan, c.name, Dpcon) == s.effDpcon.value,
            s.effDpio.value == s.effDpio.request + extraOf(intent, c.name, Dpio),
            s.effDpbp.value == s.effDpbp.request + extraOf(intent, c.name, Dpbp),
            s.effDpmcp.value == s.effDpmcp.request + extraOf(intent, c.name, Dpmcp),
            s.effDpcon.value == s.effDpcon.request + extraOf(intent, c.name, Dpcon),
            s.effDpio.request == reqDraw.dpio,
            r.plan.objs.forall(o => (o.key.tenant == c.name and o.key.family == Dpni) implies
              match o.attrs {
                | DpniAttrs(a) => if (c.dataplane == KernelNetlink) a.numQueues == s.cpus else a.numQueues >= s.t
                | _ => false
              }),
          })
      | Refused(_) => true
    }

  // ---- the two rungs ----

  pure def planInvariants(p: Plan): bool = and {
    containmentByTenant(p), edgesTypedAndSingle(p), companionsOnlyDerived(p), emissionOrderLawful(p), keysAreIdentities(p), provenanceClosed(p),
  }
  pure def compileInvariants(intent: Intent, inv: Inventory): bool = and {
    feasibleAgainstCeilings(intent, inv), companionCountsByRegime(intent, inv),
  }
}
