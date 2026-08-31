// The pure derivation `derive(intent, inventory): Plan` (openspec change
// `intent-layer`, design D3/D4/D6; ADR-0005 §2): intent plus the observed
// hardware offer become the complete object plan — every object keyed by
// (tenant, family, ordinal), every derived count carrying its rule, its
// source construct, and its evidence anchor as a provenance DAG. The
// numbers live in the ADRs, the family modules, and companions.qnt; they
// are read here by reference, never restated.
//
// The topology the fabric rules transcribe is DPAA2 User Manual §2.2.2,
// figure 6 (a)–(c): (a) a dpni↔dpmac port, (b) a dpni↔dpni pseudo-wire,
// (c) a dpsw with dpmac and dpni endpoints. A fabric carries a switching
// qualifier: Hardware is a dpsw (6c); Software is the owner bridging its
// own dpnis, seen only as 6b wires and 6a ports.
//
// This module assumes an intent the refusals of task 1.3 have not rejected
// (the total function's other half lands in intent/refuse.qnt): a claimed
// dpmac is anchored and single-owner, an override never sits below its
// request, a hardware fabric is kernel-forwarded, a rate class is seeded. The
// derivation of a refused intent is undefined, not wrong.
//
// Open items left for 1.3 (refusals) and 3.1 (Rust label rendering):
//   (a) restool caps set-label at 15 chars (dprc.md), so the rendered
//       `<tenant>/<family>/<ordinal>` overflows for long tenant names;
//       the Key is the identity, the label a lossy projection (ADR-0010).
//   (b) a tenant terminating an unseeded rate class has no thread count —
//       threadCount returns None; 1.3 refuses it UnknownRateClass. 1.2 reads
//       None as 0 and never reaches that intent. A mix of *seeded* classes
//       does have a thread count (the per-port worker sum) and compiles with
//       an unmeasured-combination warning (design D3).
//   (c) a portless poll-mode tenant takes T = 1 by the main-only rule
//       below (main + workers, no port ⇒ no worker); 1.3 may narrow it.
//   (d) Hardware-in-Hardware membership (a dpsw interface as another
//       dpsw's interface) derives nothing; 1.3 refuses it.
//   (e) a member port whose tenant differs from its fabric's forwarding
//       tenant is refused at 1.3.
//
// Named invariants: none here (intent/invariants.qnt, task 1.4).
// Apalache-marked subset: none.
module intent_derive {
  import types.* from "../core/types"           // Family, Option, Perm, ObjId
  import companions as comp from "../core/companions" // comp::Regime, comp::companionDraw (ADR-0012)
  import dpsw.* from "../families/dpsw"          // dpswParams.draw (kernel census row)
  import dpseci.* from "../families/dpseci"      // dpseciParams.draw (kernel census row)
  import intent_types.* from "./types"           // Intent, Tenant, Port, …, KERNEL

  // ---- the plan's shape (design D6) ----

  // A derived object's identity is its key: the label the MC carries is a
  // projection of it (ADR-0010 §4). Ordinals are 1-based positions.
  type Key = { tenant: str, family: Family, ordinal: int }

  // The label is a projection of the key; the Rust frontend renders
  // `<tenant>/<family>/<ordinal>` (task 3.1). Quint has no string ops,
  // so the projection is the identity.
  pure def labelOf(k: Key): Key = k

  // Where an object lives: Root is dprc.1 (the kernel's own container,
  // never a tenant); Child(tenant) is that tenant's child DPRC,
  // itself the key (tenant, Dprc, 1).
  type Container = Root | Child(str)

  // Per-family create-config the plan carries. Unsized families expose no
  // sizing knob here: dpio, dpbp, dpmcp, dpcon (priorities opaque,
  // DPCON-I3) and dprtc (DPRTC-I5).
  type Attrs =
    | Unsized
    | DpniAttrs({ numQueues: int })
    | DpseciAttrs({ numQueues: int, hasCg: bool })
    | DpswAttrs({ numIfs: int, maxFdbs: int, perFdbFlooding: bool, perFdbBroadcast: bool, ctrlIf: bool })
    | DprcAttrs({ options: Set[Perm] })

  // A provenance node's address: (tenant, rule, construct). Tenant-level
  // count rules use construct "" (one node per tenant); per-construct
  // rules carry the construct name — the fabric for dpsw/fabric-edge/
  // fabric-wire, the port for port-edge, the link for link-edge — so two
  // fabrics of one owner no longer collide on a single node.
  type ProvKey = { tenant: str, rule: str, construct: str }

  // Whether the value stands on measured evidence or on the declared,
  // visibly unmeasured rate-class table (design D3).
  type Mark = Measured | Unmeasured

  // A node of the provenance DAG (design D6): `inputs` are the ProvKeys it
  // consumed, `constructs` the declared construct names it bottoms out in,
  // `anchor` the ADR/baseline section the rule cites. The request/limit
  // idiom (design D5): `value = limit` when a limit exists else `request`;
  // for a non-count node request = value and limit = None.
  type ProvNode = {
    rule: str, anchor: str, mark: Mark,
    request: int, limit: Option[int], value: int,
    inputs: Set[ProvKey], constructs: Set[str],
  }

  // A planned object: its key, its container, its create-config, and the
  // node that emitted it.
  type PlanObj = { key: Key, container: Container, attrs: Attrs, prov: ProvKey }

  // A connect endpoint: an object's port surface, or a bare dpmac.
  type End = ObjEnd({ key: Key, port: int }) | MacEnd(int)

  type Edge = { a: End, b: End, prov: ProvKey }

