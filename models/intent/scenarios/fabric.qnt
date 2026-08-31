// Scenario (1) of the intent gate (openspec change `intent-layer`, design
// D8; task 2.1): a hardware-switched fabric over dpmac.7/8 at 10G,
// kernel-steered, with a poll-mode router joined as a consumer member and
// terminating dpmac.9/10 at 2×10G — the seeded rate class (design D3), so
// the router's T = 5 derives from the table and `max_cores` = 8 admits it.
// fabric.toml beside this file is what the operator types for the same
// intent; the pairing is by convention until dpaa2-verify's test (task 3.4).
//
// The twin: the same intent with `max_cores` = 4 sits below the derived
// T = 5 and is refused as CoreBudgetExceeded naming the consumer, T, and
// the budget. The kernel owner's budget never binds under the online-CPU
// reading of open question 2, so the poll-mode member is what makes the
// refusal reachable (design D8) — pinned by kernelBudgetNeverBindsTest.
//
// Anchors: dpsw.md kernel-bindability predicate (read-not-verified);
// ADR-0012 companion draws; DPAA2 User Manual §2.2.2 fig. 6c.
//
// Named invariants: none new — the runs assert planInvariants /
//   compileInvariants (INTENT_I1..I8) over the scenario and its twin.
// Apalache-marked subset: none (directed runs; the alphabet rung carries
//   the marked laws).
module fabric {
  import types.* from "../../core/types"           // Family
  import companions.* from "../../core/companions" // Regime = Kernel | PollMode
  import intent_types.* from "../types"            // Intent, KERNEL, kernelConsumer, …
  import intent_inventory.* from "../inventory"    // REF_INVENTORY
  import intent_derive.* from "../derive"          // Plan, PlanObj, End, DpswAttrs, …
  import intent_refuse.* from "../refuse"          // compile, CoreBudgetExceeded
  import intent_invariants.* from "../invariants"  // planInvariants, compileInvariants

  pure def objsOfFam(p: Plan, name: str, fam: Family): Set[PlanObj] =
    p.objs.filter(o => o.key.consumer == name and o.key.family == fam)
  pure def countFam(p: Plan, name: str, fam: Family): int =
    objsOfFam(p, name, fam).size()

  // The intent fabric.toml states, parameterized on the router's budget so
  // the accepted scenario and its twin differ in `max_cores` alone.
  pure def fabricIntent(m: int): Intent = {
    consumers: [kernelConsumer(16),
                { name: "router", regime: PollMode, maxCores: m, cryptoFlows: 0 }],
    ports: [{ name: "lan0", dpmac: 7, rate: 10000, owner: KERNEL },
            { name: "lan1", dpmac: 8, rate: 10000, owner: KERNEL },
            { name: "wan0", dpmac: 9, rate: 10000, owner: "router" },
            { name: "wan1", dpmac: 10, rate: 10000, owner: "router" }],
    links: [],
    fabrics: [{ name: "lan", switching: Hardware, owner: KERNEL,
                members: [MPort("lan0"), MPort("lan1"), MConsumer("router")] }],
    crypto: Set(),
    limits: Set(),
  }

  pure val accepted: Intent = fabricIntent(8)  // fabric.toml's max_cores
  pure val twin: Intent = fabricIntent(4)      // below the derived T = 5

