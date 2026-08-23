// Scenario module for board suite V-DPRC-1 (task 5.2; traffic-inventory
// §1): the scratch-container lifecycle sweep's positive faces — two
// scratch containers under the Linux root, a dpni created in the first,
// assigned across to the second, plugged, unplugged, and destroyed;
// then both containers destroyed (a dprc destroy requires an empty
// container, which the trace proves by ordering). The refusal faces the
// inventory row also names (non-empty destroy, DPRC-I3 plugged-move)
// are not traceable — a refused action is a disabled action, not a
// step — and stay with the online driver (models/board/README.md).
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same nine-step trace:
//
//   pnpm exec quint run models/board/vdprc1.qnt --main=vdprc1 \
//     --mbt --max-steps=9 --n-traces=1 --max-samples=1 \
//     --out-itf=models/traces/vdprc1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc1 {
  import types.* from "../core/types"
  import params.* from "../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../core/machine"

  // Model ids the run creates, deterministic from the per-family
  // counters (dprc numbering starts at 2, dpni at 0 — the model dpni.0
  // is symbolic; the board names creates itself).
  pure val scratchA: ObjId = { fam: Dprc, num: 2 }
  pure val scratchB: ObjId = { fam: Dprc, num: 3 }
  pure val theDpni: ObjId = { fam: Dpni, num: 0 }

  action init = m::init

  // 1–2. Two scratch containers under the Linux root; the counter guard
  //      stops the action after the second.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) < 4,
      m::createContainerAt(parent),
    }
  }

  // 3. One dpni in scratch A, only once both containers exist.
  action createObject = {
    nondet fam = Set(Dpni).oneOf()
    nondet c = Set(scratchA).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchB),
      m::s.nextNum.get(Dpni) == 0,
      m::createObjectAt(fam, c),
    }
  }

  // 4. Assign the (unplugged) dpni across, A → B; disabled thereafter
  //    because the parent no longer matches.
  action assignChild = {
    nondet o = Set(theDpni).oneOf()
    nondet dst = Set(scratchB).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      m::s.objs.get(theDpni).parent == Some(scratchA),
      m::assignChildAt(o, dst),
    }
  }

  // 5. Plug it in B — only while A still exists, so the unplug phase
  //    below cannot re-enter here.
  action plug = {
    nondet o = Set(theDpni).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      m::s.objs.get(theDpni).parent == Some(scratchB),
      m::s.dprcs.keys().contains(scratchA),
      m::plugAt(o),
    }
  }

  // 7. Unplug — enabled only once A is gone (step 6 below), which is
  //    what serializes the teardown phase.
  action unplug = {
    nondet o = Set(theDpni).oneOf()
    all {
      not(m::s.dprcs.keys().contains(scratchA)),
      m::unplugAt(o),
    }
  }

  // 6/8/9. Destroys, phase-keyed: the now-empty A while the dpni is
  //        plugged in B; the dpni once unplugged; B once empty.
  action destroy = {
    nondet o = Set(scratchA, scratchB, theDpni).oneOf()
    all {
      if (o == scratchA) and {
        m::s.objs.keys().contains(theDpni),
        m::s.objs.get(theDpni).plugged,
      } else if (o == theDpni) and {
        not(m::s.dprcs.keys().contains(scratchA)),
        m::s.objs.keys().contains(theDpni),
        not(m::s.objs.get(theDpni).plugged),
      } else and {
        not(m::s.dprcs.keys().contains(scratchA)),
        not(m::s.objs.keys().contains(theDpni)),
      },
      m::destroyAt(o),
    }
  }

  action step = any { createContainer, createObject, assignChild, plug, unplug, destroy }
}