  type Plan = {
    objs: Set[PlanObj],
    edges: Set[Edge],
    order: List[Key],           // emission order (object-model.md §5 step 1)
    prov: ProvKey -> ProvNode,  // the provenance DAG
  }

  // ---- knobs the gate resolves (design open questions) ----

  // Open question 2 (design.md): whether the kernel's dpio budget is the
  // online-CPU count (ADR-0012) or `max_cores`. Both readings carried as a
  // parameter until the gate picks one; false = online CPUs.
  pure val KERNEL_BUDGET_IS_DPIO_COUNT: bool = false

  // The child-DPRC option mask restool creates by default, verified on the
  // reference child (dprc.md ~line 58).
  pure val DPRC_DEFAULT_OPTIONS: Set[Perm] = Set(Spawn, Alloc, ObjCreate, IrqCfg)

  // ---- the worker table (design D3) ----

  // The per-rate-class worker count, seeded from the reference board and
  // visibly unmeasured on every T it prices (the T provenance node carries
  // Mark = Unmeasured). 10G (10000) ⇒ 2 workers: the seed, decomposed from
  // the verified configuration — two 10G ports in poll-mode ran
  // T = 5 = 1 main + 2·2 workers, the configuration ADR-0012's companion
  // numbers were verified against. 25G (25000) ⇒ 5 workers: declared,
  // linear-in-rate from the 10G row, unmeasured, operator sign-off pending at
  // the 2.6 gate. A rate class absent here has no worker count, so its
  // tenant's T is undefined (threadCount None) and intent/refuse.qnt refuses
  // it UnknownRateClass.
  pure val WORKER_TABLE: int -> int = Map(10000 -> 2, 25000 -> 5)

  // Workers a single port of the given rate class draws (design D3), or None
  // for an unseeded class (e.g. a 40G port).
  pure def workersPerPort(rate: int): Option[int] =
    if (WORKER_TABLE.keys().contains(rate)) Some(WORKER_TABLE.get(rate)) else None

  // T = 1 main + Σ workers(rate) over the ports the tenant terminates (design
  // D3, the explicit formula). A portless tenant is main-only ⇒ Some(1) (no
  // port ⇒ no worker); a port whose rate class has no worker row makes the
  // whole sum undefined ⇒ None, and refuse.qnt turns that into
  // UnknownRateClass. Every derived T is visibly unmeasured.
  pure def threadCount(ports: List[Port]): Option[int] =
    ports.foldl(Some(1), (acc, p) =>
      match acc {
        | None => None
        | Some(t) => match workersPerPort(p.rate) { | Some(w) => Some(t + w) | None => None }
      })

  pure def imin(a: int, b: int): int = if (a < b) a else b

  // ---- fabric member helpers (design D6; DPAA2 UM §2.2.2 fig. 6) ----

  pure def fabricByName(intent: Intent, n: str): Option[Fabric] =
    intent.fabrics.foldl(None, (acc, f) => if (f.name == n) Some(f) else acc)

  pure def portByName(intent: Intent, pn: str): Option[Port] =
    intent.ports.foldl(None, (acc, p) => if (p.name == pn) Some(p) else acc)

  pure def memberIsPort(m: Member, pn: str): bool =
    match m { | MPort(p) => p == pn | _ => false }
  pure def memberIsFabric(m: Member, fn: str): bool =
    match m { | MFabric(h) => h == fn | _ => false }

  // The tenant a member resolves to: a tenant member, or a software
  // fabric member's owner (its bridging). Ports and hardware fabrics resolve
  // to no tenant.
  pure def memberTenant(intent: Intent, m: Member): Option[str] =
    match m {
      | MTenant(c) => Some(c)
      | MFabric(h) => match fabricByName(intent, h) {
          | Some(f) => match f.switching { | Software => Some(f.forwardedBy) | Hardware => None }
          | None => None
        }
      | MPort(_) => None
    }

  // A port is hardware-switched (yields no dpni and no port-edge; its dpmac
  // is a dpsw interface instead) iff some Hardware fabric names it. A port
  // in a Software fabric is still terminated by the normal port rule (6a).
  pure def isHwSwitchedPort(intent: Intent, portName: str): bool =
    intent.fabrics.select(f =>
      f.switching == Hardware and
      f.members.select(m => memberIsPort(m, portName)).length() > 0).length() > 0

  // The tenant's terminated ports (owned and not switched into a hardware
  // fabric), in declaration order.
  pure def terminatedPorts(intent: Intent, name: str): List[Port] =
    intent.ports.select(p => p.tenant == name and not(isHwSwitchedPort(intent, p.name)))

  // The distinct seeded rate classes a tenant's terminated ports span. A set
  // larger than one is a cross-class mix: the formula prices it (each port by
  // its own worker row) but it carries an unmeasured-combination warning
  // (design D3; the warning is raised in intent/refuse.qnt). An unseeded class
  // is excluded here — it has no row and is refused UnknownRateClass instead.
  pure def seededRateClasses(intent: Intent, name: str): Set[int] =
    terminatedPorts(intent, name).foldl(Set(), (acc, p) =>
      if (WORKER_TABLE.keys().contains(p.rate)) acc.union(Set(p.rate)) else acc)

  // Whether X is attached to Hardware fabric f through f's own members: a
  // tenant member X, or a software-fabric member owned by X.
  pure def ownerAttachedViaMembers(intent: Intent, f: Fabric, x: str): bool =
    f.members.select(m => match m {
      | MTenant(c) => c == x
      | MFabric(h) => match fabricByName(intent, h) {
          | Some(g) => g.switching == Software and g.forwardedBy == x
          | None => false
        }
      | MPort(_) => false
    }).length() > 0

