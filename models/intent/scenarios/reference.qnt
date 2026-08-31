// The fit check of the intent gate (openspec change `intent-layer`, design
// D8/D12; task 2.5): the reference LX2160A board's deployed provisioning —
// the reserved kernel tenant plus ONE userspace-poll tenant (a child dprc)
// terminating dpmac.7 and dpmac.9 at 10G — compiled and diffed object-for-
// object against the recovered boot census in
// models/board/baselines/reference.json. Unlike scenarios (1)–(3) this
// scenario has NO twin intent: its counterpart is the census, not a refusal,
// and the diff — not a second compile — is what the runs assert.
//
// The census is the BOOT state (root dprc.1 only, no child): the child dprc
// is created post-boot, so the whole child side of the plan is absent from it
// and is dispositioned deferred-to-live — the live sitting (tasks 4.1/4.2)
// diffs the child against a live census (design D12). Re-running the snapshot
// recovery over the board reproduces reference.json byte-for-byte, so the
// hand-encoded per-family census below is stable evidence, not a guess.
//
// Disposition table (recorded for the 2.6 gate; each difference is a
// divergence or an override, none is silent drift):
//   root/kernel side (census vs the portless kernel's derived draw):
//     - dpni.0   census 1, derived 0 — ForeignDpl: DPL-owned management
//                RGMII (dpni.0<->dpmac.17), never claimed (ADR-0001 §4;
//                already `foreign` in intent/inventory.qnt).
//     - dpbp.0   census 1, derived 0 — BootResident: DPL static pool object.
//     - dpseci.0 census 1, derived 0 — BootResident: the kernel CAAM accel,
//                DPL static (no crypto in this intent).
//     - dpcon    census 16, derived 0 — BootResident: the DPL static dpcon
//                pool; a portless kernel (no dpni to poll) derives none.
//     - dpmcp    census 52, derived 16 — BootResident: the DPL static MC-
//                portal pool. The kernel consumes 16 (cpus + dpnis = 16 + 0,
//                ADR-0012); the remaining 36 are idle boot portals, each an
//                `mcp` drawn for the boot (DPMCP-I6).
//     - dpio     census 16, derived 16 — no difference (one per online CPU).
//     - dprtc.0  census 1, derived 1 — no difference (DPRTC-I2 kernel-owned,
//                pinned in Root).
//     dpmacs are the inventory offer (intent/inventory.qnt), not derived plan
//     objects, so they are outside the object diff by construction.
//   child side (poll tenant, absent at boot):
//     - dprc/dpni/dpio/dpbp/dpcon — DeferredToLive: created post-boot, diffed
//       at the 4.1/4.2 live sitting (design D12).
//     - dpmcp    derived 1, board 3 — DpmcpThreeVsOne, open question 1
//       (design.md ~lines 361-371), DECIDED AT THE 2.6 GATE. The derivation
//       gives one MC portal per process (companions.qnt; DPDK's fslmc bus
//       maps one dpmcp per process, all drivers sharing MC_PORTAL_INDEX 0);
//       the deployed board's child carried 3 = 1 needed + 1 possible
//       secondary + 1 idle boot portal (DPMCP-I6, models/board/README.md).
//       Two readings, neither taken here:
//         divergence — the model is right; the board's two extra dpmcps are
//           over-provisioned residue the diff reports as explained divergence.
//         override — an operator could legitimately provision 3 for a
//           secondary process; the intent vocabulary stays silent and the
//           request/limit override channel (design D5) would carry it.
//
// Anchors: ADR-0012 companion draws; ADR-0011 ceilings; ADR-0001 §4 DPL
// ownership; ADR-0003 §3 reserved dpmacs; design D4/D8/D12; DPMCP-I6
// (models/board/README.md); DPAA2 User Manual §2.2.2 fig. 6a the dpni<->dpmac
// port. reference.toml beside this file is what the operator types; the
// pairing is by convention until dpaa2-verify's test (task 3.4).
//
// Named invariants: none new — the runs assert planInvariants /
//   compileInvariants (INTENT_I1..I8) over the scenario.
// Apalache-marked subset: none (directed runs; the alphabet rung carries the
//   marked laws).
module reference {
  import types.* from "../../core/types"           // Family
  import intent_types.* from "../types"            // Intent, KERNEL, kernelTenant, UserspacePoll
  import intent_inventory.* from "../inventory"    // REF_INVENTORY
  import intent_derive.* from "../derive"          // Plan, PlanObj, Container, DpniAttrs, End, …
  import intent_refuse.* from "../refuse"          // compile
  import intent_invariants.* from "../invariants"  // planInvariants, compileInvariants

  pure def objsOfFam(p: Plan, name: str, fam: Family): Set[PlanObj] =
    p.objs.filter(o => o.key.tenant == name and o.key.family == fam)
  pure def countFam(p: Plan, name: str, fam: Family): int =
    objsOfFam(p, name, fam).size()