  // The expected plan shape (design D4/D6): one kernel dpsw with num_ifs =
  // 3 (two member dpmacs, the router's attach dpni) under the
  // kernel-bindable predicate; the switched member ports yield no kernel
  // dpni and no port-edge; the router takes three dpnis (wan0, wan1, the
  // attach) at T = 5 queues with the ADR-0012 poll-mode draws, T visibly
  // unmeasured (design D3).
  run fabricPlanShapeTest =
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) =>
          val p = res.plan
          all {
            p.objs.filter(o => o.key.family == Dpsw) ==
              Set({ key: { consumer: KERNEL, family: Dpsw, ordinal: 1 }, container: Root,
                    attrs: DpswAttrs({ numIfs: 3, maxFdbs: 3, perFdbFlooding: true,
                                       perFdbBroadcast: true, ctrlIf: true }),
                    prov: { consumer: KERNEL, rule: "dpsw", construct: "lan" } }),
            countFam(p, KERNEL, Dpni) == 0,
            countFam(p, "router", Dpni) == 3,
            objsOfFam(p, "router", Dpni).forall(o =>
              o.container == Child("router") and
              match o.attrs { | DpniAttrs(a) => a.numQueues == 5 | _ => false }),
            countFam(p, "router", Dpio) == 10,   // 2·T (ADR-0012)
            countFam(p, "router", Dpbp) == 2,
            countFam(p, "router", Dpmcp) == 1,
            countFam(p, "router", Dpcon) == 15,  // dpnis · T (DPCON-I1)
            // dpsw interfaces in member order (fig. 6c)
            p.edges.contains({ a: ObjEnd({ key: { consumer: KERNEL, family: Dpsw, ordinal: 1 }, port: 0 }),
                               b: MacEnd(7), prov: { consumer: KERNEL, rule: "fabric-edge", construct: "lan" } }),
            p.edges.contains({ a: ObjEnd({ key: { consumer: KERNEL, family: Dpsw, ordinal: 1 }, port: 1 }),
                               b: MacEnd(8), prov: { consumer: KERNEL, rule: "fabric-edge", construct: "lan" } }),
            p.edges.contains({ a: ObjEnd({ key: { consumer: KERNEL, family: Dpsw, ordinal: 1 }, port: 2 }),
                               b: ObjEnd({ key: { consumer: "router", family: Dpni, ordinal: 3 }, port: 0 }),
                               prov: { consumer: KERNEL, rule: "fabric-edge", construct: "lan" } }),
            // the router's terminated ports (fig. 6a)
            p.edges.contains({ a: ObjEnd({ key: { consumer: "router", family: Dpni, ordinal: 1 }, port: 0 }),
                               b: MacEnd(9), prov: { consumer: "router", rule: "port-edge", construct: "wan0" } }),
            p.edges.contains({ a: ObjEnd({ key: { consumer: "router", family: Dpni, ordinal: 2 }, port: 0 }),
                               b: MacEnd(10), prov: { consumer: "router", rule: "port-edge", construct: "wan1" } }),
            p.edges.filter(e => e.prov.rule == "port-edge").size() == 2,
            p.prov.get({ consumer: "router", rule: "T", construct: "" }).value == 5,
            p.prov.get({ consumer: "router", rule: "T", construct: "" }).mark == Unmeasured,
          }
      | Refused(_) => false
    }

  // The invariant run: INTENT_I7/I8 over both intents, INTENT_I1..I6 over
  // the accepted plan.
  run fabricInvariantsTest = all {
    compileInvariants(accepted, REF_INVENTORY),
    compileInvariants(twin, REF_INVENTORY),
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) => planInvariants(res.plan)
      | Refused(_) => false
    },
  }

  // The twin: max_cores = 4 below T = 5 is refused as CoreBudgetExceeded
  // and nothing else — the file's one error, named (design D3/D8).
  run twinCoreBudgetTest =
    match compile(twin, REF_INVENTORY) {
      | Refused(rs) => rs == Set(CoreBudgetExceeded({ consumer: "router", t: 5, maxCores: 4 }))
      | Ok(_) => false
    }

  // Why the twin needs the poll-mode member: under the online-CPU reading
  // (open question 2, KERNEL_BUDGET_IS_DPIO_COUNT = false) the kernel
  // owner's budget never binds — the same fabric with a starved kernel
  // budget still compiles (design D8).
  run kernelBudgetNeverBindsTest =
    val lowKernel = accepted.with("consumers",
      [kernelConsumer(1),
       { name: "router", regime: PollMode, maxCores: 8, cryptoFlows: 0 }])
    match compile(lowKernel, REF_INVENTORY) {
      | Ok(_) => true
      | Refused(_) => false
    }
}
