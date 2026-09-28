// Scenario module for board suite V-POOL-7 (pool-objects task 4.1;
// pool-objects design D5 root-bind floor): the pool free/drain path observed
// through the kernel face at ROOT scope. A root-container dpni's dpaa2-eth
// bind draws its companions from the root allocator pool (DPBP-I2's
// root-kernel half); the sysfs unbind returns them (DPBP-I3's dirty return,
// DPCON-I5's free path, DPMCP-I3's observable half); a re-bind proves the
// freed units are re-drawable. Observing, not driving — the free path is the
// kernel's (pool-objects design D3), so the model ends at the bind and the
// unbind/free/rebind cycle is the hook drain.sh.
//
// The trace is V-LIFE-DPNI-1 / V-DPNI-3's, verbatim: the §5 canonical forward
// order for a dpni in the Linux root — companions (dpmcp, dpbp, dpcon)
// created and plugged, bus rescan, kernel probe to a bound netdev. Objects
// live in dprc.1 by necessity (kernel binds happen in the Linux root only,
// DPRC-I6). `--pool-record` captures the root pool per trace step, so the
// bind's draw is a file; the hook reads it again across the unbind and
// re-bind. The judged trace ends at the bind; the unconditional teardown
// (unbind, unplug, destroy, newest first) restores the root container.
//
// Guards leave exactly one action enabled per state, so any seed freezes the
// same ten-step trace:
//
//   pnpm exec quint run models/board/V-POOL-7/vpool7.qnt --main=vpool7 \
//     --mbt --max-steps=10 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-POOL-7/vpool7.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vpool7 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

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

  // 9. Root bus rescan makes the four visible; disabled once the dpni is.
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
  //     companions (dpni draw 1/1/1) from the root allocator pool.
  action kernelBind = {
    nondet o = Set(theDpni).oneOf()
    nondet mcp = Set(theDpmcp).oneOf()
    nondet bp = Set(theDpbp).oneOf()
    nondet con = Set(theDpcon).oneOf()
    m::kernelBindAt(o, Set(mcp), Set(bp), Set(con))
  }

  action step = any { createObject, plug, rescan, kernelBind }
}
