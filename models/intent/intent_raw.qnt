// The config surface enters the model (openspec change `intent-layer`, task
// 3.3e, bead gqf.48; ADR-0013 §2 the constructs, §5 the refusal vocabulary;
// design D1/D5/D10). Until this module the model's state space began AFTER the
// lossy parse: `Intent` is what `dpaa2-config` already validated, so a
// duplicated block, a dangling reference, or the reserved `kernel` declared
// were unrepresentable — the gqf.39/gqf.40 bug class was a question Quint could
// not state. `RawIntent` mirrors the post-3.3b/3.3c/3.3d TOML surface faithfully, and
// `parse` mirrors `crates/dpaa2-config/src/parse.rs`'s convert/validation
// semantics, so the surface pathologies are representable and the laws that
// refuse them are checkable.
//
// What is keyed in TOML is a map here, where the format itself makes an
// intra-family duplicate unrepresentable (a second `[tenant.<name>]` /
// `[fabric.<name>]` / `[port.<name>]` / `[link.<name>]` table or a repeated
// `family = count` under one `[extra.<tenant>]` is a TOML key-redefinition
// parse error, ADR-0013 §2): `tenants`, `fabrics`, `ports`, `links`, `extras`.
// Task 3.3d hoisted the port/link name into the key (ADR-0015 decision 1), so
// two ports of one name are no longer representable; a name shared ACROSS
// families (a port and a fabric both "x") IS still representable across the
// separate maps and IS refused DuplicateName. What stays positional or
// referential stays representable-dirty: `crypto` as a `List` (ADR-0015
// decision 4 — genuinely anonymous, declaration order IS the dpseci ordinal),
// every cross-construct reference as a raw `str` (a dangling one is
// representable), every count as an `int` (zero/negative representable), the
// reserved `"kernel"` representable, and an extra family as a raw `str` (an
// unknown family representable).
//
// DELIBERATELY NOT MODELLED (format/lexical rejections `parse.rs` makes over
// fields the neutral `Intent` does not carry — the serde/format boundary, not
// the surface-vs-model representability gap this parcel targets, and analogous
// to TOML's own key-uniqueness and enum-value rejection): the `dpmac = "N"`
// string lexing (`dpmac` is carried as `int`, as the model `Port` does), the
// `mac`/`mac_mode`/`dpni`-pinning slots, the six rejected derived-count keys,
// the `name` IFNAMSIZ/charset check (the same lexical rule the `renamed.from`
// VALUE must satisfy — task 6.3; the `from` value's name check lives in parse.rs
// only, exactly like the declared `name`, so the model carries `from` as a raw
// `str` and checks only rule (i), the declared-and-not-itself-renamed clause),
// the `[intent]` schema version, and an
// unknown `dataplane`/`isolation`/`switching` enum value (`Dataplane` /
// `Isolation` / `Switching` are reused verbatim — a bad enum value is a format
// error, exactly like a duplicate key). Each is reported in the parcel summary.
//
// DEVIATION (`parse.rs` short-circuits on the first `?`; `parse` here collects
// every violation and refuses iff the set is non-empty, matching refuse.qnt's
// "all violations at once" compiler idiom): the accept/reject decision is
// identical to `parse.rs`; the refusal SET is a superset of `parse.rs`'s single
// reported error, which the near-miss laws below rely on.
//
// Named invariants: parseOkWellFormed (surface-refusal law), acceptedSurvives
//   (no-surprise law), nearMissRefusedByName (each near-miss refuses by name) —
//   collected as intent/raw_alphabet.qnt `rawInvariants`, simulator-only.
// Apalache-marked subset: none (the quantifier shape is measured in the parcel
//   summary; the seal decision is upstream, per the compileLaws precedent).
module intent_raw {
  import types.* from "../core/types"            // Family, Option, and the 16 family constructors
  import intent_types.* from "./types"           // Intent, Tenant, Port, Link, Fabric, Crypto, Extra, Member, Dataplane, Isolation, Switching, KERNEL
  import intent_refuse.* from "./refuse"         // Refusal (TenantAbsent reused verbatim for a dangling tenant reference)

  // ---- the raw TOML surface (ADR-0013 §2) ----

  // DEVIATION (vocabulary-v2 D1): the raw surface keeps `isolation` and `pool` as
  // TWO independent fields, so the TOML contradictions (a `restricted` isolation
  // with no `pool` key, or a `pool` key on a non-`restricted` tenant) stay
  // representable and parse still names them. The neutral `Isolation` now folds the
  // pool into its `Restricted` payload, so the raw surface can no longer reuse it;
  // it carries its own payload-free `RawIsolation` tri-state instead — the exact
  // shape `crates/dpaa2-config/src/schema.rs` `RawIsolation` + optional `pool`
  // already has (Rust led here). A bad enum value is still a format error, not a
  // surface pathology (like a duplicate key). `toIntent` folds the two raw fields
  // back into the neutral `Restricted(pool)` payload once parse has accepted them.
  type RawIsolation = RPublic | RRestricted | RIsolated

  // A `[tenant.<name>]` value (the name lives in the map key). `dataplane` reuses
  // the neutral enum; `isolation` is the payload-free `RawIsolation`. `pool` is a
  // raw `str` — "" means absent; a dangling holder is representable and left to
  // compile (parse.rs does not resolve the pool).
  // `from` (TOML key `renamed.from`, ADR-0015 decision 10 / task 6.3) declares the
  // tenant's prior name; "" means absent. Refined rule (i) refuses it only when the
  // target is a declared tenant not itself renamed away.
  type RawTenant = { dataplane: Dataplane, maxCores: int, isolation: RawIsolation, pool: str, from: str }

