// Scenario module for board suite V-DPRC-2-TOPO-1 (task 5.10): the
// TOPOLOGY_CHANGES half of the DPRC option-bit permission matrix (dprc.md
// unknowns 3 and 6). The model carries no notion of container options, so
// the mask rides on `--create-args dprc=--options=<mask>` and never enters
// the trace.
//
// The suite builds dprc.2 with restool's default option mask PLUS
// TOPOLOGY_CHANGES (SPAWN,ALLOC,OBJ_CREATE,IRQ_CFG,TOPOLOGY_CHANGES). This
// is the positive control for V-DPCI-1's No-privilege refusal: a
// default-created container's own portal cannot connect anything, which is
// why every generated connect renders against the root ancestor
// (adapter.rs ConnectEdge; README §"Regenerating"). Here dprc.2 holds the
// topology-change privilege, so a connect issued ON dprc.2 should succeed —
// and only the hook can issue it there, because the generator always
// renders the trace's connect against the root.
//
// The trace, in order:
//   1. a scratch dprc.2 under the Linux root;
//   2-3. two dpcis inside dprc.2 (dpci.0, dpci.1) — a same-family pair
//        (connect.legalPair carries (Dpci, Dpci), the corpus's only
//        same-family-only edge);
//   4. a connect of the two dpcis — canConnect places no container gate,
//      so the model accepts the pair; the generator renders it against the
//      root (dprc.1), which holds topology privilege, so the trace connect
//      succeeds and stands the connected pair for the hook.
//
// The hook topo.sh then, on dprc.2's OWN portal (the whole point): it
// disconnects the pair via `dprc disconnect dprc.2 …` and re-connects it
// via `dprc connect dprc.2 …`. PASS if the child-scoped connect succeeds
// (exit 0 and each dpci names the other), which is the finding —
// TOPOLOGY_CHANGES on the container lets its own portal drive connects,
// where a default container is refused No privilege. It leaves the pair
// connected, matching the trace's final state.
//
// dprc.md unknown 2 (options mutable post-create) needs no probe: restool
// has no verb to change a container's options after create.
//
// dpci has no kernel driver, so nothing binds; the connected pair is
// destroyed while connected at teardown (the edge dies with the object, as
// V-DPCI-1 settled), spaced per ADR-0008.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same four-step trace (1 container + 2 dpci creates + 1 connect):
//
//   pnpm exec quint run models/board/V-DPRC-2-TOPO-1/vdprc2topo.qnt \
//     --main=vdprc2topo --mbt --max-steps=4 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-2-TOPO-1/vdprc2topo.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc2topo {
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

  // 2-3. Two dpcis inside dprc.2, serialized by the dpci counter.
  action createObject = {
    nondet fam = Set(Dpci).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchDprc),
      m::s.nextNum.get(Dpci) < 2,
      m::createObjectAt(fam, c),
    }
  }

  // 4. The pair's edge, once both dpcis exist; re-firing is impossible
  //    afterwards (both endpoints already connected).
  action connectEdge = {
    nondet a = Set({ obj: dpciA, port: 0 }).oneOf()
    nondet b = Set({ obj: dpciB, port: 0 }).oneOf()
    all {
      m::s.nextNum.get(Dpci) == 2,
      m::connectEdgeAt(a, b),
    }
  }

  action step = any { createContainer, createObject, connectEdge }
}
