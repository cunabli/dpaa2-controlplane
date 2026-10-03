// dpmac family parameters (dpmac.md). DPC-born in the root at boot and
// pinned there: the runtime lifecycle is connect/disconnect, not
// create/move (DPMAC-I1) — creatable=false plus RootOnly encode it.
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPMAC-I1 core/invariants.qnt DPMAC_I1; DPMAC-I5 main.qnt DPMAC_I5Test;
//   DPMAC-I8 main.qnt DPSECI_I8Test (class witness) + ph.4 adapter law.
//   DPMAC-I2/I3/I6/I7 dpmac_lifecycle below (DPMAC_I2/I3/I6/I7); DPMAC-I4
//   is structural there (the directional channel types, requests-down
//   unreadable). Apalache-marked subset: DPMAC-I1 (core); and the
//   dpmac_lifecycle stateInvariants conjunction (its own header).
module dpmac {
  import types.* from "../core/types"

  pure val dpmacParams: FamilyParams = {
    creatable: false,             // boot-born from the DPC (DPMAC-I1)
    placement: RootOnly,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // standalone driver binds the leftovers (§3)
    hotBindAvailable: true,       // boot-born; never hot-created
    endpointPorts: 1,             // faces dpni / dpsw uplink / dpdmux.N.0 (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // standalone dpmac: 1 dpmcp (§3 census)
  }
}

// The dpmac P4 "boot-born offer" reference shape (ADR-0019 §P4): the board
// births the dpmac from the DPC, so there is no create path — the control
// plane only observes, judges availability, and connects. The isomorphic
// twin (ADR-0002 §3, quint-is-the-spec) of the Rust family module a later
// task mirrors: authored model-first, so the sums/payloads below are what
// the Rust encoding must carry. Everything traces to docs/baseline/dpmac.md.
//
// The one typestate axis of a dpmac is driver arbitration (dpmac.md
// "Kernel-side behavior", DPMAC-I6), carried as phase markers in the P1
// idiom (ADR-0019 §P1). The phase is never commanded — there is no create
// and no "set state" action: it is *judged from observation*, the endpoint
// query (fsl_mc_get_endpoint) plus the driver-link / macN netdev read-back.
// Every transition takes that observation evidence as input.
//
// Named invariants (dispositions in models/COVERAGE.md):
//   DPMAC_I2  the MAC address is immutable through every surface — no setter
//             exists, so no action writes it (dpmac.md "Attribute
//             mutability"; ADR-0001 C2).
//   DPMAC_I3  the attribute surface reads back constant across the lifecycle
//             except the two runtime exceptions eth_if and IPG — the stable
//             projection is invariant under those two (dpmac.md "Attribute
//             mutability").
//   DPMAC_I6  standalone driver bound ⟺ no same-container host-managed peer;
//             the macN netdev tracks the standalone binding; the judged phase
//             agrees with the endpoint-query evidence (dpmac.md
//             "Kernel-side behavior").
//   DPMAC_I7  the counter vocabulary is firmware-version-indexed and never
//             zero-defaulted — a read of a counter outside the firmware's
//             vocabulary is NotInVocabulary, a constructor distinct from
//             Known(0): absence ≠ zero is unrepresentable (dpmac.md "MC API
//             notes", V-DPMAC-1).
//   DPMAC_SeverOrder  the dpni↔dpmac teardown ordering law: the kernel-face
//             is released only after the edge is severed, so the ADR-0008 §8
//             hazard order (unbind before sever, stranding the port) is
//             unreachable; the law itself is typed on the edge kind at the
//             connection surface (connect.edgeDemandsSeveredWitness, ADR-0019
//             edge facet) and consumed by unbindKernelFaceAt (dpmac.md
//             "Kernel-side behavior").
// DPMAC-I4 (the directional link channels) is structural, not a predicate:
//   the state-up and requests-down channels are distinct named types, and
//   requests-down carries no readable payload (dpmac.md "MC API notes").
// Apalache-marked subset: the stateInvariants conjunction — all five are
//   state-expressible over world or finite-quantified constants, per the
//   DoD model gate (mirrors dpni_lifecycle).
module dpmac_lifecycle {
  // Family constructors (Dpni/Dpmac/…) and the connection-surface teardown
  // law (edgeDemandsSeveredWitness) the sever/unbind actions consume — the
  // law lives on the edge kind, not in this family (ADR-0019 edge facet).
  import types.* from "../core/types"
  import connect.* from "../core/connect"