  // A `[port.<name>]` value (the name lives in the map key, task 3.3d / ADR-0015
  // decision 1). An intra-family duplicate is a TOML key redefinition,
  // unrepresentable; a name shared with a link or fabric is a cross-family
  // collision that IS representable and refused DuplicateName. `tenant` is a raw
  // `str` ("" means the reserved kernel terminates it, parse.rs convert_port), a
  // dangling tenant representable; `dpmac`/`rate` are ints (the "dpmac.N" lexing
  // is the format boundary, not modelled here).
  // `from` (TOML key `renamed.from`, ADR-0015 decision 10 / task 6.3) declares the
  // port's prior name; "" means absent. It shares the port/link/fabric namespace.
  type RawPort = { dpmac: int, rate: int, tenant: str, from: str }

  // A `[link.<name>]` value (the name lives in the map key, task 3.3d): a
  // dpni↔dpni pseudo-wire between two tenant ends. Both ends are raw `str`
  // (dangling representable); interfaceA == interfaceB is a self-loop parse.rs
  // refuses (compile has no such refusal — a new raw variant).
  // `from` (TOML key `renamed.from`, ADR-0015 decision 10 / task 6.3) declares the
  // link's prior name; "" means absent. It shares the port/link/fabric namespace.
  type RawLink = { interfaceA: str, interfaceB: str, from: str }

  // A `[fabric.<name>]` value (the name lives in the map key). `forwardedBy` is a
  // raw `str`; `members` are raw `str`s parse classifies into a `Member` or
  // refuses unresolved (parse.rs classify_member) — a duplicate/dangling member
  // is representable.
  // `from` (TOML key `renamed.from`, ADR-0015 decision 10 / task 6.3) declares the
  // fabric's prior name; "" means absent. It shares the port/link/fabric namespace.
  type RawFabric = { switching: Switching, forwardedBy: str, members: List[str], from: str }

  // A `[[crypto]]` table: one accelerator for one tenant, `flows` a raw int.
  type RawCrypto = { tenant: str, flows: int }

  // The whole document. Keyed constructs are maps (an intra-family duplicate is
  // unrepresentable — the format refuses it): `tenants`, `ports`, `links`,
  // `fabrics`. `crypto` stays a `List` (ADR-0015 decision 4 — anonymous,
  // declaration order IS the dpseci ordinal). Referential fields stay raw
  // strings (dangles representable). `extras` is a tenant → (family-name →
  // count) map: a duplicate (tenant, family) is a TOML key redefinition
  // (ADR-0013 §2), the family key a raw `str` an unknown value is representable
  // in.
  type RawIntent = {
    tenants: str -> RawTenant,
    ports: str -> RawPort,
    links: str -> RawLink,
    fabrics: str -> RawFabric,
    crypto: List[RawCrypto],
    extras: str -> (str -> int),
  }

  pure val emptyRaw: RawIntent =
    { tenants: Map(), ports: Map(), links: Map(), fabrics: Map(), crypto: [], extras: Map() }

  // ---- the refusal vocabulary ----

  // A parse refusal. `FromCompile` reuses refuse.qnt's `Refusal` verbatim where
  // parse.rs refuses something the compile-stage vocabulary already names
  // (TenantAbsent for a dangling tenant reference). The remaining variants are
  // raw-surface refusals parse.rs makes that the compile vocabulary lacks: the
  // reserved kernel declared, a duplicate construct name TOML keys cannot dedupe,
  // a self-loop link, a fabric member string that classifies to nothing (compile's
  // MemberUnresolved carries an already-classified `Member`, so it cannot name an
  // unclassifiable string), an unknown extra family, and — since vocabulary-v2 D1
  // made both illegal shapes unrepresentable in the neutral `Isolation` — the two
  // isolation/pool contradictions, now raw-native rather than `FromCompile` of a
  // compile variant (parse still names them; compile no longer has the variant).
  // The vocabulary graduates to ADR-0013 §5 + R14 when dpaa2-config exposes a typed parse-error enum.
  type RawRefusal =
    | FromCompile(Refusal)                                // TenantAbsent (a dangling tenant reference)
    | PoolWithoutRestricted({ tenant: str, pool: str })   // a `pool` key on a non-`restricted` tenant (parse.rs convert_tenant)
    | RestrictedWithoutPool({ tenant: str })              // a `restricted` tenant with no `pool` key (parse.rs convert_tenant)
    | ReservedKernel({ name: str })                       // [tenant.kernel] declares the reserved name (parse.rs convert)
    | DuplicateName({ name: str })                        // a port/link/fabric name TOML keys cannot dedupe (parse.rs convert)
    | LinkSelfLoop({ link: str, tenant: str })            // a link naming one tenant at both ends (parse.rs convert_link)
    | RawMemberUnresolved({ fabric: str, member: str })   // a fabric member string resolving to no port/tenant/fabric (parse.rs classify_member)
    | UnknownExtraFamily({ tenant: str, family: str })    // an [extra.<tenant>] family not one of the 16 (parse.rs parse_family)
    // A `renamed = { from }` whose target is currently declared and NOT itself
    // renamed away — ADR-0015 decision 10 rule (i) as refined (task 6.3): the swap
    // and the chain declare every `from`-target renamed away, so this refuses only
    // the genuine contradiction where an object would be claimed twice (parse.rs
    // convert). Namespace-agnostic: `construct`/`from` are a tenant pair or a
    // port/link/fabric pair (the two namespaces are checked separately).
    | RenamedFromDeclared({ construct: str, from: str })

