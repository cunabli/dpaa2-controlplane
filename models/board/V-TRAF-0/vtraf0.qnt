// Scenario module for board suite V-TRAF-0 (task 5.5; traffic-inventory
// §3): the traffic-bearing reachability pattern of design D7 step 5,
// instantiated on the smallest configured object group there is — one
// kernel-bound dpni on the flagged dpmac.7 with its census companions.
// The frames themselves are not model steps (reachability is a property
// of a standing group, not a lifecycle transition), so this module only
// brings the group up: the setup is V-LINK-2's, verbatim and for the
// same reasons, through the kernel bind and the consumer enable to a
// confirmed link-up, and it stops there. The traffic face is the suite
// hook traffic.sh beside this file: the generated script sources it
// after the last step, with the group standing, and its teardown trap
// reclaims the scratch set afterwards whatever the hook found.
//
// On a bare boot the boot pair dpni.1 ↔ dpmac.7 is not there (the
// reference capture is a provisioned moment), so the disconnect step
// no-ops and reads back restool's "does not exist", which is the
// absent-with-no-peer the step expects; the scratch dpni may be minted
// the id the capture shows on the boot pair. Neither changes the run.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same fourteen-step trace:
//
//   pnpm exec quint run models/board/V-TRAF-0/vtraf0.qnt --main=vtraf0 \
//     --mbt --max-steps=14 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-TRAF-0/vtraf0.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vtraf0 {
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
  //       run created until the rescan and never false again.
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

  // 14. Confirm the link is up. Nothing else is enabled from the enable
  //     onward, so the trace length fixes this at one step: the group is
  //     left standing, link up, for the probe plan to push frames through.
  action linkChange = {
    nondet o = Set(theDpni).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      m::s.objs.get(theDpni).enabled,
      m::linkChangeAt(o),
    }
  }

  action step = any {
    disconnectEdge, createObject, connectEdge, plug, rescan, kernelBind,
    enable, linkChange,
  }
}