  // ---- the MAC address (DPMAC-I2) ----

  // The port MAC as six bytes (the Rust runtime slot MacAddr([u8;6])). It is
  // programmed pre-Linux (DPC and/or bootloader) and has no setter anywhere
  // in the API — the stable, globally-unique address the connected dpni
  // inherits (dpmac.md "Attribute mutability", ADR-0001 C2; V-DPMAC verified).
  type MacBytes = List[int]
  pure val BOOT_MAC: MacBytes = [2, 0, 0, 0, 0, 7]

  // ---- the DPC-born attribute surface (DPMAC-I3) ----

  // link_type: the DPC board_info port descriptor (dpmac.md "Option
  // inventory"); only PHY/FIXED/BACKPLANE appear in the corpus.
  type LinkType = PhyManaged | Fixed | Backplane

  // fec_mode: "none" is the only value in the entire DPC corpus (dpmac.md
  // "Option inventory"). Leading `|` so Quint reads a one-constructor sum,
  // not a type alias.
  type FecMode = | FecNone

  // eth_if: only USXGMII ever appears; every other mode comes from the RCW
  // SerDes protocol, outside the corpus (dpmac.md "Option inventory"). This
  // field is a runtime exception — dpmac_set_protocol (MC ≥ 10.32) moves it.
  type EthIf = Usxgmii | OtherSerdesProtocol

  // dpmac_attr as the model observes it: four DPC-born read-only fields plus
  // the two runtime-mutable exceptions, eth_if and ipgLength (dpmac.md
  // "Attribute mutability": dpmac_set_protocol, dpmac_set_params).
  type DpmacAttributes = {
    maxRate: int,       // DPC-born, read-only
    linkType: LinkType, // DPC-born, read-only
    fecMode: FecMode,   // DPC-born, read-only
    serdesCfg: int,     // DPC-born, read-only (provenance unresolved, dpmac.md)
    ethIf: EthIf,       // runtime exception: dpmac_set_protocol
    ipgLength: int,     // runtime exception: dpmac_set_params
  }

  pure val BOOT_ATTRS: DpmacAttributes = {
    maxRate: 25000, linkType: PhyManaged, fecMode: FecNone,
    serdesCfg: 0, ethIf: Usxgmii, ipgLength: 12,
  }

  // The stable surface is the DPC-born fields only — the two runtime
  // exceptions are projected out, so drift can never be claimed on them
  // (mirror of dpni_lifecycle's write-only projection; dpmac.md "Attribute
  // mutability").
  type StableAttributes = {
    maxRate: int, linkType: LinkType, fecMode: FecMode, serdesCfg: int,
  }

  pure def stableAttributes(a: DpmacAttributes): StableAttributes = {
    maxRate: a.maxRate, linkType: a.linkType, fecMode: a.fecMode,
    serdesCfg: a.serdesCfg,
  }

  // ---- the two directional link channels (DPMAC-I4) ----

  // state-up (MC→peer view): PHY-observed reality the MAC driver pushes into
  // MC via dpmac_set_link_state, surfaced to the peer as dpni_get_link_state
  // (dpmac.md "MC API notes"). Readable.
  type LinkStateUp = { up: bool, rate: int, pauseOptions: int }

  pure val BOOT_LINK_STATE: LinkStateUp = { up: false, rate: 0, pauseOptions: 0 }

  // requests-down (peer→MC view): peer requests flowing down via
  // dpmac_get_link_cfg (fed by dpni_set_link_cfg), pulled on a LINK_CFG_REQ
  // interrupt (dpmac.md "MC API notes"). On the restool transport this read
  // is outside the /dev/dprc.N whitelist and has no kernel-side observable
  // (dpmac.md DPMAC-I4, V-LINK-4 rev 2). The constructor carries no payload,
  // so no expression can read a requests-down value: unreadability is a type
  // property, not a comment. Leading `|` so Quint reads a one-constructor
  // sum (whose only constructor carries no payload), not a type alias.
  type RequestsDownChannel = | Unreadable

  // ---- the firmware-indexed counter vocabulary (DPMAC-I7) ----