  // Ok(Intent) or Refused(refusals). Named apart from Compiled's Ok/Refused
  // (refuse.qnt), whose constructors are already in scope through the flat import.
  type Parsed = ParseOk(Intent) | ParseRefused(Set[RawRefusal])

  // ---- family-name mapping (parse.rs parse_family over ALL_FAMILIES) ----

  pure val FAMILY_NAMES: List[(str, Family)] = [
    ("dprc", Dprc), ("dpni", Dpni), ("dpmac", Dpmac), ("dpbp", Dpbp),
    ("dpio", Dpio), ("dpcon", Dpcon), ("dpmcp", Dpmcp), ("dpseci", Dpseci),
    ("dpsw", Dpsw), ("dpdmux", Dpdmux), ("dpaiop", Dpaiop), ("dpci", Dpci),
    ("dpdcei", Dpdcei), ("dpdmai", Dpdmai), ("dprtc", Dprtc), ("dpdbg", Dpdbg)]

  pure def familyOfName(s: str): Option[Family] =
    FAMILY_NAMES.foldl(None, (acc, pair) => if (pair._1 == s) Some(pair._2) else acc)

  // ---- shared lookups over the raw surface ----

  pure def declaredTenants(raw: RawIntent): Set[str] = raw.tenants.keys()

  // A tenant reference resolves: a declared tenant, or the reserved kernel
  // (parse.rs `resolves`; the kernel is never declared yet always nameable).
  pure def rawResolves(raw: RawIntent, n: str): bool =
    n == KERNEL or raw.tenants.keys().contains(n)

  pure def portNamesSet(raw: RawIntent): Set[str] = raw.ports.keys()

  // The port/link/fabric name namespace parse.rs checks for collisions; tenant
  // names live in their own namespace and are not included (parse.rs convert).
  // Each family is now keyed, so a name appears at most once per family; a name
  // appearing twice in this list is therefore a CROSS-family collision (task
  // 3.3d) — the only DuplicateName path left.
  pure def constructNamesL(raw: RawIntent): List[str] =
    raw.ports.keys().fold([], (a, n) => a.append(n))
      .concat(raw.links.keys().fold([], (a, n) => a.append(n)))
      .concat(raw.fabrics.keys().fold([], (a, n) => a.append(n)))

  // The port/link/fabric key union — the one namespace a construct's `renamed.from`
  // is checked against (parse.rs; task 6.3). Tenants live in their own namespace.
  pure def constructKeys(raw: RawIntent): Set[str] =
    raw.ports.keys().union(raw.links.keys()).union(raw.fabrics.keys())

  // The `from` clause carried by whichever family holds `n`, or "" when `n` is not
  // a declared construct (parse.rs looks the target up across port/link/fabric).
  pure def constructOwnFrom(raw: RawIntent, n: str): str =
    if (raw.ports.keys().contains(n)) raw.ports.get(n).from
    else if (raw.links.keys().contains(n)) raw.links.get(n).from
    else if (raw.fabrics.keys().contains(n)) raw.fabrics.get(n).from
    else ""

  // Whether a fabric member string classifies to a declared port, tenant, or
  // fabric (parse.rs classify_member; port first, then tenant, then fabric).
  pure def memberResolvesRaw(raw: RawIntent, m: str): bool =
    portNamesSet(raw).contains(m) or rawResolves(raw, m) or raw.fabrics.keys().contains(m)

  pure def classifyMember(raw: RawIntent, m: str): Member =
    if (portNamesSet(raw).contains(m)) MPort(m)
    else if (rawResolves(raw, m)) MTenant(m)
    else MFabric(m)

  // ---- the refusals, one clause per parse.rs check ----

  // parse.rs convert: `[tenant.kernel]` declares the reserved name.
  pure def reservedKernelRefusals(raw: RawIntent): Set[RawRefusal] =
    if (raw.tenants.keys().contains(KERNEL)) Set(ReservedKernel({ name: KERNEL })) else Set()

  // parse.rs convert: a construct name shared ACROSS families (a port and a
  // fabric, or a link and a port, of one name). Intra-family duplicates are now
  // TOML key redefinitions the map cannot represent (task 3.3d), so only the
  // cross-family collision remains for this rule to catch.
  pure def duplicateNameRefusals(raw: RawIntent): Set[RawRefusal] =
    val ns = constructNamesL(raw)
    ns.foldl(Set(), (acc, n) =>
      if (ns.select(x => x == n).length() >= 2) acc.union(Set(DuplicateName({ name: n }))) else acc)

  // parse.rs convert_tenant: an isolation/pool contradiction (holder resolution
  // is compile's, not parse's — so it is not checked here).
  pure def poolContradictionRefusals(raw: RawIntent): Set[RawRefusal] =
    raw.tenants.keys().fold(Set(), (acc, n) =>
      val t = raw.tenants.get(n)
      if (t.isolation == RRestricted and t.pool == "")
        acc.union(Set(RestrictedWithoutPool({ tenant: n })))
      else if (t.isolation != RRestricted and t.pool != "")
        acc.union(Set(PoolWithoutRestricted({ tenant: n, pool: t.pool })))
      else acc)

