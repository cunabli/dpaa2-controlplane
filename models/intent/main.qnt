// Entry point of the intent corpus (openspec change `intent-layer`): the
// directed `<what>Test` runs that pin the vocabulary, the reference
// inventory, and — from task 1.2 on — the derivation and its refusals.
//
//   pnpm exec quint test models/intent/main.qnt --main=intent_main
//
// Named invariants: none yet (task 1.4 adds intent/invariants.qnt).
// Apalache-marked subset: none yet (task 1.5).
module intent_main {
  import types.* from "../core/types"
  import companions.* from "../core/companions"
  import intent_types.* from "./types"
  import intent_inventory.* from "./inventory"
  import intent_derive.* from "./derive"

  // ---- shared helpers for the derivation runs (task 1.2) ----

  pure def objsOfFam(p: Plan, name: str, fam: Family): Set[PlanObj] =
    p.objs.filter(o => o.key.consumer == name and o.key.family == fam)
  pure def countFam(p: Plan, name: str, fam: Family): int =
    objsOfFam(p, name, fam).size()
  // Position of a key in the emission order, or -1 if absent.
  pure def keyIndex(order: List[Key], k: Key): int =
    order.foldl({ i: 0, found: -1 }, (acc, x) =>
      { i: acc.i + 1, found: if (acc.found >= 0) acc.found else if (x == k) acc.i else -1 }).found

  // ---- shared intent fragments ----

  pure val router: Consumer = { name: "router", regime: PollMode, maxCores: 8, cryptoFlows: 4 }
  pure val kernel16: Consumer = kernelConsumer(16)
  pure val wan0: Port = { name: "wan0", dpmac: 7, rate: 10000, owner: "router" }
  pure val wan1: Port = { name: "wan1", dpmac: 8, rate: 10000, owner: "router" }
  pure val routerIntent: Intent =
    { consumers: [router], ports: [wan0, wan1], links: [], fabrics: [], crypto: Set(), limits: Set() }

  // The reference inventory reads as the spec's "Reserved and foreign
  // anchors are inventory facts" scenario says: dpmac.3 and dpmac.17
  // Reserved, dpni.0 Foreign, dpbp Counted, dpni Observed, dpcon Unknown.
  run refInventoryTest = all {
    match availabilityOf(REF_INVENTORY, { fam: Dpmac, num: 3 }) {
      | Reserved(_) => true
      | _ => false
    },
    match availabilityOf(REF_INVENTORY, { fam: Dpmac, num: 17 }) {
      | Reserved(_) => true
      | _ => false
    },
    availabilityOf(REF_INVENTORY, { fam: Dpmac, num: 7 }) == Free,
    availabilityOf(REF_INVENTORY, { fam: Dpni, num: 0 }) == Foreign("dpl"),
    availabilityOf(REF_INVENTORY, { fam: Dpni, num: 5 }) == Free,
    REF_INVENTORY.ceilings.get(Dpbp) == Counted(63),
    match REF_INVENTORY.ceilings.get(Dpni) {
      | Observed(o) => o.n == 18
      | _ => false
    },
    REF_INVENTORY.ceilings.get(Dpcon) == Unknown,
    REF_INVENTORY.ceilings.keys() == DERIVED_FAMILIES,
    // the DPC-born dpmacs of the reference environment, 3–10 and 17
    REF_INVENTORY.dpmacs.keys() == Set(3, 4, 5, 6, 7, 8, 9, 10, 17),
    REF_INVENTORY.dpmacs.get(7).maxRate == 10000,
    REF_INVENTORY.dpmacs.get(3).maxRate == 25000,
    kernelConsumer(16).regime == Kernel,
  }