  // A representative slice of the counter space across the two firmware
  // tiers. restool's enum carries 62 ids identical to MC 10.40; our 10.39
  // firmware defines only the first 28 (size buckets, pause, byte/frame
  // classes). The 34 extras — egress size buckets, FCS/VLAN/control, and
  // per-priority PFC both directions — exist only from 10.40 (dpmac.md
  // "MC API notes", V-DPMAC-1). Named here, but unread at 10.39.
  type Counter =
    | IngressByteCount       // 10.39 readable
    | IngressFrameCount      // 10.39 readable
    | IngressPauseFrames     // 10.39 readable
    | EgressByteCount        // 10.40 only (egress size buckets)
    | PfcPriority0Received   // 10.40 only (per-priority PFC)

  pure val ALL_COUNTERS: Set[Counter] =
    Set(IngressByteCount, IngressFrameCount, IngressPauseFrames,
        EgressByteCount, PfcPriority0Received)

  pure val COUNTERS_1039: Set[Counter] =
    Set(IngressByteCount, IngressFrameCount, IngressPauseFrames)

  pure val COUNTERS_1040_EXT: Set[Counter] =
    Set(EgressByteCount, PfcPriority0Received)

  type FirmwareVersion = Mc1039 | Mc1040

  // The readable vocabulary is indexed by firmware version, never defaulted
  // to the full set (dpmac.md "MC API notes").
  pure def counterVocabulary(fw: FirmwareVersion): Set[Counter] =
    match fw {
      | Mc1039 => COUNTERS_1039
      | Mc1040 => COUNTERS_1039.union(COUNTERS_1040_EXT)
    }

  // A counter read is Known(value) or NotInVocabulary. These are distinct
  // constructors: a present-but-zero counter is Known(0), an absent counter
  // is NotInVocabulary — so absence ≠ zero is unrepresentable (DPMAC-I7).
  // restool prints whatever succeeds and silently skips refusals, so the
  // vocabulary — not a zero-fill — is the only honest shape (dpmac.md
  // "Silent-failure notes").
  type CounterRead = Known(int) | NotInVocabulary

  pure def readCounter(fw: FirmwareVersion, c: Counter): CounterRead =
    if (counterVocabulary(fw).contains(c)) Known(0) else NotInVocabulary

  // ---- the driver-arbitration phase (DPMAC-I6) ----

  // The dpmac typestate: who owns the port, judged from observation
  // (dpmac.md "Kernel-side behavior", "Lifecycle ordering"). The P1
  // phase-marker idiom (ADR-0019 §P1), carried as data, never as a type
  // parameter outliving re-observation (ADR-0002 §3).
  type Arbitration =
    | Offered       // unconnected: the standalone fsl_dpaa2_mac driver binds the PHY
    | KernelOwned   // same-container kernel dpni bound: fsl_dpaa2_eth evicts standalone
    | RemoteOwned   // cross-container consumer: standalone keeps PHY, datapath remote

  // The endpoint-query evidence a phase is judged from (fsl_mc_get_endpoint,
  // dpmac.md "Kernel-side behavior").
  type PeerObservation =
    | NoPeer                      // no connection: standalone driver binds
    | SameContainerKernelPeer     // dpni/dpsw in the same container: eth-driver takeover
    | CrossContainerPeer          // fsl_mc_get_endpoint -EPERM: treated as unconnected

  // The driver-link read-back the endpoint evidence implies: the eth driver
  // force-unbinds the standalone driver on a same-container connect and hands
  // it back on disconnect, deterministically (dpmac.md "Kernel-side
  // behavior"). So the standalone binding is a function of the peer, not an
  // independent degree of freedom.
  pure def standaloneBoundFor(peer: PeerObservation): bool =
    match peer {
      | NoPeer => true
      | CrossContainerPeer => true
      | SameContainerKernelPeer => false
    }

  // The judgment: the phase follows the endpoint-query evidence (dpmac.md
  // "Lifecycle ordering" steps 1–3).
  pure def judgeArbitration(peer: PeerObservation): Arbitration =
    match peer {
      | NoPeer => Offered
      | CrossContainerPeer => RemoteOwned
      | SameContainerKernelPeer => KernelOwned
    }

  // ---- the dpni↔dpmac teardown ordering (ADR-0008 §8) ----

