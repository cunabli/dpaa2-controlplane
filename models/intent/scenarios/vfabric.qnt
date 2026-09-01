// Scenario (2) of the intent gate (openspec change `intent-layer`, design
// D8; task 2.2): a virtual fabric — two userspace-poll tenants, routerA and
// routerB, joined by a single link and no dpmac anywhere. No kernel
// tenant either: the scenario proves child dprcs and the dpni↔dpni
// pseudo-wire stand on their own, with no port and no hardware to lean on.
// Portless ⇒ T = 1 (main-only, ADR-0012), so each side takes one dpni with
// numQueues = 1 and the ADR-0012 poll-mode companion draws at T = 1.
// vfabric.toml beside this file is what the operator types for the same
// intent; the pairing is by convention until dpaa2-verify's test (task 3.4).
//
// The twin (model-only, no operator file): the same intent plus a third
// child dprc routerC. Each portless userspace-poll child draws 2 dpbps (ADR-0012),
// so three of them need 6; the twin's inventory lowers the dpbp ceiling to
// Counted(5) and the plan is refused Infeasible naming the family and the
// shortfall (ADR-0011, decision 1). The same twin against the reference
// pool fits — the ceiling refuses, not the shape (design D8/D2).
//
// Anchors: ADR-0011 the Counted dpbp ceiling (decision 1); ADR-0012 dpbp =
// 2 per poll-mode child and T = main + workers, portless ⇒ 1; object-model.md
// §2 / DPNI-I9 the dpni↔dpni pseudo-wire.
//
// Named invariants: none new — the runs assert planInvariants /
//   compileInvariants (INTENT_I1..I8) over the scenario and its twin.
// Apalache-marked subset: none (directed runs; the alphabet rung carries
//   the marked laws).
module vfabric {
  import types.* from "../../core/types"           // Family
  import intent_types.* from "../types"            // Intent, KERNEL, kernelTenant, …
  import intent_inventory.* from "../inventory"    // REF_INVENTORY
  import intent_derive.* from "../derive"          // Plan, PlanObj, End, DpniAttrs, …
  import intent_refuse.* from "../refuse"          // compile, Infeasible
  import intent_invariants.* from "../invariants"  // planInvariants, compileInvariants

  pure def objsOfFam(p: Plan, name: str, fam: Family): Set[PlanObj] =
    p.objs.filter(o => o.key.tenant == name and o.key.family == fam)
  pure def countFam(p: Plan, name: str, fam: Family): int =
    objsOfFam(p, name, fam).size()

  // The intent vfabric.toml states: two poll-mode tenants wired together,
  // no ports, no fabrics — mirrors linkIntent in intent/main.qnt.
  pure val accepted: Intent =
    { tenants: [{ name: "routerA", dataplane: UserspacePoll, maxCores: 8, isolation: Isolated, pool: "" },
                  { name: "routerB", dataplane: UserspacePoll, maxCores: 8, isolation: Isolated, pool: "" }],
      ports: [], links: [{ name: "wire", interfaceA: "routerA", interfaceB: "routerB" }],
      fabrics: [], crypto: [], extras: Set() }

  // The twin: the same plus a third portless tenant routerC (model-only).
  pure val twin: Intent =
    accepted.with("tenants", accepted.tenants.append(
      { name: "routerC", dataplane: UserspacePoll, maxCores: 8, isolation: Isolated, pool: "" }))

  // The twin's inventory lowers the dpbp ceiling below the cross-plan draw:
  // three portless poll-mode children need 6 dpbps against a listed pool of 5.
  pure val twinInventory: Inventory =
    REF_INVENTORY.with("ceilings", REF_INVENTORY.ceilings.put(Dpbp, Counted(5)))

  // The expected plan shape (design D6/D8): two child DPRCs, each tenant at
  // T = 1 (portless, main-only) with one dpni and the ADR-0012 poll-mode
  // draws, the pair joined by the single dpni↔dpni pseudo-wire; no dpsw and
  // no dpseci anywhere; T visibly unmeasured (design D3).
  run vfabricPlanShapeTest =
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) =>
          val p = res.plan
          all {
            p.objs.filter(o => o.key.family == Dprc).size() == 2,
            countFam(p, "routerA", Dpni) == 1,
            objsOfFam(p, "routerA", Dpni).forall(o =>
              o.container == Child("routerA") and
              match o.attrs { | DpniAttrs(a) => a.numQueues == 1 | _ => false }),
            countFam(p, "routerA", Dpio) == 2,   // 2·T (ADR-0012)
            countFam(p, "routerA", Dpbp) == 2,
            countFam(p, "routerA", Dpmcp) == 1,
            countFam(p, "routerA", Dpcon) == 1,  // dpnis · T (DPCON-I1)
            countFam(p, "routerB", Dpni) == 1,
            objsOfFam(p, "routerB", Dpni).forall(o =>
              o.container == Child("routerB") and
              match o.attrs { | DpniAttrs(a) => a.numQueues == 1 | _ => false }),
            countFam(p, "routerB", Dpio) == 2,
            countFam(p, "routerB", Dpbp) == 2,
            countFam(p, "routerB", Dpmcp) == 1,
            countFam(p, "routerB", Dpcon) == 1,
            p.objs.filter(o => o.key.family == Dpsw) == Set(),
            p.objs.filter(o => o.key.family == Dpseci) == Set(),
            // the single pseudo-wire is the whole edge set (no dpmac, no port)
            p.edges == Set({ a: ObjEnd({ key: { tenant: "routerA", family: Dpni, ordinal: 1 }, port: 0 }),
                             b: ObjEnd({ key: { tenant: "routerB", family: Dpni, ordinal: 1 }, port: 0 }),
                             prov: { tenant: "routerA", rule: "link-edge", construct: "wire" } }),
            p.prov.get({ tenant: "routerA", rule: "T", construct: "" }).value == 1,
            p.prov.get({ tenant: "routerA", rule: "T", construct: "" }).mark == Unmeasured,
          }
      | Refused(_) => false
    }

  // The invariant run: INTENT_I7/I8 over the scenario and its twin, INTENT_I1..I6
  // over the accepted plan.
  run vfabricInvariantsTest = all {
    compileInvariants(accepted, REF_INVENTORY),
    compileInvariants(twin, twinInventory),
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) => planInvariants(res.plan)
      | Refused(_) => false
    },
  }

  // The twin: three portless poll-mode children need 6 dpbps against the
  // lowered pool of 5 — refused Infeasible, named to the family and shortfall,
  // and nothing else (ADR-0011).
  run twinInfeasibleTest =
    match compile(twin, twinInventory) {
      | Refused(rs) => rs == Set(Infeasible({ family: Dpbp, needed: 6, available: 5 }))
      | Ok(_) => false
    }

  // The same twin against the unmodified reference pool compiles — the
  // ceiling is what refuses, not the shape (design D8).
  run twinFitsReferencePoolTest =
    match compile(twin, REF_INVENTORY) {
      | Ok(_) => true
      | Refused(_) => false
    }
}
