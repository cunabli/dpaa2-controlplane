// dpni family parameters (dpni.md). The created-then-bound consumer the
// canonical lifecycle order was board-validated on (C1 [verified]).
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPNI-I1 structural (LAW 1, no cfg block in state); DPNI-I2 main.qnt DPNI_I2Test;
//   DPNI-I3 retro/reconciler.qnt association runs (itf-replay);
//   DPNI-I4 machine.qnt kernelBind census guard + main.qnt DPNI_I4Test;
//   DPNI-I8 main.qnt DPNI_I8Test;
//   DPNI-I9 core/invariants.qnt DPNI_I9 + main.qnt DPNI_I9Test;
//   DPNI-I11 main.qnt IOCTL_OK/DPNI_I11Test + core/ioctl_policy.qnt.
// Apalache-marked subset: DPNI-I9, DPNI-I11.
module dpni {
  import types.* from "../core/types"
  // The consumer type is the intent vocabulary's Dataplane, not a local copy:
  // the profile is consumer-typed (baseline "Intent mapping", [verified]), and
  // ADR-0002's isomorphism binds this sum to the Rust family's consumer input.
  import intent_types.* from "../intent/types"

  pure val dpniParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: true,            // bind resets the object (DPNI-I2, breaking)
    hasKernelDriver: true,        // fsl_dpaa2_eth binds on plug
    hotBindAvailable: true,       // board-anchored (V-LIFE-DPNI-1)
    endpointPorts: 1,             // dpni↔dpmac/dpni/dpsw/dpdmux edges (§2)
    createTriggersRescan: false,  // most creates trigger no rescan (DPCI-I3)
    // fsl_dpaa2_eth probe census (§3): 1 dpmcp, 1 dpbp,
    // min(affine-dpio CPUs, num_queues) dpcons — abstracted to 1 at
    // model scope (DPCON-I1: one per polled queue); refine via a model
    // const if a phase-2 invariant needs the coupling. dpio is borrowed
    // from the shared service, never drawn.
    draw: { dpmcp: 1, dpbp: 1, dpcon: 1 },
  }

  // The interface construct an intent gives a dpni (baseline "Intent mapping";
  // ADR-0005): a physical port (dpni↔dpmac), a host-injection pair (dpni↔dpni
  // across containers), a loopback (dpni↔self), or a fabric port (dpsw/dpdmux,
  // the future endpoint the baseline names).
  type InterfaceConstruct =
    | PhysicalPort
    | InjectionPair
    | Loopback
    | FabricPort

  pure val CONSTRUCTS: Set[InterfaceConstruct] =
    Set(PhysicalPort, InjectionPair, Loopback, FabricPort)

  // The MC 10.39 option-flag vocabulary (baseline "Option inventory"). The flib
  // header carries 14 flags ending STASHING_DIS 0x2000; docs/baseline/dpni.md
  // names ten of them and no per-flag value but STASHING_DIS's — the four
  // unnamed flags are a recorded gap, not invented here. HAS_REPLICATION
  // (0x4000) is deliberately absent: it lives in restool's map but not the MC
  // 14-flag header (baseline, unknown-register #8), so it is not vocabulary.
  type DpniOpt =
    | SingleSender        // DPNI_OPT_SINGLE_SENDER
    | CustomCg            // DPNI_OPT_CUSTOM_CG
    | HasKeyMasking       // DPNI_OPT_HAS_KEY_MASKING
    | HasOpr              // DPNI_OPT_HAS_OPR
    | OprPerTc            // DPNI_OPT_OPR_PER_TC
    | TxFrmRelease        // DPNI_OPT_TX_FRM_RELEASE
    | HasPolicing         // DPNI_OPT_HAS_POLICING
    | SharedCongestion    // DPNI_OPT_SHARED_CONGESTION
    | NoMacFilter         // DPNI_OPT_NO_MAC_FILTER
    | StashingDis         // DPNI_OPT_STASHING_DIS (0x2000)

  pure val MC_VOCABULARY: Set[DpniOpt] =
    Set(SingleSender, CustomCg, HasKeyMasking, HasOpr, OprPerTc,
        TxFrmRelease, HasPolicing, SharedCongestion, NoMacFilter, StashingDis)

  // The raw-mask escape, a first-class provenance-carrying constructor, not a
  // backdoor (dpni-typestate design D2). restool's --options parser falls back to strtoull, so
  // dprc-script passes 0x80000000 (PFDR_IN_PEB) — deployed and working, never
  // named in any header (baseline "Option inventory"/"--options parsing", and
  // unknown-register #10).
  type RawEscape =
    | PfdrInPeb           // 0x80000000, deployed [verified in use], unnamed in headers

  pure def rawValue(e: RawEscape): int =
    match e { | PfdrInPeb => 2147483648 }   // 0x80000000

  // The options mask: a typed set over the MC vocabulary plus the raw escapes.
  // The shim emits the raw mask it computed itself, never operator tokens
  // (design Risks); the two halves are the single source.
  type OptionMask = { flags: Set[DpniOpt], escapes: Set[RawEscape] }

  // The twelve live create options as one create block (baseline "Option
  // inventory" table): the options mask, ten numeric fields with their restool
  // ranges, and the --container placement token. Every dpni_cfg field is
  // create-time-immutable (baseline "Attribute mutability", LAW 1), so this
  // block is the immutable type parameter of a DPNI (dpni-typestate design D1).
  type CreateCfg = {
    options: OptionMask,
    numQueues: int,          // 1–32
    numTcs: int,             // 1–16
    macFilterEntries: int,   // 1–80
    vlanFilterEntries: int,  // 1–16
    qosEntries: int,         // 1–64
    fsEntries: int,          // 1–1024
    numCgs: int,             // 1–128
    distKeySize: int,        // 1–56 (write-only, dpni-typestate design D4)
    numCeetmCh: int,         // 1–32
    numOpr: int,             // 1–128
    rootContainer: bool,     // --container: root dprc (true) or a child (false)
  }

  // ---- the two board-verified profiles (dpni-typestate design D3; baseline production list) ----

  type OptionProfile = ProfilePmd | ProfileKernel

  // PMD-facing dpni: SINGLE_SENDER, CUSTOM_CG, HAS_KEY_MASKING, HAS_OPR,
  // OPR_PER_TC and the raw 0x80000000, 16 queues / 16 TCs.
  pure val PMD_PROFILE: OptionMask =
    { flags: Set(SingleSender, CustomCg, HasKeyMasking, HasOpr, OprPerTc),
      escapes: Set(PfdrInPeb) }

  // kernel-facing dpni (kdpni injection pairs): HAS_KEY_MASKING only, 1 queue /
  // 1 TC — PFDR_IN_PEB and SINGLE_SENDER absent so the kernel side tx's freely.
  pure val KERNEL_PROFILE: OptionMask =
    { flags: Set(HasKeyMasking), escapes: Set() }

  pure val PROFILE_MASKS: Set[OptionMask] = Set(PMD_PROFILE, KERNEL_PROFILE)

  pure def profileMask(p: OptionProfile): OptionMask =
    match p { | ProfilePmd => PMD_PROFILE | ProfileKernel => KERNEL_PROFILE }

  // The full create block each profile derives. Fields the production scripts
  // leave unset are 0 (MC default): PMD sets 16q/16tc, vlan 16, qos 64, fs 1 and
  // num_cgs = num_queues + 8 = 24 (the deployed CUSTOM_CG heuristic, baseline
  // "Intent mapping", unknown-register #3), one CEETM channel; mac_filter,
  // dist_key_size and num_opr ride their MC defaults (0). kernel is 1q/1tc with
  // everything else at its default.
  pure def profileCfg(p: OptionProfile): CreateCfg =
    match p {
      | ProfilePmd => {
          options: PMD_PROFILE,
          numQueues: 16, numTcs: 16,
          macFilterEntries: 0, vlanFilterEntries: 16,
          qosEntries: 64, fsEntries: 1,
          numCgs: 24, distKeySize: 0,
          numCeetmCh: 1, numOpr: 0,
          rootContainer: false,
        }
      | ProfileKernel => {
          options: KERNEL_PROFILE,
          numQueues: 1, numTcs: 1,
          macFilterEntries: 0, vlanFilterEntries: 0,
          qosEntries: 0, fsEntries: 0,
          numCgs: 0, distKeySize: 0,
          numCeetmCh: 0, numOpr: 0,
          rootContainer: false,
        }
    }

  // Profile derivation is a pure function of (Dataplane, interface construct)
  // (dpni-typestate design D3): the operator never writes an option. The board keys the profile
  // on the consumer — a userspace-poll process gets PMD, the kernel gets kernel —
  // so the construct rides the signature for totality but does not split the two
  // current profiles (a third profile is a documented amendment, dpni-typestate design D3).
  // UserspaceEvent is unpriced (ADR-0012) and refused upstream (intent/refuse.qnt),
  // so it derives no dpni profile.
  type ProfileOutcome = Derived(OptionProfile) | Unpriced

  pure def deriveProfile(dp: Dataplane, ic: InterfaceConstruct): ProfileOutcome =
    match dp {
      | KernelNetlink => Derived(ProfileKernel)
      | UserspacePoll => Derived(ProfilePmd)
      | UserspaceEvent => Unpriced
    }

  // Local value-uses of the Dataplane constructors, so the verify-path flattener
  // keeps them (QNT404; the same reason families/params.qnt hosts ALL_PARAMS).
  pure val DATAPLANE_DRAWS: Set[Dataplane] =
    Set(KernelNetlink, UserspacePoll, UserspaceEvent)
}

