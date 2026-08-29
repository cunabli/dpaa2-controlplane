// Scenario module for board suite V-CONC-1 (task 5.11; sitting-5.11 note
// "two writers and a rapid create/destroy loop"): ADR-0006 assumes one
// initiating writer during a pass and calls a second an operational
// violation, not a modeled transition. Whether that assumption is
// load-bearing at the MC — whether two portals issuing creates and
// destroys into one container can lose an object, corrupt a listing or
// hang a command — is what this run learns. Every restool invocation
// opens its own portal (V-POOL-3's law), so two restool loops are two
// writers. The model has no concurrency notion, so the whole probe lives
// in the hook conc.sh; the trace is the smallest there is — one scratch
// child — so the suite has a teardown and a results directory to write
// into. Every create and destroy the hook issues stays inside that child
// (ADR-0008 §7: a hook never destroys in the root).
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same one-step trace:
//
//   pnpm exec quint run models/board/V-CONC-1/vconc1.qnt --main=vconc1 \
//     --mbt --max-steps=1 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-CONC-1/vconc1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vconc1 {
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
