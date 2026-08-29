// Scenario module for board suite V-DPSW-4 (task 5.9): the refusal face
// of V-DPSW-1 (DPSW-I1's refusal face, DPSW-I2). The switch driver takes
// a runtime dpsw only when it is built in the one shape it accepts —
// control interface on, flooding and broadcast both scoped per FDB. This
// suite builds the switch the way restool's silent defaults build it
// (flooding PER_VLAN, broadcast PER_OBJECT), which is the shape the
// reference kernel's dpaa2-switch driver rejects at probe: its
// dpaa2_switch_supports_cpu_traffic() bails with a logged reason and the
// object never binds. V-DPSW-1 carries the flags explicitly and binds;
// this one omits them (via `--create-args dpsw=--num-ifs=2`, dropping the
// PER_FDB flags the adapter's default table would add) and stays unbound.
//
// The trace is V-DPSW-1's up to the bus rescan, then stops: dpmcp, dpbp
// and dpsw created and plugged in the Linux root, the switch's uplink
// connected to a spare dpmac, one rescan — eight steps. There is no
// kernelBind step, because the kernel does not bind this object; the
// hook refusal.sh reads the negative outcome instead (driver link empty,
// the driver's refusal line in the kernel log, the flooding/broadcast
// config restool reports). Judging the refusal on the driver link plus
// the log text rather than on any exit code is why it needs a hook: a
// probe refusal is a disabled action, not a trace step.
//
// The objects live in dprc.1 by necessity: kernel probes happen in the
// Linux root only (DPRC-I6). There are no destroy steps — destroying a
// resident of the root container races the bus rescan (ADR-0008 §4-§6) —
// so the judged trace ends at the rescan and the suite's unconditional
// teardown trap (unbind, unplug, destroy, newest first) does the cleanup.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same eight-step trace:
//
//   pnpm exec quint run models/board/V-DPSW-4/vdpsw4.qnt --main=vdpsw4 \
//     --mbt --max-steps=8 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPSW-4/vdpsw4.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpsw4 {
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
  //    named first so a later read-back probes the switch's own info,
  //    which reports a peer per interface.
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

  // 8. Root bus rescan makes the three visible and triggers the kernel's
  //    probe attempt; disabled once the dpsw is visible, so the run
  //    cannot rescan twice. The probe fails inside the kernel (the switch
  //    shape is unacceptable), so no bind follows and the trace ends here.
  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(theDpsw),
      m::s.objs.get(theDpsw).plugged,
      not(m::s.objs.get(theDpsw).busVisible),
      m::rescanAt(c),
    }
  }

  action step = any { createObject, connectEdge, plug, rescan }
}