  // A poll-mode router over two 10G ports: the child DPRC, two dpnis at
  // T = 5 transmit queues, the 2·T dpio / dpbp 2 / dpmcp 1 / dpcon 10
  // companion draws, the dpni↔dpmac edges, and the emission order.
  run pollModeRouterDeriveTest =
    val p = derive(routerIntent, REF_INVENTORY)
    all {
      p.objs.filter(o => o.key.family == Dprc) ==
        Set({ key: { consumer: "router", family: Dprc, ordinal: 1 }, container: Root,
              attrs: DprcAttrs({ options: DPRC_DEFAULT_OPTIONS }), prov: { consumer: "router", rule: "dprc", construct: "" } }),
      countFam(p, "router", Dpni) == 2,
      objsOfFam(p, "router", Dpni).forall(o =>
        o.container == Child("router") and match o.attrs { | DpniAttrs(a) => a.numQueues == 5 | _ => false }),
      countFam(p, "router", Dpio) == 10,
      countFam(p, "router", Dpbp) == 2,
      countFam(p, "router", Dpmcp) == 1,
      countFam(p, "router", Dpcon) == 10,
      p.objs.filter(o => o.key.consumer == "router" and o.key.family != Dprc).forall(o => o.container == Child("router")),
      objsOfFam(p, "router", Dpio).map(o => o.key.ordinal) == 1.to(10),
      p.prov.get({ consumer: "router", rule: "dpio", construct: "" }).value == 10,
      p.prov.get({ consumer: "router", rule: "dpio", construct: "" }).request == 10,
      p.prov.get({ consumer: "router", rule: "dpio", construct: "" }).limit == None,
      p.prov.get({ consumer: "router", rule: "dpio", construct: "" }).inputs == Set({ consumer: "router", rule: "T", construct: "" }),
      p.prov.get({ consumer: "router", rule: "T", construct: "" }).value == 5,
      p.prov.get({ consumer: "router", rule: "T", construct: "" }).mark == Unmeasured,
      p.prov.get({ consumer: "router", rule: "T", construct: "" }).constructs == Set("wan0", "wan1"),
      p.edges == Set(
        { a: ObjEnd({ key: { consumer: "router", family: Dpni, ordinal: 1 }, port: 0 }), b: MacEnd(7),
          prov: { consumer: "router", rule: "port-edge", construct: "wan0" } },
        { a: ObjEnd({ key: { consumer: "router", family: Dpni, ordinal: 2 }, port: 0 }), b: MacEnd(8),
          prov: { consumer: "router", rule: "port-edge", construct: "wan1" } }),
      keyIndex(p.order, { consumer: "router", family: Dprc, ordinal: 1 }) <
        keyIndex(p.order, { consumer: "router", family: Dpio, ordinal: 1 }),
      keyIndex(p.order, { consumer: "router", family: Dpio, ordinal: 10 }) <
        keyIndex(p.order, { consumer: "router", family: Dpmcp, ordinal: 1 }),
      keyIndex(p.order, { consumer: "router", family: Dpmcp, ordinal: 1 }) <
        keyIndex(p.order, { consumer: "router", family: Dpni, ordinal: 1 }),
      p.order.nth(0) == { consumer: KERNEL, family: Dprtc, ordinal: 1 },
    }

  // A per-family limit raises the request (design D5): 12 dpios, provenance
  // prints both request 10 and limit 12.
  run limitRaisesRequestTest =
    val p = derive({ consumers: [router], ports: [wan0, wan1], links: [], fabrics: [], crypto: Set(),
                     limits: Set({ consumer: "router", family: Dpio, limit: 12 }) }, REF_INVENTORY)
    all {
      countFam(p, "router", Dpio) == 12,
      p.prov.get({ consumer: "router", rule: "dpio", construct: "" }).request == 10,
      p.prov.get({ consumer: "router", rule: "dpio", construct: "" }).limit == Some(12),
      p.prov.get({ consumer: "router", rule: "dpio", construct: "" }).value == 12,
    }

  // The kernel regime: no child DPRC, dpio per online CPU, dpmcp cpus+dpnis,
  // dpbp per dpni, dpcon per polled queue, the pinned dprtc, dpio ≺ dpmcp.
  run kernelDeriveTest =
    val p = derive({ consumers: [kernel16],
                     ports: [{ name: "lan0", dpmac: 9, rate: 10000, owner: "kernel" }],
                     links: [], fabrics: [], crypto: Set(), limits: Set() }, REF_INVENTORY)
    all {
      p.objs.filter(o => o.key.family == Dprc) == Set(),
      countFam(p, "kernel", Dpio) == 16,
      countFam(p, "kernel", Dpmcp) == 17,
      countFam(p, "kernel", Dpbp) == 1,
      countFam(p, "kernel", Dpcon) == 16,
      countFam(p, "kernel", Dpni) == 1,
      objsOfFam(p, "kernel", Dpni).forall(o =>
        o.container == Root and match o.attrs { | DpniAttrs(a) => a.numQueues == 16 | _ => false }),
      p.objs.filter(o => o.key.family == Dprtc) ==
        Set({ key: { consumer: KERNEL, family: Dprtc, ordinal: 1 }, container: Root, attrs: Unsized,
              prov: { consumer: KERNEL, rule: "dprtc", construct: "" } }),
      keyIndex(p.order, { consumer: "kernel", family: Dpio, ordinal: 16 }) <
        keyIndex(p.order, { consumer: "kernel", family: Dpmcp, ordinal: 1 }),
    }

