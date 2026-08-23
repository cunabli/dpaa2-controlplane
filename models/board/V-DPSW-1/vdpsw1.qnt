// Scenario module for board suite V-DPSW-1 (task 5.2, batch 3): the
// dpsw positive create+connect face — companions created and plugged in
// the Linux root, the switch's uplink connected to a spare dpmac, bus
// rescan, then the kernel's own probe awaited and judged through the
// sysfs driver link.
//
// The create is not bare, and cannot be. The kernel's switch driver
// refuses to probe an object whose flooding or broadcast domain is
// scoped anywhere but per FDB, and restool's unstated defaults are the
// other value in both cases — so a default-created dpsw is rejected at
// probe with a logged reason. The adapter's create arguments ask for the
// configuration the driver accepts; this suite is the board anchor for
// dpsw being hot-bindable at all (ADR-0008 lists it as driver-present
// but unsat until now).
//
// The census draw is a dpmcp and a dpbp (dpsw.qnt draw 1/1/0): the
// driver opens a portal and takes a buffer pool for its control
// interface. Both companions are created here rather than borrowed —
// the board's boot dpbp already belongs to the boot dpni, so an
// unsatisfied draw would stall the probe in the silent retry loop
// (DPNI-I5). The dpio the driver needs is borrowed from the shared
// per-CPU service and drawn from no pool (DPIO-I1).
//
// The connect comes before the rescan on purpose: the driver should
// probe a settled topology, the way it would at a DPL boot, rather than
// meet a switch whose ports change under it. Connect carries no
// plug-state guard either way (§2).
//
// The objects live in dprc.1 by necessity, not preference: kernel binds
// happen in the Linux root only (DPRC-I6). There are no destroy steps —
// destroying a bound resident of the root container is exactly the race
// that silently detaches a bystander's driver (ADR-0008 §4-§6), so the
// judged trace ends at the bind and the suite's unconditional teardown
// trap (unbind, unplug, destroy, newest first) does the cleanup.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same nine-step trace:
//
//   pnpm exec quint run models/board/V-DPSW-1/vdpsw1.qnt --main=vdpsw1 \
//     --mbt --max-steps=9 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPSW-1/vdpsw1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpsw1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val theDpmcp: ObjId = { fam: Dpmcp, num: 0 }
  pure val theDpbp: ObjId = { fam: Dpbp, num: 0 }
  pure val theDpsw: ObjId = { fam: Dpsw, num: 0 }
  // A spare dpmac: wired to nothing, and outside the total-deny set the
  // safety envelope screens for.
  pure val theDpmac: ObjId = { fam: Dpmac, num: 5 }

  action init = m::init

  // 1-3. Companions first, consumer last (§5 order), serialized by the
  //      per-family counters.
  action createObject = {
    nondet fam = Set(Dpmcp, Dpbp, Dpsw).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      if (fam == Dpmcp) m::s.nextNum.get(Dpmcp) == 0
      else if (fam == Dpbp) and {
        m::s.nextNum.get(Dpmcp) == 1,
        m::s.nextNum.get(Dpbp) == 0,
      } else and {
        m::s.nextNum.get(Dpbp) == 1,
        m::s.nextNum.get(Dpsw) == 0,
      },
      m::createObjectAt(fam, c),
    }
  }

  // 4. The switch's uplink to the spare dpmac. The switch endpoint is
  //    named first so the read-back probes the switch's own info, which
  //    reports a peer per interface; the dpmac side would report the
  //    switch three-part regardless of port and prove less.
  action connectEdge = {
    nondet a = Set({ obj: theDpsw, port: 0 }).oneOf()
    nondet b = Set({ obj: theDpmac, port: 0 }).oneOf()
    all {
      m::s.nextNum.get(Dpsw) == 1,
      m::connectEdgeAt(a, b),
    }
  }

  // 5-7. Plug in creation order, only once the edge stands.
  action plug = {
    nondet o = Set(theDpmcp, theDpbp, theDpsw).oneOf()
    all {
      m::s.conns.size() > 0,
      if (o == theDpmcp) true
      else if (o == theDpbp) m::s.objs.get(theDpmcp).plugged
      else m::s.objs.get(theDpbp).plugged,
      m::plugAt(o),
    }
  }

  // 8. Root bus rescan makes the three visible; disabled once the dpsw
  //    is, so the run cannot rescan twice.
  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(theDpsw),
      m::s.objs.get(theDpsw).plugged,
      not(m::s.objs.get(theDpsw).busVisible),
      m::rescanAt(c),
    }
  }

  // 9. The kernel's probe, awaited: the census draws exactly the two
  //    companions (dpsw draw 1/1/0).
  action kernelBind = {
    nondet o = Set(theDpsw).oneOf()
    nondet mcp = Set(theDpmcp).oneOf()
    nondet bp = Set(theDpbp).oneOf()
    m::kernelBindAt(o, Set(mcp), Set(bp), Set())
  }

  action step = any { createObject, connectEdge, plug, rescan, kernelBind }
}
