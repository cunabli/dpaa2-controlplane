// ADR-0012's companion derivation as a pure function: the count of
// companion objects a consumer draws is a quantity of the consumer's
// regime and thread count, not of its port count. The port-count model
// the shipped `ls-addni` script encodes is correct for the kernel regime
// and silently wrong for a poll-mode userspace consumer.
//
// The numbers come from ADR-0012 ("What the board answered"): the kernel
// container's per-CPU portal service and per-dpni buffer pool, the
// poll-mode child's 2 × T exclusive portals and fixed dpbp pair. The
// poll-mode dpmcp count is one per process on the DPDK fslmc bus — the
// bus maps a single dpmcp per process (primary or secondary) regardless
// of object count, and every driver's commands cross that one portal
// (sitting-5.12 note in models/board/README.md; dpdk-26.03
// drivers/bus/fslmc/fslmc_vfio.c).
//
// Named invariants: none.

module companions {
  // The two consumer regimes ADR-0012 separates. Kernel: the container's
  // own network driver, borrowing a shared per-CPU portal service.
  // PollMode: a DPDK-bus userspace consumer in a child container, taking
  // portals exclusively for its lifetime.
  type Regime = Kernel | PollMode

  // The companion families whose counts derive from the regime. dpio is
  // unpooled (DPIO-I1) but still sized here; dpni transmit queues are a
  // dpni-side count, not a companion object, so they stay in the ADR.
  type CompanionDraw = { dpio: int, dpbp: int, dpmcp: int }

  // ADR-0012's decision table made executable. cpus is the container's
  // online CPU count (kernel regime), threads is the consumer's thread
  // count T = main + workers (poll-mode regime), processes is the number
  // of bus processes attached (poll-mode regime, primary + secondaries),
  // and dpnis is the port count the consumer drives.
  pure def companionDraw(regime: Regime, cpus: int, threads: int,
                         processes: int, dpnis: int): CompanionDraw =
    match regime {
      // Kernel regime (ADR-0012 decision, first bullet): dpios are a
      // container-wide per-CPU portal service, at most one per online CPU;
      // dpbps one per dpni; dpmcps one per probing consumer — the dpios
      // included, the draw the port-count model forgets. Other kernel
      // consumer families (dpsw, dpseci, dprtc, dpdmai, dpdmux) each add
      // one dpmcp per FamilyParams.draw and are out of this signature's
      // scope.
      | Kernel => { dpio: cpus, dpbp: dpnis, dpmcp: cpus + dpnis }
      // Poll-mode regime (ADR-0012 decision, second bullet): a general and
      // a receive portal per thread, each an exclusive dpio; a fixed dpbp
      // pair (the consumer's pool and the driver's scatter-gather pool),
      // independent of dpnis; and one MC portal per process (primary or
      // secondary), every object's commands going through it.
      | PollMode => { dpio: 2 * threads, dpbp: 2, dpmcp: processes }
    }

  // The reference board's two containers, giving each Regime constructor a
  // value use inside this module (the QNT404 rung: a sum type whose only
  // value use sits in main.qnt loses its constructors in the `quint
  // verify` flattener — see families/params.qnt). Kernel container: 16
  // CPUs, one boot dpni. Poll-mode child: T = 5 driving two dpnis, one
  // process.
  pure val REF_KERNEL_DRAW = companionDraw(Kernel, 16, 0, 1, 1)
  pure val REF_POLLMODE_DRAW = companionDraw(PollMode, 0, 5, 1, 2)
}