  // A hardware-switched fabric (kernel-steered) plus a crypto accelerator:
  // one dpsw with num_ifs = ports + consumers, the fabric-member ports
  // switched (no dpni, no port-edge) and attached to the dpsw interfaces,
  // the member consumer joined through one fabric-membership dpni, and one
  // dpseci sized to the owner's crypto_flows.
  run fabricAndCryptoTest =
    val p = derive({
      consumers: [kernel16, router],
      ports: [{ name: "p7", dpmac: 7, rate: 10000, owner: "kernel" },
              { name: "p8", dpmac: 8, rate: 10000, owner: "kernel" }],
      links: [],
      fabrics: [{ name: "lan", switching: Hardware, owner: "kernel",
                  members: [MPort("p7"), MPort("p8"), MConsumer("router")] }],
      crypto: Set({ owner: "router" }),
      limits: Set(),
    }, REF_INVENTORY)
    all {
      p.objs.filter(o => o.key.family == Dpsw) ==
        Set({ key: { consumer: "kernel", family: Dpsw, ordinal: 1 }, container: Root,
              attrs: DpswAttrs({ numIfs: 3, maxFdbs: 3, perFdbFlooding: true, perFdbBroadcast: true, ctrlIf: true }),
              prov: { consumer: "kernel", rule: "dpsw", construct: "lan" } }),
      countFam(p, "router", Dpni) == 1,
      objsOfFam(p, "router", Dpni).forall(o => match o.attrs { | DpniAttrs(a) => a.numQueues == 1 | _ => false }),
      countFam(p, "router", Dpio) == 2,
      countFam(p, "router", Dpcon) == 1,
      countFam(p, "kernel", Dpni) == 0,
      countFam(p, "kernel", Dpmcp) == 17,
      countFam(p, "kernel", Dpbp) == 1,
      countFam(p, "kernel", Dpcon) == 0,
      p.objs.filter(o => o.key.family == Dpseci) ==
        Set({ key: { consumer: "router", family: Dpseci, ordinal: 1 }, container: Child("router"),
              attrs: DpseciAttrs({ numQueues: 4, hasCg: true }), prov: { consumer: "router", rule: "dpseci", construct: "" } }),
      p.edges.contains({ a: ObjEnd({ key: { consumer: "kernel", family: Dpsw, ordinal: 1 }, port: 0 }),
                         b: MacEnd(7), prov: { consumer: "kernel", rule: "fabric-edge", construct: "lan" } }),
      p.edges.contains({ a: ObjEnd({ key: { consumer: "kernel", family: Dpsw, ordinal: 1 }, port: 1 }),
                         b: MacEnd(8), prov: { consumer: "kernel", rule: "fabric-edge", construct: "lan" } }),
      p.edges.contains({ a: ObjEnd({ key: { consumer: "kernel", family: Dpsw, ordinal: 1 }, port: 2 }),
                         b: ObjEnd({ key: { consumer: "router", family: Dpni, ordinal: 1 }, port: 0 }),
                         prov: { consumer: "kernel", rule: "fabric-edge", construct: "lan" } }),
      p.edges.filter(e => e.prov.rule == "port-edge") == Set(),
    }

