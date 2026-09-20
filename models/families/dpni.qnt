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

  // The raw-mask escapes, first-class provenance-carrying constructors, not a
  // backdoor (dpni-typestate design D2). restool's --options parser falls back to strtoull, so
  // dprc-script passes 0x80000000 (PFDR_IN_PEB) — deployed and working, never
  // named in any header (baseline "Option inventory"/"--options parsing", and
  // unknown-register #10), and it persists bit-exact on read-back (board-verified
  // V-DPNI-5, 2026-09-19). 0x4000 (HAS_REPLICATION) rides restool's strtoull map
  // the same way but the MC silently clears it on create (board-verified V-DPNI-9,
  // 2026-09-19); it is carried here as an escape, not DpniOpt vocabulary, so a
  // create can request it and the observation law (McClearedFlags) predicts its
  // erasure. Clearing is per-bit, not a raw-bits rule: 0x80000000 survives while
  // 0x4000 does not.
  type RawEscape =
    | PfdrInPeb           // 0x80000000, deployed [verified in use], persists on read-back (V-DPNI-5)
    | HasReplication      // 0x4000, restool-mapped, MC silently clears (V-DPNI-9, 2026-09-19)

  pure def rawValue(e: RawEscape): int =
    match e { | PfdrInPeb => 2147483648 | HasReplication => 16384 }   // 0x80000000, 0x4000

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

  // The numQueues envelope ceiling, named once so the create-surface envelope
  // (inEnvelope) and the intent-side QueueEnvelopeExceeded fences share one bound
  // (baseline "Option inventory" 1–32; bead guu.4a; Rust twin NumQueues::HI).
  pure val NUM_QUEUES_HI: int = 32

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
//                       observation (dpni-typestate design D4; DPNI-I12);
//   McClearedFlags      observation is invariant under the bits the MC silently
//                       clears — SHARED_CONGESTION and raw 0x4000 (V-DPNI-7/9);
//   SizingCoupledToFlags without CUSTOM_CG num_cgs observes 1, without HAS_OPR
//                       num_opr observes 0 (V-DPNI-8).
// Apalache-marked subset: the stateInvariants conjunction — all six
// (create-range and dead-parity are state-expressible over world; totality,
// write-only, cleared-flags and sizing-coupling are finite-quantified
// constants), per the DoD model gate.
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
    inRange(cfg.numQueues, NUM_QUEUES_HI),
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

  // The bits the MC silently clears on a bare create (board-verified V-DPNI-7 rev 2
  // + V-DPNI-9 rev 2, 2026-09-19): SHARED_CONGESTION (0x8, a named flag) and the
  // raw 0x4000 HAS_REPLICATION escape. The create is accepted rc=0 with no refusal,
  // yet the bits are absent from the dpni_attr.options read-back — so observation
  // must strip them, not echo them.
  pure val MC_CLEARED_FLAGS: OptionMask =
    { flags: Set(SharedCongestion), escapes: Set(HasReplication) }

  pure def clearMcBits(m: OptionMask): OptionMask = {
    flags: m.flags.exclude(MC_CLEARED_FLAGS.flags),
    escapes: m.escapes.exclude(MC_CLEARED_FLAGS.escapes),
  }

  // Observation predicts the board read-back, not the request (board-verified
  // V-DPNI-5/7/8/9, 2026-09-19): the cleared bits are stripped, num_cgs is honored
  // only under CUSTOM_CG (else the MC forces 1), and num_opr is observed only under
  // HAS_OPR (else 0; under HAS_OPR an unset request fills the MC default 8, V-DPNI-5).
  pure def observe(cfg: CreateCfg): Observation = {
    options: clearMcBits(cfg.options),
    numQueues: cfg.numQueues, numTcs: cfg.numTcs,
    macFilterEntries: cfg.macFilterEntries, vlanFilterEntries: cfg.vlanFilterEntries,
    qosEntries: cfg.qosEntries, fsEntries: cfg.fsEntries,
    numCgs: if (cfg.options.flags.contains(CustomCg)) cfg.numCgs else 1,
    numCeetmCh: cfg.numCeetmCh,
    numOpr: if (cfg.options.flags.contains(HasOpr))
              (if (cfg.numOpr == 0) 8 else cfg.numOpr) else 0,
  }

  // ---- the unrepresentable options (dpni-typestate design D2; baseline dead/never-settable) ----

  // The eleven dead options — create-time --mac-addr and the ten v9-era --max-*
  // flags (baseline "Dead options") — plus num_rx_tcs, wired into the MC command
  // but writable by no restool option (baseline "Never settable"). num_rx_tcs is a
  // read-only echo of num_tcs on the board: 8 on the 16-TC PMD create (V-DPNI-5)
  // and 1 on the 1q/1tc kernel create (V-DPNI-6), 2026-09-19 — never settable, so
  // it stays in this parity class. None has a field in CreateCfg: they are
  // unrepresentable by construct. The enumeration exists so a refusal names each
  // one and the dpni-typestate design-D11 parity rows stay two-sided (dpni-typestate
  // design D2, the vocabulary-v2 parity precedent).
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
  // (dpni-typestate design D4; baseline "Attribute mutability", DPNI-I12;
  // board-verified V-DPNI-5, 2026-09-19 — dpni_attr carries no dist_key_size slot).
  pure val DIST_KEY_SIZES: Set[int] = Set(0, 1, 24, 56)

  val WriteOnlyDistKeySize: bool =
    DIST_KEY_SIZES.forall(a => DIST_KEY_SIZES.forall(b =>
      observe(profileCfg(ProfilePmd).with("distKeySize", a))
        == observe(profileCfg(ProfilePmd).with("distKeySize", b))))

  // McClearedFlags — the MC silently clears SHARED_CONGESTION and the raw 0x4000
  // HAS_REPLICATION on create (board-verified V-DPNI-7 rev 2: requested 0xd read
  // back 0x5; V-DPNI-9 rev 2: requested 0x4000 read back 0x0, 2026-09-19). The
  // observation of a create block is invariant under adding or removing those
  // bits, so — exactly as with dist_key_size — the reconciler cannot detect drift
  // on them (mirror of WriteOnlyDistKeySize; dpni-typestate design D2/D4). Base is
  // the V-DPNI-7 residue TX_FRM_RELEASE + HAS_POLICING (0x5); the four masks differ
  // only in the two cleared bits.
  pure val CLEARED_VARIANT_MASKS: Set[OptionMask] = Set(
    { flags: Set(TxFrmRelease, HasPolicing), escapes: Set() },
    { flags: Set(TxFrmRelease, HasPolicing, SharedCongestion), escapes: Set() },
    { flags: Set(TxFrmRelease, HasPolicing), escapes: Set(HasReplication) },
    { flags: Set(TxFrmRelease, HasPolicing, SharedCongestion), escapes: Set(HasReplication) })

  val McClearedFlags: bool =
    CLEARED_VARIANT_MASKS.forall(a => CLEARED_VARIANT_MASKS.forall(b =>
      observe(profileCfg(ProfileKernel).with("options", a))
        == observe(profileCfg(ProfileKernel).with("options", b))))

  // SizingCoupledToFlags — two sizing fields are silently coupled to their option
  // flags (board-verified V-DPNI-8 rev 2, 2026-09-19): num_cgs is observed only
  // under CUSTOM_CG (else the MC forces 1 for every requested 1/8/24/64/128) and
  // num_opr only under HAS_OPR (else 0/label-absent for every requested 1/16/128).
  // With the gating flag absent, the observation is invariant under the requested
  // value — so, like the write-only fields, the reconciler cannot detect drift on
  // them. The Kernel profile carries neither flag, so it is the without-flag base.
  pure val NUM_CGS_DRAWS: Set[int] = Set(1, 8, 24, 64, 128)
  pure val NUM_OPR_DRAWS: Set[int] = Set(1, 16, 128)

  val SizingCoupledToFlags: bool = and {
    NUM_CGS_DRAWS.forall(a => NUM_CGS_DRAWS.forall(b =>
      observe(profileCfg(ProfileKernel).with("numCgs", a))
        == observe(profileCfg(ProfileKernel).with("numCgs", b)))),
    NUM_OPR_DRAWS.forall(a => NUM_OPR_DRAWS.forall(b =>
      observe(profileCfg(ProfileKernel).with("numOpr", a))
        == observe(profileCfg(ProfileKernel).with("numOpr", b)))),
  }

  // The Apalache/simulator conjunction (pnpm model:simulate / model:verify pass
  // --invariant=stateInvariants): all six named invariants, marked per the DoD
  // model gate (dpni-typestate design D8).
  val stateInvariants: bool = and {
    CreateRangeRefusal,
    ProfileTotality,
    DeadOptionParity,
    WriteOnlyDistKeySize,
    McClearedFlags,
    SizingCoupledToFlags,
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

  // The cleared-flags law: observation is invariant under the silently-cleared bits.
  run mcClearedFlagsTest = init.then(check(McClearedFlags))

  // The sizing-coupling law: without the gating flag, observation is invariant
  // under the requested num_cgs / num_opr.
  run sizingCoupledToFlagsTest = init.then(check(SizingCoupledToFlags))

  // The with-flag read-back the coupling gates on (board-verified V-DPNI-5,
  // 2026-09-19): the PMD block rides CUSTOM_CG and HAS_OPR, so num_cgs is honored
  // (24) and the unset num_opr fills the MC default 8.
  run sizingUnderFlagsTest =
    init.then(check(and {
      observe(profileCfg(ProfilePmd)).numCgs == 24,
      observe(profileCfg(ProfilePmd)).numOpr == 8,
    }))
}

