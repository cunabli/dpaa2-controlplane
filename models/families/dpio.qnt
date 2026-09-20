// dpio family parameters (dpio.md). NOT pooled — there is no
// FSL_MC_POOL_DPIO (DPIO-I1); the kernel binds each dpio to the dpio
// driver as a per-CPU shared service, and DPDK claims them exclusively
// per thread. The dpio driver itself draws a dpmcp at probe — the
// dpio→dpmcp ordering edge inside the pool-companion phase (§4).
//
// dpio is unpooled, so it owns its own stateful machine `dpio_lifecycle` (below)
// rather than instantiating the pooled pool_lifecycle substrate — it reuses the
// shared core transforms (create_allocate/lifecycle) for the dpmcp it draws at
// probe, re-deriving nothing (pool-objects design D7 as amended, ADR-0019).
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPIO-I1 core/invariants.qnt DPIO_I1 + main.qnt DPMCP_I1Test
//     + dpio_lifecycle probeDrawTest (the probe-time dpmcp draw);
//   DPIO-I2 dpio_lifecycle DPIO_I2_seatBound + seatBoundRefusedTest
//     (regime-typed seat arithmetic; docs/baseline/dpio.md "Lifecycle ordering
//     and dependencies");
//   DPIO-I3 dpio_lifecycle DPIO_I3_modeDead + noChannelReportsPrioritiesTest
//     (Breaking: notification capability from reported num_priorities, the
//     channel_mode attribute dead in the kernel; docs/baseline/dpio.md
//     "Kernel-side behavior");
//   DPIO-I4 structural (state carries no dpio-to-CPU pairing).
// Apalache-marked subset: DPIO-I1 (core); DPIO-I2, DPIO-I3 (dpio_lifecycle
// stateInvariants).
module dpio {
  import types.* from "../core/types"

  pure val dpioParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,                // regime-typed, never pooled (DPIO-I1/I2)
    resetOnBind: false,
    hasKernelDriver: true,
    hotBindAvailable: false,      // every per-CPU seat taken at boot
    endpointPorts: 0,             // no endpoint surface (§2)
    createTriggersRescan: false,
    draw: { dpmcp: 1, dpbp: 0, dpcon: 0 },  // the forgotten draw (§3 census)
  }

  // Seat-regime vocabulary (dpio.md "Option inventory"/"Lifecycle ordering").
  // dpio is seat-typed, not pooled (DPIO-I1): the pool_lifecycle machine counts
  // dpios by regime, never as one pool (DPIO-I2). channel_mode is a create field
  // the kernel later ignores (DPIO-I3, Breaking).
  type ChannelMode = LocalChannel | NoChannel   // DPIO_LOCAL_CHANNEL | DPIO_NO_CHANNEL
  type SeatRegime = KernelSeat | DpdkSeat        // per-CPU service | per-thread exclusive

  // The finite draw alphabets, hosted here so every constructor has a value use
  // outside the pattern machine's main module — the QNT404 flattener rule the
  // verify rung needs (families/params.qnt header; companions.qnt REF_* precedent).
  pure val CHANNEL_MODES: Set[ChannelMode] = Set(LocalChannel, NoChannel)
  pure val SEAT_REGIMES: Set[SeatRegime] = Set(KernelSeat, DpdkSeat)
}

// The dpio seat machine (pool-objects task 1.2; docs/baseline/dpio.md). Unpooled
// (DPIO-I1): a regime-typed seat, not pool custody. It reuses the shared core
// transforms (createT/plugT/markAllocated/freePool) for the dpmcp it draws at
// probe — the dpio→dpmcp ordering edge — re-deriving no pool mechanics.
//
// D4 facet evidence (pool-objects design D4; judgment at task 2.3, not here):
// channel_mode is the one dpio create-cfg field that looks configurable, yet the
// kernel ignores it (DPIO-I3) — a wrong mode changes nothing in the kernel path,
// so the dpio create-cfg carries NO kernel-side hazard class. Evidence AGAINST a
// P2 cfg facet for the kernel regime; the DPDK-regime cfg hazard, if any, is
// unobserved at model scope.
//
// Named invariants (Apalache-marked via `stateInvariants`): DPIO_I2_seatBound
// (dpio.md "Lifecycle ordering"), DPIO_I3_modeDead (Breaking, dpio.md
// "Kernel-side behavior"). Directed runs: probeDrawTest, probeExhaustionDisabledTest,
// seatBoundRefusedTest, noChannelReportsPrioritiesTest.
module dpio_lifecycle {
  import types.* from "../core/types"
  import create_allocate.* from "../core/create_allocate"
  import lifecycle.* from "../core/lifecycle"
  import params.* from "./params"
  import dpio.*