  // A software fabric (figure 6b): vpp bridges its own two 10G ports and a
  // pseudo-wire to the kernel. No dpsw exists — the MC never sees a software
  // switch. vpp takes three dpnis (two ports, one wire end); the kernel
  // takes its wire end. (1.3 refuses this exact intent for other reasons;
  // the shape assertions still hold. Two ports keep T seeded: 2×10G ⇒ 5.)
  run softwareFabricTest =
    val vpp = { name: "vpp", regime: PollMode, maxCores: 8, cryptoFlows: 0 }
    val p = derive({
      consumers: [kernel16, vpp],
      ports: [{ name: "p7", dpmac: 7, rate: 10000, owner: "vpp" },
              { name: "p8", dpmac: 8, rate: 10000, owner: "vpp" }],
      links: [],
      fabrics: [{ name: "br", switching: Software, owner: "vpp",
                  members: [MPort("p7"), MPort("p8"), MConsumer("kernel")] }],
      crypto: Set(),
      limits: Set(),
    }, REF_INVENTORY)
    all {
      p.objs.filter(o => o.key.family == Dpsw) == Set(),
      countFam(p, "vpp", Dpni) == 3,
      objsOfFam(p, "vpp", Dpni).forall(o => match o.attrs { | DpniAttrs(a) => a.numQueues == 5 | _ => false }),
      countFam(p, "vpp", Dpio) == 10,
      countFam(p, "kernel", Dpni) == 1,
      p.edges.contains({ a: ObjEnd({ key: { consumer: "vpp", family: Dpni, ordinal: 1 }, port: 0 }),
                         b: MacEnd(7), prov: { consumer: "vpp", rule: "port-edge", construct: "p7" } }),
      p.edges.contains({ a: ObjEnd({ key: { consumer: "vpp", family: Dpni, ordinal: 2 }, port: 0 }),
                         b: MacEnd(8), prov: { consumer: "vpp", rule: "port-edge", construct: "p8" } }),
      p.edges.contains({ a: ObjEnd({ key: { consumer: "vpp", family: Dpni, ordinal: 3 }, port: 0 }),
                         b: ObjEnd({ key: { consumer: "kernel", family: Dpni, ordinal: 1 }, port: 0 }),
                         prov: { consumer: "vpp", rule: "fabric-wire", construct: "br" } }),
    }

  // The operator's chain of switches: a hardware fabric (kernel dpsw over two
  // ports) is a member of a software fabric vpp bridges alongside its own
  // port. vpp attaches to the dpsw via a dpni; kernel terminates nothing
  // itself. vpp owns two terminated 10G ports (p7, p10) to keep T seeded ⇒
  // three dpnis, the attach dpni last. No pseudo-wire (both software members
  // resolve to no peer consumer).
  run chainTest =
    val vpp = { name: "vpp", regime: PollMode, maxCores: 8, cryptoFlows: 0 }
    val p = derive({
      consumers: [kernel16, vpp],
      ports: [{ name: "p7", dpmac: 7, rate: 10000, owner: "vpp" },
              { name: "p8", dpmac: 8, rate: 10000, owner: "kernel" },
              { name: "p9", dpmac: 9, rate: 10000, owner: "kernel" },
              { name: "p10", dpmac: 10, rate: 10000, owner: "vpp" }],
      links: [],
      fabrics: [{ name: "hw", switching: Hardware, owner: "kernel",
                  members: [MPort("p8"), MPort("p9")] },
                { name: "sw", switching: Software, owner: "vpp",
                  members: [MPort("p7"), MFabric("hw")] }],
      crypto: Set(),
      limits: Set(),
    }, REF_INVENTORY)
    all {
      p.objs.filter(o => o.key.family == Dpsw) ==
        Set({ key: { consumer: "kernel", family: Dpsw, ordinal: 1 }, container: Root,
              attrs: DpswAttrs({ numIfs: 3, maxFdbs: 3, perFdbFlooding: true, perFdbBroadcast: true, ctrlIf: true }),
              prov: { consumer: "kernel", rule: "dpsw", construct: "hw" } }),
      countFam(p, "vpp", Dpni) == 3,
      countFam(p, "kernel", Dpni) == 0,
      countFam(p, "kernel", Dpmcp) == 17,
      countFam(p, "kernel", Dpbp) == 1,
      p.edges.contains({ a: ObjEnd({ key: { consumer: "kernel", family: Dpsw, ordinal: 1 }, port: 0 }),
                         b: MacEnd(8), prov: { consumer: "kernel", rule: "fabric-edge", construct: "hw" } }),
      p.edges.contains({ a: ObjEnd({ key: { consumer: "kernel", family: Dpsw, ordinal: 1 }, port: 1 }),
                         b: MacEnd(9), prov: { consumer: "kernel", rule: "fabric-edge", construct: "hw" } }),
      p.edges.contains({ a: ObjEnd({ key: { consumer: "kernel", family: Dpsw, ordinal: 1 }, port: 2 }),
                         b: ObjEnd({ key: { consumer: "vpp", family: Dpni, ordinal: 3 }, port: 0 }),
                         prov: { consumer: "kernel", rule: "fabric-edge", construct: "hw" } }),
      p.edges.contains({ a: ObjEnd({ key: { consumer: "vpp", family: Dpni, ordinal: 1 }, port: 0 }),
                         b: MacEnd(7), prov: { consumer: "vpp", rule: "port-edge", construct: "p7" } }),
      p.edges.filter(e => e.prov.rule == "fabric-wire") == Set(),
    }