// The frozen-trace scenario twin (dpni-typestate task 5.1, dpni-typestate design D8): the
// create/read-back/primary-MAC walk `dpaa2-verify`'s tests/dpni_replay.rs replays
// against dpaa2_api::families::dpni, board-free. dpni_lifecycle's World carries no
// primary MAC (the runtime slot the Rust family adds beside the immutable cfg,
// dpni-typestate design D1), so this scenario module wraps that world with the MAC field and a
// mac-mutation step, restricting the action set to the create → read-back →
// mac-mutation scenario the twins pin. It reuses dpni_lifecycle's create semantics
// verbatim (envelope guard, dead-option parity, profile derivation) and adds nothing
// to its invariants — the runs are directed, frozen via `pnpm model:freeze-dpni`,
// and each `.expect` is the freeze-time check.
//
// Every directed run's world is one var (`world`), so the frozen ITF is the
// single-var shape tests/dpni_replay.rs decodes (mirroring dprc_lifecycle).
module dpni_scenario {
  import intent_types.* from "../intent/types"
  import dpni.*
  // dpni_lifecycle's create surface, reused so the scenario never restates a
  // semantics the invariants pin (selective imports: the lifecycle `var world`
  // does not leak into this module's state).
  import dpni_lifecycle.DpniState
  import dpni_lifecycle.Absent
  import dpni_lifecycle.Created
  import dpni_lifecycle.Outcome
  import dpni_lifecycle.Accepted
  import dpni_lifecycle.Refused
  import dpni_lifecycle.RangeViolation
  import dpni_lifecycle.DeadOptionRefusal
  import dpni_lifecycle.UnpricedDataplane
  import dpni_lifecycle.Unrepresentable
  import dpni_lifecycle.MaxSenders
  import dpni_lifecycle.NumRxTcs
  import dpni_lifecycle.inEnvelope
  import dpni_lifecycle.MC_CLEARED_FLAGS