  // The create-cfg a dpio carries: mode and priorities (dpio.md "Attribute
  // mutability"), its seat regime, and whether the kernel driver has probed it.
  type DpioSeat = { mode: ChannelMode, numPriorities: int, regime: SeatRegime, probed: bool }

  // Regime-typed seat ceilings (DPIO-I2; dpio.md "Lifecycle ordering"): one dpio
  // per online CPU (extras -ERANGE), two exclusive portals per DPDK thread.
  pure val NUM_CPUS: int = 2
  pure val NUM_THREADS: int = 1
  pure def seatCeiling(r: SeatRegime): int =
    match r { | KernelSeat => NUM_CPUS | DpdkSeat => 2 * NUM_THREADS }

  // DPIO-I3 (Breaking): capability reads the reported num_priorities ALONE, never
  // channel_mode — no mode argument, so a mode-derived capability is inexpressible.
  pure def dpioNotifyCapable(seat: DpioSeat): bool = seat.numPriorities != 0
  pure def dpioEightPriority(seat: DpioSeat): bool = seat.numPriorities == 8

  pure val dprc0: ObjId = { fam: Dprc, num: 0 }
  pure val poolContainer: ObjId = { fam: Dprc, num: 1 }

  pure def container(parent: Option[ObjId], hw: int): ObjState = {
    parent: parent, owner: parent, plugged: true, busVisible: true,
    bind: Unbound, enabled: false, pristine: true, linkUp: false,
    allocatedBy: None, hwId: hw,
  }

  pure val initialState: CoreState = {
    objs: Map(dprc0 -> container(None, 0), poolContainer -> container(Some(dprc0), 1)),
    dprcs: Map(
      dprc0 -> { icid: 0, perms: Set(Spawn, Alloc, ObjCreate, IrqCfg, TopologyChanges), locked: false },
      poolContainer -> { icid: 1, perms: Set(Spawn, Alloc, ObjCreate, IrqCfg), locked: false }
    ),
    conns: Set(),
    nextNum: FAMILIES.mapBy(f => if (f == Dprc) 2 else 0),
    nextHw: 100,
  }

  var s: CoreState
  var dpioSeats: ObjId -> DpioSeat

  pure def dpioSeatCount(seats: ObjId -> DpioSeat, r: SeatRegime): int =
    seats.keys().filter(o => seats.get(o).regime == r).size()

  action init = all { s' = initialState, dpioSeats' = Map() }

  // dpmcp pool setup (reusing the shared core transforms) so the probe has a
  // free dpmcp to draw.
  action createDpmcpAt: bool = all {
    canCreate(s, ALL_PARAMS, Dpmcp, poolContainer),
    s' = createT(s, ALL_PARAMS, Dpmcp, poolContainer),
    dpioSeats' = dpioSeats,
  }
  action plugAt(o: ObjId): bool = all {
    s.objs.keys().contains(o),
    not(s.objs.get(o).plugged),
    s' = plugT(s, o),
    dpioSeats' = dpioSeats,
  }

