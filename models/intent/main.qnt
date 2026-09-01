// Entry point of the intent corpus (openspec change `intent-layer`): the
// directed `<what>Test` runs that pin the vocabulary, the reference
// inventory, and — from task 1.2 on — the derivation and its refusals.
//
//   pnpm exec quint test models/intent/main.qnt --main=intent_main
//
// `pnpm model:test` now covers these directed runs alongside the core corpus.
//
// Named invariants: containmentByTenant … companionCountsByRegime
//   (ids INTENT_I1..I8) in intent/invariants.qnt; planInvariants /
//   compileInvariants gather them, pinned by the planInvariantsHoldTest /
//   compileInvariantsHoldTest / invariantsBiteTest runs below.
// Apalache-marked subset: feasibleAgainstCeilings and companionCountsByRegime
//   (INTENT_I7/I8) — collected as intent/alphabet.qnt `compileLaws`;
//   simulator-only today (heap wall — see intent/alphabet.qnt header).
module intent_main {
  import types.* from "../core/types"
  import intent_types.* from "./types"
  import intent_inventory.* from "./inventory"
  import intent_derive.* from "./derive"
  import intent_refuse.* from "./refuse"
  import intent_invariants.* from "./invariants"

  // ---- shared helpers for the derivation runs (task 1.2) ----

  pure def objsOfFam(p: Plan, name: str, fam: Family): Set[PlanObj] =
    p.objs.filter(o => o.key.tenant == name and o.key.family == fam)
  pure def countFam(p: Plan, name: str, fam: Family): int =
    objsOfFam(p, name, fam).size()
  // keyIndex (position of a key in the emission order, or -1) comes from
  // intent_invariants via the flat import above.

  // ---- shared helpers for the refusal runs (task 1.3) ----

  pure def refusedWith(c: Compiled, pred: (Refusal) => bool): bool =
    match c { | Refused(rs) => rs.exists(pred) | _ => false }
  pure def refusalSet(c: Compiled): Set[Refusal] =
    match c { | Refused(rs) => rs | _ => Set() }

  // Inventory variants: one ceiling replaced, or dpmac.7 made DPL-foreign.
  pure def invWithCeiling(fam: Family, ceil: Ceiling): Inventory =
    REF_INVENTORY.with("ceilings", REF_INVENTORY.ceilings.put(fam, ceil))
  pure val invWithForeignDpmac7: Inventory =
    REF_INVENTORY.with("dpmacs", REF_INVENTORY.dpmacs.put(7, REF_INVENTORY.dpmacs.get(7).with("avail", Foreign("dpl"))))
  // dpmac.4 bumped to 40G so a 40G port refuses on the unseeded rate class
  // alone, not on OverRate — the board has no 40G nic, this is a test fixture.
  pure val invWith40GMac: Inventory =
    REF_INVENTORY.with("dpmacs", REF_INVENTORY.dpmacs.put(4, REF_INVENTORY.dpmacs.get(4).with("maxRate", 40000)))

  // ---- shared intent fragments ----

  pure val router: Tenant = { name: "router", dataplane: UserspacePoll, maxCores: 8 }
  pure val kernel16: Tenant = kernelTenant(16)
  pure val wan0: Port = { name: "wan0", dpmac: 7, rate: 10000, tenant: "router" }
  pure val wan1: Port = { name: "wan1", dpmac: 8, rate: 10000, tenant: "router" }
  pure val routerIntent: Intent =
    { tenants: [router], ports: [wan0, wan1], links: [], fabrics: [], crypto: Set(), limits: Set() }
  // A poll-mode router terminating two 10G ports and one 25G: T = 1 + 2 + 2 +
  // 5 = 10, a cross-class mix the worker formula prices but flags unmeasured
  // (design D3). max_cores raised to 16 so T = 10 fits under the budget.
  pure val wan2: Port = { name: "wan2", dpmac: 4, rate: 25000, tenant: "router" }
  pure val mixedClassIntent: Intent =
    { tenants: [router.with("maxCores", 16)], ports: [wan0, wan1, wan2],
      links: [], fabrics: [], crypto: Set(), limits: Set() }