  // parse.rs convert_port/convert_link/convert_fabric/convert_crypto/convert_extra:
  // a construct naming an undeclared tenant refuses TenantAbsent (reused from
  // compile); a link naming one tenant at both ends refuses LinkSelfLoop.
  pure def tenantRefRefusals(raw: RawIntent): Set[RawRefusal] =
    val fromPorts = raw.ports.keys().fold(Set(), (acc, n) =>
      val p = raw.ports.get(n)
      // An omitted port tenant ("") is the reserved kernel, which always resolves
      // (vocabulary-v2 D2 folds the retired ""→kernel port default); only a NAMED
      // undeclared tenant is absent, and its raw string is the TenantAbsent payload.
      if (p.tenant == "" or rawResolves(raw, p.tenant)) acc
      else acc.union(Set(FromCompile(TenantAbsent({ construct: n, tenant: p.tenant })))))
    val fromLinks = raw.links.keys().fold(fromPorts, (acc, n) =>
      val l = raw.links.get(n)
      val a1 = if (rawResolves(raw, l.interfaceA)) acc
               else acc.union(Set(FromCompile(TenantAbsent({ construct: n, tenant: l.interfaceA }))))
      val a2 = if (rawResolves(raw, l.interfaceB)) a1
               else a1.union(Set(FromCompile(TenantAbsent({ construct: n, tenant: l.interfaceB }))))
      if (l.interfaceA == l.interfaceB) a2.union(Set(LinkSelfLoop({ link: n, tenant: l.interfaceA }))) else a2)
    val fromFabrics = raw.fabrics.keys().fold(fromLinks, (acc, n) =>
      if (rawResolves(raw, raw.fabrics.get(n).forwardedBy)) acc
      else acc.union(Set(FromCompile(TenantAbsent({ construct: n, tenant: raw.fabrics.get(n).forwardedBy })))))
    val fromCrypto = raw.crypto.foldl(fromFabrics, (acc, k) =>
      if (rawResolves(raw, k.tenant)) acc
      else acc.union(Set(FromCompile(TenantAbsent({ construct: "crypto", tenant: k.tenant })))))
    raw.extras.keys().fold(fromCrypto, (acc, t) =>
      if (rawResolves(raw, t)) acc
      else acc.union(Set(FromCompile(TenantAbsent({ construct: "extra", tenant: t })))))

  // parse.rs classify_member: a fabric member that classifies to nothing.
  pure def memberRefusals(raw: RawIntent): Set[RawRefusal] =
    raw.fabrics.keys().fold(Set(), (acc, n) =>
      raw.fabrics.get(n).members.foldl(acc, (a2, m) =>
        if (memberResolvesRaw(raw, m)) a2
        else a2.union(Set(RawMemberUnresolved({ fabric: n, member: m })))))

  // parse.rs parse_family: an [extra.<tenant>] family name not one of the 16.
  pure def extraFamilyRefusals(raw: RawIntent): Set[RawRefusal] =
    raw.extras.keys().fold(Set(), (acc, t) =>
      raw.extras.get(t).keys().fold(acc, (a2, fam) =>
        match familyOfName(fam) {
          | Some(_) => a2
          | None => a2.union(Set(UnknownExtraFamily({ tenant: t, family: fam })))
        }))

  // parse.rs convert: a `renamed = { from }` naming a currently-declared construct
  // that is NOT itself renamed away — ADR-0015 decision 10 rule (i) as refined (task
  // 6.3). A tenant's `from` is checked against declared tenant keys; a
  // port/link/fabric's `from` against the shared construct key union, the target's
  // own `from` looked up in whichever family holds it. A `from`-target that itself
  // carries a `renamed` clause is admitted (the swap and the chain), and the
  // reserved `kernel` is never declared, so `from = "kernel"` passes and is inert.
  pure def renamedFromRefusals(raw: RawIntent): Set[RawRefusal] =
    val tenantR = raw.tenants.keys().fold(Set(), (acc, n) =>
      val f = raw.tenants.get(n).from
      if (f != "" and raw.tenants.keys().contains(f) and raw.tenants.get(f).from == "")
        acc.union(Set(RenamedFromDeclared({ construct: n, from: f })))
      else acc)
    val ck = constructKeys(raw)
    val familyR = ck.fold(tenantR, (acc, n) =>
      val f = constructOwnFrom(raw, n)
      if (f != "" and ck.contains(f) and constructOwnFrom(raw, f) == "")
        acc.union(Set(RenamedFromDeclared({ construct: n, from: f })))
      else acc)
    familyR

  // Every clause runs; the refusal set is their union (the compiler idiom).
  pure def parseRefusals(raw: RawIntent): Set[RawRefusal] =
    reservedKernelRefusals(raw)
      .union(duplicateNameRefusals(raw))
      .union(poolContradictionRefusals(raw))
      .union(tenantRefRefusals(raw))
      .union(memberRefusals(raw))
      .union(extraFamilyRefusals(raw))
      .union(renamedFromRefusals(raw))

  // ---- conversion into the neutral Intent (parse.rs convert) ----

