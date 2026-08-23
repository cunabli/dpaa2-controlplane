// Scenario module for board suite V-DPCI-1 (task 5.2; traffic-inventory
// §1): the GPP↔GPP dpci pair — two bare creates in a scratch container,
// the connect (the corpus's only same-family-only edge, DPCI-I1), then
// destroy of one side *while connected*: whether the MC demands
// disconnect first is open (dpci.md unknown #2) and the model holds
// that edges vanish with the object — a board refusal here amends that.
// The read-back after the destroy also shows what the surviving peer's
// endpoint reports. The asymmetric-priority and rescan-visibility faces
// of the inventory row stay with the online driver
// (models/board/README.md).
//
// Neither dpci is plugged: connect carries no plug-state guard (§2,
// connect-then-plug and plug-then-connect both legal), and dpci has no
// kernel driver to probe.
//
// The connect names the root container, not the scratch one the pair
// lives in. `dprc connect` runs on whichever container it is given, and
// a scratch container restool created with default options carries no
// topology-change privilege, so the MC refuses the connect there (board:
// No privilege, status 0x4). restool only asks for a common ancestor,
// and the root is the ancestor that holds the privilege.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same seven-step trace:
//
//   pnpm exec quint run models/board/V-DPCI-1/vdpci1.qnt --main=vdpci1 \
//     --mbt --max-steps=7 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPCI-1/vdpci1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpci1 {
  import types.* from "../../core/types"
  import connect.* from "../../core/connect"
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

  // 2–3. The pair, bare creates — the counter serializes them.
  action createObject = {
    nondet fam = Set(Dpci).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.nextNum.get(Dpci) < 2,
      m::createObjectAt(fam, c),
    }
  }

  // 4. The pair's edge; re-firing is impossible once A is destroyed
  //    (a connect endpoint must exist) and before that by cardinality
  //    (both endpoints already connected).
  action connectEdge = {
    nondet a = Set({ obj: dpciA, port: 0 }).oneOf()
    nondet b = Set({ obj: dpciB, port: 0 }).oneOf()
    m::connectEdgeAt(a, b)
  }

  // 5. Destroy A while its edge stands (the dpci.md unknown #2 probe).
  // 6. Destroy the surviving B.
  // 7. Destroy the emptied container.
  action destroy = {
    nondet o = Set(scratchDprc, dpciA, dpciB).oneOf()
    all {
      if (o == dpciA) objConnected(m::s, dpciA)
      else if (o == dpciB) and {
        m::s.nextNum.get(Dpci) == 2,
        not(m::s.objs.keys().contains(dpciA)),
        m::s.objs.keys().contains(dpciB),
      } else and {
        m::s.nextNum.get(Dpci) == 2,
        not(m::s.objs.keys().contains(dpciB)),
      },
      m::destroyAt(o),
    }
  }

  action step = any { createContainer, createObject, connectEdge, destroy }
}
