// Scenario module for board suite V-LINK-1 (task 5.3; traffic-inventory
// §1): the dpci pair's liveness face — does a dpci link come up on the
// connect alone, or only once both ends have been enabled? (DPCI-I5;
// dpci.md unknown 1.)
//
// restool never enables anything, so a batch trace's connect *is* the
// "connect alone" experiment: two bare dpcis in a scratch container, the
// edge, and nothing else. Neither dpci is ever plugged and no kernel
// consumer exists, so nothing on the board can have enabled either end
// by the time the read-back runs. V-DPCI-1 already anchored that
// connecting unplugged endpoints is legal.
//
// The expectation is link *down*: the model gives every object linkUp
// false at create and connectEdge sets no link state at all, which is
// exactly what DPCI_I5Test asserts in models/main.qnt. A board that
// reports the link up here refutes DPCI-I5 — connection alone would then
// carry liveness — and the model is amended to raise linkUp on connect.
//
// The connect names the root container, not the scratch one the pair
// lives in: a scratch container restool created with default options
// carries no topology-change privilege and the MC refuses the connect
// there (V-DPCI-1's board finding).
//
// There are no destroy steps; the suite's unconditional teardown trap
// removes the pair and its container whether the run passed or not.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same four-step trace:
//
//   pnpm exec quint run models/board/V-LINK-1/vlink1.qnt --main=vlink1 \
//     --mbt --max-steps=4 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-LINK-1/vlink1.itf.json
//
// Named invariants: none
// TLA+ escalation: none
module vlink1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }
  pure val dpciA: ObjId = { fam: Dpci, num: 0 }
  pure val dpciB: ObjId = { fam: Dpci, num: 1 }

  action init = m::init

  // 1. One scratch container under the Linux root.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // 2-3. The pair, bare creates — the counter serializes them.
  action createObject = {
    nondet fam = Set(Dpci).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.nextNum.get(Dpci) < 2,
      m::createObjectAt(fam, c),
    }
  }

  // 4. The edge, and the whole experiment: A's read-back reports both
  //    the peer and A's link status. Re-firing is impossible — both
  //    endpoints are connected by cardinality.
  action connectEdge = {
    nondet a = Set({ obj: dpciA, port: 0 }).oneOf()
    nondet b = Set({ obj: dpciB, port: 0 }).oneOf()
    m::connectEdgeAt(a, b)
  }

  action step = any { createContainer, createObject, connectEdge }
}
