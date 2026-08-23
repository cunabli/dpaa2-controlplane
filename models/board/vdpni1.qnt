// Scenario module for board suite V-DPNI-1 (task 5.2; traffic-inventory
// §1): the bare-create default probe — `dpni create` with no options in
// a scratch container (the read-back probes capture the defaults in the
// results for DPNI-I7 analysis), plugged, then destroyed *while
// plugged*: the model holds that restool's in_use check reads
// root-container sysfs only, so a plugged child-container dpni destroys
// cleanly (destroyAt; the dpdmux.md race note). A board refusal here is
// a discovery that amends that guard.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same five-step trace:
//
//   pnpm exec quint run models/board/vdpni1.qnt --main=vdpni1 \
//     --mbt --max-steps=5 --n-traces=1 --max-samples=1 \
//     --out-itf=models/traces/vdpni1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpni1 {
  import types.* from "../core/types"
  import params.* from "../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }
  pure val theDpni: ObjId = { fam: Dpni, num: 0 }

  action init = m::init

  // 1. One scratch container under the Linux root.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // 2. The bare dpni create (no options — MC defaults, DPNI-I7).
  action createObject = {
    nondet fam = Set(Dpni).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 0,
      m::createObjectAt(fam, c),
    }
  }

  // 3. Plug it (no kernel probe follows: a child-container object is
  //    never bus-visible, DPRC-I6).
  action plug = {
    nondet o = Set(theDpni).oneOf()
    m::plugAt(o)
  }

  // 4. Destroy while plugged — the child-container face of the in_use
  //    blindness; enabled only after the plug by the plugged guard.
  // 5. Destroy the emptied container.
  action destroy = {
    nondet o = Set(scratchDprc, theDpni).oneOf()
    all {
      if (o == theDpni) and {
        m::s.objs.keys().contains(theDpni),
        m::s.objs.get(theDpni).plugged,
      } else and {
        // Only after the dpni existed and is gone again — the counter
        // is the monotone witness that step 4 already happened.
        m::s.nextNum.get(Dpni) == 1,
        not(m::s.objs.keys().contains(theDpni)),
      },
      m::destroyAt(o),
    }
  }

  action step = any { createContainer, createObject, plug, destroy }
}
