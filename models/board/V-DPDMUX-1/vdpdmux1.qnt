// Scenario module for board suite V-DPDMUX-1 (task 5.2, batch 3): the
// dpdmux positive create+connect face — the portal companion created
// and plugged in the Linux root, the demux uplink connected to a spare
// dpmac, bus rescan, then the kernel's own probe awaited and judged
// through the sysfs driver link.
//
// The create carries one argument because restool insists on it: a
// dpdmux create without an interface count is refused. The count
// excludes the uplink, so asking for one downlink yields the two ports
// the model gives the family. Everything else is left to restool's
// defaults, which the reference kernel's demux driver accepts — it
// gates only on the object's API version, not on its configuration, so
// unlike the switch there is no configuration to get right.
//
// The census draw is a single dpmcp (dpdmux.qnt draw 1/0/0): the driver
// opens a portal and takes nothing from the buffer or channel pools.
//
// Port 0 is the uplink and the only port that may face a dpmac since MC
// 10.37; the model already refuses any other pairing (connect.legalPorts),
// so the connect here exercises the legal face of that rule. It comes
// before the rescan so the driver probes a settled topology, the way it
// would at a DPL boot.
//
// The objects live in dprc.1 by necessity, not preference: kernel binds
// happen in the Linux root only (DPRC-I6). There are no destroy steps —
// destroying a bound resident of the root container is exactly the race
// that silently detaches a bystander's driver (ADR-0008 §4-§6), so the
// judged trace ends at the bind and the suite's unconditional teardown
// trap (unbind, unplug, destroy, newest first) does the cleanup.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same seven-step trace:
//
//   pnpm exec quint run models/board/V-DPDMUX-1/vdpdmux1.qnt --main=vdpdmux1 \
//     --mbt --max-steps=7 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPDMUX-1/vdpdmux1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpdmux1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val theDpmcp: ObjId = { fam: Dpmcp, num: 0 }
  pure val theDpdmux: ObjId = { fam: Dpdmux, num: 0 }
  // A spare dpmac: wired to nothing, and outside the total-deny set the
  // safety envelope screens for.
  pure val theDpmac: ObjId = { fam: Dpmac, num: 4 }

  action init = m::init

  // 1-2. Companion first, consumer last (§5 order), serialized by the
  //      per-family counters.
  action createObject = {
    nondet fam = Set(Dpmcp, Dpdmux).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      if (fam == Dpmcp) m::s.nextNum.get(Dpmcp) == 0
      else and {
        m::s.nextNum.get(Dpmcp) == 1,
        m::s.nextNum.get(Dpdmux) == 0,
      },
      m::createObjectAt(fam, c),
    }
  }

  // 3. The uplink to the spare dpmac. The demux endpoint is named first
  //    so the read-back probes the demux's own info, which reports a
  //    peer per interface.
  action connectEdge = {
    nondet a = Set({ obj: theDpdmux, port: 0 }).oneOf()
    nondet b = Set({ obj: theDpmac, port: 0 }).oneOf()
    all {
      m::s.nextNum.get(Dpdmux) == 1,
      m::connectEdgeAt(a, b),
    }
  }

  // 4-5. Plug in creation order, only once the edge stands.
  action plug = {
    nondet o = Set(theDpmcp, theDpdmux).oneOf()
    all {
      m::s.conns.size() > 0,
      if (o == theDpmcp) true else m::s.objs.get(theDpmcp).plugged,
      m::plugAt(o),
    }
  }

  // 6. Root bus rescan makes both visible; disabled once the dpdmux is,
  //    so the run cannot rescan twice.
  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(theDpdmux),
      m::s.objs.get(theDpdmux).plugged,
      not(m::s.objs.get(theDpdmux).busVisible),
      m::rescanAt(c),
    }
  }

  // 7. The kernel's probe, awaited: the census draws exactly the portal
  //    companion (dpdmux draw 1/0/0).
  action kernelBind = {
    nondet o = Set(theDpdmux).oneOf()
    nondet mcp = Set(theDpmcp).oneOf()
    m::kernelBindAt(o, Set(mcp), Set(), Set())
  }

  action step = any { createObject, connectEdge, plug, rescan, kernelBind }
}
