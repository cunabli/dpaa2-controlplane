// Scenario module for board suite V-LINK-2 (task 5.3; traffic-inventory
// §1): link state and connection state are two variables, and this is
// the suite that pulls them apart on live hardware. A real flap is
// performed on the peer's side of dpmac.7 while nothing touches the DPRC
// topology, and both faces are read back in the same step: `dpni info`'s
// `link status:` line must follow the cable down and up again, while the
// `endpoint:` line's `, link is up` — which is what `dprc_get_connection`
// returned, not MAC link state — is captured beside it as the evidence
// that it did not move. That is DPMAC-I5 stated as an experiment.
//
// The second finding rides along free (DPMAC-I9; dpmac.md unknown 2):
// every link push the kernel makes carries state_valid=0, which leaves
// open whether the MC honours the up bit at all on that path. A flap
// that the peer sees, and that comes back up when the peer's port does,
// is evidence it does. Noted here for the fold-back; the suite does not
// judge it.
//
// Setup is V-LINK-5's, verbatim and for the same reason: the port
// belongs to the boot edge dpni.1 ↔ dpmac.7 (dpni.1 lives in the VPP
// consumer container dprc.2), so the suite severs that edge, stands a
// scratch dpni in the *root* container — kernel binds happen in the
// Linux root only (DPRC-I6) — connects it to dpmac.7, and brings it all
// the way to a live link. The VPP consumer loses dpmac.7 for the whole
// run; the teardown trap destroys the scratch set and reconnects the
// boot edge whether the run passed, failed or aborted.
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
// Three link steps follow the enable — confirm up, flap down, restore
// up — and the toggle needs no disambiguation: from the enable onward
// `linkChange` is the only enabled action, so the trace is determined by
// its length alone and no scenario-local variable is needed. Each step
// carries its own operator prompt and its own judged expectation, so a
// board that reports the link still up after the flap fails that step.
//
// Rev-2 finding, from a run whose flap-down step read the link still up
// right after the operator acked: the peer's admin-down leaves its
// transmitter lit, so carrier never drops on the board side and there is
// no link-down stimulus on this wiring but a physical cable pull. And
// once the cable is pulled, the MC-visible link state lags the local
// carrier flag by longer than the probe delay, so a carrier-only ack
// still races propagation. Rev 3 therefore folds restool's own `link
// status:` read-back into the ack criteria for both directions.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same sixteen-step trace:
//
//   pnpm exec quint run models/board/V-LINK-2/vlink2.qnt --main=vlink2 \
//     --mbt --max-steps=16 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-LINK-2/vlink2.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vlink2 {
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

  // 14-16. The flap: confirm up, take the peer port down, bring it back.
  //        The action toggles, so the three steps alternate on their own
  //        and each one's expectation comes from the state it produced.
  //        Nothing else is enabled from the enable onward, so the trace
  //        length is what fixes the count at three.
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