// The dpni create-option surface as a sum-typed state machine — the
// isomorphic twin (ADR-0002 §3, quint-is-the-spec) of the Rust typestates
// this change grows (dpni-typestate design D8, model first). Authored model-first;
// sums/payloads are what the Rust encoding must mirror. Everything below
// traces to docs/baseline/dpni.md — the twelve live options and their
// ranges from "Option inventory: used vs available", the flag vocabulary
// and the raw escape from the same section, the two profiles from its
// production-profiles list, the dead options and num_rx_tcs from its
// "Dead options"/"Never settable" notes, and the observation asymmetry
// from "Attribute mutability".
//
// The pure option surface (types, profiles, deriveProfile) lives in `module
// dpni` above so the intent compiler can import it (intent/derive.qnt); this
// lifecycle module imports it and adds the stateful create/destroy machine.
//
// Named invariants (dispositions in models/COVERAGE.md "dpni create-surface
// invariants"):
//   CreateRangeRefusal  no accepted create sits outside the restool envelope;
//   ProfileTotality     every (Dataplane, interface construct) maps to exactly
//                       one of the two board-verified option sets (dpni-typestate design D3);
//   DeadOptionParity    the eleven dead options and num_rx_tcs never ride an
//                       accepted create (unrepresentable by construct, dpni-typestate design D2);
//   WriteOnlyDistKeySize dist_key_size participates in create, never in
//                       observation (dpni-typestate design D4; DPNI-I12).
// Apalache-marked subset: the stateInvariants conjunction — all four
// (create-range and dead-parity are state-expressible over world; totality
// and write-only are finite-quantified constants), per the DoD model gate.
module dpni_lifecycle {
  import intent_types.* from "../intent/types"
  import dpni.*

