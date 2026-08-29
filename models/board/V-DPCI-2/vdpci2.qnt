// Scenario module for board suite V-DPCI-2 (task 5.10): dpci.md unknowns
// 3, 4 and 5 — same-container connect in the root (unknown 3), the
// per-container dpci ceiling (unknown 4), and mismatched-priority pairing
// (unknown 5). The model has no dpci ceiling and no priority attribute, so
// the trace drives the walk the board is expected to complete and the
// hook carries the mismatched-priority connect; a board refusal in either
// is the finding.
//
// The trace, in order:
//   1. a scratch dprc.2 under the Linux root;
//   2. sixteen dpci creates inside dprc.2 (dpci.0..dpci.15) — the bounded
//      ceiling walk (unknown 4). The model admits all sixteen; if the
//      board refuses the k-th (k < 16) that create step diverges and the
//      finding is the ceiling, with the teardown coping either way;
//   3. two dpci creates inside dprc.1, the root (dpci.16, dpci.17);
//   4. a connect of the two root dpcis — a same-family pair in one
//      container, issued (like every generated connect) against the root
//      ancestor (unknown 3).
// Every dpci is created at num-priorities=2 via
// `--create-args dpci=--num-priorities=2`; the count is not a model
// attribute, so it rides the create flag, not the trace.
//
// The hook pair.sh then creates ONE fixture dpci at num-priorities=1 in
// dprc.2 — a create is allowed there because the CONNECT is the thing
// under test, not the create (ADR-0007 §3) — connects it on the root to
// the 2-priority dpci.0, reads both ends back (unknown 5), then
// disconnects it. The fixture is never destroyed by the hook: it is an
// owned resident of dprc.2 and dies with the container at teardown.
//
// The model accepts the root pair: connect.legalPair carries (Dpci, Dpci)
// — the corpus's only same-family-only edge — and canConnect places no
// container gate, so two dpcis in one container may be paired. dpci has no
// kernel driver, so nothing binds; the connected root pair is destroyed
// while connected at teardown (the edge dies with the object, as V-DPCI-1
// settled), spaced per ADR-0008.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same twenty-step trace (1 container + 18 dpci creates + 1 connect):
//
//   pnpm exec quint run models/board/V-DPCI-2/vdpci2.qnt --main=vdpci2 \
//     --mbt --max-steps=20 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPCI-2/vdpci2.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpci2 {
  import types.* from "../../core/types"
  import connect.* from "../../core/connect"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }
  // The two root dpcis: dpci.16 and dpci.17 (the first sixteen fill
  // dprc.2). They are the same-container pair unknown 3 probes.
  pure val rootDpciA: ObjId = { fam: Dpci, num: 16 }
  pure val rootDpciB: ObjId = { fam: Dpci, num: 17 }

  action init = m::init

  // 1. One scratch container under the Linux root.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // 2-3. Eighteen dpci creates, serialized by the counter: the first
  //      sixteen into dprc.2 (the ceiling walk), the last two into the
  //      root. The container is forced per counter value so exactly one
  //      (fam, c) pick is enabled per state.
  action createObject = {
    nondet fam = Set(Dpci).oneOf()
    nondet c = Set(scratchDprc, m::dprc1).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchDprc),
      m::s.nextNum.get(Dpci) < 18,
      if (m::s.nextNum.get(Dpci) < 16) c == scratchDprc else c == m::dprc1,
      m::createObjectAt(fam, c),
    }
  }

  // 4. The root pair's edge, only once both root dpcis exist; re-firing
  //    is impossible afterwards (both endpoints already connected).
  action connectEdge = {
    nondet a = Set({ obj: rootDpciA, port: 0 }).oneOf()
    nondet b = Set({ obj: rootDpciB, port: 0 }).oneOf()
    all {
      m::s.nextNum.get(Dpci) == 18,
      m::connectEdgeAt(a, b),
    }
  }

  action step = any { createContainer, createObject, connectEdge }
}