  // toIntent carries every construct's `from` clause through into the neutral
  // `Intent` (task 6.5, ADR-0015 decision 10): parse validates rule (i) up front,
  // and the accepted clause now feeds the rename matcher's acceptance set instead
  // of being dropped ("" = absent, the `pool` convention). compile still ignores
  // `from` (intent/derive.qnt) — the clause is the matcher's, not the compiler's.
  // Fold the raw surface's two isolation fields into the neutral `Isolation`
  // payload (vocabulary-v2 D1): a `restricted` raw isolation carries its `pool`
  // key into `Restricted(pool)`. Called only on an accepted raw, where parse has
  // already refused a `restricted` with no pool and a pool on a non-`restricted`
  // tenant, so the pool value is exactly the accepted holder name.
  pure def rawIsolationToIsolation(iso: RawIsolation, pool: str): Isolation =
    match iso {
      | RPublic => Public
      | RRestricted => Restricted(pool)
      | RIsolated => Isolated
    }

  pure def toIntent(raw: RawIntent): Intent =
    { // Tenants emit in canonical NAME order (task 3.3d), matching the Rust parser's
      // BTreeMap-sorted emission: the neutral Intent keeps a List, but the order carries
      // no identity — the derivation sorts by name (intent/derive.qnt fullOrder) — so
      // emitting name-sorted keeps the conversion deterministic and position-independent.
      tenants: sortByRank(raw.tenants.keys().fold([], (acc, n) => acc.append(n)), n => nameRank(n))
        .foldl([], (acc, n) =>
          val t = raw.tenants.get(n)
          acc.append({ name: n, dataplane: t.dataplane, maxCores: t.maxCores,
            isolation: rawIsolationToIsolation(t.isolation, t.pool), from: t.from })),
      // Ports and links emit in canonical NAME order (task 3.3d): the neutral
      // Intent keeps a List, but the order carries no identity — the derivation
      // sorts by name (intent/derive.qnt) — so emitting name-sorted keeps the
      // conversion deterministic and matches the Rust parser's sorted emission.
      ports: sortByRank(raw.ports.keys().fold([], (acc, n) => acc.append(n)), n => nameRank(n))
        .foldl([], (acc, n) =>
          val p = raw.ports.get(n)
          // Lower the raw tenant string into the neutral `TenantRef` sum: "" (the
          // omitted-tenant default) and "kernel" both become Kernel, else Named
          // (vocabulary-v2 D2, the parse-boundary normalisation `tenantRefOf`).
          acc.append({ name: n, dpmac: p.dpmac, rate: p.rate, tenant: tenantRefOf(p.tenant), from: p.from })),
      links: sortByRank(raw.links.keys().fold([], (acc, n) => acc.append(n)), n => nameRank(n))
        .foldl([], (acc, n) =>
          val l = raw.links.get(n)
          // Lower each raw link end into the neutral `TenantRef` sum: an explicit
          // "kernel" end becomes Kernel, else Named (vocabulary-v2 D2).
          acc.append({ name: n, interfaceA: tenantRefOf(l.interfaceA), interfaceB: tenantRefOf(l.interfaceB), from: l.from })),
      fabrics: raw.fabrics.keys().fold([], (acc, n) =>
        val f = raw.fabrics.get(n)
        acc.append({ name: n, switching: f.switching, forwardedBy: f.forwardedBy,
          members: f.members.foldl([], (a2, m) => a2.append(classifyMember(raw, m))), from: f.from })),
      crypto: raw.crypto.foldl([], (acc, k) => acc.append({ tenant: k.tenant, flows: k.flows })),
      extras: raw.extras.keys().fold(Set(), (acc, t) =>
        raw.extras.get(t).keys().fold(acc, (a2, fam) =>
          match familyOfName(fam) {
            | Some(f) => a2.union(Set({ tenant: t, family: f, count: raw.extras.get(t).get(fam) }))
            | None => a2
          })),
    }

  // The total parse: Ok(Intent) with an empty refusal set, else Refused(set).
  pure def parse(raw: RawIntent): Parsed =
    val rs = parseRefusals(raw)
    if (rs == Set()) ParseOk(toIntent(raw)) else ParseRefused(rs)

  // ---- refusal predicates (for the near-miss laws and witnesses) ----

  pure def isReservedKernel(r: RawRefusal): bool = match r { | ReservedKernel(_) => true | _ => false }
  pure def isDuplicateName(r: RawRefusal): bool = match r { | DuplicateName(_) => true | _ => false }
  pure def isLinkSelfLoop(r: RawRefusal): bool = match r { | LinkSelfLoop(_) => true | _ => false }
  pure def isRawMemberUnresolved(r: RawRefusal): bool = match r { | RawMemberUnresolved(_) => true | _ => false }
  pure def isUnknownExtraFamily(r: RawRefusal): bool = match r { | UnknownExtraFamily(_) => true | _ => false }
  pure def isRenamedFromDeclared(r: RawRefusal): bool = match r { | RenamedFromDeclared(_) => true | _ => false }
  pure def isTenantAbsent(r: RawRefusal): bool =
    match r { | FromCompile(x) => match x { | TenantAbsent(_) => true | _ => false } | _ => false }
  pure def isPoolWithoutRestricted(r: RawRefusal): bool =
    match r { | PoolWithoutRestricted(_) => true | _ => false }
  pure def isRestrictedWithoutPool(r: RawRefusal): bool =
    match r { | RestrictedWithoutPool(_) => true | _ => false }

  // ---- shape predicates (the near-miss is present in the raw surface) ----
  // Each is derived from parse's resolution independently of parseRefusals, so
  // an omission in a refusal clause fails nearMissRefusedByName rather than
  // hiding — the gqf.39 hole was exactly a shape the generator could not state.