  // A field is in the restool envelope when it is either 0 (the option omitted,
  // so the MC applies its own default — baseline "Option inventory": an omitted
  // flag sends literal 0) or an explicit value inside the restool range.
  pure def inRange(v: int, hi: int): bool = v == 0 or (v >= 1 and v <= hi)

  // No accepted create sits outside the verified restool envelope (baseline
  // "Option inventory" ranges; the tightest verified bounds, dpni-typestate design D2).
  pure def inEnvelope(cfg: CreateCfg): bool = and {
    inRange(cfg.numQueues, 32),
    inRange(cfg.numTcs, 16),
    inRange(cfg.macFilterEntries, 80),
    inRange(cfg.vlanFilterEntries, 16),
    inRange(cfg.qosEntries, 64),
    inRange(cfg.fsEntries, 1024),
    inRange(cfg.numCgs, 128),
    inRange(cfg.distKeySize, 56),
    inRange(cfg.numCeetmCh, 32),
    inRange(cfg.numOpr, 128),
    cfg.options.flags.subseteq(MC_VOCABULARY),
  }

  // The observation surface (baseline "Attribute mutability", read-back
  // asymmetry): dpni_attr omits dist_key_size, so it can never be read back and
  // is write-only (dpni-typestate design D4). The model observation mirrors that omission — it
  // carries no distKeySize field, so the reconciler cannot claim drift on it.
  type Observation = {
    options: OptionMask,
    numQueues: int, numTcs: int,
    macFilterEntries: int, vlanFilterEntries: int,
    qosEntries: int, fsEntries: int,
    numCgs: int, numCeetmCh: int, numOpr: int,
  }

