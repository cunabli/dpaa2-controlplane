// Scenario (4) of the intent gate (openspec change `intent-layer`, design
// D1/D6a; task 2.6d): the kernel-namespace pseudo-wires. One intent exercises
// all three link shapes a network namespace can take, and is the FIRST
// scenario to witness the child-resident kernel draw — a declared kernel-
// netlink namespace contributes ZERO dpio, because dpio services are one
// kernel-global per-CPU list every container shares (drivers/soc/fsl/dpio/
// dpio-service.c l.54), so its companion draw is companionDraw(Kernel, cpus=0)
// pricing: {dpio 0, dpbp one per dpni, dpmcp one per dpni}. Each isolated
// namespace still lands in its OWN kernel-bound child dprc (drivers/bus/fsl-mc/
// dprc-driver.c binds any obj_type "dprc" and rescans nested child dprcs), and
// its dpnis still run the online CPUs' worth of transmit queues and price a
// dpcon each (the online CPUs are still there to poll):
//   - ns1, ns2: kernel-netlink, Isolated, small max_cores — the veth pair
//     (ns ↔ ns) plus one uplink each; two dpnis each, so {dpio 0, dpbp 2,
//     dpmcp 2, dpcon 2·16 = 32}, numQueues = cpus = 16, in Child(nsX).
//   - the reserved kernel: undeclared, materialised by effectiveTenants
//     because a link names it (ns ↔ kernel); its root dpni carries the full
//     per-CPU kernel draw {dpio 16, dpbp 1, dpmcp 17, dpcon 16} in Root.
//   - vpp: userspace-poll, Isolated, portless ⇒ T = 1 — the ns ↔ userspace
//     wire; one dpni, the ADR-0012 poll-mode draws at T = 1 {dpio 2, dpbp 2,
//     dpmcp 1, dpcon 1}, in Child("vpp").
// Under the online-CPU reading of open question 2 the kernel-netlink budget
// never binds, so the ns tenants' small max_cores admits the 16-queue dpnis.
// vwire.toml beside this file is what the operator types; the pairing is by
// convention until dpaa2-verify's test (task 3.4).
//
// The twin (model-only, no operator file): vpp turned into a userspace-poll
// drawer that pools the reserved kernel (Restricted, pool "kernel"). The
// reserved kernel is public and counts as kernel-netlink, so a userspace-poll
// drawer pooling it is a dataplane mismatch — refused with EXACTLY
// { PoolDataplaneMismatch } naming vpp, its drawer dataplane and the holder's,
// and nothing else: pool set keeps RestrictedWithoutPool/PoolWithoutRestricted
// quiet, and the kernel branch never reaches HolderNotPublic/PoolChain.
//
// Anchors: drivers/bus/fsl-mc/dprc-driver.c the nested child-dprc drive;
// drivers/bus/fsl-mc/fsl-mc-allocator.c companions drawn from the consumer's
// parent container; drivers/soc/fsl/dpio/dpio-service.c the kernel-global
// per-CPU dpio list (a child-resident tenant draws zero dpio); object-model.md
// §2 / DPNI-I9 the dpni↔dpni pseudo-wire; ADR-0012 the companion draws.
//
// Named invariants: none new — the runs assert planInvariants /
//   compileInvariants (INTENT_I1..I9) over the scenario and its twin.
// Apalache-marked subset: none (directed runs; the alphabet rung carries
//   the marked laws).
module vwire {
  import types.* from "../../core/types"           // Family
  import intent_types.* from "../types"            // Intent, KernelNetlink, UserspacePoll, KERNEL, kernelTenant, …
  import intent_inventory.* from "../inventory"    // REF_INVENTORY
  import intent_derive.* from "../derive"          // Plan, PlanObj, End, DpniAttrs, Child, Root, …
  import intent_refuse.* from "../refuse"          // compile, PoolDataplaneMismatch
  import intent_invariants.* from "../invariants"  // planInvariants, compileInvariants

  pure def objsOfFam(p: Plan, name: str, fam: Family): Set[PlanObj] =
    p.objs.filter(o => o.key.tenant == name and o.key.family == fam)
  pure def countFam(p: Plan, name: str, fam: Family): int =
    objsOfFam(p, name, fam).size()

  // The intent vwire.toml states: two kernel-netlink namespaces and one
  // userspace-poll tenant, joined by three links — the veth pair (ns1↔ns2),
  // the uplink to the reserved kernel (ns1↔kernel, kernel undeclared), and the
  // app wire (ns2↔vpp). No dpio, dpbp, dpcon, dpmcp, queue or worker count
  // appears — every object and count is derived (ADR-0005).
  pure val accepted: Intent =
    { tenants: [{ name: "ns1", dataplane: KernelNetlink, maxCores: 2, isolation: Isolated, pool: "" },
                { name: "ns2", dataplane: KernelNetlink, maxCores: 2, isolation: Isolated, pool: "" },
                { name: "vpp", dataplane: UserspacePoll, maxCores: 8, isolation: Isolated, pool: "" }],
      ports: [],
      links: [{ name: "veth", interfaceA: "ns1", interfaceB: "ns2" },
              { name: "uplink", interfaceA: "ns1", interfaceB: KERNEL },
              { name: "app", interfaceA: "ns2", interfaceB: "vpp" }],
      fabrics: [],
      crypto: [],
      extras: Set() }

