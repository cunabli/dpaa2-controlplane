// The pure derivation `derive(intent, inventory): Plan` (openspec change
// `intent-layer`, design D3/D4/D6; ADR-0005 §2): intent plus the observed
// hardware offer become the complete object plan — every object keyed by
// (consumer, family, ordinal), every derived count carrying its rule, its
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
// request, a fabric owner is kernel-steered, a rate class is seeded. The
// derivation of a refused intent is undefined, not wrong.
//
// Open items left for 1.3 (refusals) and 3.1 (Rust label rendering):
//   (a) restool caps set-label at 15 chars (dprc.md), so the rendered
//       `<consumer>/<family>/<ordinal>` overflows for long consumer names;
//       the Key is the identity, the label a lossy projection (ADR-0010).
//   (b) a consumer with mixed rate classes, or an unseeded (rate, count)
//       row, has no thread count — threadCount returns None; 1.3 refuses
//       it. 1.2 reads None as 0 and never reaches that intent.
//   (c) a portless poll-mode consumer takes T = 1 by the main-only rule
//       below (main + workers, no port ⇒ no worker); 1.3 may narrow it.
//   (d) Hardware-in-Hardware membership (a dpsw interface as another
//       dpsw's interface) derives nothing; 1.3 refuses it.
//   (e) a port owner differing from its fabric owner is refused at 1.3.
//
// Named invariants: none here (intent/invariants.qnt, task 1.4).
// Apalache-marked subset: none.
module intent_derive {
  import types.* from "../core/types"           // Family, Option, Perm, ObjId
  import companions.* from "../core/companions" // Regime, companionDraw (ADR-0012)
  import dpsw.* from "../families/dpsw"          // dpswParams.draw (kernel census row)
  import dpseci.* from "../families/dpseci"      // dpseciParams.draw (kernel census row)
  import intent_types.* from "./types"           // Intent, Consumer, Port, …, KERNEL

  // ---- the plan's shape (design D6) ----

  // A derived object's identity is its key: the label the MC carries is a
  // projection of it (ADR-0010 §4). Ordinals are 1-based positions.
  type Key = { consumer: str, family: Family, ordinal: int }

  // The label is a projection of the key; the Rust frontend renders
  // `<consumer>/<family>/<ordinal>` (task 3.1). Quint has no string ops,
  // so the projection is the identity.
  pure def labelOf(k: Key): Key = k

  // Where an object lives: Root is dprc.1 (the kernel's own container,
  // never a consumer); Child(consumer) is that consumer's child DPRC,
  // itself the key (consumer, Dprc, 1).
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

  // A provenance node's address: (consumer, rule, construct). Consumer-level
  // count rules use construct "" (one node per consumer); per-construct
  // rules carry the construct name — the fabric for dpsw/fabric-edge/
  // fabric-wire, the port for port-edge, the link for link-edge — so two
  // fabrics of one owner no longer collide on a single node.
  type ProvKey = { consumer: str, rule: str, construct: str }

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

  // ---- the rate-class table (design D3) ----

  type RateRow = { rate: int, ports: int, threads: int, mark: Mark }

  // The one seeded row: the configuration ADR-0012's companion numbers
  // were verified against — two 10G ports in poll-mode ⇒ T = 5. Printed
  // *unmeasured*; no extrapolation, a class not here has no thread count.
  pure val RATE_TABLE: Set[RateRow] =
    Set({ rate: 10000, ports: 2, threads: 5, mark: Unmeasured })

  // The row whose rate equals every entry and whose port count equals the
  // length, else None (a mixed or unseeded class).
  pure def rateRow(rates: List[int]): Option[RateRow] =
    RATE_TABLE.fold(None, (acc, r) =>
      if (r.ports == rates.length() and rates.indices().forall(i => rates.nth(i) == r.rate))
        Some(r)
      else acc)