  // X attaches to Hardware fabric f iff a member of f resolves to X, or a
  // Software fabric X owns lists f as a member (a chain of switches).
  pure def attachesToHwFabric(intent: Intent, x: str, f: Fabric): bool =
    val viaMembers = ownerAttachedViaMembers(intent, f, x)
    val viaSoftwareChain =
      intent.fabrics.select(g =>
        g.switching == Software and g.forwardedBy == x and
        g.members.select(m => memberIsFabric(m, f.name)).length() > 0).length() > 0
    viaMembers or viaSoftwareChain

  // ---- dpni origins (design D6; object-model.md §2; UM §2.2.2 fig. 6) ----

  // Each dpni a tenant terminates, in the concatenation order that fixes
  // its ordinal: an owned terminated port (6a), a link end (6b, a self-link
  // yields two), an attachment to a Hardware fabric (a dpsw interface, 6c),
  // the owner's end of a Software fabric's pseudo-wire (6b), or a member's
  // end of a Software fabric it does not own (6b).
  type Origin =
      OPort(Port)
    | OLink({ link: Link, side: int })
    | OAttach(str)
    | OWireOwner({ fabric: str, peer: str })
    | OWireMember(str)

  pure def isPort(o: Origin, pn: str): bool =
    match o { | OPort(p) => p.name == pn | _ => false }
  pure def isLinkSide(o: Origin, ln: str, sd: int): bool =
    match o { | OLink(x) => x.link.name == ln and x.side == sd | _ => false }
  pure def isAttach(o: Origin, fn: str): bool =
    match o { | OAttach(n) => n == fn | _ => false }
  pure def isWireOwnerOf(o: Origin, g: str, peer: str): bool =
    match o { | OWireOwner(x) => x.fabric == g and x.peer == peer | _ => false }
  pure def isWireMemberOf(o: Origin, g: str): bool =
    match o { | OWireMember(n) => n == g | _ => false }

  pure def portOrigins(intent: Intent, name: str): List[Origin] =
    terminatedPorts(intent, name).foldl([], (acc, p) => acc.append(OPort(p)))

  pure def linkOrigins(intent: Intent, name: str): List[Origin] =
    intent.links.foldl([], (acc, l) =>
      val a1 = if (l.a == name) acc.append(OLink({ link: l, side: 0 })) else acc
      if (l.b == name) a1.append(OLink({ link: l, side: 1 })) else a1)

  // One OAttach per Hardware fabric the tenant attaches to, once per
  // (X, f) however many ways the attachment is implied.
  pure def attachOrigins(intent: Intent, name: str): List[Origin] =
    intent.fabrics.foldl([], (acc, f) =>
      if (f.switching == Hardware and attachesToHwFabric(intent, name, f))
        acc.append(OAttach(f.name))
      else acc)

  // One OWireOwner per member of a Software fabric X owns that resolves to a
  // tenant C ≠ X — X's end of the 6b pseudo-wire.
  pure def wireOwnerOrigins(intent: Intent, name: str): List[Origin] =
    intent.fabrics.foldl([], (acc, g) =>
      if (g.switching == Software and g.forwardedBy == name)
        g.members.foldl(acc, (a2, m) =>
          match memberTenant(intent, m) {
            | Some(c) => if (c != name) a2.append(OWireOwner({ fabric: g.name, peer: c })) else a2
            | None => a2
          })
      else acc)

  // One OWireMember per Software fabric X does not own but resolves into —
  // X's end of the wire.
  pure def wireMemberOrigins(intent: Intent, name: str): List[Origin] =
    intent.fabrics.foldl([], (acc, g) =>
      if (g.switching == Software and g.forwardedBy != name and
          g.members.select(m => memberTenant(intent, m) == Some(name)).length() > 0)
        acc.append(OWireMember(g.name))
      else acc)

  // The ordered dpni sources of a tenant: terminated ports, link ends,
  // hardware-fabric attachments, then software-fabric wire ends (owner then
  // member). Position + 1 is the dpni ordinal.
  pure def originList(intent: Intent, name: str): List[Origin] =
    portOrigins(intent, name)
      .concat(linkOrigins(intent, name))
      .concat(attachOrigins(intent, name))
      .concat(wireOwnerOrigins(intent, name))
      .concat(wireMemberOrigins(intent, name))

  // 1-based ordinal of the first origin the predicate accepts (0 if none).
  pure def ordinalWhere(list: List[Origin], pred: (Origin) => bool): int =
    list.foldl({ i: 0, found: -1 }, (acc, o) =>
      { i: acc.i + 1,
        found: if (acc.found >= 0) acc.found else if (pred(o)) acc.i else -1 }).found + 1

  pure def terminatedPortNames(intent: Intent, name: str): Set[str] =
    terminatedPorts(intent, name).foldl(Set(), (acc, p) => acc.union(Set(p.name)))

  // The construct names of a tenant's dpnis: a port name, a link name, or
  // — for a hardware-fabric attachment or either wire end — the fabric name.
  pure def dpniConstructs(intent: Intent, name: str): Set[str] =
    originList(intent, name).foldl(Set(), (acc, o) =>
      match o {
        | OPort(p) => acc.union(Set(p.name))
        | OLink(x) => acc.union(Set(x.link.name))
        | OAttach(fn) => acc.union(Set(fn))
        | OWireOwner(x) => acc.union(Set(x.fabric))
        | OWireMember(fn) => acc.union(Set(fn))
      })

  // ---- hardware-fabric interfaces (dpsw.if numbering, 0-based) ----

