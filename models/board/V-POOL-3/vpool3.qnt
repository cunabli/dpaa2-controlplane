// Scenario module for board suite V-POOL-3 (task 5.11; sitting-5.11 note
// "the uapi opener law"): DPMCP-I2 — N simultaneous openers of
// /dev/dprc.1 need N-1 free dpmcps in the root pool (the first opener
// rides the root's own portal), and exhaustion is open() failing with
// ENXIO. This scenario is inherently root-side: the uapi is the root
// container's device. Nothing in the model expresses opening a portal, so
// the whole probe lives in the hook pool3.sh; the trace is the smallest
// there is — one scratch child — so the suite has a teardown and a
// results directory to write into.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same one-step trace:
//
//   pnpm exec quint run models/board/V-POOL-3/vpool3.qnt --main=vpool3 \
//     --mbt --max-steps=1 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-POOL-3/vpool3.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vpool3 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }

  action init = m::init

  // One scratch container under the Linux root; disabled once it exists,
  // so the trace is exactly one step.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      not(m::s.dprcs.keys().contains(scratchDprc)),
      m::createContainerAt(parent),
    }
  }

  action step = any { createContainer }
}