  pure def observe(cfg: CreateCfg): Observation = {
    options: cfg.options,
    numQueues: cfg.numQueues, numTcs: cfg.numTcs,
    macFilterEntries: cfg.macFilterEntries, vlanFilterEntries: cfg.vlanFilterEntries,
    qosEntries: cfg.qosEntries, fsEntries: cfg.fsEntries,
    numCgs: cfg.numCgs, numCeetmCh: cfg.numCeetmCh, numOpr: cfg.numOpr,
  }

  // ---- the unrepresentable options (dpni-typestate design D2; baseline dead/never-settable) ----

  // The eleven dead options — create-time --mac-addr and the ten v9-era --max-*
  // flags (baseline "Dead options") — plus num_rx_tcs, wired into the MC command
  // but writable by no restool option (baseline "Never settable"). None has a
  // field in CreateCfg: they are unrepresentable by construct. The enumeration
  // exists so a refusal names each one and the dpni-typestate design-D11 parity rows stay
  // two-sided (dpni-typestate design D2, the vocabulary-v2 parity precedent).
  type Unrepresentable =
    | MacAddrCreate
    | MaxSenders
    | MaxTcs
    | MaxDistPerTc
    | MaxFsEntriesPerTc
    | MaxUnicastFilters
    | MaxMulticastFilters
    | MaxVlanFilters
    | MaxQosEntries
    | MaxQosKeySize
    | MaxDistKeySize
    | NumRxTcs

  pure val DEAD_OPTIONS: Set[Unrepresentable] =
    Set(MacAddrCreate, MaxSenders, MaxTcs, MaxDistPerTc, MaxFsEntriesPerTc,
        MaxUnicastFilters, MaxMulticastFilters, MaxVlanFilters, MaxQosEntries,
        MaxQosKeySize, MaxDistKeySize)

  pure val UNREPRESENTABLE: Set[Unrepresentable] =
    DEAD_OPTIONS.union(Set(NumRxTcs))

  // ---- outcomes ----

  type Refusal =
    | RangeViolation                     // a cfg field outside the restool envelope
    | DeadOptionRefusal(Unrepresentable) // a dead option / num_rx_tcs requested
    | UnpricedDataplane                  // UserspaceEvent has no dpni profile (ADR-0012)

  type Outcome = Accepted | Refused(Refusal)

  pure def isDeadRefusal(o: Outcome): bool =
    match o {
      | Accepted => false
      | Refused(r) => match r { | DeadOptionRefusal(_) => true | _ => false }
    }

  // ---- state ----

  type DpniState = Absent | Created(CreateCfg)

  type World = {
    dpni: DpniState,
    lastOutcome: Outcome,
  }

  var world: World

  action init = world' = { dpni: Absent, lastOutcome: Accepted }

  // ---- transitions ----

  // The managed create face: the compiler derives the whole block from
  // (Dataplane, construct), carrying no operator-supplied option (dpni-typestate design D3).
  // A priced consumer lands its profile's create block (always in-envelope by
  // construction); UserspaceEvent is refused unpriced.
  action createDerivedAt(dp: Dataplane, ic: InterfaceConstruct): bool = all {
    world.dpni == Absent,
    match deriveProfile(dp, ic) {
      | Derived(p) =>
          world' = world
            .with("dpni", Created(profileCfg(p)))
            .with("lastOutcome", Accepted)
      | Unpriced =>
          world' = world.with("lastOutcome", Refused(UnpricedDataplane))
    },
  }