  // The facing dpni's kernel-face across a teardown (dpmac.md "Kernel-side
  // behavior", ADR-0008 §8). A same-container dpni that binds fsl_dpaa2_eth
  // takes the port's datapath (KernelFaceBound); the teardown releases it
  // only after the edge is severed (KernelFaceReleased). NoKernelDpni is the
  // boot state and the clean state after a full teardown cycle.
  type KernelFace =
    | NoKernelDpni
    | KernelFaceBound
    | KernelFaceReleased

  // The severed witness: proof the dpni↔dpmac edge was torn — the dpmac
  // handed back from KernelOwned to Offered, the standalone driver
  // re-attached at once — before the kernel-face unbinds. sever stamps it;
  // the kernel-face unbind demands it (ADR-0008 §8). The typed witness
  // task 2.2 mirrors in Rust (dpmac-typestate; ADR-0002).
  type SeveredWitness =
    | NotSevered
    | Severed

  // ---- state ----

  type World = {
    arbitration: Arbitration,
    peer: PeerObservation,            // the last endpoint-query evidence
    standaloneBound: bool,            // driver-link read-back
    macNetdev: bool,                  // macN netdev presence (CONFIG_FSL_DPAA2_MAC_NETDEVS)
    macAddr: MacBytes,                // immutable (DPMAC-I2)
    attributes: DpmacAttributes,      // observe-only surface (DPMAC-I3)
    firmware: FirmwareVersion,        // counter vocabulary index (DPMAC-I7)
    linkStateUp: LinkStateUp,         // readable directional channel (DPMAC-I4)
    requestsDown: RequestsDownChannel, // unreadable directional channel (DPMAC-I4)
    kernelFace: KernelFace,            // facing dpni's kernel-face (ADR-0008 §8)
    severed: SeveredWitness,           // edge-sever witness (ADR-0008 §8)
  }

  var world: World

