// Scenario module for board suite V-POOL-2 (task 5.11; sitting-5.11 note
// "what a pool object carries across owners"): DPBP-I3 and DPCON-I5 (the
// kernel frees a pool object as drain -> disable -> close, no reset, so
// the next allocator's reset is what cleans it) and DPMCP-I3 (no reset
// anywhere; restool exposes no portal state, so that half has no
// observable here). The group and trace are V-POOL-1's, verbatim — a
// scratch child holding two dpmcps, two dpcons, two dpbps and two
// unconnected dpnis, everything plugged except the second dpbp — bound
// the same way. Only the hook differs: pool2.sh drives a dpni
// unbind/rebind cycle and reads its dpbp across it, where V-POOL-1's
// pool1.sh drives the exhaustion/top-up. The model has no notion of a
// kernel freeing a pool object, so the cycle lives entirely in the hook.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same seventeen-step trace (1 container + 8 creates + 7 plugs + the
// child-dprc plug). The final step plugs the scratch child itself so the
// kernel's autorescan binds it: a restool-created child is unplugged and
// unbindable (V-POOL-1 rev 1), so the hook waits for the plug-driven bind
// rather than writing to `bind` by hand.
//
//   pnpm exec quint run models/board/V-POOL-2/vpool2.qnt --main=vpool2 \
//     --mbt --max-steps=17 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-POOL-2/vpool2.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vpool2 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  // The scratch child and its residents (model ids are symbolic — the
  // board names creates itself; dprc numbering starts at 2).
  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }
  pure val theDpmcp0: ObjId = { fam: Dpmcp, num: 0 }
  pure val theDpmcp1: ObjId = { fam: Dpmcp, num: 1 }
  pure val theDpcon0: ObjId = { fam: Dpcon, num: 0 }
  pure val theDpcon1: ObjId = { fam: Dpcon, num: 1 }
  pure val theDpbp0: ObjId = { fam: Dpbp, num: 0 }
  pure val theDpbp1: ObjId = { fam: Dpbp, num: 1 }
  pure val theDpni0: ObjId = { fam: Dpni, num: 0 }
  pure val theDpni1: ObjId = { fam: Dpni, num: 1 }

  action init = m::init

  // 1. One scratch container under the Linux root; disabled once it
  //    exists.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      not(m::s.dprcs.keys().contains(scratchDprc)),
      m::createContainerAt(parent),
    }
  }

  // 2-9. Two of each family, in the fixed order mcp, con, bp, ni, two
  //       apiece, serialized by the per-family counters so exactly one
  //       (fam, c) pick is enabled per state.
  action createObject = {
    nondet fam = Set(Dpmcp, Dpcon, Dpbp, Dpni).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchDprc),
      if (m::s.nextNum.get(Dpmcp) < 2) fam == Dpmcp
      else if (m::s.nextNum.get(Dpcon) < 2) fam == Dpcon
      else if (m::s.nextNum.get(Dpbp) < 2) fam == Dpbp
      else and { m::s.nextNum.get(Dpni) < 2, fam == Dpni },
      m::createObjectAt(fam, c),
    }
  }

  // 10-17. Plug in the fixed order mcp0, mcp1, con0, con1, bp0, ni0, ni1,
  //         then the scratch child dprc itself — the second dpbp is
  //         deliberately never plugged (one pool object short of the
  //         batch). The child-dprc plug is the last, so the kernel's
  //         autorescan (=1) binds it to fsl_mc_dprc once plugged (a
  //         restool-created child is unplugged and unbindable otherwise,
  //         V-POOL-1 rev 1); the hook then waits for that bind rather than
  //         writing to `bind`. `plugAt` admits a dprc whose parent is not
  //         locked. Enabled only once all eight objects exist, each gated
  //         on its predecessor being plugged.
  action plug = {
    nondet o = Set(theDpmcp0, theDpmcp1, theDpcon0, theDpcon1, theDpbp0, theDpni0, theDpni1, scratchDprc).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 2,
      m::s.nextNum.get(Dpbp) == 2,
      if (not(m::s.objs.get(theDpmcp0).plugged)) o == theDpmcp0
      else if (not(m::s.objs.get(theDpmcp1).plugged)) o == theDpmcp1
      else if (not(m::s.objs.get(theDpcon0).plugged)) o == theDpcon0
      else if (not(m::s.objs.get(theDpcon1).plugged)) o == theDpcon1
      else if (not(m::s.objs.get(theDpbp0).plugged)) o == theDpbp0
      else if (not(m::s.objs.get(theDpni0).plugged)) o == theDpni0
      else if (not(m::s.objs.get(theDpni1).plugged)) o == theDpni1
      else and { not(m::s.objs.get(scratchDprc).plugged), o == scratchDprc },
      m::plugAt(o),
    }
  }

  action step = any { createContainer, createObject, plug }
}