  // The intents the derivation/refusal runs exercise, factored out so the
  // task-1.4 invariant runs can assert planInvariants/compileInvariants on the
  // same plans (the runs' own assertions are unchanged).
  pure val limitIntent: Intent =
    { tenants: [router], ports: [wan0, wan1], links: [], fabrics: [], crypto: Set(),
      limits: Set({ tenant: "router", family: Dpio, limit: 12 }) }
  pure val kernelIntent: Intent =
    { tenants: [kernel16],
      ports: [{ name: "lan0", dpmac: 9, rate: 10000, tenant: "kernel" }],
      links: [], fabrics: [], crypto: Set(), limits: Set() }
  pure val fabricCryptoIntent: Intent =
    { tenants: [kernel16, router],
      ports: [{ name: "p7", dpmac: 7, rate: 10000, tenant: "kernel" },
              { name: "p8", dpmac: 8, rate: 10000, tenant: "kernel" }],
      links: [],
      fabrics: [{ name: "lan", switching: Hardware, forwardedBy: "kernel",
                  members: [MPort("p7"), MPort("p8"), MTenant("router")] }],
      crypto: Set({ tenant: "router", flows: 4 }),
      limits: Set() }
  pure val softwareFabricIntent: Intent =
    { tenants: [kernel16, { name: "vpp", dataplane: UserspacePoll, maxCores: 8 }],
      ports: [{ name: "p7", dpmac: 7, rate: 10000, tenant: "vpp" },
              { name: "p8", dpmac: 8, rate: 10000, tenant: "vpp" }],
      links: [],
      fabrics: [{ name: "br", switching: Software, forwardedBy: "vpp",
                  members: [MPort("p7"), MPort("p8"), MTenant("kernel")] }],
      crypto: Set(),
      limits: Set() }
  pure val chainIntent: Intent =
    { tenants: [kernel16, { name: "vpp", dataplane: UserspacePoll, maxCores: 8 }],
      ports: [{ name: "p7", dpmac: 7, rate: 10000, tenant: "vpp" },
              { name: "p8", dpmac: 8, rate: 10000, tenant: "kernel" },
              { name: "p9", dpmac: 9, rate: 10000, tenant: "kernel" },
              { name: "p10", dpmac: 10, rate: 10000, tenant: "vpp" }],
      links: [],
      fabrics: [{ name: "hw", switching: Hardware, forwardedBy: "kernel",
                  members: [MPort("p8"), MPort("p9")] },
                { name: "sw", switching: Software, forwardedBy: "vpp",
                  members: [MPort("p7"), MFabric("hw")] }],
      crypto: Set(),
      limits: Set() }
  pure val hwInHwIntent: Intent =
    { tenants: [kernel16],
      ports: [{ name: "p8", dpmac: 8, rate: 10000, tenant: "kernel" },
              { name: "p9", dpmac: 9, rate: 10000, tenant: "kernel" }],
      links: [],
      fabrics: [{ name: "hw1", switching: Hardware, forwardedBy: "kernel", members: [MPort("p8")] },
                { name: "hw2", switching: Hardware, forwardedBy: "kernel", members: [MFabric("hw1"), MPort("p9")] }],
      crypto: Set(), limits: Set() }
  pure val linkIntent: Intent =
    { tenants: [{ name: "routerA", dataplane: UserspacePoll, maxCores: 8 },
                  { name: "routerB", dataplane: UserspacePoll, maxCores: 8 }],
      ports: [], links: [{ name: "wire", interfaceA: "routerA", interfaceB: "routerB" }],
      fabrics: [], crypto: Set(), limits: Set() }
  // The refused intents the compile-side runs exercise.
  pure val coreBudgetIntent: Intent = routerIntent.with("tenants", [router.with("maxCores", 4)])
  pure val infeasibleIntent: Intent =
    { tenants: [{ name: "a", dataplane: UserspacePoll, maxCores: 2 },
                  { name: "b", dataplane: UserspacePoll, maxCores: 2 },
                  { name: "d", dataplane: UserspacePoll, maxCores: 2 }],
      ports: [], links: [], fabrics: [], crypto: Set(), limits: Set() }
  pure val allViolationsIntent: Intent =
    { tenants: [router.with("maxCores", 4), kernel16],
      ports: [wan0, wan1, { name: "mgmt", dpmac: 17, rate: 1000, tenant: "kernel" }],
      links: [], fabrics: [], crypto: Set(), limits: Set() }

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
    kernelTenant(16).dataplane == KernelNetlink,
  }

  // A poll-mode router over two 10G ports: the child DPRC, two dpnis at
  // T = 5 transmit queues, the 2·T dpio / dpbp 2 / dpmcp 1 / dpcon 10
  // companion draws, the dpni↔dpmac edges, and the emission order.
  run pollModeRouterDeriveTest =
    val p = derive(routerIntent, REF_INVENTORY)
    all {
      p.objs.filter(o => o.key.family == Dprc) ==
        Set({ key: { tenant: "router", family: Dprc, ordinal: 1 }, container: Root,
              attrs: DprcAttrs({ options: DPRC_DEFAULT_OPTIONS }), prov: { tenant: "router", rule: "dprc", construct: "" } }),
      countFam(p, "router", Dpni) == 2,
      objsOfFam(p, "router", Dpni).forall(o =>
        o.container == Child("router") and match o.attrs { | DpniAttrs(a) => a.numQueues == 5 | _ => false }),
      countFam(p, "router", Dpio) == 10,
      countFam(p, "router", Dpbp) == 2,
      countFam(p, "router", Dpmcp) == 1,
      countFam(p, "router", Dpcon) == 10,
      p.objs.filter(o => o.key.tenant == "router" and o.key.family != Dprc).forall(o => o.container == Child("router")),
      objsOfFam(p, "router", Dpio).map(o => o.key.ordinal) == 1.to(10),
      p.prov.get({ tenant: "router", rule: "dpio", construct: "" }).value == 10,
      p.prov.get({ tenant: "router", rule: "dpio", construct: "" }).request == 10,
      p.prov.get({ tenant: "router", rule: "dpio", construct: "" }).limit == None,
      p.prov.get({ tenant: "router", rule: "dpio", construct: "" }).inputs == Set({ tenant: "router", rule: "T", construct: "" }),
      p.prov.get({ tenant: "router", rule: "T", construct: "" }).value == 5,
      p.prov.get({ tenant: "router", rule: "T", construct: "" }).mark == Unmeasured,
      p.prov.get({ tenant: "router", rule: "T", construct: "" }).constructs == Set("wan0", "wan1"),
      p.edges == Set(
        { a: ObjEnd({ key: { tenant: "router", family: Dpni, ordinal: 1 }, port: 0 }), b: MacEnd(7),
          prov: { tenant: "router", rule: "port-edge", construct: "wan0" } },
        { a: ObjEnd({ key: { tenant: "router", family: Dpni, ordinal: 2 }, port: 0 }), b: MacEnd(8),
          prov: { tenant: "router", rule: "port-edge", construct: "wan1" } }),
      keyIndex(p.order, { tenant: "router", family: Dprc, ordinal: 1 }) <
        keyIndex(p.order, { tenant: "router", family: Dpio, ordinal: 1 }),
      keyIndex(p.order, { tenant: "router", family: Dpio, ordinal: 10 }) <
        keyIndex(p.order, { tenant: "router", family: Dpmcp, ordinal: 1 }),
      keyIndex(p.order, { tenant: "router", family: Dpmcp, ordinal: 1 }) <
        keyIndex(p.order, { tenant: "router", family: Dpni, ordinal: 1 }),
      p.order.nth(0) == { tenant: KERNEL, family: Dprtc, ordinal: 1 },
    }

  // A per-family limit raises the request (design D5): 12 dpios, provenance
  // prints both request 10 and limit 12.
  run limitRaisesRequestTest =
    val p = derive(limitIntent, REF_INVENTORY)
    all {
      countFam(p, "router", Dpio) == 12,
      p.prov.get({ tenant: "router", rule: "dpio", construct: "" }).request == 10,
      p.prov.get({ tenant: "router", rule: "dpio", construct: "" }).limit == Some(12),
      p.prov.get({ tenant: "router", rule: "dpio", construct: "" }).value == 12,
    }

  // The kernel dataplane: no child DPRC, dpio per online CPU, dpmcp cpus+dpnis,
  // dpbp per dpni, dpcon per polled queue, the pinned dprtc, dpio ≺ dpmcp.
  run kernelDeriveTest =
    val p = derive(kernelIntent, REF_INVENTORY)
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
        Set({ key: { tenant: KERNEL, family: Dprtc, ordinal: 1 }, container: Root, attrs: Unsized,
              prov: { tenant: KERNEL, rule: "dprtc", construct: "" } }),
      keyIndex(p.order, { tenant: "kernel", family: Dpio, ordinal: 16 }) <
        keyIndex(p.order, { tenant: "kernel", family: Dpmcp, ordinal: 1 }),
    }

  // A hardware-switched fabric (kernel-forwarded) plus a crypto accelerator:
  // one dpsw with num_ifs = ports + tenants, the fabric-member ports
  // switched (no dpni, no port-edge) and attached to the dpsw interfaces,
  // the member tenant joined through one fabric-membership dpni, and one
  // dpseci sized to the crypto construct's flows.
  run fabricAndCryptoTest =
    val p = derive(fabricCryptoIntent, REF_INVENTORY)
    all {
      p.objs.filter(o => o.key.family == Dpsw) ==
        Set({ key: { tenant: "kernel", family: Dpsw, ordinal: 1 }, container: Root,
              attrs: DpswAttrs({ numIfs: 3, maxFdbs: 3, perFdbFlooding: true, perFdbBroadcast: true, ctrlIf: true }),
              prov: { tenant: "kernel", rule: "dpsw", construct: "lan" } }),
      countFam(p, "router", Dpni) == 1,
      objsOfFam(p, "router", Dpni).forall(o => match o.attrs { | DpniAttrs(a) => a.numQueues == 1 | _ => false }),
      countFam(p, "router", Dpio) == 2,
      countFam(p, "router", Dpcon) == 1,
      countFam(p, "kernel", Dpni) == 0,
      countFam(p, "kernel", Dpmcp) == 17,
      countFam(p, "kernel", Dpbp) == 1,
      countFam(p, "kernel", Dpcon) == 0,
      p.objs.filter(o => o.key.family == Dpseci) ==
        Set({ key: { tenant: "router", family: Dpseci, ordinal: 1 }, container: Child("router"),
              attrs: DpseciAttrs({ numQueues: 4, hasCg: true }), prov: { tenant: "router", rule: "dpseci", construct: "" } }),
      p.edges.contains({ a: ObjEnd({ key: { tenant: "kernel", family: Dpsw, ordinal: 1 }, port: 0 }),
                         b: MacEnd(7), prov: { tenant: "kernel", rule: "fabric-edge", construct: "lan" } }),
      p.edges.contains({ a: ObjEnd({ key: { tenant: "kernel", family: Dpsw, ordinal: 1 }, port: 1 }),
                         b: MacEnd(8), prov: { tenant: "kernel", rule: "fabric-edge", construct: "lan" } }),
      p.edges.contains({ a: ObjEnd({ key: { tenant: "kernel", family: Dpsw, ordinal: 1 }, port: 2 }),
                         b: ObjEnd({ key: { tenant: "router", family: Dpni, ordinal: 1 }, port: 0 }),
                         prov: { tenant: "kernel", rule: "fabric-edge", construct: "lan" } }),
      p.edges.filter(e => e.prov.rule == "port-edge") == Set(),
    }

  // A software fabric (figure 6b): vpp bridges its own two 10G ports and a
  // pseudo-wire to the kernel. No dpsw exists — the MC never sees a software
  // switch. vpp takes three dpnis (two ports, one wire end); the kernel
  // takes its wire end. (1.3 refuses this exact intent for other reasons;
  // the shape assertions still hold. Two ports keep T seeded: 2×10G ⇒ 5.)
  run softwareFabricTest =
    val p = derive(softwareFabricIntent, REF_INVENTORY)
    all {
      p.objs.filter(o => o.key.family == Dpsw) == Set(),
      countFam(p, "vpp", Dpni) == 3,
      objsOfFam(p, "vpp", Dpni).forall(o => match o.attrs { | DpniAttrs(a) => a.numQueues == 5 | _ => false }),
      countFam(p, "vpp", Dpio) == 10,
      countFam(p, "kernel", Dpni) == 1,
      p.edges.contains({ a: ObjEnd({ key: { tenant: "vpp", family: Dpni, ordinal: 1 }, port: 0 }),
                         b: MacEnd(7), prov: { tenant: "vpp", rule: "port-edge", construct: "p7" } }),
      p.edges.contains({ a: ObjEnd({ key: { tenant: "vpp", family: Dpni, ordinal: 2 }, port: 0 }),
                         b: MacEnd(8), prov: { tenant: "vpp", rule: "port-edge", construct: "p8" } }),
      p.edges.contains({ a: ObjEnd({ key: { tenant: "vpp", family: Dpni, ordinal: 3 }, port: 0 }),
                         b: ObjEnd({ key: { tenant: "kernel", family: Dpni, ordinal: 1 }, port: 0 }),
                         prov: { tenant: "vpp", rule: "fabric-wire", construct: "br" } }),
    }

  // The operator's chain of switches: a hardware fabric (kernel dpsw over two
  // ports) is a member of a software fabric vpp bridges alongside its own
  // port. vpp attaches to the dpsw via a dpni; kernel terminates nothing
  // itself. vpp owns two terminated 10G ports (p7, p10) to keep T seeded ⇒
  // three dpnis, the attach dpni last. No pseudo-wire (both software members
  // resolve to no peer tenant).
  run chainTest =
    val p = derive(chainIntent, REF_INVENTORY)
    all {
      p.objs.filter(o => o.key.family == Dpsw) ==
        Set({ key: { tenant: "kernel", family: Dpsw, ordinal: 1 }, container: Root,
              attrs: DpswAttrs({ numIfs: 3, maxFdbs: 3, perFdbFlooding: true, perFdbBroadcast: true, ctrlIf: true }),
              prov: { tenant: "kernel", rule: "dpsw", construct: "hw" } }),
      countFam(p, "vpp", Dpni) == 3,
      countFam(p, "kernel", Dpni) == 0,
      countFam(p, "kernel", Dpmcp) == 17,
      countFam(p, "kernel", Dpbp) == 1,
      p.edges.contains({ a: ObjEnd({ key: { tenant: "kernel", family: Dpsw, ordinal: 1 }, port: 0 }),
                         b: MacEnd(8), prov: { tenant: "kernel", rule: "fabric-edge", construct: "hw" } }),
      p.edges.contains({ a: ObjEnd({ key: { tenant: "kernel", family: Dpsw, ordinal: 1 }, port: 1 }),
                         b: MacEnd(9), prov: { tenant: "kernel", rule: "fabric-edge", construct: "hw" } }),
      p.edges.contains({ a: ObjEnd({ key: { tenant: "kernel", family: Dpsw, ordinal: 1 }, port: 2 }),
                         b: ObjEnd({ key: { tenant: "vpp", family: Dpni, ordinal: 3 }, port: 0 }),
                         prov: { tenant: "kernel", rule: "fabric-edge", construct: "hw" } }),
      p.edges.contains({ a: ObjEnd({ key: { tenant: "vpp", family: Dpni, ordinal: 1 }, port: 0 }),
                         b: MacEnd(7), prov: { tenant: "vpp", rule: "port-edge", construct: "p7" } }),
      p.edges.filter(e => e.prov.rule == "fabric-wire") == Set(),
    }

  // Hardware-in-Hardware derives nothing (open item (d)): hw2 lists hw1 as a
  // member, but a dpsw interface cannot be another dpsw's interface — hw2's
  // dpsw counts only its own port. Both fabrics still emit their dpsw.
  run hwInHwDerivesNothingTest =
    val p = derive(hwInHwIntent, REF_INVENTORY)
    all {
      countFam(p, "kernel", Dpsw) == 2,
      objsOfFam(p, "kernel", Dpsw).forall(o => match o.attrs { | DpswAttrs(a) => a.numIfs == 1 | _ => false }),
      // hw2's dpsw (ordinal 2) connects only to its own port, never to hw1's dpsw
      p.edges.filter(e => match e.a { | ObjEnd(x) => x.key == { tenant: "kernel", family: Dpsw, ordinal: 2 } | _ => false }) ==
        Set({ a: ObjEnd({ key: { tenant: "kernel", family: Dpsw, ordinal: 2 }, port: 0 }),
              b: MacEnd(9), prov: { tenant: "kernel", rule: "fabric-edge", construct: "hw2" } }),
    }

  // A virtual fabric: a link between two userspace-poll tenants, no dpmac —
  // each takes one dpni and the pair is the single edge; two child DPRCs.
  run linkTest =
    val p = derive(linkIntent, REF_INVENTORY)
    all {
      countFam(p, "routerA", Dpni) == 1,
      countFam(p, "routerB", Dpni) == 1,
      objsOfFam(p, "routerA", Dpni).forall(o => match o.attrs { | DpniAttrs(a) => a.numQueues == 1 | _ => false }),
      objsOfFam(p, "routerB", Dpni).forall(o => match o.attrs { | DpniAttrs(a) => a.numQueues == 1 | _ => false }),
      p.edges.contains({ a: ObjEnd({ key: { tenant: "routerA", family: Dpni, ordinal: 1 }, port: 0 }),
                         b: ObjEnd({ key: { tenant: "routerB", family: Dpni, ordinal: 1 }, port: 0 }),
                         prov: { tenant: "routerA", rule: "link-edge", construct: "wire" } }),
      p.objs.filter(o => o.key.family == Dprc).size() == 2,
    }

  // The worker formula prices the seeded classes (25G ⇒ 1 + 5 = 6), refuses
  // an unseeded one with None, and gives a portless tenant the main-only
  // thread count.
  run unknownRateClassTest = all {
    threadCount([{ name: "x", dpmac: 99, rate: 40000, tenant: "c" }]) == None,
    threadCount([{ name: "y", dpmac: 4, rate: 25000, tenant: "c" }]) == Some(6),
    threadCount([]) == Some(1),
  }

  // The derivation is a pure function of its two inputs (design D5).
  run deterministicTest = all {
    derive(routerIntent, REF_INVENTORY) == derive(routerIntent, REF_INVENTORY),
  }

  // ---- refusals: the total function's other half (task 1.3) ----

  // A clean intent compiles: the plan is the derivation, and every derived
  // family whose ceiling is Unknown with a non-zero count warns (Dprc 1,
  // Dpio 10, Dpcon 10; Dpseci/Dpsw are zero). No refusal.
  run compileOkTest =
    match compile(routerIntent, REF_INVENTORY) {
      | Ok(res) => all {
          res.plan == derive(routerIntent, REF_INVENTORY),
          res.warnings == Set(
            UnknownCeiling({ family: Dprc, needed: 1 }),
            UnknownCeiling({ family: Dpio, needed: 10 }),
            UnknownCeiling({ family: Dpcon, needed: 10 })),
        }
      | Refused(_) => false
    }

  // T = 5 above max_cores = 4 refuses by name and nothing else (design D3).
  run coreBudgetExceededTest =
    refusalSet(compile(coreBudgetIntent, REF_INVENTORY)) ==
      Set(CoreBudgetExceeded({ tenant: "router", t: 5, maxCores: 4 }))

  // Two independent violations reported at once (spec.md "All violations at
  // once"): a reserved anchor and an exceeded core budget, and only those.
  run allViolationsAtOnceTest =
    val why17 = match REF_INVENTORY.dpmacs.get(17).avail { | Reserved(w) => w | _ => "" }
    val c = compile(allViolationsIntent, REF_INVENTORY)
    all {
      refusedWith(c, r => r == ReservedAnchor({ port: "mgmt", dpmac: 17, why: why17 })),
      refusedWith(c, r => r == CoreBudgetExceeded({ tenant: "router", t: 5, maxCores: 4 })),
      refusalSet(c).size() == 2,
    }

  // Three portless poll-mode tenants draw 6 dpbps against a listed pool of
  // 5: cross-plan infeasibility named to the family and shortfall (ADR-0011).
  run infeasibleTest =
    refusalSet(compile(infeasibleIntent, invWithCeiling(Dpbp, Counted(5)))) ==
      Set(Infeasible({ family: Dpbp, needed: 6, available: 5 }))

  // A dpmac claimed by two ports, and a port switched into two fabrics.
  run doubleClaimedTest =
    val twoPorts = { tenants: [router],
      ports: [wan0, { name: "wan1", dpmac: 7, rate: 10000, tenant: "router" }],
      links: [], fabrics: [], crypto: Set(), limits: Set() }
    val twoFabrics = { tenants: [kernel16],
      ports: [{ name: "p8", dpmac: 8, rate: 10000, tenant: "kernel" }],
      links: [],
      fabrics: [{ name: "f1", switching: Hardware, forwardedBy: "kernel", members: [MPort("p8")] },
                { name: "f2", switching: Hardware, forwardedBy: "kernel", members: [MPort("p8")] }],
      crypto: Set(), limits: Set() }
    all {
      refusedWith(compile(twoPorts, REF_INVENTORY),
        r => r == DoubleClaimed({ dpmac: 7, constructs: Set("wan0", "wan1") })),
      refusedWith(compile(twoFabrics, REF_INVENTORY),
        r => r == DoubleClaimed({ dpmac: 8, constructs: Set("f1", "f2") })),
    }

  // A hardware fabric a userspace-poll tenant forwards, and a kernel fabric
  // that lists a router-terminated port (design D4/D5).
  run fabricNotKernelForwardedTest =
    val tenantForwarded = { tenants: [router],
      ports: [wan0],
      links: [],
      fabrics: [{ name: "rsw", switching: Hardware, forwardedBy: "router", members: [MPort("wan0")] }],
      crypto: Set(), limits: Set() }
    val mismatch = { tenants: [kernel16, router],
      ports: [{ name: "px", dpmac: 8, rate: 10000, tenant: "router" }],
      links: [],
      fabrics: [{ name: "ksw", switching: Hardware, forwardedBy: "kernel", members: [MPort("px")] }],
      crypto: Set(), limits: Set() }
    all {
      refusedWith(compile(tenantForwarded, REF_INVENTORY),
        r => r == FabricNotKernelForwarded({ fabric: "rsw", forwardedBy: "router" })),
      refusedWith(compile(mismatch, REF_INVENTORY),
        r => r == PortTenantMismatch({ fabric: "ksw", port: "px", tenant: "router" })),
    }

  // A limit below its request refuses; a limit on a family whose count is
  // fixed by constructs is never silently ignored (design D5/D7).
  run limitRefusalsTest =
    val below = routerIntent.with("limits", Set({ tenant: "router", family: Dpbp, limit: 1 }))
    val notDerived = routerIntent.with("limits", Set({ tenant: "router", family: Dpni, limit: 5 }))
    all {
      refusedWith(compile(below, REF_INVENTORY),
        r => r == LimitBelowRequest({ tenant: "router", family: Dpbp, request: 2, limit: 1 })),
      refusedWith(compile(notDerived, REF_INVENTORY),
        r => r == LimitNotDerived({ tenant: "router", family: Dpni })),
    }

  // A 40G port is a rate class the seeded worker table does not know: refused
  // rather than extrapolated (design D3; derive.qnt open item (b)). The anchor
  // is a 40G-capable dpmac so OverRate does not fire — UnknownRateClass is the
  // one refusal.
  run unknownRateClassRefusedTest =
    val intent = { tenants: [router],
      ports: [{ name: "w", dpmac: 4, rate: 40000, tenant: "router" }],
      links: [], fabrics: [], crypto: Set(), limits: Set() }
    refusalSet(compile(intent, invWith40GMac)) ==
      Set(UnknownRateClass({ tenant: "router", rates: [40000] }))

  // Two seeded rate classes on one poll-mode tenant compile: T sums the
  // per-port workers to 1 + 2 + 2 + 5 = 10 (dpio = 2·T = 20), and the accepted
  // compile carries an unmeasured-combination warning naming the tenant and
  // the classes mixed (design D3) — the review's escape-hatch-warns rule.
  run mixedRateClassWarnsTest =
    match compile(mixedClassIntent, REF_INVENTORY) {
      | Ok(res) => all {
          res.plan.prov.get({ tenant: "router", rule: "T", construct: "" }).value == 10,
          res.plan.prov.get({ tenant: "router", rule: "T", construct: "" }).mark == Unmeasured,
          countFam(res.plan, "router", Dpio) == 20,
          res.warnings.contains(UnmeasuredCombination({ tenant: "router", rates: Set(10000, 25000) })),
        }
      | Refused(_) => false
    }

  // A single seeded class stays T = 5 and carries no mix warning — the flag
  // fires only on a genuine cross-class combination (design D3).
  run singleClassNoMixWarnTest =
    match compile(routerIntent, REF_INVENTORY) {
      | Ok(res) => all {
          res.plan.prov.get({ tenant: "router", rule: "T", construct: "" }).value == 5,
          not(res.warnings.exists(w => match w { | UnmeasuredCombination(_) => true | _ => false })),
        }
      | Refused(_) => false
    }

  // An unanchored dpmac, a DPL-foreign anchor, and a rate above the anchor's
  // maximum (design D2); each asserted by containment (refusedWith), since a
  // rejected port may also perturb sizing.
  run unanchoredForeignTest =
    val unanchored = { tenants: [router],
      ports: [{ name: "w", dpmac: 99, rate: 10000, tenant: "router" }],
      links: [], fabrics: [], crypto: Set(), limits: Set() }
    val overRate = { tenants: [router],
      ports: [{ name: "wan0", dpmac: 7, rate: 25000, tenant: "router" }],
      links: [], fabrics: [], crypto: Set(), limits: Set() }
    all {
      refusedWith(compile(unanchored, REF_INVENTORY),
        r => r == Unanchored({ port: "w", dpmac: 99 })),
      refusedWith(compile(routerIntent, invWithForeignDpmac7),
        r => r == ForeignAnchor({ port: "wan0", dpmac: 7, owner: "dpl" })),
      refusedWith(compile(overRate, REF_INVENTORY),
        r => r == OverRate({ port: "wan0", rate: 25000, maxRate: 10000 })),
    }

  // A hardware fabric inside a hardware fabric is refused (object-model.md §2;
  // derive.qnt open item (d)): the same intent hwInHwDerivesNothingTest derives.
  run unsupportedEdgeTest =
    val intent = { tenants: [kernel16],
      ports: [{ name: "p8", dpmac: 8, rate: 10000, tenant: "kernel" },
              { name: "p9", dpmac: 9, rate: 10000, tenant: "kernel" }],
      links: [],
      fabrics: [{ name: "hw1", switching: Hardware, forwardedBy: "kernel", members: [MPort("p8")] },
                { name: "hw2", switching: Hardware, forwardedBy: "kernel", members: [MFabric("hw1"), MPort("p9")] }],
      crypto: Set(), limits: Set() }
    refusedWith(compile(intent, REF_INVENTORY),
      r => r == UnsupportedEdge({ fabric: "hw2", member: "hw1" }))

  // A construct naming a tenant that was never declared (design D5).
  run tenantAbsentTest =
    val intent = { tenants: [router],
      ports: [{ name: "wan0", dpmac: 7, rate: 10000, tenant: "ghost" }],
      links: [], fabrics: [], crypto: Set(), limits: Set() }
    refusedWith(compile(intent, REF_INVENTORY),
      r => r == TenantAbsent({ construct: "wan0", tenant: "ghost" }))

  // ---- plan relationships as named invariants (task 1.4) ----

  // planInvariants (INTENT_I1..I6) hold on every plan the derivation runs
  // build: the relationships D6 locks by construction are true of each.
  run planInvariantsHoldTest = all {
    planInvariants(derive(routerIntent, REF_INVENTORY)),
    planInvariants(derive(limitIntent, REF_INVENTORY)),
    planInvariants(derive(kernelIntent, REF_INVENTORY)),
    planInvariants(derive(fabricCryptoIntent, REF_INVENTORY)),
    planInvariants(derive(softwareFabricIntent, REF_INVENTORY)),
    planInvariants(derive(chainIntent, REF_INVENTORY)),
    planInvariants(derive(hwInHwIntent, REF_INVENTORY)),
    planInvariants(derive(linkIntent, REF_INVENTORY)),
  }

  // compileInvariants (INTENT_I7 feasibility, INTENT_I8 companion counts) hold
  // over the same intents and over the refused ones — a refusal set is never
  // empty, and Ok plans keep every count within its ceiling and dataplane draw.
  run compileInvariantsHoldTest = all {
    compileInvariants(routerIntent, REF_INVENTORY),
    compileInvariants(limitIntent, REF_INVENTORY),
    compileInvariants(kernelIntent, REF_INVENTORY),
    compileInvariants(fabricCryptoIntent, REF_INVENTORY),
    compileInvariants(softwareFabricIntent, REF_INVENTORY),
    compileInvariants(chainIntent, REF_INVENTORY),
    compileInvariants(hwInHwIntent, REF_INVENTORY),
    compileInvariants(linkIntent, REF_INVENTORY),
    compileInvariants(coreBudgetIntent, REF_INVENTORY),
    compileInvariants(infeasibleIntent, invWithCeiling(Dpbp, Counted(5))),
    compileInvariants(allViolationsIntent, REF_INVENTORY),
  }

  // The invariants bite: four hand-built bad plans, each a mutation of the
  // router plan, each rejected by the invariant whose relationship it breaks —
  // a dpmac↔dpmac edge (I2), a companion in Root (I1), a reversed emission
  // order (I4), and a companion count past its node value (I3).
  run invariantsBiteTest =
    val base = derive(routerIntent, REF_INVENTORY)
    val badEdge = base.with("edges", base.edges.union(Set(
      { a: MacEnd(7), b: MacEnd(8), prov: { tenant: "router", rule: "mac-mac", construct: "bogus" } })))
    val badContainer = base.with("objs", base.objs.map(o =>
      if (o.key.family == Dpio and o.key.ordinal == 1) o.with("container", Root) else o))
    val badOrder = base.with("order", base.order.foldl([], (acc, k) => [k].concat(acc)))
    val badCompanion = base.with("objs", base.objs.union(Set(
      { key: { tenant: "router", family: Dpio, ordinal: 11 },
        container: Child("router"), attrs: Unsized,
        prov: { tenant: "router", rule: "dpio", construct: "" } })))
    all {
      not(edgesTypedAndSingle(badEdge)),
      not(containmentByTenant(badContainer)),
      not(emissionOrderLawful(badOrder)),
      not(companionsOnlyDerived(badCompanion)),
    }
}