  pure def hasReservedKernelDecl(raw: RawIntent): bool = raw.tenants.keys().contains(KERNEL)

  pure def hasDuplicateConstruct(raw: RawIntent): bool =
    val ns = constructNamesL(raw)
    ns.foldl(Set(), (a, x) => a.union(Set(x))).size() != ns.length()

  pure def hasSelfLoop(raw: RawIntent): bool =
    raw.links.keys().exists(n => raw.links.get(n).interfaceA == raw.links.get(n).interfaceB)

  pure def hasUnknownExtraFamily(raw: RawIntent): bool =
    raw.extras.keys().exists(t => raw.extras.get(t).keys().exists(fam => familyOfName(fam) == None))

  pure def hasUnresolvedMember(raw: RawIntent): bool =
    raw.fabrics.keys().exists(n =>
      raw.fabrics.get(n).members.foldl(false, (a, m) => a or not(memberResolvesRaw(raw, m))))

  // A `renamed = { from }` present whose target is a declared construct not itself
  // renamed away (ADR-0015 decision 10 rule (i) as refined, task 6.3), across both
  // namespaces — derived independently of renamedFromRefusals.
  pure def hasRenamedFromDeclared(raw: RawIntent): bool = or {
    raw.tenants.keys().exists(n =>
      val f = raw.tenants.get(n).from
      f != "" and raw.tenants.keys().contains(f) and raw.tenants.get(f).from == ""),
    constructKeys(raw).exists(n =>
      val f = constructOwnFrom(raw, n)
      f != "" and constructKeys(raw).contains(f) and constructOwnFrom(raw, f) == ""),
  }

  pure def hasDanglingTenantRef(raw: RawIntent): bool = or {
    // A port dangles only when it NAMES a tenant ("" is the always-resolving
    // kernel default, vocabulary-v2 D2) that does not resolve.
    raw.ports.keys().exists(n =>
      val t = raw.ports.get(n).tenant
      t != "" and not(rawResolves(raw, t))),
    raw.links.keys().exists(n => not(rawResolves(raw, raw.links.get(n).interfaceA)) or not(rawResolves(raw, raw.links.get(n).interfaceB))),
    raw.fabrics.keys().exists(n => not(rawResolves(raw, raw.fabrics.get(n).forwardedBy))),
    raw.crypto.foldl(false, (a, k) => a or not(rawResolves(raw, k.tenant))),
    raw.extras.keys().exists(t => not(rawResolves(raw, t))),
  }

  // ---- the named laws ----

  // Well-formed: the neutral Intent carries none of the surface pathologies parse
  // guards — no reserved kernel among the declared tenants, unique construct
  // names, every tenant reference resolved, no self-loop link, the isolation/pool
  // contradiction absent, and every fabric member classified to a real construct.
  pure def intentTenantNames(i: Intent): Set[str] = i.tenants.foldl(Set(), (a, t) => a.union(Set(t.name)))
  pure def intentPortNames(i: Intent): Set[str] = i.ports.foldl(Set(), (a, p) => a.union(Set(p.name)))
  pure def intentFabricNames(i: Intent): Set[str] = i.fabrics.foldl(Set(), (a, f) => a.union(Set(f.name)))
  pure def refResolves(i: Intent, n: str): bool = n == KERNEL or intentTenantNames(i).contains(n)
  pure def intentConstructNamesL(i: Intent): List[str] =
    i.ports.foldl([], (a, p) => a.append(p.name))
      .concat(i.links.foldl([], (a, l) => a.append(l.name)))
      .concat(i.fabrics.foldl([], (a, f) => a.append(f.name)))
  pure def memberResolvesI(i: Intent, m: Member): bool =
    match m {
      | MPort(p) => intentPortNames(i).contains(p)
      | MTenant(c) => refResolves(i, c)
      | MFabric(h) => intentFabricNames(i).contains(h)
    }

  pure def wellFormed(i: Intent): bool =
    val cn = intentConstructNamesL(i)
    and {
      not(intentTenantNames(i).contains(KERNEL)),
      cn.foldl(Set(), (a, x) => a.union(Set(x))).size() == cn.length(),
      // Port owner and link ends are `TenantRef` (vocabulary-v2 D2): resolve each
      // to its name; the Kernel case always resolves (it is the reserved kernel).
      i.ports.foldl(true, (a, p) => a and refResolves(i, tenantRefName(p.tenant))),
      i.links.foldl(true, (a, l) => a and refResolves(i, tenantRefName(l.interfaceA)) and refResolves(i, tenantRefName(l.interfaceB)) and l.interfaceA != l.interfaceB),
      i.fabrics.foldl(true, (a, f) => a and refResolves(i, f.forwardedBy) and f.members.foldl(true, (b, m) => b and memberResolvesI(i, m))),
      i.crypto.foldl(true, (a, k) => a and refResolves(i, k.tenant)),
      i.extras.forall(e => refResolves(i, e.tenant)),
      // The isolation/pool contradiction is unrepresentable in the neutral
      // `Isolation` (vocabulary-v2 D1): a Restricted tenant carries its pool in the
      // payload and a non-Restricted one carries none, so there is no clause to
      // check here — parse's raw-native refusals guard the raw surface instead.
    }

  // Surface-refusal law: everything the surface can express that `Intent` cannot
  // represent is refused — a parse that accepts yields a well-formed Intent.
  pure def parseOkWellFormed(raw: RawIntent): bool =
    match parse(raw) { | ParseOk(i) => wellFormed(i) | ParseRefused(_) => true }