  // Hardware-in-Hardware derives nothing (open item (d)): hw2 lists hw1 as a
  // member, but a dpsw interface cannot be another dpsw's interface — hw2's
  // dpsw counts only its own port. Both fabrics still emit their dpsw.
  run hwInHwDerivesNothingTest =
    val p = derive({
      consumers: [kernel16],
      ports: [{ name: "p8", dpmac: 8, rate: 10000, owner: "kernel" },
              { name: "p9", dpmac: 9, rate: 10000, owner: "kernel" }],
      links: [],
      fabrics: [{ name: "hw1", switching: Hardware, owner: "kernel", members: [MPort("p8")] },
                { name: "hw2", switching: Hardware, owner: "kernel", members: [MFabric("hw1"), MPort("p9")] }],
      crypto: Set(),
      limits: Set(),
    }, REF_INVENTORY)
    all {
      countFam(p, "kernel", Dpsw) == 2,
      objsOfFam(p, "kernel", Dpsw).forall(o => match o.attrs { | DpswAttrs(a) => a.numIfs == 1 | _ => false }),
      // hw2's dpsw (ordinal 2) connects only to its own port, never to hw1's dpsw
      p.edges.filter(e => match e.a { | ObjEnd(x) => x.key == { consumer: "kernel", family: Dpsw, ordinal: 2 } | _ => false }) ==
        Set({ a: ObjEnd({ key: { consumer: "kernel", family: Dpsw, ordinal: 2 }, port: 0 }),
              b: MacEnd(9), prov: { consumer: "kernel", rule: "fabric-edge", construct: "hw2" } }),
    }

  // A virtual fabric: a link between two poll-mode containers, no dpmac —
  // each takes one dpni and the pair is the single edge; two child DPRCs.
  run linkTest =
    val ra = { name: "routerA", regime: PollMode, maxCores: 8, cryptoFlows: 0 }
    val rb = { name: "routerB", regime: PollMode, maxCores: 8, cryptoFlows: 0 }
    val p = derive({ consumers: [ra, rb], ports: [], links: [{ name: "wire", a: "routerA", b: "routerB" }],
                     fabrics: [], crypto: Set(), limits: Set() }, REF_INVENTORY)
    all {
      countFam(p, "routerA", Dpni) == 1,
      countFam(p, "routerB", Dpni) == 1,
      objsOfFam(p, "routerA", Dpni).forall(o => match o.attrs { | DpniAttrs(a) => a.numQueues == 1 | _ => false }),
      objsOfFam(p, "routerB", Dpni).forall(o => match o.attrs { | DpniAttrs(a) => a.numQueues == 1 | _ => false }),
      p.edges.contains({ a: ObjEnd({ key: { consumer: "routerA", family: Dpni, ordinal: 1 }, port: 0 }),
                         b: ObjEnd({ key: { consumer: "routerB", family: Dpni, ordinal: 1 }, port: 0 }),
                         prov: { consumer: "routerA", rule: "link-edge", construct: "wire" } }),
      p.objs.filter(o => o.key.family == Dprc).size() == 2,
    }

  // The rate table refuses an unseeded class and gives a portless consumer
  // the main-only thread count.
  run unknownRateClassTest = all {
    threadCount([{ name: "x", dpmac: 3, rate: 25000, owner: "c" }]) == None,
    threadCount([]) == Some(1),
  }

  // The derivation is a pure function of its two inputs (design D5).
  run deterministicTest = all {
    derive(routerIntent, REF_INVENTORY) == derive(routerIntent, REF_INVENTORY),
  }
}