  // A dpni endpoint by tenant + attaching fabric: that tenant's OAttach
  // dpni on the fabric's dpsw.
  pure def attachEnd(intent: Intent, tenant: str, fabricName: str): End =
    ObjEnd({ key: { tenant: tenant, family: Dpni,
                    ordinal: ordinalWhere(originList(intent, tenant), o => isAttach(o, fabricName)) }, port: 0 })

  // The ordered endpoints of a Hardware fabric's dpsw interfaces: f.members
  // in list order — a member port's dpmac (MacEnd), a member tenant's or
  // member software-fabric-owner's attach dpni (ObjEnd), Hardware-in-
  // Hardware skipped (open item (d), refused at 1.3); then one interface per
  // Software fabric listing f (a chain of switches) whose owner is not
  // already attached through f's own members.
  pure def hwFabricEnds(intent: Intent, f: Fabric): List[End] =
    val fromMembers = f.members.foldl([], (acc, m) =>
      match m {
        | MPort(p) => match portByName(intent, p) {
            | Some(port) => acc.append(MacEnd(port.dpmac))
            | None => acc
          }
        | MTenant(c) => acc.append(attachEnd(intent, c, f.name))
        | MFabric(h) => match fabricByName(intent, h) {
            | Some(g) => if (g.switching == Software) acc.append(attachEnd(intent, g.forwardedBy, f.name)) else acc
            | None => acc
          }
      })
    intent.fabrics.foldl(fromMembers, (acc, g) =>
      if (g.switching == Software and
          g.members.select(m => memberIsFabric(m, f.name)).length() > 0 and
          not(ownerAttachedViaMembers(intent, f, g.forwardedBy)))
        acc.append(attachEnd(intent, g.forwardedBy, f.name))
      else acc)

  // ---- per-tenant sizing (design D3/D4/D5) ----

  type Eff = { request: int, limit: Option[int], value: int }

  // The request/limit idiom (design D5): a matching per-family Limit raises
  // the request to its value; else the request stands. Applies to the four
  // companion count families only. (1.3 refuses limit < request.)
  pure def effective(intent: Intent, name: str, fam: Family, request: int): Eff =
    val ls = intent.limits.filter(l => l.tenant == name and l.family == fam)
    if (ls.size() == 0) { request: request, limit: None, value: request }
    else val lv = ls.fold(0, (_, l) => l.limit)
         { request: request, limit: Some(lv), value: lv }

  // Only Hardware fabrics emit a dpsw; the software owner bridges its own
  // dpnis (open item), so "owned fabrics" for dpsw purposes are the Hardware
  // fabrics owned by X.
  pure def hwOwnedFabrics(intent: Intent, name: str): List[Fabric] =
    intent.fabrics.select(f => f.switching == Hardware and f.forwardedBy == name)

  pure def cryptoCount(intent: Intent, name: str): int =
    intent.crypto.filter(k => k.tenant == name).size()

  pure def containerOf(c: Tenant): Container =
    if (c.name == KERNEL) Root else Child(c.name)

  // ADR-0012 prices only the Kernel and UserspacePoll dataplanes; a
  // UserspaceInterrupt tenant has no companion draw, so the derivation never
  // sizes it — compile refuses it (intent/refuse.qnt UnpricedDataplane).
  pure def hasPricing(c: Tenant): bool =
    match c.dataplane { | UserspaceInterrupt => false | _ => true }

  // The kernel's dpio budget: online CPUs (ADR-0012) or max_cores — the
  // gate picks one (open question 2).
  pure def kernelCores(inv: Inventory, c: Tenant): int =
    if (KERNEL_BUDGET_IS_DPIO_COUNT) c.maxCores else inv.cpus

  pure def tOf(intent: Intent, c: Tenant): int =
    match threadCount(terminatedPorts(intent, c.name)) { | Some(t) => t | None => 0 }

  type Sizing = {
    name: str, isKernel: bool, container: Container,
    dpnis: int, t: int, cpus: int, numQueues: int,
    effDpio: Eff, effDpbp: Eff, effDpmcp: Eff, effDpcon: Eff,
    numDpseci: int, cryptoFlows: int, numDpsw: int,
  }

  // Companion draws by reference to companionDraw (ADR-0012). Poll-mode:
  // dpio 2·T, dpbp 2, dpmcp one per process (the primary; a secondary is a
  // dpmcp limit); dpni transmit queues ≥ T. Kernel: dpio one per online
  // CPU, dpbp/dpmcp one per consuming object plus one per dpio, and each
  // dpsw/dpseci adds its own family census row (families/dpsw.qnt,
  // families/dpseci.qnt draw). dpcon is one per polled queue (dpcon.md,
  // DPCON-I1): poll-mode dpnis·T, kernel imin(cpus, numQueues) per dpni.
  pure def sizeTenant(intent: Intent, inv: Inventory, c: Tenant): Sizing =
    val nm = c.name
    val isK = c.dataplane == Kernel
    val dpnis = originList(intent, nm).length()
    val t = tOf(intent, c)
    val cpus = kernelCores(inv, c)
    val numQ = if (isK) cpus else t
    val nDpsw = hwOwnedFabrics(intent, nm).length()
    val nDpseci = cryptoCount(intent, nm)
    val base = if (isK) comp::companionDraw(comp::Kernel, cpus, 0, 1, dpnis)
               else comp::companionDraw(comp::PollMode, 0, t, 1, dpnis)
    val reqDpio = base.dpio
    val reqDpbp = if (isK) base.dpbp + nDpsw * dpswParams.draw.dpbp + nDpseci * dpseciParams.draw.dpbp
                  else base.dpbp
    val reqDpmcp = if (isK) base.dpmcp + nDpsw * dpswParams.draw.dpmcp + nDpseci * dpseciParams.draw.dpmcp
                   else base.dpmcp
    val reqDpcon = if (isK) dpnis * imin(cpus, numQ) else dpnis * t
    { name: nm, isKernel: isK, container: containerOf(c),
      dpnis: dpnis, t: t, cpus: cpus, numQueues: numQ,
      effDpio: effective(intent, nm, Dpio, reqDpio),
      effDpbp: effective(intent, nm, Dpbp, reqDpbp),
      effDpmcp: effective(intent, nm, Dpmcp, reqDpmcp),
      effDpcon: effective(intent, nm, Dpcon, reqDpcon),
      numDpseci: nDpseci, cryptoFlows: c.cryptoFlows, numDpsw: nDpsw }

