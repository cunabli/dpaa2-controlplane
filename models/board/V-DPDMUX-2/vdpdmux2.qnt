// Scenario module for board suite V-DPDMUX-2 (task 5.9): DPDMUX-I8's
// refusal face — the demux uplink (interface 0) may only face a dpmac; a
// dpni belongs on a downlink. The model forbids the illegal pairing
// outright (core/connect.qnt legalPorts refuses interface 0 to anything
// but a dpmac), so the refused connect is not a trace step — a disabled
// action never is — and it lives in the hook uplink.sh instead.
//
// The trace only stands the two objects the hook needs: a dpdmux (created
// with restool's mandatory --num-ifs=1, one downlink plus the uplink) and
// a dpni, both in the Linux root, neither plugged and nothing connected —
// two steps. The hook then issues both the illegal uplink connect (to
// prove it is refused and leaves interface 0 unconnected) and, as its
// control, the legal downlink connect to the same dpni (to prove the port
// rule is what refused the first, not the objects). Reading a per-interface
// connection line back is the whole observation, so presence in the root
// container is all the trace has to establish.
//
// The objects live in dprc.1 so the hook's connects render on the root's
// own handle, the one container guaranteed to hold the topology-change
// privilege (a default-created child does not, and its connect comes back
// No privilege — V-DPCI-1 rev 1). There are no destroy steps; the suite's
// unconditional teardown trap destroys both objects, newest first.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same two-step trace:
//
//   pnpm exec quint run models/board/V-DPDMUX-2/vdpdmux2.qnt --main=vdpdmux2 \
//     --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPDMUX-2/vdpdmux2.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpdmux2 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val theDpdmux: ObjId = { fam: Dpdmux, num: 0 }
  pure val theDpni: ObjId = { fam: Dpni, num: 0 }

  action init = m::init

  // 1. The demux first, 2. the dpni once it stands. A family is due once
  //    the one before it exists and its own counter is untouched, so the
  //    pick set is a singleton in every state.
  action createObject = {
    nondet fam = Set(Dpdmux, Dpni).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      if (fam == Dpdmux) m::s.nextNum.get(Dpdmux) == 0
      else and {
        m::s.nextNum.get(Dpdmux) == 1,
        m::s.nextNum.get(Dpni) == 0,
      },
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createObject }
}
