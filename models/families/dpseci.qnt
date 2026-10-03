// dpseci family parameters (dpseci.md). Created-then-bound: dpaa2_caam
// or VFIO/PMD binds. Not in the reset-on-bind class; unbind-side resets
// are version-gated instead (≤ 5.3 leaves armed state behind,
// DPSECI-I5) — the core's unbind transform already models "no
// guarantee".
//
// The dpseci_lifecycle module below grows the stub into the ADR-0019 P2
// configured-object shape, on the models/families/dpni.qnt reference idiom
// (closed opt enum + raw escape + OptionMask + CreateCfg, then the
// restool refusal surface, create/destroy machine, and directed runs).
// dpseci is simpler than dpni: no profile/derivation table lives in the
// model (priorities and option derivation are a Rust-phase concern,
// dpseci-typestate design D3/D4), and the surface carries no mutating verb
// at all, so cfg immutability (DPSECI-I1) holds by construction. Unlike
// dpni — whose option surface lives in `module dpni` because
// intent/derive.qnt imports it — dpseci's create-surface types sit in
// `dpseci_lifecycle`: no external module consumes them, and the generic
// names (CreateCfg, OptionMask, RawEscape, MC_VOCABULARY, HasOpr) would
// collide with dpni's in `params.qnt`, which imports every family with `.*`.
// `module dpseci` therefore stays the thin params face the corpus imports.
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPSECI-I1 dpseci_lifecycle CfgImmutable (create-time values persist;
//     no mutating verb exists on the family surface);
//   DPSECI-I4 dpseci_lifecycle CongestionBirthCapability (the congestion
//     backstop exists iff HAS_CG was set at create, fixed for life);
//   DPSECI-I9 dpseci SEC_COUNTERS_BLOCK_GLOBAL (block-global counter law,
//     per-object attribution structurally impossible — a comment-anchored
//     law, not a state machine);
//   DPSECI-I5 main.qnt DPSECI_I5Test; DPSECI-I7 structural (LAW 5);
//   DPSECI-I8 main.qnt DPSECI_I8Test.
// Apalache-marked subset: none.
module dpseci {
  import types.* from "../core/types"

  // The queue-pair ceiling of one dpseci device: a single object carries at
  // most 16 Tx/Rx queue pairs, so a crypto block demanding more flows than
  // this cannot be realized by one device (intent/refuse.qnt refuses it; the
  // remedy is splitting the demand across blocks). Verified in the NXP trees:
  //   DPSECI_MAX_QUEUE_NUM == 16 —
  //   .build/src/linux/drivers/crypto/caam/dpseci.h:25 (kernel),
  //   .build/src/dpdk/drivers/crypto/dpaa2_sec/mc/fsl_dpseci.h:25 (DPDK MC).
  pure val DPSECI_MAX_QUEUE_NUM: int = 16

  pure val dpseciParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // dpaa2_caam (§3)
    hotBindAvailable: false,      // crypto names claimed for the boot
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // §3 census; dpio borrowed
  }

  // DPSECI-I9 (baseline "MC API notes"): dpseci_get_sec_counters returns seven
  // u64 counters that are global to the SEC block, not per-dpseci — two
  // dpsecis (the board's exact layout) read the same set. Per-object counter
  // attribution is structurally impossible, so the model offers no per-object
  // counter observable at all; the law is that absence, recorded as a
  // comment-anchored constant rather than a state machine.
  pure val SEC_COUNTERS_BLOCK_GLOBAL: bool = true
}

// The dpseci create-option surface as a sum-typed state machine — the
// isomorphic twin (ADR-0002 §3, quint-is-the-spec) of the Rust typestates
// the dpseci-typestate change grows, on the dpni_lifecycle reference. This
// module hosts the whole create surface — the option types, CreateCfg, the
// congestion birth capability — plus the stateful create/destroy machine,
// the restool-layer refusal surface, and the named invariants (the types
// sit here, not in `module dpseci`, so their generic names do not collide
// with dpni's in the corpus-wide params.qnt `.*` import).
//
// Everything below traces to docs/baseline/dpseci.md — the queue/priority
// ranges and option vocabulary from "Option inventory: used vs available",
// the restool-parser refusals from the same section and V-DPSECI-1 rev 1,
// and the immutability law from "Attribute mutability".
//
// Named invariants (dispositions in models/COVERAGE.md "dpseci create-surface
// invariants"):
//   CfgImmutable               no reachable object carries a cfg the restool
//                              layer would refuse — create-time values persist
//                              (DPSECI-I1, by construction: no mutating verb);
//   CongestionBirthCapability  the congestion backstop presence equals the
//                              create-time HAS_CG bit in every reachable state
//                              (DPSECI-I4).
// Apalache-marked subset: none (simulator-only, per the model gate).
module dpseci_lifecycle {
  import dpseci.*