  // ---- objects ----

  // Apalache rejects a symbolic range bound ("Expected a constant integer
  // expression"), so every sizing fold runs over a constant range filtered to
  // n — the core model's machine.qnt MAX_ENDPOINT_PORTS pattern. The bound is
  // the largest count the sizing can produce: the kernel dpcon draw
  // dpnis·cpus with the reference offer's 6 usable dpmacs and 16 CPUs
  // (intent/inventory.qnt). Poll-mode ties, never exceeds, that bound — three
  // 25G ports give T = 16 = cpus (design D3 worker table), so a poll tenant's
  // dpnis·T reaches the same 96. A count past it truncates the fold, which
  // companionsOnlyDerived and keysAreIdentities trip loudly — never a silent
  // pass.
  pure val MAX_DERIVED_COUNT: int = 96
  pure def countRange(n: int): List[int] =
    range(1, MAX_DERIVED_COUNT + 1).select(i => i <= n)
  pure def countSet(n: int): Set[int] =
    1.to(MAX_DERIVED_COUNT).filter(i => i <= n)

  // A set comprehension, not a list fold: Apalache encodes filter/map over
  // a constant set symbolically, while an unrolled foldl over the 96-element
  // list multiplies the term at every inline site (the front-end, not Z3,
  // exhausts the heap).
  pure def sizedObjs(name: str, fam: Family, container: Container, n: int, attrs: Attrs, rule: str): Set[PlanObj] =
    countSet(n).map(i =>
      { key: { tenant: name, family: fam, ordinal: i },
        container: container, attrs: attrs,
        prov: { tenant: name, rule: rule, construct: "" } })

  // Every tenant whose name ≠ KERNEL gets one child DPRC in Root
  // (dprc.md ~line 281); the kernel tenant's objects live in Root.
  pure def dprcObjs(s: Sizing): Set[PlanObj] =
    if (s.isKernel) Set()
    else Set({ key: { tenant: s.name, family: Dprc, ordinal: 1 },
               container: Root, attrs: DprcAttrs({ options: DPRC_DEFAULT_OPTIONS }),
               prov: { tenant: s.name, rule: "dprc", construct: "" } })

  // One dpsw per owned Hardware fabric, ordinal by fabric list position among
  // the owner's Hardware fabrics; the kernel-bindable predicate (dpsw.md,
  // read-not-verified): num_ifs = the fabric's interface count, max_fdbs =
  // num_ifs, PER_FDB flooding and broadcast, control interface enabled. The
  // dpsw node is keyed by the fabric name (construct), so two fabrics of one
  // owner do not collide.
  pure def dpswObjs(intent: Intent, s: Sizing): Set[PlanObj] =
    hwOwnedFabrics(intent, s.name).foldl({ i: 0, set: Set() }, (acc, f) =>
      val ni = hwFabricEnds(intent, f).length()
      { i: acc.i + 1,
        set: acc.set.union(Set({
          key: { tenant: s.name, family: Dpsw, ordinal: acc.i + 1 },
          container: s.container,
          attrs: DpswAttrs({ numIfs: ni, maxFdbs: ni,
                             perFdbFlooding: true, perFdbBroadcast: true, ctrlIf: true }),
          prov: { tenant: s.name, rule: "dpsw", construct: f.name } })) }).set

  pure def tenantObjs(intent: Intent, inv: Inventory, c: Tenant): Set[PlanObj] =
    val s = sizeTenant(intent, inv, c)
    dprcObjs(s)
      .union(sizedObjs(s.name, Dpio, s.container, s.effDpio.value, Unsized, "dpio"))
      .union(sizedObjs(s.name, Dpmcp, s.container, s.effDpmcp.value, Unsized, "dpmcp"))
      .union(sizedObjs(s.name, Dpbp, s.container, s.effDpbp.value, Unsized, "dpbp"))
      .union(sizedObjs(s.name, Dpcon, s.container, s.effDpcon.value, Unsized, "dpcon"))
      .union(sizedObjs(s.name, Dpni, s.container, s.dpnis, DpniAttrs({ numQueues: s.numQueues }), "dpnis"))
      // one dpseci per Crypto owned by c (dpseci.md): num_queues ≥
      // crypto_flows, DPSECI_OPT_HAS_CG safety bit.
      .union(sizedObjs(s.name, Dpseci, s.container, s.numDpseci, DpseciAttrs({ numQueues: s.cryptoFlows, hasCg: true }), "dpseci"))
      .union(dpswObjs(intent, s))

  // dprtc.0 is pinned in Root (key ordinal 1), exactly one, kernel-owned (dprtc.md,
  // DPRTC-I1 singleton / DPRTC-I2 kernel-owned). dpdbg is never derived.
  pure val dprtcKey: Key = { tenant: KERNEL, family: Dprtc, ordinal: 1 }
  pure val dprtcObj: PlanObj =
    { key: dprtcKey, container: Root, attrs: Unsized, prov: { tenant: KERNEL, rule: "dprtc", construct: "" } }

