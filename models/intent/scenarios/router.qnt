// Scenario (3) of the intent gate (openspec change `intent-layer`, design
// D8; task 2.3): a userspace-poll router over N×10G + 1×25G with an
// accelerator, terminating dpmac.9/10 at 2×10G (N = 2) and dpmac.4 at 25G.
// flows = 2 ≤ N. It is the scenario that exercises the rate table
// across two seeded classes: T = 1 + 2·workers(10G) + workers(25G) =
// 1 + 2·2 + 5 = 10 (design D3), and the 10G+25G mix carries the
// UnmeasuredCombination warning — flagged, never silent. `max_cores` = 16 is
// the reference board's online-CPU count (intent/inventory.qnt), comfortably
// above the derived T = 10. router.toml beside this file is what the operator
// types; the pairing is by convention until dpaa2-verify's test (task 3.4).
//
// The twin (model-only, no operator file): a second userspace-poll tenant
// router2 claims dpmac.9 with a port of its own — one dpmac claimed by two
// tenants. router2 is otherwise a valid single-port shape, so the refusal set
// is exactly DoubleClaimed naming the dpmac and the two claiming ports (design
// D5), and nothing else.
//
// Anchors: dpseci.md num_queues ≥ flows + DPSECI_OPT_HAS_CG; DPMAC-I3
// the immutable 25G rate class read by `dpmac info`; design D3 worker table
// (10G⇒2, 25G⇒5, unmeasured); ADR-0012 poll-mode companion draws; DPAA2 User
// Manual §2.2.2 fig. 6a the dpni↔dpmac port.
//
// Named invariants: none new — the runs assert planInvariants /
//   compileInvariants (INTENT_I1..I8) over the scenario and its twin.
// Apalache-marked subset: none (directed runs; the alphabet rung carries
//   the marked laws).
module router {
  import types.* from "../../core/types"           // Family
  import intent_types.* from "../types"            // Intent, UserspacePoll, …
  import intent_inventory.* from "../inventory"    // REF_INVENTORY
  import intent_derive.* from "../derive"          // Plan, PlanObj, End, DpniAttrs, DpseciAttrs, …
  import intent_refuse.* from "../refuse"          // compile, DoubleClaimed, UnmeasuredCombination
  import intent_invariants.* from "../invariants"  // planInvariants, compileInvariants

  pure def objsOfFam(p: Plan, name: str, fam: Family): Set[PlanObj] =
    p.objs.filter(o => o.key.tenant == name and o.key.family == fam)
  pure def countFam(p: Plan, name: str, fam: Family): int =
    objsOfFam(p, name, fam).size()

  // The intent router.toml states: one poll-mode tenant terminating two 10G
  // ports and one 25G port, with flows = 2 (≤ N = 2) and one crypto
  // accelerator. No dpio, dpbp, dpcon, dpmcp, dpseci, queue or worker count
  // appears — every object and count is derived (ADR-0005).
  pure val accepted: Intent =
    { tenants: [{ name: "router", dataplane: UserspacePoll, maxCores: 16 }],
      ports: [{ name: "wan0", dpmac: 9, rate: 10000, tenant: "router" },
              { name: "wan1", dpmac: 10, rate: 10000, tenant: "router" },
              { name: "up0", dpmac: 4, rate: 25000, tenant: "router" }],
      links: [],
      fabrics: [],
      crypto: Set({ tenant: "router", flows: 2 }),
      limits: Set() }

  // The twin (model-only): a second poll-mode tenant router2 claims dpmac.9
  // with its own port clash0 — the double claim. router2 is otherwise valid
  // (one 10G port ⇒ T = 3 ≤ max_cores = 16), so the double claim is the file's
  // one error.
  pure val twin: Intent =
    accepted
      .with("tenants", accepted.tenants.append(
        { name: "router2", dataplane: UserspacePoll, maxCores: 16 }))
      .with("ports", accepted.ports.append(
        { name: "clash0", dpmac: 9, rate: 10000, tenant: "router2" }))