  // The board's deployed intent (design D1/D8): the reserved kernel tenant
  // (its budget never binds under the online-CPU reading of open question 2)
  // and one userspace-poll tenant terminating the two cabled 10G ports. No
  // fabric, no link, no crypto, no limit — the historical userspace-poll
  // deployment was exactly this. No dpio/dpbp/dpcon/dpmcp/queue/worker count
  // appears; every object and count is derived (ADR-0005).
  pure val accepted: Intent =
    { tenants: [kernelTenant(16),
                { name: "router", dataplane: UserspacePoll, maxCores: 16, cryptoFlows: 0 }],
      ports: [{ name: "wan0", dpmac: 7, rate: 10000, tenant: "router" },
              { name: "wan1", dpmac: 9, rate: 10000, tenant: "router" }],
      links: [],
      fabrics: [],
      crypto: Set(),
      limits: Set() }

  // ---- the recovered boot census (models/board/baselines/reference.json) ----

  // Per-family object counts of root dprc.1 at boot; a rerun of the snapshot
  // recovery reproduces reference.json (and so this map) byte-for-byte. dpmac
  // rows are the inventory offer, not plan objects, and are excluded. dpio 16
  // and dprtc 1 match the kernel draw and raise no finding.
  pure val bootCensus: Family -> int = Map(
    Dpni -> 1,     // dpni.0, DPL management RGMII (Foreign, ADR-0001 §4)
    Dpbp -> 1,     // dpbp.0
    Dpseci -> 1,   // dpseci.0, kernel CAAM
    Dpmcp -> 52,   // the DPL static MC-portal pool (DPMCP-I6)
    Dprtc -> 1,    // dprtc.0 (DPRTC-I2)
    Dpio -> 16,    // one per online CPU (ADR-0012)
    Dpcon -> 16,   // the DPL static dpcon pool
  )

  // The child families the poll tenant derives (no crypto ⇒ no dpseci, no
  // fabric ⇒ no dpsw), each absent from the boot census.
  pure val CHILD_FAMS: Set[Family] = Set(Dprc, Dpni, Dpio, Dpbp, Dpmcp, Dpcon)

  // ---- the 3-vs-1 dpmcp finding (open question 1; DPMCP-I6) ----

  pure val DERIVED_CHILD_DPMCP: int = 1  // companions.qnt: one MC portal per process
  pure val BOARD_CHILD_DPMCP: int = 3    // 1 needed + 1 possible secondary + 1 idle boot portal
  pure val EXTRA_DPMCP: int = BOARD_CHILD_DPMCP - DERIVED_CHILD_DPMCP  // the two extra dpmcps

  // ---- the object-for-object diff ----

  type Disposition =
    | ForeignDpl              // DPL-owned, never claimed (ADR-0001 §4)
    | BootResident(str)       // DPL static boot layout, reused not created
    | DeferredToLive(str)     // child object absent at boot, diffed at 4.1/4.2
    | DpmcpThreeVsOne         // open question 1: child dpmcp 1 vs board 3 (DPMCP-I6)

  type Finding = { where: str, family: Family, derived: int, census: int, disp: Disposition }

  pure val BOOT_NOTE: str =
    "DPL static boot layout (ADR-0001 §4): reused, not created; idle pool members remain"
  pure val LIVE_NOTE: str =
    "child created post-boot: absent at boot, diffed at the live sitting (tasks 4.1/4.2, design D12)"

  pure def classifyRoot(f: Family): Disposition =
    if (f == Dpni) ForeignDpl else BootResident(BOOT_NOTE)
  pure def classifyChild(f: Family): Disposition =
    if (f == Dpmcp) DpmcpThreeVsOne else DeferredToLive(LIVE_NOTE)

  // The root/kernel differences: every census family whose count differs from
  // the kernel's derived draw, each dispositioned.
  pure def rootFindings(p: Plan): Set[Finding] =
    bootCensus.keys()
      .filter(f => countFam(p, KERNEL, f) != bootCensus.get(f))
      .map(f => { where: "root", family: f, derived: countFam(p, KERNEL, f),
                  census: bootCensus.get(f), disp: classifyRoot(f) })

  // The child differences: every child family the plan derives (census 0),
  // each dispositioned; the dpmcp one carries the 3-vs-1 marker.
  pure def childFindings(p: Plan): Set[Finding] =
    CHILD_FAMS
      .filter(f => countFam(p, "router", f) != 0)
      .map(f => { where: "child", family: f, derived: countFam(p, "router", f),
                  census: 0, disp: classifyChild(f) })

  pure def findings(p: Plan): Set[Finding] =
    rootFindings(p).union(childFindings(p))