  // dpio create: unpooled seat (DPIO-I1), seat-ceiling gated (DPIO-I2, the
  // -ERANGE refusal as a disabled guard). num_priorities in 1..8 (dpio.md);
  // channel_mode recorded but kernel-ignored (DPIO-I3).
  action createDpioAt(mode: ChannelMode, numPriorities: int, regime: SeatRegime): bool =
    val o = { fam: Dpio, num: s.nextNum.get(Dpio) }
    all {
      canCreate(s, ALL_PARAMS, Dpio, poolContainer),
      dpioSeatCount(dpioSeats, regime) < seatCeiling(regime),
      numPriorities >= 1 and numPriorities <= 8,
      s' = createT(s, ALL_PARAMS, Dpio, poolContainer),
      dpioSeats' = dpioSeats.put(o,
        { mode: mode, numPriorities: numPriorities, regime: regime, probed: false }),
    }

  action plugDpioAt(o: ObjId): bool = all {
    dpioSeats.keys().contains(o),
    not(s.objs.get(o).plugged),
    s' = plugT(s, o),
    dpioSeats' = dpioSeats,
  }

  // The dpio driver probe draws ONE free dpmcp (DPIO-I1/DPMCP-I1; dpio.md
  // "Kernel-side behavior"). An exhausted pool disables it — the -EPROBE_DEFER
  // shape (core/pools.qnt drawSatisfiable).
  action probeDpioAt(o: ObjId, mcp: ObjId): bool = all {
    dpioSeats.keys().contains(o),
    s.objs.get(o).plugged,
    not(dpioSeats.get(o).probed),
    freePool(s, ALL_PARAMS, poolContainer, Dpmcp).contains(mcp),
    s' = markAllocated(s, o, Set(mcp)),
    dpioSeats' = dpioSeats.set(o, dpioSeats.get(o).with("probed", true)),
  }

  action stutter = all { s' = s, dpioSeats' = dpioSeats }

  // ---- nondet wrappers ----

  action doCreateDpmcp = createDpmcpAt
  action doPlug = { nondet o = s.objs.keys().oneOf() plugAt(o) }
  action doCreateDpio = {
    nondet mode = CHANNEL_MODES.oneOf()
    nondet prio = Set(1, 8).oneOf()
    nondet regime = SEAT_REGIMES.oneOf()
    createDpioAt(mode, prio, regime)
  }
  action doPlugDpio = { nondet o = s.objs.keys().oneOf() plugDpioAt(o) }
  action doProbeDpio = {
    nondet o = s.objs.keys().oneOf()
    nondet mcp = s.objs.keys().oneOf()
    probeDpioAt(o, mcp)
  }

  action step = any {
    doCreateDpmcp, doPlug, doCreateDpio, doPlugDpio, doProbeDpio, stutter,
  }

  // ---- named state invariants ----

  // DPIO-I2: dpio seats never exceed their regime ceiling.
  val DPIO_I2_seatBound: bool =
    SEAT_REGIMES.forall(r => dpioSeatCount(dpioSeats, r) <= seatCeiling(r))

  // DPIO-I3 (Breaking): capability is mode-independent — flipping channel_mode
  // never changes capability or 8-priority status; the mode is dead in the kernel.
  val DPIO_I3_modeDead: bool =
    dpioSeats.keys().forall(o =>
      val seat = dpioSeats.get(o)
      val flipped = seat.with("mode", if (seat.mode == LocalChannel) NoChannel else LocalChannel)
      and {
        dpioNotifyCapable(seat) == dpioNotifyCapable(flipped),
        dpioEightPriority(seat) == dpioEightPriority(flipped),
      })

  val stateInvariants: bool = and { DPIO_I2_seatBound, DPIO_I3_modeDead }

  // ---- directed runs ----

  pure val dpio0: ObjId = { fam: Dpio, num: 0 }
  pure val dpmcp0: ObjId = { fam: Dpmcp, num: 0 }

  // DPIO-I1/DPMCP-I1: the probe draws one free dpmcp; it leaves the free pool.
  run probeDrawTest =
    init
      .then(createDpmcpAt)
      .then(plugAt(dpmcp0))
      .then(createDpioAt(LocalChannel, 8, KernelSeat))
      .then(plugDpioAt(dpio0))
      .then(probeDpioAt(dpio0, dpmcp0))
      .expect(and {
        s.objs.get(dpmcp0).allocatedBy == Some(dpio0),
        dpioSeats.get(dpio0).probed,
        not(freePool(s, ALL_PARAMS, poolContainer, Dpmcp).contains(dpmcp0)),
      })

  // DPIO-I1: an exhausted dpmcp pool disables the probe (the -EPROBE_DEFER shape).
  run probeExhaustionDisabledTest =
    init
      .then(createDpioAt(LocalChannel, 8, KernelSeat))
      .then(plugDpioAt(dpio0))
      .then(probeDpioAt(dpio0, dpmcp0).fail())

  // DPIO-I2: kernel seats top out at NUM_CPUS; the next create is refused (-ERANGE).
  run seatBoundRefusedTest =
    init
      .then(createDpioAt(LocalChannel, 8, KernelSeat))
      .then(createDpioAt(LocalChannel, 8, KernelSeat))
      .expect(dpioSeatCount(dpioSeats, KernelSeat) == NUM_CPUS)
      .then(createDpioAt(LocalChannel, 8, KernelSeat).fail())

  // DPIO-I3 (Breaking): a NO_CHANNEL dpio at 8 priorities is still capable and
  // 8-priority — the reported num_priorities decides, the mode does not fold them
  // away (dpio.md "Kernel-side behavior"; V-READBACK-1).
  run noChannelReportsPrioritiesTest =
    init
      .then(createDpioAt(NoChannel, 8, KernelSeat))
      .expect(and {
        dpioNotifyCapable(dpioSeats.get(dpio0)),
        dpioEightPriority(dpioSeats.get(dpio0)),
      })
}
