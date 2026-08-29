// Scenario module for board suite V-CEIL-1 (task 5.11; sitting-5.11 note
// "MC resource ceilings by create-until-refused"): the MC's pools (bp 63,
// mcp 203, swp 49, fq 1981, cg 253, qd 253, opr 256; V-DPRC-4) and
// whether a create is refused exactly when its pool runs dry, with which
// status, and whether every refused create leaves the census untouched.
// A scratch child with the ALLOC bit draws the rest from the same MC
// pools without starving boot residents (which already hold their draws).
// The model has no notion of a finite pool, so the create-until-refused
// sweep lives in the hook ceil.sh; the trace is the smallest there is —
// one scratch child — so the suite has a teardown and a results
// directory. Every create and destroy the hook issues stays inside that
// child (ADR-0008 §7: a hook never destroys in the root).
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same one-step trace:
//
//   pnpm exec quint run models/board/V-CEIL-1/vceil1.qnt --main=vceil1 \
//     --mbt --max-steps=1 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-CEIL-1/vceil1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vceil1 {
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
