// RETIRED — ran once, 2026-08-23; do not regenerate. The closing unplug
// was refused by the board: exit -EBUSY, the object still plugged and
// the driver still bound. That answers dprc.md unknown 9 — `assign
// --plugged=0` is not available under a bound driver, so the question
// was never a race. The model now refuses unplug of a driver-bound
// object, which means this trace can no longer be produced from it. The
// module and its frozen artifacts are kept as the record of the run that
// settled the question.
//
// Scenario module for board suite V-LINK-5 (task 5.3; traffic-inventory
// §1): the plugged race. What does `dprc assign --plugged=0` do to an
// object the kernel is holding, on a live port — does the MC refuse it,
// does the plug bit clear while the driver keeps the device, or does the
// unplug tear the driver off as DPRC-I2 says it should? (DPRC-I2;
// dprc.md unknown 9.)
//
// The model says unplug is the unbind trigger: `unplugT` releases the
// bind with the plug, so the suite expects the object unplugged *and*
// its sysfs driver link gone. Three divergences are possible and each
// answers the unknown by itself: the MC refusing the command outright
// (the plug bit stays set) says --plugged=0 is not available under a
// bound driver; the plug bit clearing while the driver link survives
// says the unbind is asynchronous or absent, and the model's unplugT
// must stop releasing the bind; both clearing is the model confirmed on
// the hardest case there is — a bound, enabled, link-up netdev.
//
// Getting to that case needs the port, and the port is the boot edge's:
// the reference environment wires dpni.1 (inside the VPP consumer
// container dprc.2) to dpmac.7. So the suite severs that edge, stands a
// scratch dpni of its own in the *root* container — kernel binds happen
// in the Linux root only, a child-container object is never bus-visible
// (DPRC-I6) — connects it to dpmac.7, and takes it all the way up:
// census companions, plug, rescan, the kernel's probe, the operator's
// `ip link set up`, and a confirmed live link against the peer. Only
// then is the unplug worth anything.
//
// Precondition, and the reason this suite is flagged: the VPP consumer
// loses dpmac.7 for the whole run. The unconditional teardown trap
// destroys the scratch set and then reconnects dpni.1 ↔ dpmac.7,
// whether the run passed, failed or aborted.
//
// On a bare boot none of that consumer side is there: the
// reference-environment capture the setup is modeled on was taken at a
// provisioned moment, so dprc.2 and its dpnis do not exist yet. The
// disconnect step then no-ops and its read-back is restool's "does not
// exist" for dpni.1 — an absent object, truthfully with no peer, which
// is exactly what the step expects — and the scratch dpni may be minted
// the same id the capture shows on the boot pair. Neither changes what
// the suite tests.
//
// The companion recipe (dpmcp/dpbp/dpcon created and plugged in the root,
// rescan, then the awaited probe drawing exactly 1/1/1) is V-LIFE-DPNI-1's,
// unchanged — that suite already proved it lands a bound dpni.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same fifteen-step trace:
//
//   pnpm exec quint run models/board/V-LINK-5/vlink5.qnt --main=vlink5 \
//     --mbt --max-steps=15 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-LINK-5/vlink5.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vlink5 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  // The boot pair the suite borrows: the consumer container's dpni and
  // the wired 10G port it holds.
  pure val bootDpni: ObjId = { fam: Dpni, num: 1 }
  pure val theDpmac: ObjId = { fam: Dpmac, num: 7 }

  // The scratch set, all of it in the root container.
  pure val theDpmcp: ObjId = { fam: Dpmcp, num: 0 }
  pure val theDpbp: ObjId = { fam: Dpbp, num: 0 }
  pure val theDpcon: ObjId = { fam: Dpcon, num: 0 }
  // dpni.0 and dpni.1 are taken at boot, so the scratch dpni is dpni.2.
  pure val theDpni: ObjId = { fam: Dpni, num: 2 }

  action init = m::initWiredAt(bootDpni, theDpmac)

  // 1. Sever the boot edge: an endpoint carries at most one peer, so the
  //    port cannot be shared. Fires once — the edge is never restored
  //    inside the trace (the suite's teardown trap does that).
  action disconnectEdge = {
    nondet e = Set({ obj: bootDpni, port: 0 }).oneOf()
    m::disconnectEdgeAt(e)
  }

  // 2-5. Companions first, consumer last (§5 order), serialized by the
  //      per-family counters and gated on the port being free.
  action createObject = {
    nondet fam = Set(Dpmcp, Dpbp, Dpcon, Dpni).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.conns.size() == 0,
      if (fam == Dpmcp) m::s.nextNum.get(Dpmcp) == 0
      else if (fam == Dpbp) and {
        m::s.nextNum.get(Dpmcp) == 1,
        m::s.nextNum.get(Dpbp) == 0,
      } else if (fam == Dpcon) and {
        m::s.nextNum.get(Dpbp) == 1,
        m::s.nextNum.get(Dpcon) == 0,
      } else and {
        m::s.nextNum.get(Dpcon) == 1,
        m::s.nextNum.get(Dpni) == 2,
      },
      m::createObjectAt(fam, c),
    }
  }

  // 6. The scratch dpni takes the port. Before the plugs, so the driver
  //    probes a settled topology the way it would at a DPL boot.
  action connectEdge = {
    nondet a = Set({ obj: theDpni, port: 0 }).oneOf()
    nondet b = Set({ obj: theDpmac, port: 0 }).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 3,
      m::connectEdgeAt(a, b),
    }
  }

  // 7-10. Plug in creation order, only once the edge stands. Bus
  //       visibility is the phase marker: it is false for everything the
  //       run created until the rescan and never false again, so this
  //       cannot re-fire after the closing unplug.
  action plug = {
    nondet o = Set(theDpmcp, theDpbp, theDpcon, theDpni).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      not(m::s.objs.get(theDpni).busVisible),
      m::s.conns.size() > 0,
      if (o == theDpmcp) true
      else if (o == theDpbp) m::s.objs.get(theDpmcp).plugged
      else if (o == theDpcon) m::s.objs.get(theDpbp).plugged
      else m::s.objs.get(theDpcon).plugged,
      m::plugAt(o),
    }
  }

  // 11. Root bus rescan makes the four visible; disabled once the dpni
  //     is, so the run cannot rescan twice.
  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      m::s.objs.get(theDpni).plugged,
      not(m::s.objs.get(theDpni).busVisible),
      m::rescanAt(c),
    }
  }

  // 12. The kernel's probe, awaited: the census draws exactly the three
  //     companions (dpni draw 1/1/1).
  action kernelBind = {
    nondet o = Set(theDpni).oneOf()
    nondet mcp = Set(theDpmcp).oneOf()
    nondet bp = Set(theDpbp).oneOf()
    nondet con = Set(theDpcon).oneOf()
    m::kernelBindAt(o, Set(mcp), Set(bp), Set(con))
  }

  // 13. The consumer enable, awaited: restool enables nothing (§5 step
  //     7), so the operator brings the netdev up.
  action enable = {
    nondet o = Set(theDpni).oneOf()
    m::enableAt(o)
  }

  // 14. The link, awaited: the operator confirms the port is up against
  //     the live peer. Judged through `dpni info`'s link status, which
  //     is the only line that reports link and not connection state.
  action linkChange = {
    nondet o = Set(theDpni).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      m::s.objs.get(theDpni).enabled,
      not(m::s.objs.get(theDpni).linkUp),
      m::linkChangeAt(o),
    }
  }

  // 15. The race, and the only driven step that matters: --plugged=0 on
  //     a bound, enabled, link-up netdev.
  action unplug = {
    nondet o = Set(theDpni).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      m::s.objs.get(theDpni).linkUp,
      m::unplugAt(o),
    }
  }

  action step = any {
    disconnectEdge, createObject, connectEdge, plug, rescan, kernelBind,
    enable, linkChange, unplug,
  }
}