  // The primary MAC as six bytes — the Rust runtime slot MacAddr([u8;6])
  // (dpni-typestate design D1). All-zero is the unset MAC (MacAddr::ZERO), the
  // value create lands until a setter writes it.
  type MacBytes = List[int]
  pure val ZERO_MAC: MacBytes = [0, 0, 0, 0, 0, 0]

  // The scenario world: dpni_lifecycle's create state plus the runtime MAC.
  type World = {
    dpni: DpniState,
    primaryMac: MacBytes,
    lastOutcome: Outcome,
  }

  var world: World

  action init = world' = { dpni: Absent, primaryMac: ZERO_MAC, lastOutcome: Accepted }

  // The managed create face (dpni_lifecycle.createDerivedAt, carrying the MAC).
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

  // The bare create face (dpni_lifecycle.createRawAt, carrying the MAC): the shared
  // envelope guard, so refusal semantics never drift from dpni_lifecycle.
  action createRawAt(cfg: CreateCfg): bool = all {
    world.dpni == Absent,
    if (inEnvelope(cfg))
      world' = world.with("dpni", Created(cfg)).with("lastOutcome", Accepted)
    else
      world' = world.with("lastOutcome", Refused(RangeViolation)),
  }

  // A create carrying a dead option / num_rx_tcs: refused by name, nothing created.
  action createUnrepresentableAt(u: Unrepresentable): bool = all {
    world.dpni == Absent,
    world' = world.with("lastOutcome", Refused(DeadOptionRefusal(u))),
  }

  // The one runtime mutation in today's create contract (dpni-typestate design D1): the primary
  // MAC moves, the immutable create block is untouched.
  action setPrimaryMacTo(mac: MacBytes): bool = all {
    world.dpni != Absent,
    world' = world.with("primaryMac", mac).with("lastOutcome", Accepted),
  }

  // A non-zero primary MAC for the mutation walk (locally administered, last octet 7).
  pure val SAMPLE_MAC: MacBytes = [2, 0, 0, 0, 0, 7]

  // ---- directed twins (one per task-5.1 deliverable) ----

