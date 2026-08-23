// Scenario module for board suite V-LIFE-DPNI-1 (task 5.2; design D7
// step 2's "per-family lifecycle scenarios"): the §5 canonical forward
// order for a dpni, on the board — companions (dpmcp, dpbp, dpcon)
// created and plugged in the Linux root, bus rescan, then the kernel's
// own probe: the awaited bind is judged through the sysfs driver link,
// which is the read-back witness that the census draw (§3: 1 dpmcp,
// 1 dpbp, 1 dpcon) was satisfiable — the positive face of DPBP-I4.
//
// The objects live in dprc.1 by necessity, not preference: kernel binds
// happen in the Linux root only (a child-container object is never
// bus-visible, DPRC-I6). The judged trace ends at the bind; the suite's
// unconditional teardown trap (unplug then destroy, newest first)
// restores the root container.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same ten-step trace:
//
//   pnpm exec quint run models/board/vlife_dpni1.qnt --main=vlife_dpni1 \
//     --mbt --max-steps=10 --n-traces=1 --max-samples=1 \
//     --out-itf=models/traces/vlife_dpni1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vlife_dpni1 {
  import types.* from "../core/types"
  import params.* from "../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../core/machine"

  pure val theDpmcp: ObjId = { fam: Dpmcp, num: 0 }
  pure val theDpbp: ObjId = { fam: Dpbp, num: 0 }
  pure val theDpcon: ObjId = { fam: Dpcon, num: 0 }
  pure val theDpni: ObjId = { fam: Dpni, num: 0 }

  action init = m::init

  // 1–4. Companions first, consumer last (§5 order), serialized by the
  //      per-family counters.
  action createObject = {
    nondet fam = Set(Dpmcp, Dpbp, Dpcon, Dpni).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      if (fam == Dpmcp) m::s.nextNum.get(Dpmcp) == 0
      else if (fam == Dpbp) and {
        m::s.nextNum.get(Dpmcp) == 1,
        m::s.nextNum.get(Dpbp) == 0,
      } else if (fam == Dpcon) and {
        m::s.nextNum.get(Dpbp) == 1,
        m::s.nextNum.get(Dpcon) == 0,
      } else and {
        m::s.nextNum.get(Dpcon) == 1,
        m::s.nextNum.get(Dpni) == 0,
      },
      m::createObjectAt(fam, c),
    }
  }

  // 5–8. Plug in the same order, only once all four exist.
  action plug = {
    nondet o = Set(theDpmcp, theDpbp, theDpcon, theDpni).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 1,
      if (o == theDpmcp) true
      else if (o == theDpbp) m::s.objs.get(theDpmcp).plugged
      else if (o == theDpcon) m::s.objs.get(theDpbp).plugged
      else m::s.objs.get(theDpcon).plugged,
      m::plugAt(o),
    }
  }

  // 9. Root bus rescan makes the four visible; disabled once the dpni
  //    is, so the run cannot rescan twice.
  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      m::s.objs.get(theDpni).plugged,
      not(m::s.objs.get(theDpni).busVisible),
      m::rescanAt(c),
    }
  }

  // 10. The kernel's probe, awaited: the census draws exactly the three
  //     companions (dpni draw 1/1/1).
  action kernelBind = {
    nondet o = Set(theDpni).oneOf()
    nondet mcp = Set(theDpmcp).oneOf()
    nondet bp = Set(theDpbp).oneOf()
    nondet con = Set(theDpcon).oneOf()
    m::kernelBindAt(o, Set(mcp), Set(bp), Set(con))
  }

  action step = any { createObject, plug, rescan, kernelBind }
}