  // Boot-born: the port is Offered, the standalone driver bound, the MAC and
  // attributes read from the DPC, the board firmware 10.39 (reference
  // environment). requests-down is the unreadable marker.
  action init = world' = {
    arbitration: Offered,
    peer: NoPeer,
    standaloneBound: true,
    macNetdev: true,
    macAddr: BOOT_MAC,
    attributes: BOOT_ATTRS,
    firmware: Mc1039,
    linkStateUp: BOOT_LINK_STATE,
    requestsDown: Unreadable,
    kernelFace: NoKernelDpni,   // boot: no kernel dpni holds the port
    severed: NotSevered,        // boot: no edge torn yet
  }

  // ---- transitions (observation-judged; no create, no set-state) ----

  // Re-observe the port: feed fresh endpoint-query evidence and recompute the
  // phase and its implied driver read-back. The MAC and attributes are never
  // touched — there is no setter for either (DPMAC-I2/I3). This is the only
  // arbitration-advancing action, and it is driven by observation, never by a
  // command (dpmac.md "Kernel-side behavior").
  action observeArbitrationAt(peer: PeerObservation): bool = all {
    world' = world
      .with("peer", peer)
      .with("standaloneBound", standaloneBoundFor(peer))
      .with("macNetdev", standaloneBoundFor(peer))
      .with("arbitration", judgeArbitration(peer)),
  }

  // Re-observe the state-up link channel: the PHY reality pushed into MC. It
  // is pure observation — it touches no other surface (dpmac.md "MC API
  // notes"; the DPMAC-I5 class: connection state is not link state).
  action observeLinkStateUpAt(ls: LinkStateUp): bool = all {
    world' = world.with("linkStateUp", ls),
  }

  // A same-container kernel dpni binds fsl_dpaa2_eth and takes the port: the
  // eth driver force-unbinds the standalone driver (KernelOwned) and owns the
  // MAC's kernel face. A fresh teardown opens — no edge severed yet (dpmac.md
  // "Kernel-side behavior", ADR-0008 §8). Enabled only on an Offered port.
  action kernelDpniTakesPortAt: bool = all {
    world.arbitration == Offered,
    world' = world
      .with("peer", SameContainerKernelPeer)
      .with("arbitration", KernelOwned)
      .with("standaloneBound", false)
      .with("macNetdev", false)
      .with("kernelFace", KernelFaceBound)
      .with("severed", NotSevered),
  }

  // sever: disconnect the dpni↔dpmac edge while the dpni is still bound. The
  // peer's endpoint-changed interrupt runs the standalone driver's re-attach
  // with the edge already gone, so it binds at once — KernelOwned → Offered —
  // and the severed witness is stamped (ADR-0008 §8, "sever, then unbind").
  // Enabled only while a kernel dpni holds the port; the kernel-face is NOT
  // released here — the dpni's own eth driver is unbound separately, after.
  // The planner emits Disconnect more widely than this guard admits (for
  // never-bound dpnis too); severAt is narrower by design, firing only on
  // the bound dpni↔dpmac edge of dpmac-typestate design D3's single-edge
  // scope — no hardware hazard, guard-for-guard holds on the unbind side.
  action severAt: bool = all {
    world.arbitration == KernelOwned,
    world.kernelFace == KernelFaceBound,
    world' = world
      .with("peer", NoPeer)
      .with("arbitration", Offered)
      .with("standaloneBound", true)
      .with("macNetdev", true)
      .with("severed", Severed),
  }

  // The dpni kernel-face unbind (fsl_dpaa2_eth remove). Facing a dpmac, the
  // per-edge-kind law (connect.edgeDemandsSeveredWitness) demands the severed
  // witness first: unbinding before the sever is the §8 hazard order that
  // strands the port driverless, so the action is disabled without the
  // witness — the hazard order is unreachable, not merely unlikely. For a
  // non-dpmac edge the demand is vacuous; this module only models the dpmac
  // edge, so the guard is consulted with (Dpni, Dpmac) (ADR-0008 §8).
  action unbindKernelFaceAt: bool = all {
    world.kernelFace == KernelFaceBound,
    edgeDemandsSeveredWitness(Dpni, Dpmac) implies world.severed == Severed,
    world' = world.with("kernelFace", KernelFaceReleased),
  }

  action stutter: bool = world' = world

  // ---- nondet wrappers (random simulation) ----

  pure val PEER_OBSERVATIONS: Set[PeerObservation] =
    Set(NoPeer, SameContainerKernelPeer, CrossContainerPeer)

  pure val LINK_STATE_DRAWS: Set[LinkStateUp] =
    Set({ up: false, rate: 0, pauseOptions: 0 },
        { up: true, rate: 25000, pauseOptions: 3 })

  action doObserveArbitration = {
    nondet p = PEER_OBSERVATIONS.oneOf()
    observeArbitrationAt(p)
  }
  action doObserveLinkStateUp = {
    nondet ls = LINK_STATE_DRAWS.oneOf()
    observeLinkStateUpAt(ls)
  }

  action step = any {
    doObserveArbitration,
    doObserveLinkStateUp,
    kernelDpniTakesPortAt,
    severAt,
    unbindKernelFaceAt,
    stutter,
  }

  // ---- named invariants (dispositions in models/COVERAGE.md) ----

  // DPMAC-I2 — the MAC address is immutable through every surface: no setter
  // exists, so no action writes it and it stays the boot-programmed value
  // across every observation sequence (dpmac.md "Attribute mutability";
  // ADR-0001 C2).
  val DPMAC_I2: bool = world.macAddr == BOOT_MAC

  // DPMAC-I3 — the attribute surface reads back constant across the lifecycle
  // except eth_if and IPG: the stable projection of the live attributes equals
  // the boot projection, so a future action mutating any DPC-born field (e.g.
  // maxRate) violates it, while the two runtime exceptions are quantified away
  // — drift can never be claimed on eth_if/IPG (dpmac.md "Attribute
  // mutability"). The RHS is anchored to BOOT_ATTRS, not to world.attributes:
  // comparing the projection with itself would be a tautology that never
  // constrains world.
  pure val ETH_IF_DRAWS: Set[EthIf] = Set(Usxgmii, OtherSerdesProtocol)
  pure val IPG_DRAWS: Set[int] = Set(0, 12, 96)

  val DPMAC_I3: bool =
    ETH_IF_DRAWS.forall(e => IPG_DRAWS.forall(i =>
      stableAttributes(world.attributes.with("ethIf", e).with("ipgLength", i))
        == stableAttributes(BOOT_ATTRS)))

  // DPMAC-I6 — driver arbitration: the standalone driver is bound iff no
  // same-container host-managed peer is connected; the macN netdev tracks
  // that binding; and the judged phase agrees with the endpoint-query
  // evidence (dpmac.md "Kernel-side behavior").
  val DPMAC_I6: bool = and {
    world.standaloneBound iff (world.arbitration != KernelOwned),
    world.macNetdev == world.standaloneBound,
    world.arbitration == judgeArbitration(world.peer),
    world.standaloneBound == standaloneBoundFor(world.peer),
  }

  // DPMAC-I7 — the counter vocabulary is firmware-indexed and never
  // zero-defaulted: every counter outside the firmware's vocabulary reads
  // NotInVocabulary, which is a distinct constructor from Known(0) — absence
  // ≠ zero (dpmac.md "MC API notes", V-DPMAC-1).
  val DPMAC_I7: bool =
    ALL_COUNTERS.forall(c =>
      not(counterVocabulary(world.firmware).contains(c))
        implies readCounter(world.firmware, c) == NotInVocabulary)

  // DPMAC-I6.sever — the dpni↔dpmac teardown ordering law (ADR-0008 §8): the
  // kernel-face is released only after the edge is severed. Whenever the
  // facing dpni's kernel-face reads KernelFaceReleased, the severed witness is
  // present — so the §8 hazard order (unbind before sever, which strands the
  // port driverless) holds in no reachable state. The bad combination
  // (KernelFaceReleased with NotSevered) is representable but unreachable: the
  // unbindKernelFaceAt guard, routed through connect.edgeDemandsSeveredWitness,
  // forbids it (dpmac.md "Kernel-side behavior").
  val DPMAC_SeverOrder: bool =
    (world.kernelFace == KernelFaceReleased) implies (world.severed == Severed)

  // The Apalache/simulator conjunction (pnpm model:simulate / model:verify
  // pass --invariant=stateInvariants): all five named invariants, marked per
  // the DoD model gate (mirrors dpni_lifecycle).
  val stateInvariants: bool = and {
    DPMAC_I2,
    DPMAC_I3,
    DPMAC_I6,
    DPMAC_I7,
    DPMAC_SeverOrder,
  }

  // A stutter-check step for directed runs.
  action check(p: bool): bool = all { p, world' = world }

  // ---- directed runs (rung 2a): the boot-born offer lifecycle ----

  // Boot-born: the port starts Offered, standalone driver bound, macN present.
  run offeredAtBootTest =
    init.then(check(and {
      world.arbitration == Offered,
      world.standaloneBound,
      world.macNetdev,
    }))

  // A same-container kernel dpni connects: the eth driver evicts the
  // standalone driver, the phase becomes KernelOwned, macN goes away.
  run kernelOwnedOnConnectTest =
    init
      .then(observeArbitrationAt(SameContainerKernelPeer))
      .expect(and {
        world.arbitration == KernelOwned,
        not(world.standaloneBound),
        not(world.macNetdev),
      })

  // A cross-container consumer (VFIO/VPP dpni): the standalone driver keeps
  // the PHY, the datapath is remote, the phase is RemoteOwned.
  run remoteOwnedOnCrossContainerTest =
    init
      .then(observeArbitrationAt(CrossContainerPeer))
      .expect(and {
        world.arbitration == RemoteOwned,
        world.standaloneBound,
      })

  // Disconnect hands the port back: a KernelOwned port re-observed with no
  // peer returns to Offered (dpmac.md "Lifecycle ordering" step 3).
  run arbitrationHandbackTest =
    init
      .then(observeArbitrationAt(SameContainerKernelPeer))
      .then(observeArbitrationAt(NoPeer))
      .expect(and {
        world.arbitration == Offered,
        world.standaloneBound,
      })

  // The arbitration law holds after each kind of observation.
  run arbitrationLawTest =
    init
      .then(observeArbitrationAt(SameContainerKernelPeer))
      .then(check(DPMAC_I6))
      .then(observeArbitrationAt(CrossContainerPeer))
      .then(check(DPMAC_I6))
      .then(observeArbitrationAt(NoPeer))
      .then(check(DPMAC_I6))

  // DPMAC-I2: the MAC address survives every observation sequence unchanged.
  run macAddressImmutableTest =
    init
      .then(observeArbitrationAt(SameContainerKernelPeer))
      .then(observeArbitrationAt(NoPeer))
      .then(check(and { DPMAC_I2, world.macAddr == BOOT_MAC }))

  // DPMAC-I3: the stable attribute projection stays anchored to the boot
  // value across an observation step (not merely at init), while ignoring the
  // two runtime exceptions.
  run attributeConstancyTest =
    init
      .then(check(DPMAC_I3))
      .then(observeArbitrationAt(SameContainerKernelPeer))
      .then(check(DPMAC_I3))

  // DPMAC-I7: at 10.39 a 10.40-only counter reads NotInVocabulary — distinct
  // from Known(0), which a present-but-zero 10.39 counter reads — so absence
  // is never confused with zero.
  run counterVocabularyFirmwareIndexedTest =
    init.then(check(and {
      DPMAC_I7,
      readCounter(Mc1039, EgressByteCount) == NotInVocabulary,
      readCounter(Mc1039, EgressByteCount) != Known(0),
      readCounter(Mc1039, IngressByteCount) == Known(0),
    }))

  // The 10.40 extension is readable only on 10.40 firmware — the vocabulary
  // grows with the version, it is not defaulted on.
  run firmwareExtensionReadableAt1040Test =
    init.then(check(and {
      readCounter(Mc1040, EgressByteCount) == Known(0),
      readCounter(Mc1040, PfcPriority0Received) == Known(0),
    }))

  // DPMAC-I4: the state-up channel is readable — a PHY link-up observation
  // surfaces its fields. (The requests-down channel has no reader anywhere in
  // this module: RequestsDownChannel carries no payload to read.)
  run linkStateUpReadableTest =
    init
      .then(observeLinkStateUpAt({ up: true, rate: 25000, pauseOptions: 3 }))
      .expect(and {
        world.linkStateUp.up,
        world.linkStateUp.rate == 25000,
      })

  // ---- directed runs (rung 2a): the dpni↔dpmac teardown law (ADR-0008 §8) ----

  // The legal teardown path: a kernel dpni takes the port, the edge is
  // severed (handback to Offered, witness stamped), and only then is the
  // kernel-face unbound — the §8 "sever, then unbind" order.
  run severThenUnbindTest =
    init
      .then(kernelDpniTakesPortAt)
      .then(check(and {
        world.arbitration == KernelOwned,
        world.kernelFace == KernelFaceBound,
      }))
      .then(severAt)
      .then(check(and {
        world.arbitration == Offered,
        world.standaloneBound,
        world.severed == Severed,
        world.kernelFace == KernelFaceBound,
      }))
      .then(unbindKernelFaceAt)
      .expect(world.kernelFace == KernelFaceReleased)

  // The hazard order is unreachable: unbinding the kernel-face before the
  // sever is disabled — no severed witness, so the per-edge-kind demand
  // (connect.edgeDemandsSeveredWitness) is unmet (ADR-0008 §8).
  run hazardUnbindBeforeSeverRefusedTest =
    init
      .then(kernelDpniTakesPortAt)
      .then(unbindKernelFaceAt.fail())

  // The sever-order law holds after each teardown step.
  run severOrderLawTest =
    init
      .then(kernelDpniTakesPortAt)
      .then(check(DPMAC_SeverOrder))
      .then(severAt)
      .then(check(DPMAC_SeverOrder))
      .then(unbindKernelFaceAt)
      .then(check(DPMAC_SeverOrder))

  // Non-dpmac edges carry no witness demand — the law is typed on the edge
  // kind and stays vacuous for every other edge (ADR-0019 edge facet): dpni↔
  // dpni (no driver handback), dpni↔dpdmux and dpdmux↔dpmac (un-disconnectable,
  // ADR-0009), dpsw↔dpmac, dpci↔dpci — each waits for its own change.
  run nonDpmacEdgesNoWitnessTest =
    init.then(check(and {
      edgeDemandsSeveredWitness(Dpni, Dpmac),
      edgeDemandsSeveredWitness(Dpmac, Dpni),
      not(edgeDemandsSeveredWitness(Dpni, Dpni)),
      not(edgeDemandsSeveredWitness(Dpni, Dpdmux)),
      not(edgeDemandsSeveredWitness(Dpdmux, Dpmac)),
      not(edgeDemandsSeveredWitness(Dpsw, Dpmac)),
      not(edgeDemandsSeveredWitness(Dpci, Dpci)),
    }))
}