  // The bare create face (a caller handing a raw cfg): accepted only inside the
  // verified envelope, else refused RangeViolation with nothing created. The two
  // faces share this envelope guard so the refusal semantics never drift.
  action createRawAt(cfg: CreateCfg): bool = all {
    world.dpni == Absent,
    if (inEnvelope(cfg))
      world' = world
        .with("dpni", Created(cfg))
        .with("lastOutcome", Accepted)
    else
      world' = world.with("lastOutcome", Refused(RangeViolation)),
  }

  // A create carrying a dead option or num_rx_tcs: unrepresentable by construct,
  // so it can only be refused, and it names which one. Nothing is created.
  action createUnrepresentableAt(u: Unrepresentable): bool = all {
    world.dpni == Absent,
    world' = world.with("lastOutcome", Refused(DeadOptionRefusal(u))),
  }

  // destroy + re-create is the only resize path (baseline "Attribute
  // mutability", LAW 1): every cfg field is immutable, so drift is destroy, not
  // repair. Modeled so the machine can cycle under random simulation.
  action destroy: bool = all {
    world.dpni != Absent,
    world' = world.with("dpni", Absent).with("lastOutcome", Accepted),
  }

  action stutter: bool = world' = world

  // ---- nondet wrappers ----

  pure val CFG_DRAWS: Set[CreateCfg] = Set(
    profileCfg(ProfilePmd),
    profileCfg(ProfileKernel),
    profileCfg(ProfilePmd).with("numQueues", 33),  // out of envelope (>32)
  )

  action doCreateDerived = {
    nondet dp = DATAPLANE_DRAWS.oneOf()
    nondet ic = CONSTRUCTS.oneOf()
    createDerivedAt(dp, ic)
  }
  action doCreateRaw = { nondet cfg = CFG_DRAWS.oneOf() createRawAt(cfg) }
  action doCreateUnrepresentable = {
    nondet u = UNREPRESENTABLE.oneOf()
    createUnrepresentableAt(u)
  }

  action step = any {
    doCreateDerived,
    doCreateRaw,
    doCreateUnrepresentable,
    destroy,
    stutter,
  }

  // ---- named invariants (dispositions in models/COVERAGE.md) ----

  // CreateRangeRefusal — no accepted create sits outside the verified restool
  // envelope: an existing object's create block is always in-envelope (baseline
  // "Option inventory"; dpni-typestate design D2).
  val CreateRangeRefusal: bool =
    match world.dpni {
      | Created(cfg) => inEnvelope(cfg)
      | Absent => true
    }

  // ProfileTotality — every (Dataplane, interface construct) maps to exactly one
  // of the two board-verified option sets, and Unpriced arises only for the
  // unpriced UserspaceEvent (dpni-typestate design D3; baseline "Intent mapping").
  val ProfileTotality: bool =
    DATAPLANE_DRAWS.forall(dp => CONSTRUCTS.forall(ic =>
      match deriveProfile(dp, ic) {
        | Derived(p) => PROFILE_MASKS.contains(profileMask(p))
        | Unpriced => dp == UserspaceEvent
      }))

  // DeadOptionParity — the eleven dead options and num_rx_tcs never ride an
  // accepted create: they are not DpniOpt (unrepresentable by construct), so any
  // accepted create's flags sit within the MC vocabulary, and a dead-option
  // request creates nothing (dpni-typestate design D2; baseline "Dead options"/"Never settable").
  val DeadOptionParity: bool = and {
    match world.dpni {
      | Created(cfg) => cfg.options.flags.subseteq(MC_VOCABULARY)
      | Absent => true
    },
    isDeadRefusal(world.lastOutcome) implies world.dpni == Absent,
  }

  // WriteOnlyDistKeySize — dist_key_size participates in create but never in
  // observation: the observation of a create block is invariant under any
  // in-range dist_key_size, so the reconciler cannot detect drift on it
  // (dpni-typestate design D4; baseline "Attribute mutability", DPNI-I12).
  pure val DIST_KEY_SIZES: Set[int] = Set(0, 1, 24, 56)

