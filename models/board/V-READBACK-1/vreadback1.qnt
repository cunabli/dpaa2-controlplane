// Scenario module for board suite V-READBACK-1 (task 5.7): the read-back
// face the per-family lifecycle suites never took. It bare-creates five
// objects in a scratch container and reads each back with
// `restool <fam> info` to settle the MC defaults those creates leave
// unnamed — DPNI-I7 (a bare dpni's MC defaults), DPDMAI-I5 (the queue
// count MC picks for a bare dpdmai), DPIO-I3's observable half (what MC
// reports as priorities for a DPIO_NO_CHANNEL dpio), DPBP-I5 (does bpid
// equal object id) and DPDCEI-I1's version half (the dpdcei info API
// version). Nothing is plugged and nothing is bound: a child-container
// object is never bus-visible (DPRC-I6), so presence and the read-back
// are the whole observable surface, which is why this face fits a scratch
// container.
//
// Every create is bare except two the restool CLI will not accept bare:
// dpdcei (dpdcei_commands.c mandates --engine/--priority, so it is made
// with DPDCEI_ENGINE_DECOMPRESSION at priority 1) and dpio (mandates
// --channel-mode/--num-priorities). The dpio here is DPIO_NO_CHANNEL,
// rendered through the generator's `--create-args`, not the model —
// channel mode is not a lifecycle attribute the object machine tracks.
//
// The read-back face is the hook readback.sh, sourced after the last
// step and before the generated script's teardown trap (which reclaims
// every object here — the trace destroys nothing).
//
// Guards are chained on the per-family nextNum counters so exactly one
// action is enabled per state; any seed freezes the same six-step trace:
//
//   pnpm exec quint run models/board/V-READBACK-1/vreadback1.qnt --main=vreadback1 \
//     --mbt --max-steps=6 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-READBACK-1/vreadback1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vreadback1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }

  action init = m::init

  // 1. One scratch container under the Linux root.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // 2–6. One create per family, in a fixed order — dpni (the DPNI-I7
  //      default question), dpdmai (DPDMAI-I5's queue count), dpio
  //      (DPIO_NO_CHANNEL via --create-args, DPIO-I3's read-back half),
  //      dpbp (DPBP-I5's bpid-equals-id question), dpdcei (DPDCEI-I1's
  //      version half). A family is due once the one before it exists
  //      and its own counter is still untouched, so the pick set is a
  //      singleton in every state.
  def due(f) = and {
    m::s.nextNum.get(f) == 0,
    if (f == Dpni) true
    else if (f == Dpdmai) m::s.nextNum.get(Dpni) == 1
    else if (f == Dpio) m::s.nextNum.get(Dpdmai) == 1
    else if (f == Dpbp) m::s.nextNum.get(Dpio) == 1
    else m::s.nextNum.get(Dpbp) == 1,
  }

  action createObject = {
    nondet fam = Set(Dpni, Dpdmai, Dpio, Dpbp, Dpdcei).filter(due).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 3,
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createContainer, createObject }
}