  pure def allObjs(intent: Intent, inv: Inventory): Set[PlanObj] =
    intent.tenants.foldl(Set(dprtcObj), (acc, c) =>
      if (hasPricing(c)) acc.union(tenantObjs(intent, inv, c)) else acc)

  // ---- emission order (object-model.md §5 step 1) ----
  // A kernel dpio draws a dpmcp, so dpio precedes dpmcp (ADR-0012).

  pure def companionKeys(name: str, fam: Family, n: int): List[Key] =
    countRange(n).foldl([], (acc, i) => acc.append({ tenant: name, family: fam, ordinal: i }))

  pure def tenantOrder(intent: Intent, s: Sizing): List[Key] =
    (if (s.isKernel) [] else [{ tenant: s.name, family: Dprc, ordinal: 1 }])
      .concat(companionKeys(s.name, Dpio, s.effDpio.value))
      .concat(companionKeys(s.name, Dpmcp, s.effDpmcp.value))
      .concat(companionKeys(s.name, Dpbp, s.effDpbp.value))
      .concat(companionKeys(s.name, Dpcon, s.effDpcon.value))
      .concat(companionKeys(s.name, Dpni, s.dpnis))
      .concat(companionKeys(s.name, Dpseci, s.numDpseci))
      .concat(companionKeys(s.name, Dpsw, s.numDpsw))

  pure def fullOrder(intent: Intent, inv: Inventory): List[Key] =
    [dprtcKey].concat(intent.tenants.foldl([], (acc, c) =>
      if (hasPricing(c)) acc.concat(tenantOrder(intent, sizeTenant(intent, inv, c))) else acc))

  // ---- provenance nodes ----

  pure def dpioNode(s: Sizing): ProvNode =
    { rule: "dpio", anchor: "ADR-0012 (companions.qnt companionDraw)", mark: Measured,
      request: s.effDpio.request, limit: s.effDpio.limit, value: s.effDpio.value,
      inputs: Set({ tenant: s.name, rule: if (s.isKernel) "cpus" else "T", construct: "" }), constructs: Set() }

  pure def dpbpNode(s: Sizing): ProvNode =
    { rule: "dpbp",
      anchor: if (s.isKernel) "ADR-0012 (companions.qnt companionDraw); families/dpsw.qnt + dpseci.qnt draw"
              else "ADR-0012 (companions.qnt companionDraw)",
      mark: Measured, request: s.effDpbp.request, limit: s.effDpbp.limit, value: s.effDpbp.value,
      inputs: if (s.isKernel) Set({ tenant: s.name, rule: "dpnis", construct: "" }) else Set(), constructs: Set() }

  pure def dpmcpNode(s: Sizing): ProvNode =
    { rule: "dpmcp",
      anchor: if (s.isKernel) "ADR-0012 (companions.qnt companionDraw); families/dpsw.qnt + dpseci.qnt draw"
              else "companions.qnt: one MC portal per process",
      mark: Measured, request: s.effDpmcp.request, limit: s.effDpmcp.limit, value: s.effDpmcp.value,
      inputs: if (s.isKernel) Set({ tenant: s.name, rule: "cpus", construct: "" }, { tenant: s.name, rule: "dpnis", construct: "" }) else Set(),
      constructs: Set() }

  pure def dpniQueuesNode(s: Sizing): ProvNode =
    { rule: "dpni-queues",
      anchor: if (s.isKernel) "dpni.md line 64/226-236: ls-addni injects nproc; DPCON-I1"
              else "ADR-0012: transmit queues >= T",
      mark: Measured, request: s.numQueues, limit: None, value: s.numQueues,
      inputs: Set({ tenant: s.name, rule: if (s.isKernel) "cpus" else "T", construct: "" }), constructs: Set() }

  pure def dpconNode(s: Sizing): ProvNode =
    { rule: "dpcon", anchor: "dpcon.md DPCON-I1: one dpcon per polled queue", mark: Measured,
      request: s.effDpcon.request, limit: s.effDpcon.limit, value: s.effDpcon.value,
      inputs: Set({ tenant: s.name, rule: "dpnis", construct: "" }, { tenant: s.name, rule: "dpni-queues", construct: "" }),
      constructs: Set() }

  // One dpsw node per Hardware fabric, keyed by the fabric name (construct).
  pure def dpswNode(intent: Intent, f: Fabric): ProvNode =
    val ni = hwFabricEnds(intent, f).length()
    { rule: "dpsw", anchor: "dpsw.md kernel bindability predicate (read-not-verified)", mark: Measured,
      request: ni, limit: None, value: ni, inputs: Set(), constructs: Set(f.name) }

  pure val dprtcNode: ProvNode =
    { rule: "dprtc", anchor: "DPRTC-I1 singleton, DPRTC-I2 kernel-owned", mark: Measured,
      request: 1, limit: None, value: 1, inputs: Set(), constructs: Set() }

  pure def linkEdgeNode(l: Link): ProvNode =
    { rule: "link-edge", anchor: "object-model.md §2, DPNI-I9 pseudo-wire", mark: Measured,
      request: 0, limit: None, value: 0,
      inputs: Set({ tenant: l.a, rule: "dpnis", construct: "" }, { tenant: l.b, rule: "dpnis", construct: "" }),
      constructs: Set(l.name) }