  // ---- the create-option surface (dpseci-typestate design D1; baseline
  //      "Option inventory: used vs available") ----

  // The closed MC option vocabulary restool accepts for dpseci (baseline
  // "Option inventory": HAS_CG 0x20, HAS_OPR 0x40, OPR_SHARED 0x80). The
  // deployed dprc-script sets all three, mirroring the vendor default; the
  // kernel DPL sets HAS_CG only. No derivation decision lives in the model
  // (which bits an intent derives is a Rust-phase concern,
  // dpseci-typestate design D4) — the vocabulary just observes the three
  // verified bits.
  type DpseciOpt =
    | HasCg       // DPSECI_OPT_HAS_CG (0x20) — the congestion backstop (DPSECI-I4)
    | HasOpr      // DPSECI_OPT_HAS_OPR (0x40)
    | OprShared   // DPSECI_OPT_OPR_SHARED (0x80)

  pure val MC_VOCABULARY: Set[DpseciOpt] = Set(HasCg, HasOpr, OprShared)

  // The raw-mask escape, a provenance-carrying constructor, not a backdoor
  // (dpni OptionMask idiom, dpni.qnt RawEscape). restool's --options parser
  // falls back to strtoull, so an arbitrary hex bit reaches the MC unnamed
  // (baseline "Option inventory": "unknown tokens fall back to raw hex —
  // arbitrary bits reach MC"). dpseci banks no specific escape bit the way
  // dpni does (PFDR_IN_PEB), so the escape carries its raw value: an unknown
  // firmware bit is attributed in the escapes set, never silently merged
  // into the named flags.
  type RawEscape = RawBits(int)

  pure def rawValue(e: RawEscape): int = match e { | RawBits(b) => b }

  // The options mask: a typed set over the MC vocabulary plus the raw escapes
  // (dpni OptionMask idiom). The shim emits the raw mask it computed itself;
  // the two halves are the single source.
  type OptionMask = { flags: Set[DpseciOpt], escapes: Set[RawEscape] }

  // The dpseci create block (baseline "Option inventory" table). Every field
  // is create-time-immutable (baseline "Attribute mutability": options,
  // num_tx/rx_queues and the per-queue priorities[] have no setter at all),
  // so this block is the immutable type parameter of a DPSECI — no mutating
  // verb exists on the family surface (DPSECI-I1 by construction). restool
  // drives one --num-queues for both tx and rx, and --priorities is a vector
  // whose length must equal the queue count, each entry 1..8.
  type CreateCfg = {
    options: OptionMask,
    numQueues: int,          // 1..DPSECI_MAX_QUEUE_NUM (16)
    priorities: List[int],   // length == numQueues, each 1..8
  }

  // DPSECI-I4 as a birth capability (dpseci-typestate design D1; baseline
  // "Invariant candidates", "Kernel-side behavior": congestion is configured
  // only when the object has HAS_CG; without it the kernel consumer runs with
  // no backpressure at all). The backstop is a pure function of the create
  // options and nothing else — no transition can mint or drop it.
  pure def congestionBackstop(cfg: CreateCfg): bool =
    cfg.options.flags.contains(HasCg)

  // ---- the restool-layer envelope (baseline "Option inventory"; V-DPSECI-1
  //      rev 1 — each check refuses at restool's own parser, exit 234, before
  //      any MC command is built) ----

  pure def inQueueRange(n: int): bool =
    n >= 1 and n <= DPSECI_MAX_QUEUE_NUM

  // Every priority entry sits in restool's accepted 1..8 (baseline "Option
  // inventory": each 1–8; priority 0 and a priority above 8 are both refused).
  pure def allPrioritiesInRange(priorities: List[int]): bool =
    priorities.foldl(true, (ok, p) => ok and p >= 1 and p <= 8)

  // ---- outcomes (dpni_lifecycle Refusal/Outcome idiom) ----

  // The three restool-parser refusals V-DPSECI-1 rev 1 banked, plus the queue
  // envelope: priority 0 / a priority above 8 (PriorityOutOfRange), a
  // priority count ≠ num-queues (PriorityCountMismatch), and a queue count
  // outside 1..16 (QueueCountOutOfRange). No MC-layer validation is modeled —
  // it is unreachable through restool (baseline unknown #1; dpseci-typestate
  // non-goal, deferred to #10).
  type Refusal =
    | QueueCountOutOfRange     // num-queues outside 1..16
    | PriorityCountMismatch    // priorities count ≠ num-queues
    | PriorityOutOfRange       // a priority is 0 or above 8