  // T = main + workers (ADR-0012). Zero ports ⇒ Some(1): a portless
  // consumer is main-only (no port ⇒ no worker); still visibly unmeasured.
  // Otherwise the seeded row's thread count, or None for an unseeded class.
  pure def threadCount(ports: List[Port]): Option[int] =
    if (ports.length() == 0) Some(1)
    else match rateRow(ports.foldl([], (acc, p) => acc.append(p.rate))) {
      | Some(r) => Some(r.threads)
      | None => None
    }

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

  // The consumer a member resolves to: a consumer member, or a software
  // fabric member's owner (its bridging). Ports and hardware fabrics resolve
  // to no consumer.
  pure def memberConsumer(intent: Intent, m: Member): Option[str] =
    match m {
      | MConsumer(c) => Some(c)
      | MFabric(h) => match fabricByName(intent, h) {
          | Some(f) => match f.switching { | Software => Some(f.owner) | Hardware => None }
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

  // The consumer's terminated ports (owned and not switched into a hardware
  // fabric), in declaration order.
  pure def ownedPorts(intent: Intent, name: str): List[Port] =
    intent.ports.select(p => p.owner == name and not(isHwSwitchedPort(intent, p.name)))

  // Whether X is attached to Hardware fabric f through f's own members: a
  // consumer member X, or a software-fabric member owned by X.
  pure def ownerAttachedViaMembers(intent: Intent, f: Fabric, x: str): bool =
    f.members.select(m => match m {
      | MConsumer(c) => c == x
      | MFabric(h) => match fabricByName(intent, h) {
          | Some(g) => g.switching == Software and g.owner == x
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
        g.switching == Software and g.owner == x and
        g.members.select(m => memberIsFabric(m, f.name)).length() > 0).length() > 0
    viaMembers or viaSoftwareChain

  // ---- dpni origins (design D6; object-model.md §2; UM §2.2.2 fig. 6) ----

  // Each dpni a consumer terminates, in the concatenation order that fixes
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
    ownedPorts(intent, name).foldl([], (acc, p) => acc.append(OPort(p)))

  pure def linkOrigins(intent: Intent, name: str): List[Origin] =
    intent.links.foldl([], (acc, l) =>
      val a1 = if (l.a == name) acc.append(OLink({ link: l, side: 0 })) else acc
      if (l.b == name) a1.append(OLink({ link: l, side: 1 })) else a1)

  // One OAttach per Hardware fabric the consumer attaches to, once per
  // (X, f) however many ways the attachment is implied.
  pure def attachOrigins(intent: Intent, name: str): List[Origin] =
    intent.fabrics.foldl([], (acc, f) =>
      if (f.switching == Hardware and attachesToHwFabric(intent, name, f))
        acc.append(OAttach(f.name))
      else acc)

  // One OWireOwner per member of a Software fabric X owns that resolves to a
  // consumer C ≠ X — X's end of the 6b pseudo-wire.
  pure def wireOwnerOrigins(intent: Intent, name: str): List[Origin] =
    intent.fabrics.foldl([], (acc, g) =>
      if (g.switching == Software and g.owner == name)
        g.members.foldl(acc, (a2, m) =>
          match memberConsumer(intent, m) {
            | Some(c) => if (c != name) a2.append(OWireOwner({ fabric: g.name, peer: c })) else a2
            | None => a2
          })
      else acc)

  // One OWireMember per Software fabric X does not own but resolves into —
  // X's end of the wire.
  pure def wireMemberOrigins(intent: Intent, name: str): List[Origin] =
    intent.fabrics.foldl([], (acc, g) =>
      if (g.switching == Software and g.owner != name and
          g.members.select(m => memberConsumer(intent, m) == Some(name)).length() > 0)
        acc.append(OWireMember(g.name))
      else acc)

  // The ordered dpni sources of a consumer: terminated ports, link ends,
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

  pure def ownedPortNames(intent: Intent, name: str): Set[str] =
    ownedPorts(intent, name).foldl(Set(), (acc, p) => acc.union(Set(p.name)))

  // The construct names of a consumer's dpnis: a port name, a link name, or
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

  // A dpni endpoint by consumer + attaching fabric: that consumer's OAttach
  // dpni on the fabric's dpsw.
  pure def attachEnd(intent: Intent, consumer: str, fabricName: str): End =
    ObjEnd({ key: { consumer: consumer, family: Dpni,
                    ordinal: ordinalWhere(originList(intent, consumer), o => isAttach(o, fabricName)) }, port: 0 })

  // The ordered endpoints of a Hardware fabric's dpsw interfaces: f.members
  // in list order — a member port's dpmac (MacEnd), a member consumer's or
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
        | MConsumer(c) => acc.append(attachEnd(intent, c, f.name))
        | MFabric(h) => match fabricByName(intent, h) {
            | Some(g) => if (g.switching == Software) acc.append(attachEnd(intent, g.owner, f.name)) else acc
            | None => acc
          }
      })
    intent.fabrics.foldl(fromMembers, (acc, g) =>
      if (g.switching == Software and
          g.members.select(m => memberIsFabric(m, f.name)).length() > 0 and
          not(ownerAttachedViaMembers(intent, f, g.owner)))
        acc.append(attachEnd(intent, g.owner, f.name))
      else acc)

  // ---- per-consumer sizing (design D3/D4/D5) ----

  type Eff = { request: int, limit: Option[int], value: int }

  // The request/limit idiom (design D5): a matching per-family Limit raises
  // the request to its value; else the request stands. Applies to the four
  // companion count families only. (1.3 refuses limit < request.)
  pure def effective(intent: Intent, name: str, fam: Family, request: int): Eff =
    val ls = intent.limits.filter(l => l.consumer == name and l.family == fam)
    if (ls.size() == 0) { request: request, limit: None, value: request }
    else val lv = ls.fold(0, (_, l) => l.limit)
         { request: request, limit: Some(lv), value: lv }

  // Only Hardware fabrics emit a dpsw; the software owner bridges its own
  // dpnis (open item), so "owned fabrics" for dpsw purposes are the Hardware
  // fabrics owned by X.
  pure def hwOwnedFabrics(intent: Intent, name: str): List[Fabric] =
    intent.fabrics.select(f => f.switching == Hardware and f.owner == name)

  pure def cryptoCount(intent: Intent, name: str): int =
    intent.crypto.filter(k => k.owner == name).size()

  pure def containerOf(c: Consumer): Container =
    if (c.name == KERNEL) Root else Child(c.name)

  // The kernel's dpio budget: online CPUs (ADR-0012) or max_cores — the
  // gate picks one (open question 2).
  pure def kernelCores(inv: Inventory, c: Consumer): int =
    if (KERNEL_BUDGET_IS_DPIO_COUNT) c.maxCores else inv.cpus

  pure def tOf(intent: Intent, c: Consumer): int =
    match threadCount(ownedPorts(intent, c.name)) { | Some(t) => t | None => 0 }

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
  pure def sizeConsumer(intent: Intent, inv: Inventory, c: Consumer): Sizing =
    val nm = c.name
    val isK = c.regime == Kernel
    val dpnis = originList(intent, nm).length()
    val t = tOf(intent, c)
    val cpus = kernelCores(inv, c)
    val numQ = if (isK) cpus else t
    val nDpsw = hwOwnedFabrics(intent, nm).length()
    val nDpseci = cryptoCount(intent, nm)
    val base = if (isK) companionDraw(Kernel, cpus, 0, 1, dpnis)
               else companionDraw(PollMode, 0, t, 1, dpnis)
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

  pure def sizedObjs(name: str, fam: Family, container: Container, n: int, attrs: Attrs, rule: str): Set[PlanObj] =
    range(1, n + 1).foldl(Set(), (acc, i) =>
      acc.union(Set({ key: { consumer: name, family: fam, ordinal: i },
                      container: container, attrs: attrs,
                      prov: { consumer: name, rule: rule, construct: "" } })))

  // Every consumer whose name ≠ KERNEL gets one child DPRC in Root
  // (dprc.md ~line 281); the kernel consumer's objects live in Root.
  pure def dprcObjs(s: Sizing): Set[PlanObj] =
    if (s.isKernel) Set()
    else Set({ key: { consumer: s.name, family: Dprc, ordinal: 1 },
               container: Root, attrs: DprcAttrs({ options: DPRC_DEFAULT_OPTIONS }),
               prov: { consumer: s.name, rule: "dprc", construct: "" } })

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
          key: { consumer: s.name, family: Dpsw, ordinal: acc.i + 1 },
          container: s.container,
          attrs: DpswAttrs({ numIfs: ni, maxFdbs: ni,
                             perFdbFlooding: true, perFdbBroadcast: true, ctrlIf: true }),
          prov: { consumer: s.name, rule: "dpsw", construct: f.name } })) }).set

  pure def consumerObjs(intent: Intent, inv: Inventory, c: Consumer): Set[PlanObj] =
    val s = sizeConsumer(intent, inv, c)
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
  pure val dprtcKey: Key = { consumer: KERNEL, family: Dprtc, ordinal: 1 }
  pure val dprtcObj: PlanObj =
    { key: dprtcKey, container: Root, attrs: Unsized, prov: { consumer: KERNEL, rule: "dprtc", construct: "" } }

  pure def allObjs(intent: Intent, inv: Inventory): Set[PlanObj] =
    intent.consumers.foldl(Set(dprtcObj), (acc, c) => acc.union(consumerObjs(intent, inv, c)))

  // ---- emission order (object-model.md §5 step 1) ----
  // A kernel dpio draws a dpmcp, so dpio precedes dpmcp (ADR-0012).

  pure def companionKeys(name: str, fam: Family, n: int): List[Key] =
    range(1, n + 1).foldl([], (acc, i) => acc.append({ consumer: name, family: fam, ordinal: i }))

  pure def consumerOrder(intent: Intent, s: Sizing): List[Key] =
    (if (s.isKernel) [] else [{ consumer: s.name, family: Dprc, ordinal: 1 }])
      .concat(companionKeys(s.name, Dpio, s.effDpio.value))
      .concat(companionKeys(s.name, Dpmcp, s.effDpmcp.value))
      .concat(companionKeys(s.name, Dpbp, s.effDpbp.value))
      .concat(companionKeys(s.name, Dpcon, s.effDpcon.value))
      .concat(companionKeys(s.name, Dpni, s.dpnis))
      .concat(companionKeys(s.name, Dpseci, s.numDpseci))
      .concat(companionKeys(s.name, Dpsw, s.numDpsw))

  pure def fullOrder(intent: Intent, inv: Inventory): List[Key] =
    [dprtcKey].concat(intent.consumers.foldl([], (acc, c) =>
      acc.concat(consumerOrder(intent, sizeConsumer(intent, inv, c)))))

  // ---- provenance nodes ----

  pure def dpioNode(s: Sizing): ProvNode =
    { rule: "dpio", anchor: "ADR-0012 (companions.qnt companionDraw)", mark: Measured,
      request: s.effDpio.request, limit: s.effDpio.limit, value: s.effDpio.value,
      inputs: Set({ consumer: s.name, rule: if (s.isKernel) "cpus" else "T", construct: "" }), constructs: Set() }

  pure def dpbpNode(s: Sizing): ProvNode =
    { rule: "dpbp",
      anchor: if (s.isKernel) "ADR-0012 (companions.qnt companionDraw); families/dpsw.qnt + dpseci.qnt draw"
              else "ADR-0012 (companions.qnt companionDraw)",
      mark: Measured, request: s.effDpbp.request, limit: s.effDpbp.limit, value: s.effDpbp.value,
      inputs: if (s.isKernel) Set({ consumer: s.name, rule: "dpnis", construct: "" }) else Set(), constructs: Set() }

  pure def dpmcpNode(s: Sizing): ProvNode =
    { rule: "dpmcp",
      anchor: if (s.isKernel) "ADR-0012 (companions.qnt companionDraw); families/dpsw.qnt + dpseci.qnt draw"
              else "companions.qnt: one MC portal per process",
      mark: Measured, request: s.effDpmcp.request, limit: s.effDpmcp.limit, value: s.effDpmcp.value,
      inputs: if (s.isKernel) Set({ consumer: s.name, rule: "cpus", construct: "" }, { consumer: s.name, rule: "dpnis", construct: "" }) else Set(),
      constructs: Set() }

  pure def dpniQueuesNode(s: Sizing): ProvNode =
    { rule: "dpni-queues",
      anchor: if (s.isKernel) "dpni.md line 64/226-236: ls-addni injects nproc; DPCON-I1"
              else "ADR-0012: transmit queues >= T",
      mark: Measured, request: s.numQueues, limit: None, value: s.numQueues,
      inputs: Set({ consumer: s.name, rule: if (s.isKernel) "cpus" else "T", construct: "" }), constructs: Set() }

  pure def dpconNode(s: Sizing): ProvNode =
    { rule: "dpcon", anchor: "dpcon.md DPCON-I1: one dpcon per polled queue", mark: Measured,
      request: s.effDpcon.request, limit: s.effDpcon.limit, value: s.effDpcon.value,
      inputs: Set({ consumer: s.name, rule: "dpnis", construct: "" }, { consumer: s.name, rule: "dpni-queues", construct: "" }),
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
      inputs: Set({ consumer: l.a, rule: "dpnis", construct: "" }, { consumer: l.b, rule: "dpnis", construct: "" }),
      constructs: Set(l.name) }

  pure def fabricEdgeNode(f: Fabric): ProvNode =
    { rule: "fabric-edge", anchor: "DPAA2 User Manual §2.2.2 fig. 6c; dpsw.md", mark: Measured,
      request: 0, limit: None, value: 0, inputs: Set({ consumer: f.owner, rule: "dpsw", construct: f.name }),
      constructs: Set(f.name) }

  // The owner's end of every 6b pseudo-wire a Software fabric carries; inputs
  // are the owner's and each wired peer's dpnis.
  pure def fabricWireNode(intent: Intent, g: Fabric): ProvNode =
    { rule: "fabric-wire", anchor: "DPAA2 User Manual §2.2.2 fig. 6b; object-model.md §2 DPNI-I9", mark: Measured,
      request: 0, limit: None, value: 0,
      inputs: g.members.foldl(Set({ consumer: g.owner, rule: "dpnis", construct: "" }), (acc, m) =>
        match memberConsumer(intent, m) {
          | Some(c) => if (c != g.owner) acc.union(Set({ consumer: c, rule: "dpnis", construct: "" })) else acc
          | None => acc
        }),
      constructs: Set(g.name) }

  pure def addConsumerProv(intent: Intent, inv: Inventory, m: ProvKey -> ProvNode, c: Consumer): ProvKey -> ProvNode =
    val s = sizeConsumer(intent, inv, c)
    val nm = s.name
    val pn = ownedPortNames(intent, nm)
    val np = ownedPorts(intent, nm).length()
    val m1 = m
      .put({ consumer: nm, rule: "ports", construct: "" },
           { rule: "ports", anchor: "design D1: declared port constructs", mark: Measured,
             request: np, limit: None, value: np, inputs: Set(), constructs: pn })
      .put({ consumer: nm, rule: "dpnis", construct: "" },
           { rule: "dpnis", anchor: "object-model.md §2: one dpni per terminated port, link end, fabric membership",
             mark: Measured, request: s.dpnis, limit: None, value: s.dpnis, inputs: Set(),
             constructs: dpniConstructs(intent, nm) })
      .put({ consumer: nm, rule: "dpio", construct: "" }, dpioNode(s))
      .put({ consumer: nm, rule: "dpbp", construct: "" }, dpbpNode(s))
      .put({ consumer: nm, rule: "dpmcp", construct: "" }, dpmcpNode(s))
      .put({ consumer: nm, rule: "dpni-queues", construct: "" }, dpniQueuesNode(s))
      .put({ consumer: nm, rule: "dpcon", construct: "" }, dpconNode(s))
    val m2 = if (s.isKernel)
               m1.put({ consumer: nm, rule: "cpus", construct: "" },
                      { rule: "cpus", anchor: "ADR-0012: one dpio per online CPU", mark: Measured,
                        request: s.cpus, limit: None, value: s.cpus, inputs: Set(), constructs: Set() })
             else
               m1.put({ consumer: nm, rule: "T", construct: "" },
                      { rule: "T", anchor: "design D3 rate-class table, unmeasured", mark: Unmeasured,
                        request: s.t, limit: None, value: s.t,
                        inputs: Set({ consumer: nm, rule: "ports", construct: "" }), constructs: pn })
    val m3 = if (s.isKernel) m2
             else m2.put({ consumer: nm, rule: "dprc", construct: "" },
                         { rule: "dprc", anchor: "dprc.md: restool-default child options", mark: Measured,
                           request: 1, limit: None, value: 1, inputs: Set(), constructs: Set(nm) })
    val m4 = if (s.numDpseci > 0)
               m3.put({ consumer: nm, rule: "dpseci", construct: "" },
                      { rule: "dpseci", anchor: "dpseci.md: num_queues >= crypto_flows, DPSECI_OPT_HAS_CG",
                        mark: Measured, request: s.cryptoFlows, limit: None, value: s.cryptoFlows,
                        inputs: Set(), constructs: Set(nm) })
             else m3
    // one dpsw node per owned Hardware fabric (keyed by the fabric name)
    val m5 = hwOwnedFabrics(intent, nm).foldl(m4, (mm, f) =>
      mm.put({ consumer: nm, rule: "dpsw", construct: f.name }, dpswNode(intent, f)))
    // one port-edge node per owned terminated port (keyed by the port name)
    ownedPorts(intent, nm).foldl(m5, (mm, p) =>
      mm.put({ consumer: nm, rule: "port-edge", construct: p.name },
             { rule: "port-edge", anchor: "object-model.md §2: dpni<->dpmac edge", mark: Measured,
               request: 0, limit: None, value: 0,
               inputs: Set({ consumer: nm, rule: "dpnis", construct: "" }), constructs: Set(p.name) }))

  pure def fullProv(intent: Intent, inv: Inventory): ProvKey -> ProvNode =
    val base = Map({ consumer: KERNEL, rule: "dprtc", construct: "" } -> dprtcNode)
    val withC = intent.consumers.foldl(base, (m, c) => addConsumerProv(intent, inv, m, c))
    val withL = intent.links.foldl(withC, (m, l) => m.put({ consumer: l.a, rule: "link-edge", construct: l.name }, linkEdgeNode(l)))
    intent.fabrics.foldl(withL, (m, f) =>
      if (f.switching == Hardware)
        m.put({ consumer: f.owner, rule: "fabric-edge", construct: f.name }, fabricEdgeNode(f))
      else
        m.put({ consumer: f.owner, rule: "fabric-wire", construct: f.name }, fabricWireNode(intent, f)))

  // ---- edges ----

  // Port (6a): the owner's dpni for the port ↔ its dpmac (object-model.md §2).
  pure def portEdges(intent: Intent): Set[Edge] =
    intent.consumers.foldl(Set(), (acc, c) =>
      acc.union(ownedPorts(intent, c.name).foldl(Set(), (a2, p) =>
        a2.union(Set({
          a: ObjEnd({ key: { consumer: c.name, family: Dpni,
                             ordinal: ordinalWhere(originList(intent, c.name), o => isPort(o, p.name)) }, port: 0 }),
          b: MacEnd(p.dpmac),
          prov: { consumer: c.name, rule: "port-edge", construct: p.name } })))))

  // Link (6b): the two link-end dpnis.
  pure def linkEdges(intent: Intent): Set[Edge] =
    intent.links.foldl(Set(), (acc, l) =>
      acc.union(Set({
        a: ObjEnd({ key: { consumer: l.a, family: Dpni,
                           ordinal: ordinalWhere(originList(intent, l.a), o => isLinkSide(o, l.name, 0)) }, port: 0 }),
        b: ObjEnd({ key: { consumer: l.b, family: Dpni,
                           ordinal: ordinalWhere(originList(intent, l.b), o => isLinkSide(o, l.name, 1)) }, port: 0 }),
        prov: { consumer: l.a, rule: "link-edge", construct: l.name } })))

  pure def dpswOrdinalOf(intent: Intent, f: Fabric): int =
    hwOwnedFabrics(intent, f.owner).foldl({ i: 0, found: -1 }, (acc, g) =>
      { i: acc.i + 1, found: if (acc.found >= 0) acc.found else if (g.name == f.name) acc.i else -1 }).found + 1

  // Hardware fabric (6c): one edge per dpsw interface, dpsw.if index ↔ the
  // interface endpoint (hwFabricEnds order).
  pure def edgesForHwFabric(intent: Intent, f: Fabric): Set[Edge] =
    val dpswKey = { consumer: f.owner, family: Dpsw, ordinal: dpswOrdinalOf(intent, f) }
    hwFabricEnds(intent, f).foldl({ ifx: 0, set: Set() }, (acc, endB) =>
      { ifx: acc.ifx + 1,
        set: acc.set.union(Set({ a: ObjEnd({ key: dpswKey, port: acc.ifx }), b: endB,
                                 prov: { consumer: f.owner, rule: "fabric-edge", construct: f.name } })) }).set

  pure def fabricEdges(intent: Intent): Set[Edge] =
    intent.fabrics.foldl(Set(), (acc, f) =>
      if (f.switching == Hardware) acc.union(edgesForHwFabric(intent, f)) else acc)

  // Software fabric (6b): for each member resolving to a consumer C ≠ owner,
  // the owner's wire dpni ↔ C's wire dpni. MPort members contribute only
  // their port-edge; MFabric(Hardware) members contribute their fabric-edge
  // on the hardware side.
  pure def wireEdges(intent: Intent): Set[Edge] =
    intent.fabrics.foldl(Set(), (acc, g) =>
      if (g.switching == Software)
        g.members.foldl(acc, (a2, m) =>
          match memberConsumer(intent, m) {
            | Some(c) => if (c != g.owner)
                           a2.union(Set({
                             a: ObjEnd({ key: { consumer: g.owner, family: Dpni,
                                                ordinal: ordinalWhere(originList(intent, g.owner), o => isWireOwnerOf(o, g.name, c)) }, port: 0 }),
                             b: ObjEnd({ key: { consumer: c, family: Dpni,
                                                ordinal: ordinalWhere(originList(intent, c), o => isWireMemberOf(o, g.name)) }, port: 0 }),
                             prov: { consumer: g.owner, rule: "fabric-wire", construct: g.name } }))
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
    OPort({ name: "p", dpmac: 0, rate: 0, owner: "c" }),
    OLink({ link: { name: "l", a: "c", b: "d" }, side: 0 }),
    OAttach("f"),
    OWireOwner({ fabric: "f", peer: "c" }),
    OWireMember("f"))
  pure val ENDS_USE: Set[End] = Set(
    ObjEnd({ key: { consumer: "c", family: Dpni, ordinal: 0 }, port: 0 }),
    MacEnd(0))
}