  // No-surprise law: an accepted declaration survives verbatim. Stated against
  // the RAW declarations (not toIntent, which would be tautological): every
  // declared extra pair is present with its own declared count — never a sum,
  // never a collapse — the counts of tenants/ports/crypto match, and every
  // accepted `renamed = { from }` clause survives verbatim on its tenant/port/
  // link/fabric (task 6.5 extends the no-surprise law to the rename clause), so
  // nothing is dropped, merged, or invented (the gqf.40 collapse made checkable).
  pure def extraPairCount(raw: RawIntent): int =
    raw.extras.keys().fold(0, (acc, t) => acc + raw.extras.get(t).keys().size())
  pure def acceptedSurvives(raw: RawIntent): bool =
    match parse(raw) {
      | ParseRefused(_) => true
      | ParseOk(i) => and {
          i.extras.size() == extraPairCount(raw),
          raw.extras.keys().forall(t => raw.extras.get(t).keys().forall(fam =>
            match familyOfName(fam) {
              | Some(f) => i.extras.contains({ tenant: t, family: f, count: raw.extras.get(t).get(fam) })
              | None => true
            })),
          i.tenants.length() == raw.tenants.keys().size(),
          i.ports.length() == raw.ports.keys().size(),
          i.links.length() == raw.links.keys().size(),
          i.crypto.length() == raw.crypto.length(),
          raw.tenants.keys().forall(n =>
            i.tenants.foldl(false, (a, t) => a or (t.name == n and t.from == raw.tenants.get(n).from))),
          raw.ports.keys().forall(n =>
            i.ports.foldl(false, (a, p) => a or (p.name == n and p.from == raw.ports.get(n).from))),
          raw.links.keys().forall(n =>
            i.links.foldl(false, (a, l) => a or (l.name == n and l.from == raw.links.get(n).from))),
          raw.fabrics.keys().forall(n =>
            i.fabrics.foldl(false, (a, f) => a or (f.name == n and f.from == raw.fabrics.get(n).from))),
        }
    }

  // Each near-miss present in the raw surface is refused by its own name — the
  // law the gqf.39 hole violated, made statable now the surface is representable.
  pure def refusesWith(raw: RawIntent, pred: (RawRefusal) => bool): bool =
    parseRefusals(raw).exists(pred)
  pure def nearMissRefusedByName(raw: RawIntent): bool = and {
    hasReservedKernelDecl(raw) implies refusesWith(raw, isReservedKernel),
    hasDuplicateConstruct(raw) implies refusesWith(raw, isDuplicateName),
    hasSelfLoop(raw) implies refusesWith(raw, isLinkSelfLoop),
    hasUnresolvedMember(raw) implies refusesWith(raw, isRawMemberUnresolved),
    hasUnknownExtraFamily(raw) implies refusesWith(raw, isUnknownExtraFamily),
    hasDanglingTenantRef(raw) implies refusesWith(raw, isTenantAbsent),
    hasRenamedFromDeclared(raw) implies refusesWith(raw, isRenamedFromDeclared),
  }

  // ---- directed runs (pnpm model:test) ----

  pure val rtPoll: RawTenant = { dataplane: UserspacePoll, maxCores: 8, isolation: RIsolated, pool: "", from: "" }

  // The reserved kernel declared as a tenant is refused by name (parse.rs convert).
  run rawReservedKernelTest =
    val raw = emptyRaw.with("tenants", Map(KERNEL -> { dataplane: KernelNetlink, maxCores: 16, isolation: RIsolated, pool: "", from: "" }))
    refusesWith(raw, isReservedKernel)

  // A construct name shared across families (a port and a fabric both "p1") —
  // the cross-family collision TOML keys cannot dedupe once each family is keyed
  // (parse.rs convert; task 3.3d). Two ports of one name are now unrepresentable.
  run rawDuplicateNameTest =
    val raw = emptyRaw
      .with("ports", Map("p1" -> { dpmac: 7, rate: 10000, tenant: "", from: "" }))
      .with("fabrics", Map("p1" -> { switching: Hardware, forwardedBy: KERNEL, members: [], from: "" }))
    refusesWith(raw, isDuplicateName)

  // A port naming an undeclared tenant is refused TenantAbsent (reused from compile).
  run rawDanglingTenantTest =
    val raw = emptyRaw.with("ports", Map("p1" -> { dpmac: 7, rate: 10000, tenant: "ghost", from: "" }))
    all { refusesWith(raw, isTenantAbsent), match parse(raw) { | ParseRefused(_) => true | ParseOk(_) => false } }

  // A link naming one tenant at both ends — a self-loop parse.rs refuses (compile
  // does not: the alphabet accepts a self-link).
  run rawLinkSelfLoopTest =
    val raw = emptyRaw.with("tenants", Map("c1" -> rtPoll)).with("links", Map("l1" -> { interfaceA: "c1", interfaceB: "c1", from: "" }))
    refusesWith(raw, isLinkSelfLoop)

  // An [extra.<tenant>] naming a family not one of the 16 (parse.rs parse_family).
  run rawUnknownFamilyTest =
    val raw = emptyRaw.with("tenants", Map("c1" -> rtPoll)).with("extras", Map("c1" -> Map("dpwidget" -> 1)))
    refusesWith(raw, isUnknownExtraFamily)