  type Outcome = Accepted | Refused(Refusal)

  // The restool parser's verdict on a cfg, refused before any MC command. The
  // queue envelope is checked first, then the count match, then the per-entry
  // range — the order restool's own parse follows.
  pure def classify(cfg: CreateCfg): Outcome =
    if (not(inQueueRange(cfg.numQueues)))
      Refused(QueueCountOutOfRange)
    else if (cfg.priorities.length() != cfg.numQueues)
      Refused(PriorityCountMismatch)
    else if (not(allPrioritiesInRange(cfg.priorities)))
      Refused(PriorityOutOfRange)
    else
      Accepted

  // ---- state ----

  type DpseciState = Absent | Created(CreateCfg)

  type World = {
    dpseci: DpseciState,
    lastOutcome: Outcome,
  }

  var world: World

  action init = world' = { dpseci: Absent, lastOutcome: Accepted }

  // ---- transitions ----

  // The create face: accepted only when the restool parser accepts the cfg,
  // else refused by name with nothing created (baseline V-DPSECI-1 rev 1).
  action createAt(cfg: CreateCfg): bool = all {
    world.dpseci == Absent,
    match classify(cfg) {
      | Accepted =>
          world' = world
            .with("dpseci", Created(cfg))
            .with("lastOutcome", Accepted)
      | Refused(r) =>
          world' = world.with("lastOutcome", Refused(r))
    },
  }

  // destroy + re-create is the only resize path (baseline "Attribute
  // mutability": every cfg field is immutable, no setter exists), so drift is
  // destroy, not repair. Modeled so the machine can cycle under simulation.
  action destroy: bool = all {
    world.dpseci != Absent,
    world' = world.with("dpseci", Absent).with("lastOutcome", Accepted),
  }

  action stutter: bool = world' = world

  // ---- the two board-verified profiles plus refusal draws (baseline
  //      "Option inventory", reference-environment: the kernel 16-queue/
  //      priority-1/HAS_CG dpseci and the production VPP child
  //      8-queue/priority-2/HAS_CG+HAS_OPR+OPR_SHARED dpseci) ----

  pure def uniformPriorities(count: int, value: int): List[int] =
    range(0, count).foldl([], (acc, _) => acc.append(value))

  // The kernel-container dpseci: 16 queues, all priorities 1, HAS_CG only.
  pure val KERNEL_CFG: CreateCfg = {
    options: { flags: Set(HasCg), escapes: Set() },
    numQueues: 16,
    priorities: uniformPriorities(16, 1),
  }

  // The production VPP child dpseci: 8 queues, all priorities 2, all three
  // vendor-default option bits.
  pure val PRODUCTION_CFG: CreateCfg = {
    options: { flags: Set(HasCg, HasOpr, OprShared), escapes: Set() },
    numQueues: 8,
    priorities: uniformPriorities(8, 2),
  }

  // The production shape without HAS_CG: the DPSECI-I4 without-backstop pair.
  pure val NO_CG_CFG: CreateCfg =
    PRODUCTION_CFG.with("options", { flags: Set(HasOpr, OprShared), escapes: Set() })

  // A cfg carrying an unknown firmware bit as a raw escape (0x100), to witness
  // provenance: the escape rides the mask attributed, never merged into flags.
  pure val RAW_ESCAPE_CFG: CreateCfg =
    PRODUCTION_CFG.with("options", { flags: Set(HasCg), escapes: Set(RawBits(256)) })

  pure val CFG_DRAWS: Set[CreateCfg] = Set(
    KERNEL_CFG,
    PRODUCTION_CFG,
    NO_CG_CFG,
    RAW_ESCAPE_CFG,
    // queue count above the ceiling (17 > 16)
    PRODUCTION_CFG.with("numQueues", 17).with("priorities", uniformPriorities(17, 2)),
    // a priority of 0
    { options: { flags: Set(), escapes: Set() }, numQueues: 2, priorities: [1, 0] },
    // a priority above 8
    { options: { flags: Set(), escapes: Set() }, numQueues: 2, priorities: [1, 9] },
    // a priority count ≠ num-queues
    { options: { flags: Set(), escapes: Set() }, numQueues: 2, priorities: [1] },
  )

  action doCreate = { nondet cfg = CFG_DRAWS.oneOf() createAt(cfg) }

  action step = any {
    doCreate,
    destroy,
    stutter,
  }

  // ---- named invariants (dispositions in models/COVERAGE.md) ----

  // CfgImmutable (DPSECI-I1) — no reachable object carries a cfg the restool
  // layer would refuse: create is the only writer and it is guarded by
  // `classify`, and no mutating verb exists, so the create-time values persist
  // unchanged in every reachable state (baseline "Attribute mutability").
  val CfgImmutable: bool =
    match world.dpseci {
      | Created(cfg) => classify(cfg) == Accepted
      | Absent => true
    }