  // The named finding set — the whole diff, no wildcard. Equality with the
  // computed set is the "nothing else unexplained" check (spec.md ~line 262):
  // a difference the derivation grows or drops shows up as an unexpected or
  // missing entry and breaks the test.
  pure val expectedFindings: Set[Finding] = Set(
    // root/kernel side
    { where: "root", family: Dpmcp, derived: 16, census: 52, disp: BootResident(BOOT_NOTE) },
    { where: "root", family: Dpbp, derived: 0, census: 1, disp: BootResident(BOOT_NOTE) },
    { where: "root", family: Dpcon, derived: 0, census: 16, disp: BootResident(BOOT_NOTE) },
    { where: "root", family: Dpseci, derived: 0, census: 1, disp: BootResident(BOOT_NOTE) },
    { where: "root", family: Dpni, derived: 0, census: 1, disp: ForeignDpl },
    // child side (poll tenant, absent at boot)
    { where: "child", family: Dprc, derived: 1, census: 0, disp: DeferredToLive(LIVE_NOTE) },
    { where: "child", family: Dpni, derived: 2, census: 0, disp: DeferredToLive(LIVE_NOTE) },
    { where: "child", family: Dpio, derived: 10, census: 0, disp: DeferredToLive(LIVE_NOTE) },
    { where: "child", family: Dpbp, derived: 2, census: 0, disp: DeferredToLive(LIVE_NOTE) },
    { where: "child", family: Dpcon, derived: 10, census: 0, disp: DeferredToLive(LIVE_NOTE) },
    { where: "child", family: Dpmcp, derived: 1, census: 0, disp: DpmcpThreeVsOne },
  )

  // ---- runs ----

  // (a) plan-shape: the compile is Ok and both tenants derive their expected
  // shape. The poll child (design D3/D4; ADR-0012 poll draws): 2 dpnis at
  // T = 1 + 2 + 2 = 5 queues, dpio = 2·T = 10, dpbp = 2, dpmcp = 1, dpcon =
  // dpnis·T = 10, T visibly unmeasured. The reserved kernel root (portless):
  // one dpio per online CPU, dpmcp = cpus + dpnis = 16, no dpni/dpbp/dpcon of
  // its own, dprtc.0 pinned.
  run referencePlanShapeTest =
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) =>
          val p = res.plan
          all {
            countFam(p, "router", Dpni) == 2,
            objsOfFam(p, "router", Dpni).forall(o =>
              o.container == Child("router") and
              match o.attrs { | DpniAttrs(a) => a.numQueues == 5 | _ => false }),
            countFam(p, "router", Dpio) == 10,   // 2·T (ADR-0012)
            countFam(p, "router", Dpbp) == 2,
            countFam(p, "router", Dpmcp) == 1,   // one MC portal per process
            countFam(p, "router", Dpcon) == 10,  // dpnis·T (DPCON-I1)
            p.prov.get({ tenant: "router", rule: "T", construct: "" }).value == 5,
            p.prov.get({ tenant: "router", rule: "T", construct: "" }).mark == Unmeasured,
            countFam(p, KERNEL, Dpio) == 16,     // one per online CPU
            countFam(p, KERNEL, Dpmcp) == 16,    // cpus + dpnis (ADR-0012)
            countFam(p, KERNEL, Dpni) == 0,
            countFam(p, KERNEL, Dpbp) == 0,
            countFam(p, KERNEL, Dpcon) == 0,
            p.objs.contains({ key: { tenant: KERNEL, family: Dprtc, ordinal: 1 }, container: Root,
                              attrs: Unsized, prov: { tenant: KERNEL, rule: "dprtc", construct: "" } }),
            // the two terminated ports are the whole edge set (fig. 6a)
            p.edges == Set(
              { a: ObjEnd({ key: { tenant: "router", family: Dpni, ordinal: 1 }, port: 0 }),
                b: MacEnd(7), prov: { tenant: "router", rule: "port-edge", construct: "wan0" } },
              { a: ObjEnd({ key: { tenant: "router", family: Dpni, ordinal: 2 }, port: 0 }),
                b: MacEnd(9), prov: { tenant: "router", rule: "port-edge", construct: "wan1" } }),
          }
      | Refused(_) => false
    }

  // (b) fit-check: the computed diff is exactly the named finding set (every
  // difference dispositioned, nothing else unexplained), and the headline
  // finding names the child's dpmcp — derived 1 vs board 3, the two extra
  // dpmcps of open question 1 (spec.md ~line 262; DPMCP-I6).
  run referenceFitCheckTest =
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) =>
          val p = res.plan
          all {
            findings(p) == expectedFindings,
            countFam(p, "router", Dpmcp) == DERIVED_CHILD_DPMCP,
            DERIVED_CHILD_DPMCP == 1,
            BOARD_CHILD_DPMCP == 3,
            EXTRA_DPMCP == 2,
            findings(p).exists(fnd =>
              fnd.where == "child" and fnd.family == Dpmcp and fnd.disp == DpmcpThreeVsOne),
          }
      | Refused(_) => false
    }

  // (c) invariants: INTENT_I7/I8 over the intent, INTENT_I1..I6 over the plan.
  run referenceInvariantsTest = all {
    compileInvariants(accepted, REF_INVENTORY),
    match compile(accepted, REF_INVENTORY) {
      | Ok(res) => planInvariants(res.plan)
      | Refused(_) => false
    },
  }
}