  pure def fabricEdgeNode(f: Fabric): ProvNode =
    { rule: "fabric-edge", anchor: "DPAA2 User Manual §2.2.2 fig. 6c; dpsw.md", mark: Measured,
      request: 0, limit: None, value: 0, inputs: Set({ tenant: f.forwardedBy, rule: "dpsw", construct: f.name }),
      constructs: Set(f.name) }

  // The owner's end of every 6b pseudo-wire a Software fabric carries; inputs
  // are the owner's and each wired peer's dpnis.
  pure def fabricWireNode(intent: Intent, g: Fabric): ProvNode =
    { rule: "fabric-wire", anchor: "DPAA2 User Manual §2.2.2 fig. 6b; object-model.md §2 DPNI-I9", mark: Measured,
      request: 0, limit: None, value: 0,
      inputs: g.members.foldl(Set({ tenant: g.forwardedBy, rule: "dpnis", construct: "" }), (acc, m) =>
        match memberTenant(intent, m) {
          | Some(c) => if (c != g.forwardedBy) acc.union(Set({ tenant: c, rule: "dpnis", construct: "" })) else acc
          | None => acc
        }),
      constructs: Set(g.name) }

  pure def addTenantProv(intent: Intent, inv: Inventory, m: ProvKey -> ProvNode, c: Tenant): ProvKey -> ProvNode =
    val s = sizeTenant(intent, inv, c)
    val nm = s.name
    val pn = terminatedPortNames(intent, nm)
    val np = terminatedPorts(intent, nm).length()
    val m1 = m
      .put({ tenant: nm, rule: "ports", construct: "" },
           { rule: "ports", anchor: "design D1: declared port constructs", mark: Measured,
             request: np, limit: None, value: np, inputs: Set(), constructs: pn })
      .put({ tenant: nm, rule: "dpnis", construct: "" },
           { rule: "dpnis", anchor: "object-model.md §2: one dpni per terminated port, link end, fabric membership",
             mark: Measured, request: s.dpnis, limit: None, value: s.dpnis, inputs: Set(),
             constructs: dpniConstructs(intent, nm) })
      .put({ tenant: nm, rule: "dpio", construct: "" }, dpioNode(s))
      .put({ tenant: nm, rule: "dpbp", construct: "" }, dpbpNode(s))
      .put({ tenant: nm, rule: "dpmcp", construct: "" }, dpmcpNode(s))
      .put({ tenant: nm, rule: "dpni-queues", construct: "" }, dpniQueuesNode(s))
      .put({ tenant: nm, rule: "dpcon", construct: "" }, dpconNode(s))
    val m2 = if (s.isKernel)
               m1.put({ tenant: nm, rule: "cpus", construct: "" },
                      { rule: "cpus", anchor: "ADR-0012: one dpio per online CPU", mark: Measured,
                        request: s.cpus, limit: None, value: s.cpus, inputs: Set(), constructs: Set() })
             else
               m1.put({ tenant: nm, rule: "T", construct: "" },
                      { rule: "T",
                        anchor: "design D3: T = 1 + Σ workers over terminated ports; workers-per-port table (10G⇒2, 25G⇒5), unmeasured",
                        mark: Unmeasured,
                        request: s.t, limit: None, value: s.t,
                        inputs: Set({ tenant: nm, rule: "ports", construct: "" }), constructs: pn })
    val m3 = if (s.isKernel) m2
             else m2.put({ tenant: nm, rule: "dprc", construct: "" },
                         { rule: "dprc", anchor: "dprc.md: restool-default child options", mark: Measured,
                           request: 1, limit: None, value: 1, inputs: Set(), constructs: Set(nm) })
    val m4 = if (s.numDpseci > 0)
               m3.put({ tenant: nm, rule: "dpseci", construct: "" },
                      { rule: "dpseci", anchor: "dpseci.md: num_queues >= crypto_flows, DPSECI_OPT_HAS_CG",
                        mark: Measured, request: s.cryptoFlows, limit: None, value: s.cryptoFlows,
                        inputs: Set(), constructs: Set(nm) })
             else m3
    // one dpsw node per owned Hardware fabric (keyed by the fabric name)
    val m5 = hwOwnedFabrics(intent, nm).foldl(m4, (mm, f) =>
      mm.put({ tenant: nm, rule: "dpsw", construct: f.name }, dpswNode(intent, f)))
    // one port-edge node per owned terminated port (keyed by the port name)
    terminatedPorts(intent, nm).foldl(m5, (mm, p) =>
      mm.put({ tenant: nm, rule: "port-edge", construct: p.name },
             { rule: "port-edge", anchor: "object-model.md §2: dpni<->dpmac edge", mark: Measured,
               request: 0, limit: None, value: 0,
               inputs: Set({ tenant: nm, rule: "dpnis", construct: "" }), constructs: Set(p.name) }))

  pure def fullProv(intent: Intent, inv: Inventory): ProvKey -> ProvNode =
    val base = Map({ tenant: KERNEL, rule: "dprtc", construct: "" } -> dprtcNode)
    val withC = intent.tenants.foldl(base, (m, c) => if (hasPricing(c)) addTenantProv(intent, inv, m, c) else m)
    val withL = intent.links.foldl(withC, (m, l) => m.put({ tenant: l.a, rule: "link-edge", construct: l.name }, linkEdgeNode(l)))
    intent.fabrics.foldl(withL, (m, f) =>
      if (f.switching == Hardware)
        m.put({ tenant: f.forwardedBy, rule: "fabric-edge", construct: f.name }, fabricEdgeNode(f))
      else
        m.put({ tenant: f.forwardedBy, rule: "fabric-wire", construct: f.name }, fabricWireNode(intent, f)))

