// dpsw family parameters (dpsw.md, Tier B). Kernel-only consumer:
// dpaa2-switch binds; DPDK discards dpsw (DPSW-I6). In the
// reset-on-bind class — binding additionally dismantles MC-default
// state (DPSW-I4, breaking).
//
// The dpsw_lifecycle module below grows the stub into the ADR-0019 P1
// phase-machine shape with the reset facet (dpsw-typestate design D1),
// on the models/families/dpni.qnt reference idiom: the twelve-field
// immutable create block, the kernel-bindability predicate as a
// construction law (DPSW-I1), the ls-addsw vendor shape as a refusal
// (DPSW-I2), the probe as a pool consumer with a silent defer face
// (DPSW-I9), the reset facet as a visible parameter (DPSW-I4), and the
// observable-signature census (dpseci-typestate design D9, DPSW-I7).
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPSW-I1 dpsw_lifecycle BindableByConstruction + createBindableAcceptedTest;
//   DPSW-I2 dpsw_lifecycle vendorRecipeRefusedTest;
//   DPSW-I3 main.qnt DPSW_I3Test; DPSW-I4 main.qnt DPSW_I4Test
//     + dpsw_lifecycle BoundImpliesResetFacet/resetFacetTotalTest (the facet);
//   DPSW-I5 core/invariants.qnt LAW4_twoIdSpaces (DPBP_I5Test witness);
//   DPSW-I7 dpsw_lifecycle judgeCensus (census-only convergence);
//   DPSW-I9 dpsw_lifecycle probeBindAt defer face + censusCompetitorDefersTest.
// Apalache-marked subset: DPSW-I5 (the dpsw_lifecycle encodings stay
// simulator-only until dpsw-typestate task 1.3 marks them).
module dpsw {
  import types.* from "../core/types"

  pure val dpswParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: true,            // DPSW-I4, breaking
    hasKernelDriver: true,        // dpaa2-switch (kernel-only, DPSW-I6)
    // Board-anchored 2026-08-23 (V-DPSW-1), with a condition: only a
    // create in the configuration the driver accepts binds. It refuses
    // a dpsw whose control interface is off or whose flooding or
    // broadcast domain is not per FDB, which is exactly what restool's
    // defaults produce (dpaa2-switch.h, supports_cpu_traffic). The
    // refusal is logged, not silent.
    hotBindAvailable: true,
    // Switch ports address as dpsw.N.M (§2); the port count is
    // create-config — fixed to 2 at model scope, refine per-scenario.
    endpointPorts: 2,
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 1, dpcon: 0 },  // §3 census: dpbp for ctrl-if; dpio borrowed
  }
}

// The dpsw create-configure surface as a sum-typed state machine — the
// isomorphic twin (ADR-0002 §3, quint-is-the-spec) of the Rust typestates
// the dpsw-typestate change grows, on the dpni_lifecycle reference and the
// dpseci-typestate census precedent. This module hosts the whole surface —
// the twelve-field create block, the option vocabulary, the bindability
// predicate, the reset facet parameter, the create/probe/destroy machine,
// and the observable-signature census — plus the named invariants (the
// types sit here, not in `module dpsw`, so their generic names do not
// collide with dpni's / dpseci's in the corpus-wide params.qnt `.*` import).
//
// Everything below traces to docs/baseline/dpsw.md — the twelve create
// fields and their restool ranges from "Option inventory: used vs
// available", the kernel-bindability preconditions from "Kernel-side
// behavior"/"Intent mapping" (DPSW-I1), the ls-addsw default shape from
// "ls-addsw produces a kernel-unbindable switch by default" (DPSW-I2), the
// probe draw and the silent defer-on-missing-dpbp from "Lifecycle ordering
// and dependencies" (DPSW-I3/I9), the dpsw_reset-at-bind from "Kernel-side
// behavior" (DPSW-I4), and the immutability law from "Attribute mutability".
//
// Named invariants (dispositions in models/COVERAGE.md "dpsw create-surface
// invariants"):
//   BindableByConstruction   every created switch carries a kernel-bindable
//                            cfg — the unbindable shape is unrepresentable,
//                            never repaired (DPSW-I1);
//   BoundImpliesResetFacet   a bound switch has its MC-default state reset per
//                            the reset facet scope (DPSW-I4).
// Apalache-marked subset: none (simulator-only; dpsw-typestate task 1.3
// marks the subset).
module dpsw_lifecycle {
  import dpsw.*