  // PMD-profile create + read-back: a userspace-poll physical port lands the PMD
  // block, 16q/16tc; the observation is the read-back.
  run scenarioPmdCreateReadbackTest =
    init
      .then(createDerivedAt(UserspacePoll, PhysicalPort))
      .expect(and {
        world.lastOutcome == Accepted,
        world.dpni == Created(profileCfg(ProfilePmd)),
        world.primaryMac == ZERO_MAC,
      })

  // kernel-profile create + read-back: a kernel-netlink injection pair lands the
  // kernel block, 1q/1tc.
  run scenarioKernelCreateReadbackTest =
    init
      .then(createDerivedAt(KernelNetlink, InjectionPair))
      .expect(and {
        world.lastOutcome == Accepted,
        world.dpni == Created(profileCfg(ProfileKernel)),
      })

  // envelope refusal: num_queues 33 (> 32) is refused RangeViolation, nothing created.
  run scenarioEnvelopeRefusedTest =
    init
      .then(createRawAt(profileCfg(ProfilePmd).with("numQueues", 33)))
      .expect(and {
        world.lastOutcome == Refused(RangeViolation),
        world.dpni == Absent,
      })

  // dead-option refusal: create-time --max-senders is refused, named, nothing created.
  run scenarioDeadOptionRefusedTest =
    init
      .then(createUnrepresentableAt(MaxSenders))
      .expect(and {
        world.lastOutcome == Refused(DeadOptionRefusal(MaxSenders)),
        world.dpni == Absent,
      })

  // num_rx_tcs refusal: the never-settable field shares the parity class.
  run scenarioNumRxTcsRefusedTest =
    init
      .then(createUnrepresentableAt(NumRxTcs))
      .expect(and {
        world.lastOutcome == Refused(DeadOptionRefusal(NumRxTcs)),
        world.dpni == Absent,
      })

  // write-only dist_key_size: a PMD block with dist_key_size 56 is created; the
  // read-back (observation) drops dist_key_size, so it equals the plain PMD
  // observation — the write-only law the replay checks (dpni-typestate design D4).
  run scenarioWriteOnlyDistKeySizeTest =
    init
      .then(createRawAt(profileCfg(ProfilePmd).with("distKeySize", 56)))
      .expect(and {
        world.lastOutcome == Accepted,
        match world.dpni {
          | Created(cfg) => cfg.distKeySize == 56
          | Absent => false
        },
      })

  // primary-MAC mutation: create the PMD dpni, then set the primary MAC; the
  // create block is untouched, only the runtime MAC moves (dpni-typestate design D1).
  run scenarioPrimaryMacMutationTest =
    init
      .then(createDerivedAt(UserspacePoll, PhysicalPort))
      .then(setPrimaryMacTo(SAMPLE_MAC))
      .expect(and {
        world.lastOutcome == Accepted,
        world.dpni == Created(profileCfg(ProfilePmd)),
        world.primaryMac == SAMPLE_MAC,
      })

  // A PMD block augmented with the MC-cleared bits (SHARED_CONGESTION flag + raw
  // 0x4000 HAS_REPLICATION escape, MC_CLEARED_FLAGS): the create keeps them
  // verbatim, the read-back strips both (dpni-hardening bead B; V-DPNI-7/9 rev 2).
  pure val PMD_WITH_MC_CLEARED: CreateCfg =
    profileCfg(ProfilePmd).with("options",
      { flags: PMD_PROFILE.flags.union(MC_CLEARED_FLAGS.flags),
        escapes: PMD_PROFILE.escapes.union(MC_CLEARED_FLAGS.escapes) })

  // McClearedFlags create/read-back: the augmented block is created carrying both
  // cleared bits, so the replay decodes HAS_REPLICATION and checks the observation
  // strips them (dpni-hardening bead B).
  run scenarioMcClearedCreateReadbackTest =
    init
      .then(createRawAt(PMD_WITH_MC_CLEARED))
      .expect(and {
        world.lastOutcome == Accepted,
        match world.dpni {
          | Created(cfg) => and {
              cfg.options.flags.contains(SharedCongestion),
              cfg.options.escapes.contains(HasReplication),
            }
          | Absent => false
        },
      })

  // Unpriced refusal: a UserspaceEvent dataplane derives no dpni profile, so the
  // create is refused UnpricedDataplane and nothing is created (ADR-0012; bead B).
  run scenarioUnpricedRefusedTest =
    init
      .then(createDerivedAt(UserspaceEvent, Loopback))
      .expect(and {
        world.lastOutcome == Refused(UnpricedDataplane),
        world.dpni == Absent,
      })
}