  // A fabric member classifying to nothing (parse.rs classify_member).
  run rawMemberUnresolvedTest =
    val raw = emptyRaw.with("fabrics", Map("f1" -> { switching: Hardware, forwardedBy: KERNEL, members: ["ghost"], from: "" }))
    refusesWith(raw, isRawMemberUnresolved)

  // The two isolation/pool contradictions (parse.rs convert_tenant).
  run rawPoolContradictionTest =
    val restrNoPool = emptyRaw.with("tenants", Map("c1" -> rtPoll.with("isolation", RRestricted)))
    val poolNonRestr = emptyRaw.with("tenants", Map("c1" -> rtPoll.with("pool", "c2")))
    all { refusesWith(restrNoPool, isRestrictedWithoutPool), refusesWith(poolNonRestr, isPoolWithoutRestricted) }

  // A clean document parses, and the accepted extra survives verbatim — count
  // 2 stays 2, one pair stays one pair (the no-surprise law on a concrete raw).
  run rawAcceptedVerbatimTest =
    val raw = emptyRaw
      .with("tenants", Map("router" -> rtPoll))
      .with("ports", Map("wan0" -> { dpmac: 7, rate: 10000, tenant: "router", from: "" }))
      .with("extras", Map("router" -> Map("dpio" -> 2)))
    match parse(raw) {
      | ParseOk(i) => all {
          i.extras == Set({ tenant: "router", family: Dpio, count: 2 }),
          i.ports.length() == 1,
          i.tenants.length() == 1,
          acceptedSurvives(raw),
          wellFormed(i),
        }
      | ParseRefused(_) => false
    }

  // ADR-0015 decision 10 rule (i), refined (task 6.3) — the directed cases.

  // (a) A port `eth0` renamed from a currently-declared `wan0` that carries no
  // clause of its own — the genuine contradiction, refused RenamedFromDeclared.
  run rawRenamedFromDeclaredTest =
    val raw = emptyRaw.with("ports", Map(
      "eth0" -> { dpmac: 7, rate: 10000, tenant: "", from: "wan0" },
      "wan0" -> { dpmac: 8, rate: 10000, tenant: "", from: "" }))
    refusesWith(raw, isRenamedFromDeclared)

  // (b) The swap `wan0` from `wan1` AND `wan1` from `wan0`, both declared — each
  // target is itself renamed away, so the refined rule ACCEPTS (parse succeeds).
  // (Names are drawn from the ranked pool, since an accepted doc reaches toIntent.)
  run rawRenameSwapTest =
    val raw = emptyRaw.with("ports", Map(
      "wan0" -> { dpmac: 7, rate: 10000, tenant: "", from: "wan1" },
      "wan1" -> { dpmac: 8, rate: 10000, tenant: "", from: "wan0" }))
    match parse(raw) { | ParseOk(_) => true | ParseRefused(_) => false }

  // (c) The tenant namespace: `edge` renamed from a declared tenant `c1` that
  // carries no clause — refused RenamedFromDeclared (same rule, its own namespace).
  run rawRenamedTenantFromDeclaredTest =
    val raw = emptyRaw.with("tenants", Map(
      "edge" -> { dataplane: UserspacePoll, maxCores: 8, isolation: RIsolated, pool: "", from: "c1" },
      "c1" -> rtPoll))
    refusesWith(raw, isRenamedFromDeclared)

  // (d) A plain rename `e0` from `wan0` where `wan0` is NOT declared — inert, so
  // the document is accepted (the common after-a-rename case).
  run rawPlainRenameTest =
    val raw = emptyRaw.with("ports", Map("e0" -> { dpmac: 7, rate: 10000, tenant: "", from: "wan0" }))
    match parse(raw) { | ParseOk(_) => true | ParseRefused(_) => false }

  // The surface-refusal and no-surprise laws hold on a clean raw and a dirty one.
  run rawLawsHoldTest =
    val clean = emptyRaw.with("tenants", Map("c1" -> rtPoll))
    val dirty = emptyRaw.with("ports", Map("p1" -> { dpmac: 7, rate: -1, tenant: "ghost", from: "" }))
    all {
      parseOkWellFormed(clean), acceptedSurvives(clean), nearMissRefusedByName(clean),
      parseOkWellFormed(dirty), acceptedSurvives(dirty), nearMissRefusedByName(dirty),
    }

  // A value use for every sum-type constructor in the module (QNT404; see
  // intent/types.qnt ETH_IFS block and intent/refuse.qnt REFUSALS_USE).
  // A value use for every RawIsolation constructor (QNT404; the raw_alphabet
  // draw set and the flattener both need each constructor used in this module).
  pure val RAW_ISOLATIONS: Set[RawIsolation] = Set(RPublic, RRestricted, RIsolated)

  pure val RAW_REFUSALS_USE: Set[RawRefusal] = Set(
    FromCompile(TenantAbsent({ construct: "c", tenant: "t" })),
    PoolWithoutRestricted({ tenant: "c", pool: "h" }),
    RestrictedWithoutPool({ tenant: "c" }),
    ReservedKernel({ name: KERNEL }),
    DuplicateName({ name: "n" }),
    LinkSelfLoop({ link: "l", tenant: "t" }),
    RawMemberUnresolved({ fabric: "f", member: "m" }),
    UnknownExtraFamily({ tenant: "t", family: "x" }),
    RenamedFromDeclared({ construct: "c", from: "old" }))
  pure val PARSED_USE: Set[Parsed] = Set(ParseOk(toIntent(emptyRaw)), ParseRefused(Set()))
}