  // The expected plan shape (design D3/D4/D6): three dpnis (one per port) at
  // T = 10 transmit queues with the ADR-0012 poll-mode draws — dpio = 2·T =
  // 20, dpbp = 2, dpmcp = 1, dpcon = dpnis·T = 30 — one dpseci with
  // num_queues = flows = 2 and the DPSECI_OPT_HAS_CG bit, no dpsw, the
  // three port edges as the whole edge set, and T visibly unmeasured (design
  // D3).
  run routerPlanShapeTest =
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) =>
          val p = res.plan
          all {
            countFam(p, "router", Dpni) == 3,
            objsOfFam(p, "router", Dpni).forall(o =>
              o.container == Child("router") and
              match o.attrs { | DpniAttrs(a) => a.numQueues == 10 | _ => false }),
            countFam(p, "router", Dpio) == 20,   // 2·T (ADR-0012)
            countFam(p, "router", Dpbp) == 2,
            countFam(p, "router", Dpmcp) == 1,
            countFam(p, "router", Dpcon) == 30,  // dpnis · T (DPCON-I1)
            // one dpseci: num_queues = flows = 2 (≥ flows) with
            // the DPSECI_OPT_HAS_CG safety bit (dpseci.md)
            objsOfFam(p, "router", Dpseci) == Set(
              { key: { tenant: "router", family: Dpseci, ordinal: 1 }, container: Child("router"),
                attrs: DpseciAttrs({ numQueues: 2, hasCg: true }),
                prov: { tenant: "router", rule: "dpseci", construct: "" } }),
            p.objs.filter(o => o.key.family == Dpsw) == Set(),
            // the three terminated ports are the whole edge set (fig. 6a)
            p.edges == Set(
              { a: ObjEnd({ key: { tenant: "router", family: Dpni, ordinal: 1 }, port: 0 }),
                b: MacEnd(9), prov: { tenant: "router", rule: "port-edge", construct: "wan0" } },
              { a: ObjEnd({ key: { tenant: "router", family: Dpni, ordinal: 2 }, port: 0 }),
                b: MacEnd(10), prov: { tenant: "router", rule: "port-edge", construct: "wan1" } },
              { a: ObjEnd({ key: { tenant: "router", family: Dpni, ordinal: 3 }, port: 0 }),
                b: MacEnd(4), prov: { tenant: "router", rule: "port-edge", construct: "up0" } }),
            p.prov.get({ tenant: "router", rule: "T", construct: "" }).value == 10,
            p.prov.get({ tenant: "router", rule: "T", construct: "" }).mark == Unmeasured,
          }
      | Refused(_) => false
    }

  // The 10G+25G mix is the scenario that exercises the cross-class warning
  // (design D3): the accepted compile carries UnmeasuredCombination naming the
  // tenant and both seeded classes.
  run routerUnmeasuredMixTest =
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) => res.warnings.contains(UnmeasuredCombination({ tenant: "router", rates: Set(10000, 25000) }))
      | Refused(_) => false
    }

  // The invariant run: INTENT_I7/I8 over the scenario and its twin, INTENT_I1..I6
  // over the accepted plan.
  run routerInvariantsTest = all {
    compileInvariants(accepted, REF_INVENTORY),
    compileInvariants(twin, REF_INVENTORY),
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) => planInvariants(res.plan)
      | Refused(_) => false
    },
  }

  // The twin: dpmac.9 is claimed by wan0 (router) and clash0 (router2) —
  // refused DoubleClaimed, named to the dpmac and both claiming ports, and
  // nothing else (design D5).
  run twinDoubleClaimedTest =
    match compile(twin, REF_INVENTORY) {
      | Refused(rs) => rs == Set(DoubleClaimed({ dpmac: 9, constructs: Set("wan0", "clash0") }))
      | Ok(_) => false
    }
}