  // The twin (model-only): vpp becomes a Restricted userspace-poll drawer
  // pooling the reserved kernel — a dataplane mismatch, the file's one error.
  pure val twin: Intent =
    accepted.with("tenants", [
      { name: "ns1", dataplane: KernelNetlink, maxCores: 2, isolation: Isolated, pool: "" },
      { name: "ns2", dataplane: KernelNetlink, maxCores: 2, isolation: Isolated, pool: "" },
      { name: "vpp", dataplane: UserspacePoll, maxCores: 8, isolation: Restricted, pool: KERNEL }])

  // The expected plan shape (design D6a): each isolated namespace sole-tenant
  // in its own child dprc with the child-resident kernel draw (dpio 0, dpbp and
  // dpmcp one per dpni, dpcon dpnis·cpus, numQueues = cpus), the materialised
  // reserved kernel in Root with the full per-CPU draw, the portless poll-mode
  // vpp at T = 1 with the ADR-0012 poll draws, the three pseudo-wires the whole
  // edge set, and no dpsw and no dpseci anywhere.
  run vwirePlanShapeTest =
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) =>
          val p = res.plan
          all {
            // ns1: child-resident kernel draw — dpio 0 is the witness
            countFam(p, "ns1", Dpni) == 2,
            objsOfFam(p, "ns1", Dpni).forall(o =>
              o.container == Child("ns1") and
              match o.attrs { | DpniAttrs(a) => a.numQueues == 16 | _ => false }),
            countFam(p, "ns1", Dpio) == 0,    // child-resident kernel: zero dpio (dpio-service.c)
            countFam(p, "ns1", Dpbp) == 2,    // one per dpni
            countFam(p, "ns1", Dpmcp) == 2,   // one per dpni
            countFam(p, "ns1", Dpcon) == 32,  // dpnis · cpus = 2·16 (DPCON-I1)
            // ns2: the same child-resident kernel draw
            countFam(p, "ns2", Dpni) == 2,
            objsOfFam(p, "ns2", Dpni).forall(o =>
              o.container == Child("ns2") and
              match o.attrs { | DpniAttrs(a) => a.numQueues == 16 | _ => false }),
            countFam(p, "ns2", Dpio) == 0,
            countFam(p, "ns2", Dpbp) == 2,
            countFam(p, "ns2", Dpmcp) == 2,
            countFam(p, "ns2", Dpcon) == 32,
            // the reserved kernel: undeclared, materialised, full per-CPU draw
            countFam(p, KERNEL, Dpni) == 1,
            objsOfFam(p, KERNEL, Dpni).forall(o =>
              o.container == Root and
              match o.attrs { | DpniAttrs(a) => a.numQueues == 16 | _ => false }),
            countFam(p, KERNEL, Dpio) == 16,   // one per online CPU
            countFam(p, KERNEL, Dpbp) == 1,    // one per dpni
            countFam(p, KERNEL, Dpmcp) == 17,  // cpus + dpnis = 16 + 1
            countFam(p, KERNEL, Dpcon) == 16,  // dpnis · imin(cpus, numQ) = 1·16
            // vpp: portless poll-mode, T = 1, ADR-0012 draws
            countFam(p, "vpp", Dpni) == 1,
            objsOfFam(p, "vpp", Dpni).forall(o =>
              o.container == Child("vpp") and
              match o.attrs { | DpniAttrs(a) => a.numQueues == 1 | _ => false }),
            countFam(p, "vpp", Dpio) == 2,    // 2·T (ADR-0012)
            countFam(p, "vpp", Dpbp) == 2,
            countFam(p, "vpp", Dpmcp) == 1,
            countFam(p, "vpp", Dpcon) == 1,   // dpnis · T = 1·1
            p.objs.filter(o => o.key.family == Dpsw) == Set(),
            p.objs.filter(o => o.key.family == Dpseci) == Set(),
            // the three pseudo-wires are the whole edge set (no dpmac, no port)
            p.edges == Set(
              { a: ObjEnd({ key: { tenant: "ns1", family: Dpni, ordinal: 1 }, port: 0 }),
                b: ObjEnd({ key: { tenant: "ns2", family: Dpni, ordinal: 1 }, port: 0 }),
                prov: { tenant: "ns1", rule: "link-edge", construct: "veth" } },
              { a: ObjEnd({ key: { tenant: "ns1", family: Dpni, ordinal: 2 }, port: 0 }),
                b: ObjEnd({ key: { tenant: KERNEL, family: Dpni, ordinal: 1 }, port: 0 }),
                prov: { tenant: "ns1", rule: "link-edge", construct: "uplink" } },
              { a: ObjEnd({ key: { tenant: "ns2", family: Dpni, ordinal: 2 }, port: 0 }),
                b: ObjEnd({ key: { tenant: "vpp", family: Dpni, ordinal: 1 }, port: 0 }),
                prov: { tenant: "ns2", rule: "link-edge", construct: "app" } }),
          }
      | Refused(_) => false
    }

  // The invariant run: INTENT_I7/I8/I9 over the scenario and its twin,
  // INTENT_I1..I6 over the accepted plan.
  run vwireInvariantsTest = all {
    compileInvariants(accepted, REF_INVENTORY),
    compileInvariants(twin, REF_INVENTORY),
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) => planInvariants(res.plan)
      | Refused(_) => false
    },
  }

  // The twin: a userspace-poll drawer pooling the reserved kernel (which
  // counts as kernel-netlink) is refused PoolDataplaneMismatch, named to the
  // tenant, its drawer dataplane and the holder's — and nothing else.
  run twinPoolMismatchTest =
    match compile(twin, REF_INVENTORY) {
      | Refused(rs) => rs == Set(PoolDataplaneMismatch({ tenant: "vpp", drawer: UserspacePoll, holder: KernelNetlink }))
      | Ok(_) => false
    }
}