  val WriteOnlyDistKeySize: bool =
    DIST_KEY_SIZES.forall(a => DIST_KEY_SIZES.forall(b =>
      observe(profileCfg(ProfilePmd).with("distKeySize", a))
        == observe(profileCfg(ProfilePmd).with("distKeySize", b))))

  // The Apalache/simulator conjunction (pnpm model:simulate / model:verify pass
  // --invariant=stateInvariants): all four named invariants, marked per the DoD
  // model gate (dpni-typestate design D8).
  val stateInvariants: bool = and {
    CreateRangeRefusal,
    ProfileTotality,
    DeadOptionParity,
    WriteOnlyDistKeySize,
  }

  // A stutter-check step for directed runs.
  action check(p: bool): bool = all { p, world' = world }

  // ---- directed runs (rung 2a): accept and refuse paths ----

  // A userspace-poll physical port derives the PMD profile, 16q/16tc, accepted.
  run createDerivedPmdTest =
    init
      .then(createDerivedAt(UserspacePoll, PhysicalPort))
      .expect(and {
        world.lastOutcome == Accepted,
        world.dpni == Created(profileCfg(ProfilePmd)),
        match world.dpni { | Created(cfg) => cfg.options == PMD_PROFILE
                                            and cfg.numQueues == 16 and cfg.numTcs == 16
                           | Absent => false },
      })

  // A kernel-netlink injection pair derives the kernel profile, 1q/1tc.
  run createDerivedKernelTest =
    init
      .then(createDerivedAt(KernelNetlink, InjectionPair))
      .expect(and {
        world.lastOutcome == Accepted,
        match world.dpni { | Created(cfg) => cfg.options == KERNEL_PROFILE
                                            and cfg.numQueues == 1 and cfg.numTcs == 1
                           | Absent => false },
      })

  // UserspaceEvent is unpriced: no dpni profile, nothing created (ADR-0012).
  run unpricedDataplaneRefusedTest =
    init
      .then(createDerivedAt(UserspaceEvent, Loopback))
      .expect(and {
        world.lastOutcome == Refused(UnpricedDataplane),
        world.dpni == Absent,
      })

  // A bare in-envelope cfg is accepted; the raw escape carries its provenance.
  run createInEnvelopeAcceptedTest =
    init
      .then(createRawAt(profileCfg(ProfilePmd)))
      .expect(and {
        world.lastOutcome == Accepted,
        match world.dpni { | Created(cfg) => cfg.options.escapes.contains(PfdrInPeb)
                                            and rawValue(PfdrInPeb) == 2147483648
                           | Absent => false },
      })

  // A cfg past the num_queues ceiling (33 > 32) is refused RangeViolation, and
  // nothing is created (baseline "Option inventory" range 1–32).
  run createOutOfEnvelopeRefusedTest =
    init
      .then(createRawAt(profileCfg(ProfilePmd).with("numQueues", 33)))
      .expect(and {
        world.lastOutcome == Refused(RangeViolation),
        world.dpni == Absent,
      })

  // A dead option (create-time --max-senders) is refused, named, nothing created.
  run deadOptionRefusedTest =
    init
      .then(createUnrepresentableAt(MaxSenders))
      .expect(and {
        world.lastOutcome == Refused(DeadOptionRefusal(MaxSenders)),
        world.dpni == Absent,
      })

  // num_rx_tcs shares the parity class: never settable, refused, nothing created.
  run numRxTcsRefusedTest =
    init
      .then(createUnrepresentableAt(NumRxTcs))
      .expect(and {
        world.lastOutcome == Refused(DeadOptionRefusal(NumRxTcs)),
        world.dpni == Absent,
      })

  // Profile totality holds over every (Dataplane, construct) combination.
  run profileTotalityTest = init.then(check(ProfileTotality))

  // The write-only law: observation is invariant under dist_key_size.
  run writeOnlyDistKeySizeTest = init.then(check(WriteOnlyDistKeySize))
}
