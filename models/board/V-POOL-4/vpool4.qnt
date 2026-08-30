// Scenario module for board suite V-POOL-4 (task 5.12; sitting-5.12 note
// "the companion draw, measured"): what a dpio and a dpni create draw
// from the MC pools of `dprc show mc.global --resources` — the `mcp` line
// above all — and whether every draw returns on destroy. The model has no
// notion of a finite pool, so the per-create readings live in the hook
// pool4.sh; the trace is the smallest there is — one scratch child under
// dprc.1 — so the suite has a teardown and a results directory. Every
// create and destroy the hook issues stays inside that child (ADR-0008 §7:
// a hook never destroys in the root).
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same one-step trace:
//
//   pnpm exec quint run models/board/V-POOL-4/vpool4.qnt --main=vpool4 \
//     --mbt --max-steps=1 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-POOL-4/vpool4.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vpool4 {
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