  // ---- the twelve-field create block (dpsw-typestate design D1; baseline
  //      "Option inventory: used vs available" + the dead flib field) ----

  // The seven MC option flags restool exposes (baseline "Option inventory"
  // --options row). Only CTRL_IF_DIS is load-bearing here: it gates the
  // control interface the kernel bindability predicate reads (DPSW-I1). The
  // other six are carried for parity with the create surface, never derived.
  type DpswOpt =
    | FloodingDis          // DPSW_OPT_FLOODING_DIS
    | MulticastDis         // DPSW_OPT_MULTICAST_DIS
    | CtrlIfDis            // DPSW_OPT_CTRL_IF_DIS — disables the control interface (DPSW-I1)
    | FloodingMeteringDis  // DPSW_OPT_FLOODING_METERING_DIS
    | MeteringEn           // DPSW_OPT_METERING_EN
    | BpPerIf              // DPSW_OPT_BP_PER_IF
    | LagDis               // DPSW_OPT_LAG_DIS

  pure val MC_VOCABULARY: Set[DpswOpt] =
    Set(FloodingDis, MulticastDis, CtrlIfDis, FloodingMeteringDis,
        MeteringEn, BpPerIf, LagDis)

  // The three enumerated create fields (baseline "Option inventory"): the
  // component type and the flooding/broadcast domains the kernel requires
  // PER_FDB for (DPSW-I1).
  type ComponentType = CVlan | SVlan
  type FloodingCfg = FloodingPerVlan | FloodingPerFdb
  type BroadcastCfg = BroadcastPerObject | BroadcastPerFdb

  // The dpsw create block (baseline "Option inventory" table + the dead flib
  // field max_meters_per_if). Every field is create-time-immutable (baseline
  // "Attribute mutability": all twelve dpsw_cfg fields, including the three
  // the bindability predicate reads, have no setter — a wrongly-created dpsw
  // is destroyed and recreated, never repaired), so this block is the
  // immutable type parameter of a DPSW (dpsw-typestate design D1/D2). num_ifs
  // is the immutable port count switch-port membership is bounded by
  // (DPSW-I10, dpsw-typestate task 1.2). mem_size and max_meters_per_if are the two
  // unobservable fields (baseline: mem_size untouched, max_meters_per_if
  // fetched and never printed) the census signature drops.
  type CreateCfg = {
    numIfs: int,                   // --num-ifs 1..65535 (immutable port count)
    options: Set[DpswOpt],         // --options (the 7-flag vocabulary)
    maxVlans: int,                 // --max-vlans 0(MC default)..65535
    maxFdbs: int,                  // --max-fdbs 0..255 (DPSW-I1: >= num_ifs)
    maxFdbEntries: int,            // --max-fdb-entries 0..65535
    fdbAgingTime: int,             // --fdb-aging-time 0..65535
    maxFdbMcGroups: int,           // --max-fdb-mc-groups 0..65535
    memSize: int,                  // --mem-size (dead/undocumented, unknown #3)
    componentType: ComponentType,  // --component-type
    floodingCfg: FloodingCfg,      // --flooding-cfg (DPSW-I1: PER_FDB)
    broadcastCfg: BroadcastCfg,    // --broadcast-cfg (DPSW-I1: PER_FDB)
    maxMetersPerIf: int,           // dead flib field, never settable/printed
  }

  // ---- DPSW-I1: kernel-bindability as a construction law (baseline
  //      "Kernel-side behavior"/"Intent mapping"; V-DPSW-4 rev 1 banked) ----

  // The control interface is enabled unless CTRL_IF_DIS is set.
  pure def ctrlIfEnabled(cfg: CreateCfg): bool = not(cfg.options.contains(CtrlIfDis))