  // ---- edges ----

  // Port (6a): the owner's dpni for the port ↔ its dpmac (object-model.md §2).
  pure def portEdges(intent: Intent): Set[Edge] =
    intent.tenants.foldl(Set(), (acc, c) =>
      acc.union(terminatedPorts(intent, c.name).foldl(Set(), (a2, p) =>
        a2.union(Set({
          a: ObjEnd({ key: { tenant: c.name, family: Dpni,
                             ordinal: ordinalWhere(originList(intent, c.name), o => isPort(o, p.name)) }, port: 0 }),
          b: MacEnd(p.dpmac),
          prov: { tenant: c.name, rule: "port-edge", construct: p.name } })))))

  // Link (6b): the two link-end dpnis.
  pure def linkEdges(intent: Intent): Set[Edge] =
    intent.links.foldl(Set(), (acc, l) =>
      acc.union(Set({
        a: ObjEnd({ key: { tenant: l.a, family: Dpni,
                           ordinal: ordinalWhere(originList(intent, l.a), o => isLinkSide(o, l.name, 0)) }, port: 0 }),
        b: ObjEnd({ key: { tenant: l.b, family: Dpni,
                           ordinal: ordinalWhere(originList(intent, l.b), o => isLinkSide(o, l.name, 1)) }, port: 0 }),
        prov: { tenant: l.a, rule: "link-edge", construct: l.name } })))

  pure def dpswOrdinalOf(intent: Intent, f: Fabric): int =
    hwOwnedFabrics(intent, f.forwardedBy).foldl({ i: 0, found: -1 }, (acc, g) =>
      { i: acc.i + 1, found: if (acc.found >= 0) acc.found else if (g.name == f.name) acc.i else -1 }).found + 1

  // Hardware fabric (6c): one edge per dpsw interface, dpsw.if index ↔ the
  // interface endpoint (hwFabricEnds order).
  pure def edgesForHwFabric(intent: Intent, f: Fabric): Set[Edge] =
    val dpswKey = { tenant: f.forwardedBy, family: Dpsw, ordinal: dpswOrdinalOf(intent, f) }
    hwFabricEnds(intent, f).foldl({ ifx: 0, set: Set() }, (acc, endB) =>
      { ifx: acc.ifx + 1,
        set: acc.set.union(Set({ a: ObjEnd({ key: dpswKey, port: acc.ifx }), b: endB,
                                 prov: { tenant: f.forwardedBy, rule: "fabric-edge", construct: f.name } })) }).set

  pure def fabricEdges(intent: Intent): Set[Edge] =
    intent.fabrics.foldl(Set(), (acc, f) =>
      if (f.switching == Hardware) acc.union(edgesForHwFabric(intent, f)) else acc)

  // Software fabric (6b): for each member resolving to a tenant C ≠ owner,
  // the owner's wire dpni ↔ C's wire dpni. MPort members contribute only
  // their port-edge; MFabric(Hardware) members contribute their fabric-edge
  // on the hardware side.
  pure def wireEdges(intent: Intent): Set[Edge] =
    intent.fabrics.foldl(Set(), (acc, g) =>
      if (g.switching == Software)
        g.members.foldl(acc, (a2, m) =>
          match memberTenant(intent, m) {
            | Some(c) => if (c != g.forwardedBy)
                           a2.union(Set({
                             a: ObjEnd({ key: { tenant: g.forwardedBy, family: Dpni,
                                                ordinal: ordinalWhere(originList(intent, g.forwardedBy), o => isWireOwnerOf(o, g.name, c)) }, port: 0 }),
                             b: ObjEnd({ key: { tenant: c, family: Dpni,
                                                ordinal: ordinalWhere(originList(intent, c), o => isWireMemberOf(o, g.name)) }, port: 0 }),
                             prov: { tenant: g.forwardedBy, rule: "fabric-wire", construct: g.name } }))
                         else a2
            | None => a2
          })
      else acc)

  // ---- the derivation ----

  pure def derive(intent: Intent, inv: Inventory): Plan =
    { objs: allObjs(intent, inv),
      edges: portEdges(intent).union(linkEdges(intent)).union(fabricEdges(intent)).union(wireEdges(intent)),
      order: fullOrder(intent, inv),
      prov: fullProv(intent, inv) }

  // A value use for every sum-type constructor inside the module: the
  // verify-path flattener drops a constructor whose only value use sits in
  // another module (QNT404; see types.qnt ETH_IFS block).
  pure val CONTAINERS_USE: Set[Container] = Set(Root, Child("c"))
  pure val ATTRS_USE: Set[Attrs] = Set(
    Unsized,
    DpniAttrs({ numQueues: 0 }),
    DpseciAttrs({ numQueues: 0, hasCg: false }),
    DpswAttrs({ numIfs: 0, maxFdbs: 0, perFdbFlooding: false, perFdbBroadcast: false, ctrlIf: false }),
    DprcAttrs({ options: Set() }))
  pure val MARKS_USE: Set[Mark] = Set(Measured, Unmeasured)
  pure val ORIGINS_USE: Set[Origin] = Set(
    OPort({ name: "p", dpmac: 0, rate: 0, tenant: "c" }),
    OLink({ link: { name: "l", a: "c", b: "d" }, side: 0 }),
    OAttach("f"),
    OWireOwner({ fabric: "f", peer: "c" }),
    OWireMember("f"))
  pure val ENDS_USE: Set[End] = Set(
    ObjEnd({ key: { tenant: "c", family: Dpni, ordinal: 0 }, port: 0 }),
    MacEnd(0))
}