  // CongestionBirthCapability (DPSECI-I4) — the congestion backstop presence
  // is exactly the create-time HAS_CG bit in every reachable state: the named
  // backstop abstraction never diverges from the raw option, and no transition
  // moves it (baseline "Kernel-side behavior").
  val CongestionBirthCapability: bool =
    match world.dpseci {
      | Created(cfg) => congestionBackstop(cfg) iff cfg.options.flags.contains(HasCg)
      | Absent => true
    }

  // The simulator conjunction (pnpm model:simulate passes
  // --invariant=stateInvariants): both named state invariants.
  val stateInvariants: bool = and {
    CfgImmutable,
    CongestionBirthCapability,
  }

  // A stutter-check step for directed runs.
  action check(p: bool): bool = all { p, world' = world }

  // ---- directed runs (rung 2a): accept, refuse, and the I4 pair ----

  // An accepted create + read-back of the cfg: the production child block is
  // created, and its queue count, priorities and options read back verbatim.
  run createAcceptedReadbackTest =
    init
      .then(createAt(PRODUCTION_CFG))
      .expect(and {
        world.lastOutcome == Accepted,
        world.dpseci == Created(PRODUCTION_CFG),
        match world.dpseci {
          | Created(cfg) => and {
              cfg.numQueues == 8,
              cfg.priorities.length() == 8,
              cfg.options.flags == Set(HasCg, HasOpr, OprShared),
            }
          | Absent => false
        },
      })

  // The raw escape rides the mask attributed, not merged into the named flags.
  run createRawEscapeReadbackTest =
    init
      .then(createAt(RAW_ESCAPE_CFG))
      .expect(and {
        world.lastOutcome == Accepted,
        match world.dpseci {
          | Created(cfg) => and {
              cfg.options.escapes == Set(RawBits(256)),
              cfg.options.flags == Set(HasCg),
            }
          | Absent => false
        },
      })

  // Refusal: a priority of 0 is refused at the restool parser, nothing created.
  run priorityZeroRefusedTest =
    init
      .then(createAt({ options: { flags: Set(), escapes: Set() },
                       numQueues: 2, priorities: [1, 0] }))
      .expect(and {
        world.lastOutcome == Refused(PriorityOutOfRange),
        world.dpseci == Absent,
      })

  // Refusal: a priority above 8 is refused, nothing created.
  run priorityAboveEightRefusedTest =
    init
      .then(createAt({ options: { flags: Set(), escapes: Set() },
                       numQueues: 2, priorities: [1, 9] }))
      .expect(and {
        world.lastOutcome == Refused(PriorityOutOfRange),
        world.dpseci == Absent,
      })

  // Refusal: 2 queues with priorities [1] — a count mismatch, nothing created.
  run priorityCountMismatchRefusedTest =
    init
      .then(createAt({ options: { flags: Set(), escapes: Set() },
                       numQueues: 2, priorities: [1] }))
      .expect(and {
        world.lastOutcome == Refused(PriorityCountMismatch),
        world.dpseci == Absent,
      })

  // Refusal: a queue count above the ceiling (17 > 16) is refused, nothing created.
  run queueCountOutOfRangeRefusedTest =
    init
      .then(createAt(PRODUCTION_CFG.with("numQueues", 17)
                      .with("priorities", uniformPriorities(17, 2))))
      .expect(and {
        world.lastOutcome == Refused(QueueCountOutOfRange),
        world.dpseci == Absent,
      })

  // DPSECI-I4 pair, with HAS_CG: the created object carries the backstop.
  run congestionBirthWithCgTest =
    init
      .then(createAt(KERNEL_CFG))
      .expect(and {
        world.lastOutcome == Accepted,
        match world.dpseci {
          | Created(cfg) => congestionBackstop(cfg)
          | Absent => false
        },
      })

  // DPSECI-I4 pair, without HAS_CG: the created object carries no backstop.
  run congestionBirthWithoutCgTest =
    init
      .then(createAt(NO_CG_CFG))
      .expect(and {
        world.lastOutcome == Accepted,
        match world.dpseci {
          | Created(cfg) => not(congestionBackstop(cfg))
          | Absent => false
        },
      })

  // The DPSECI-I1 immutability law holds over the reachable surface.
  run cfgImmutableTest = init.then(check(CfgImmutable))

  // The DPSECI-I9 block-global counter law: per-object attribution is
  // structurally impossible, so the model offers no per-object counter.
  run secCountersBlockGlobalTest = init.then(check(SEC_COUNTERS_BLOCK_GLOBAL))
}