  // DPSW-I1 — the kernel driver (fsl_dpaa2_switch) binds only a dpsw whose
  // control interface is enabled, whose flooding AND broadcast domains are
  // PER_FDB, and whose max_fdbs covers the per-port FDB count (max_fdbs >=
  // num_ifs). All four fields are immutable, so this is a create-time
  // typestate predicate, not a tunable — the Apalache encoding
  // dpsw-typestate task 1.3 marks.
  pure def kernelBindable(cfg: CreateCfg): bool = and {
    ctrlIfEnabled(cfg),
    cfg.floodingCfg == FloodingPerFdb,
    cfg.broadcastCfg == BroadcastPerFdb,
    cfg.maxFdbs >= cfg.numIfs,
  }

  // DPSW-I2 — the ls-addsw default shape: PER_VLAN flooding + PER_OBJECT
  // broadcast (baseline "ls-addsw produces a kernel-unbindable switch by
  // default"). The vendor recipe is NOT an oracle (same law as DPDMUX-I2):
  // its default contradicts the only in-corpus consumer, so it is a refused
  // shape for kernel-steered intent, never a created switch.
  pure def isVendorRecipeShape(cfg: CreateCfg): bool = and {
    cfg.floodingCfg == FloodingPerVlan,
    cfg.broadcastCfg == BroadcastPerObject,
  }

  // ---- the reset facet, parameterized CONSERVATIVE (DPSW-I4;
  //      ADR-0019 reset facet) ----

  // DPSW-I4 reset facet scope. CONSERVATIVE default: dpsw_reset at bind is a
  // TOTAL reset of MC-default state (baseline "Kernel-side behavior":
  // dpsw_reset dismantles VLAN 1 everywhere and FDB 0 before the driver
  // rebuilds its own). The dpsw flib has no set_resetable analogue (baseline
  // unknown-register #2: "is reset always total?"), so the scope stays a
  // visible model parameter — refine this const to ResetPartial when the
  // discovery sitting (dpsw-typestate design D9) measures what reset
  // preserves, rather than burying the assumption in the bind action.
  type ResetScope = ResetTotal | ResetPartial
  pure val RESET_FACET_SCOPE: ResetScope = ResetTotal

  // MC-default state after a bind-time reset: a total reset dismantles it
  // (false); a partial reset — the future refinement — leaves it as found.
  pure def mcDefaultsAfterReset(before: bool): bool =
    match RESET_FACET_SCOPE { | ResetTotal => false | ResetPartial => before }

  // ---- outcomes (dpni_lifecycle Refusal/Outcome idiom) ----

  type Refusal =
    | VendorRecipeShape   // DPSW-I2: ls-addsw PER_VLAN/PER_OBJECT default refused
    | UnbindableCfg       // DPSW-I1: ctrl-if off / domain not PER_FDB / max_fdbs < num_ifs

  type Outcome = Accepted | Refused(Refusal)

  // The create verdict: the ls-addsw vendor shape is named its own refusal
  // (DPSW-I2), any other DPSW-I1 violation is the general unbindable refusal,
  // and only a kernel-bindable cfg is accepted. Create is the single writer,
  // so Accepted ⟹ kernelBindable holds by construction (BindableByConstruction).
  pure def classify(cfg: CreateCfg): Outcome =
    if (isVendorRecipeShape(cfg)) Refused(VendorRecipeShape)
    else if (not(kernelBindable(cfg))) Refused(UnbindableCfg)
    else Accepted

  // ---- the pool census the probe consumes (DPSW-I9, grounding DPSW-I3;
  //      baseline "Lifecycle ordering and dependencies") ----

  // The container pool the kernel bind draws from — free dpmcp and dpbp
  // counts. The draw is exactly the stub's census row (dpswParams.draw:
  // 1 dpmcp + 1 dpbp; dpio borrowed from the shared service, never drawn).
  type Pool = { dpmcp: int, dpbp: int }
  pure val PROBE_DRAW: Pool = { dpmcp: dpswParams.draw.dpmcp, dpbp: dpswParams.draw.dpbp }

  // The free dpbp a competitor can draw / be restored back to — one each, the
  // baseline census. The ceiling keeps provideDpbp from growing the pool
  // under random simulation.
  pure val POOL_DPBP_CEIL: int = 1

  // ---- state (the P1 phase machine; dpsw-typestate design D1) ----

  // The switch phase: created (MC-default state intact, not yet bound),
  // deferred (probe ran but the pool lacked the dpbp — still unbound, no
  // reset, the silent -EPROBE_DEFER face), or bound (kernel bound, reset
  // facet applied). There is no failure phase for starvation (DPSW-I9).
  type SwitchPhase = PhaseCreated | PhaseDeferred | PhaseBound

  // A created switch: its immutable cfg, its phase, and whether MC-default
  // state (VLAN 1, FDB 0) still stands — dismantled by the bind-time reset.
  type SwitchData = { cfg: CreateCfg, phase: SwitchPhase, mcDefaults: bool }

  type DpswState = Absent | Switch(SwitchData)

  type World = {
    dpsw: DpswState,
    pool: Pool,
    lastOutcome: Outcome,
  }

  var world: World

  action init = world' = {
    dpsw: Absent,
    pool: { dpmcp: PROBE_DRAW.dpmcp, dpbp: PROBE_DRAW.dpbp },  // exactly the census, one each
    lastOutcome: Accepted,
  }

  pure def isBound(w: World): bool =
    match w.dpsw { | Switch(sw) => sw.phase == PhaseBound | Absent => false }

  // ---- transitions ----

  // The create face: accepted only for a kernel-bindable cfg, else refused by
  // name with nothing created (DPSW-I1/I2). No repair transition exists — the
  // cfg is immutable, so drift is destroy-and-recreate (baseline "Attribute
  // mutability").
  action createAt(cfg: CreateCfg): bool = all {
    world.dpsw == Absent,
    match classify(cfg) {
      | Accepted =>
          world' = world
            .with("dpsw", Switch({ cfg: cfg, phase: PhaseCreated, mcDefaults: true }))
            .with("lastOutcome", Accepted)
      | Refused(r) =>
          world' = world.with("lastOutcome", Refused(r))
    },
  }

  // The kernel probe as a pool consumer (DPSW-I9, grounding DPSW-I3): it draws
  // 1 dpmcp + 1 dpbp from the container pool. With the draw satisfiable it
  // binds — reset facet applied (DPSW-I4). With the dpbp missing it DEFERS
  // silently — the switch sits PhaseDeferred, unbound, no draw, no reset, and
  // no invariant reads it as bound; this is never a failure transition (the
  // V-TRAF-1 rev 1 census-competitor class, designed in). A later probe with
  // the pool restored settles it.
  action probeBindAt: bool =
    match world.dpsw {
      | Absent => world' = world
      | Switch(sw) =>
          if (sw.phase == PhaseBound)
            world' = world
          else if (world.pool.dpmcp >= PROBE_DRAW.dpmcp and world.pool.dpbp >= PROBE_DRAW.dpbp)
            world' = world
              .with("dpsw", Switch({ cfg: sw.cfg, phase: PhaseBound,
                                     mcDefaults: mcDefaultsAfterReset(sw.mcDefaults) }))
              .with("pool", { dpmcp: world.pool.dpmcp - PROBE_DRAW.dpmcp,
                              dpbp: world.pool.dpbp - PROBE_DRAW.dpbp })
          else
            world' = world
              .with("dpsw", Switch({ cfg: sw.cfg, phase: PhaseDeferred, mcDefaults: sw.mcDefaults }))
    }

  // A second consumer draws a free dpbp from the shared container pool — the
  // census competitor that starves the probe (DPSW-I9). Guarded on a dpbp
  // being free.
  action competitorDrawsDpbp: bool = all {
    world.pool.dpbp >= 1,
    world' = world.with("pool", world.pool.with("dpbp", world.pool.dpbp - 1)),
  }

  // The pool is restored (the competitor released, or the operator created a
  // dpbp): a deferred probe can now settle (DPSW-I9 defer-then-settle).
  action provideDpbp: bool = all {
    world.pool.dpbp < POOL_DPBP_CEIL,
    world' = world.with("pool", world.pool.with("dpbp", world.pool.dpbp + 1)),
  }

  // destroy + re-create is the only resize path (baseline "Attribute
  // mutability"): every cfg field is immutable, so drift is destroy, not
  // repair. A bound switch returns its draws to the pool on destroy so the
  // machine can cycle under random simulation.
  pure def poolAfterDestroy(w: World): Pool =
    match w.dpsw {
      | Switch(sw) =>
          if (sw.phase == PhaseBound)
            { dpmcp: w.pool.dpmcp + PROBE_DRAW.dpmcp, dpbp: w.pool.dpbp + PROBE_DRAW.dpbp }
          else w.pool
      | Absent => w.pool
    }

  action destroy: bool = all {
    world.dpsw != Absent,
    world' = world
      .with("dpsw", Absent)
      .with("pool", poolAfterDestroy(world))
      .with("lastOutcome", Accepted),
  }

  action stutter: bool = world' = world

  // ---- sample cfgs (baseline README_DPSW working recipe + the refused
  //      shapes) ----

  // The one kernel-bindable shape (baseline "Intent mapping"/README_DPSW):
  // 2 ports, ctrl-if enabled, PER_FDB flooding+broadcast, max_fdbs = num_ifs.
  pure val BINDABLE_CFG: CreateCfg = {
    numIfs: 2,
    options: Set(),                // ctrl-if enabled (CTRL_IF_DIS absent)
    maxVlans: 16, maxFdbs: 2, maxFdbEntries: 1024,
    fdbAgingTime: 300, maxFdbMcGroups: 32,
    memSize: 0,
    componentType: CVlan,
    floodingCfg: FloodingPerFdb, broadcastCfg: BroadcastPerFdb,
    maxMetersPerIf: 0,
  }

  // A 4-port sibling (distinct num_ifs, max_fdbs raised to stay bindable) —
  // the census's second signature.
  pure val BINDABLE4_CFG: CreateCfg =
    BINDABLE_CFG.with("numIfs", 4).with("maxFdbs", 4)

  // The ls-addsw default shape: PER_VLAN flooding + PER_OBJECT broadcast
  // (DPSW-I2).
  pure val VENDOR_RECIPE_CFG: CreateCfg =
    BINDABLE_CFG.with("floodingCfg", FloodingPerVlan).with("broadcastCfg", BroadcastPerObject)

  // ctrl-if disabled: DPSW-I1 violated on the control-interface clause.
  pure val NO_CTRLIF_CFG: CreateCfg =
    BINDABLE_CFG.with("options", Set(CtrlIfDis))

  // max_fdbs below num_ifs: DPSW-I1 violated on the per-port FDB clause.
  pure val FEW_FDBS_CFG: CreateCfg =
    BINDABLE_CFG.with("numIfs", 4).with("maxFdbs", 2)

  pure val CFG_DRAWS: Set[CreateCfg] =
    Set(BINDABLE_CFG, BINDABLE4_CFG, VENDOR_RECIPE_CFG, NO_CTRLIF_CFG, FEW_FDBS_CFG)

  // ---- nondet wrappers ----

  action doCreate = { nondet cfg = CFG_DRAWS.oneOf() createAt(cfg) }

  action step = any {
    doCreate,
    probeBindAt,
    competitorDrawsDpbp,
    provideDpbp,
    destroy,
    stutter,
  }

  // ---- named state invariants (dispositions in models/COVERAGE.md) ----

  // BindableByConstruction (DPSW-I1) — every created switch carries a
  // kernel-bindable cfg: create is the single writer and it is guarded by
  // `classify`, so the unbindable shape is unrepresentable in every reachable
  // state, never repaired (dpsw-typestate design D1/D2).
  val BindableByConstruction: bool =
    match world.dpsw {
      | Switch(sw) => kernelBindable(sw.cfg)
      | Absent => true
    }

  // BoundImpliesResetFacet (DPSW-I4) — a bound switch has its MC-default state
  // reset per the facet scope: under the CONSERVATIVE total scope no pre-bind
  // MC-default state survives the probe (baseline "Kernel-side behavior").
  val BoundImpliesResetFacet: bool =
    match world.dpsw {
      | Switch(sw) =>
          (sw.phase == PhaseBound and RESET_FACET_SCOPE == ResetTotal) implies not(sw.mcDefaults)
      | Absent => true
    }

  // The simulator conjunction (pnpm model:simulate passes
  // --invariant=stateInvariants): both named state invariants.
  val stateInvariants: bool = and {
    BindableByConstruction,
    BoundImpliesResetFacet,
  }

  // A stutter-check step for directed runs.
  action check(p: bool): bool = all { p, world' = world }

  // ---- directed runs (rung 2a): accept, refuse, probe/defer, reset facet ----

  // A kernel-bindable create is accepted and lands PhaseCreated with its
  // MC-default state intact; the cfg reads back bindable (DPSW-I1).
  run createBindableAcceptedTest =
    init
      .then(createAt(BINDABLE_CFG))
      .expect(and {
        world.lastOutcome == Accepted,
        match world.dpsw {
          | Switch(sw) => and {
              sw.phase == PhaseCreated,
              sw.mcDefaults,
              kernelBindable(sw.cfg),
            }
          | Absent => false
        },
      })

  // DPSW-I2: the ls-addsw default shape (PER_VLAN/PER_OBJECT) is refused for
  // kernel-steered intent, nothing created — the vendor recipe is not an oracle.
  run vendorRecipeRefusedTest =
    init
      .then(createAt(VENDOR_RECIPE_CFG))
      .expect(and {
        world.lastOutcome == Refused(VendorRecipeShape),
        world.dpsw == Absent,
      })

  // DPSW-I1: a ctrl-if-disabled cfg is unbindable, refused, nothing created.
  run ctrlIfDisabledRefusedTest =
    init
      .then(createAt(NO_CTRLIF_CFG))
      .expect(and {
        world.lastOutcome == Refused(UnbindableCfg),
        world.dpsw == Absent,
      })

  // DPSW-I1: max_fdbs below num_ifs is unbindable, refused, nothing created.
  run fewFdbsRefusedTest =
    init
      .then(createAt(FEW_FDBS_CFG))
      .expect(and {
        world.lastOutcome == Refused(UnbindableCfg),
        world.dpsw == Absent,
      })

  // DPSW-I9/I3: a probe with the census satisfiable draws 1 dpmcp + 1 dpbp and
  // binds — the pool drains to zero and the switch reads bound.
  run probeDrawsAndBindsTest =
    init
      .then(createAt(BINDABLE_CFG))
      .then(probeBindAt)
      .expect(and {
        isBound(world),
        world.pool == { dpmcp: 0, dpbp: 0 },
      })

  // DPSW-I4: the reset facet dismantles MC-default state at bind — a created
  // switch (mcDefaults true) reads mcDefaults false once bound (total scope).
  run resetFacetTotalTest =
    init
      .then(createAt(BINDABLE_CFG))
      .then(check(match world.dpsw { | Switch(sw) => sw.mcDefaults | Absent => false }))
      .then(probeBindAt)
      .expect(match world.dpsw {
        | Switch(sw) => sw.phase == PhaseBound and not(sw.mcDefaults)
        | Absent => false
      })

  // DPSW-I9 defer face: a census competitor drains the dpbp, so the probe sits
  // PhaseDeferred — unbound, no invariant reads it bound. Restoring the pool
  // lets the next probe settle (defer-not-fail; the V-TRAF-1 rev 1 class).
  run censusCompetitorDefersTest =
    init
      .then(createAt(BINDABLE_CFG))
      .then(competitorDrawsDpbp)                 // pool dpbp -> 0
      .then(probeBindAt)                         // defers, no draw
      .then(check(and {
        not(isBound(world)),
        match world.dpsw { | Switch(sw) => sw.phase == PhaseDeferred | Absent => false },
      }))
      .then(provideDpbp)                         // pool restored
      .then(probeBindAt)                         // settles
      .expect(isBound(world))

  // The BindableByConstruction law holds over the reachable surface.
  run bindableByConstructionTest = init.then(check(BindableByConstruction))

  // destroy is the sole resize path (baseline "Attribute mutability"): a bound
  // switch destroyed returns to Absent and hands its draws back to the pool.
  run destroySoleResizePathTest =
    init
      .then(createAt(BINDABLE_CFG))
      .then(probeBindAt)
      .then(destroy)
      .expect(and {
        world.dpsw == Absent,
        world.pool == { dpmcp: PROBE_DRAW.dpmcp, dpbp: PROBE_DRAW.dpbp },
      })

  // ---- the observable-signature census (dpsw-typestate design D6;
  //      dpseci-typestate design D9) ----

  // Convergence is a multiset census on the observable cfg signature (the
  // dpseci-typestate D9 class): the dpsw verb surface has exit-code truth gaps
  // (DPSW-I7), so no verb outcome is trusted without re-observation. The
  // signature is the observable subset — it drops mem_size and
  // max_meters_per_if, the two fields `dpsw info` never prints (baseline
  // "Command surface"/"Option inventory").
  type ObservableSig = {
    numIfs: int, options: Set[DpswOpt],
    maxVlans: int, maxFdbs: int, maxFdbEntries: int,
    fdbAgingTime: int, maxFdbMcGroups: int,
    componentType: ComponentType,
    floodingCfg: FloodingCfg, broadcastCfg: BroadcastCfg,
  }

  pure def observableSigOf(cfg: CreateCfg): ObservableSig = {
    numIfs: cfg.numIfs, options: cfg.options,
    maxVlans: cfg.maxVlans, maxFdbs: cfg.maxFdbs, maxFdbEntries: cfg.maxFdbEntries,
    fdbAgingTime: cfg.fdbAgingTime, maxFdbMcGroups: cfg.maxFdbMcGroups,
    componentType: cfg.componentType,
    floodingCfg: cfg.floodingCfg, broadcastCfg: cfg.broadcastCfg,
  }

  // One object's read-back face (the dpseci-typestate D9 idiom): either the
  // observed signature, or typed unobservable — absence of evidence, never drift.
  type ObservedFace =
    | SigObserved(ObservableSig)
    | SigUnobservable

  // Observable-subset drift: an unobservable face is no drift judged; an
  // observed signature equal to the desired projection is no drift; any other
  // is drift (the destroy+create disruption read off the two multisets).
  pure def classifyObservableMismatch(desired: CreateCfg, face: ObservedFace): bool =
    match face {
      | SigUnobservable => false
      | SigObserved(sig) => sig != observableSigOf(desired)
    }

  // Quint carries no multiset type, so a census is a count map built from a
  // signature list. sigCount reads a missing signature as zero.
  pure def sigCount(census: ObservableSig -> int, sig: ObservableSig): int =
    if (census.keys().contains(sig)) census.get(sig) else 0

  pure def sigCensus(sigs: List[ObservableSig]): ObservableSig -> int =
    sigs.foldl(Map(), (acc, sig) => acc.put(sig, sigCount(acc, sig) + 1))

  // The multiset delta: a planned surplus is a create count, an observed
  // surplus a destroy count — a signature mismatch is one create AND one
  // destroy (the destroy-and-recreate disruption in multiset terms).
  type CensusDelta = { creates: ObservableSig -> int, destroys: ObservableSig -> int }

  pure def censusDelta(planned: ObservableSig -> int, observed: ObservableSig -> int): CensusDelta =
    val sigs = planned.keys().union(observed.keys())
    {
      creates: sigs.filter(sig => sigCount(planned, sig) > sigCount(observed, sig))
                 .mapBy(sig => sigCount(planned, sig) - sigCount(observed, sig)),
      destroys: sigs.filter(sig => sigCount(observed, sig) > sigCount(planned, sig))
                 .mapBy(sig => sigCount(observed, sig) - sigCount(planned, sig)),
    }

  // Converged when the delta is empty both ways.
  pure def censusConverged(planned: ObservableSig -> int, observed: ObservableSig -> int): bool =
    val d = censusDelta(planned, observed)
    d.creates.keys().size() == 0 and d.destroys.keys().size() == 0

  type FamilyJudgment =
    | JudgesNothing
    | Judges(CensusDelta)

  pure def observedSigsOf(faces: List[ObservedFace]): List[ObservableSig] =
    faces.foldl([], (acc, face) =>
      match face {
        | SigObserved(sig) => acc.append(sig)
        | SigUnobservable => acc
      })

  // DPSW-I7 census judge — the whole-census-poisoning law (dpseci-typestate
  // D9; ADR-0018): any unobservable member makes the whole face judge nothing
  // (absence of evidence is never drift, a spurious destroy+create is loss);
  // only an all-observed census yields a delta.
  pure def judgeCensus(planned: List[ObservableSig], faces: List[ObservedFace]): FamilyJudgment =
    val anyUnobservable = faces.foldl(false, (acc, face) =>
      acc or match face { | SigUnobservable => true | SigObserved(_) => false })
    if (anyUnobservable) JudgesNothing
    else Judges(censusDelta(sigCensus(planned), sigCensus(observedSigsOf(faces))))

  // ---- census directed runs (rung 2a; dpseci-typestate design D9) ----

  pure val sigBindable: ObservableSig = observableSigOf(BINDABLE_CFG)
  pure val sigFour: ObservableSig = observableSigOf(BINDABLE4_CFG)

  // Absent → create: a planned signature with no observed peer yields one
  // create of it, no destroy, and a non-converged verdict.
  run censusAbsentCreatesTest =
    init.then(check(
      val planned = sigCensus([sigBindable])
      val observed = sigCensus([])
      val d = censusDelta(planned, observed)
      and {
        sigCount(d.creates, sigBindable) == 1,
        d.destroys.keys().size() == 0,
        not(censusConverged(planned, observed)),
      }))

  // A signature mismatch (2-port planned, 4-port observed) is one create of
  // the planned signature AND one destroy of the observed — destroy+create.
  run censusSignatureMismatchTest =
    init.then(check(
      val planned = sigCensus([sigBindable])
      val observed = sigCensus([sigFour])
      val d = censusDelta(planned, observed)
      and {
        sigCount(d.creates, sigBindable) == 1,
        sigCount(d.destroys, sigFour) == 1,
        not(censusConverged(planned, observed)),
      }))

  // Matching multisets (reordered) → an empty delta both ways, converged —
  // position is not a hardware identity (ADR-0015 decision 5).
  run censusMatchEmptyDeltaTest =
    init.then(check(
      val planned = sigCensus([sigFour, sigBindable])
      val observed = sigCensus([sigBindable, sigFour])
      and {
        censusConverged(planned, observed),
        planned == observed,
      }))

  // The unobservable face is no drift judged; the matching signature is no
  // drift; any other observed signature is drift.
  run classifyObservableMismatchTest =
    init.then(check(and {
      not(classifyObservableMismatch(BINDABLE_CFG, SigUnobservable)),
      not(classifyObservableMismatch(BINDABLE_CFG, SigObserved(sigBindable))),
      classifyObservableMismatch(BINDABLE_CFG, SigObserved(sigFour)),
    }))

  // DPSW-I7: a poisoned census (one observed, one unobservable) judges nothing
  // — it does NOT create the missing sigFour — while the unpoisoned control
  // over the same plan judges one create of sigFour, no destroy (ADR-0018).
  run censusUnobservableMemberJudgesNothingTest =
    init.then(check(and {
      judgeCensus([sigFour, sigBindable], [SigObserved(sigBindable), SigUnobservable]) == JudgesNothing,
      match judgeCensus([sigFour, sigBindable], [SigObserved(sigBindable)]) {
        | JudgesNothing => false
        | Judges(d) => and {
            sigCount(d.creates, sigFour) == 1,
            d.destroys.keys().size() == 0,
          }
      },
    }))
}
