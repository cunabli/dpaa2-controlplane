// Scenario module for board suite V-DPNI-8 (dpni-typestate task 5.1;
// dpni-typestate design D7, per-step learning session): the sizing-field
// probe walks (docs/baseline/dpni.md unknown-register #3) — the
// semantics of `num_cgs`, `num_opr` and `dist_key_size`, present in
// `dpni_cfg` but absent from its doc block, and the rationale of the
// deployed `num_cgs = num_queues + 8` heuristic. The trace stands up one
// scratch container only; the bracketing create/read-back/destroy walk
// over each field lives in the hook sizing.sh, because the varying create
// values cannot ride a single per-family `--create-args` (adapter.rs
// `create_args` is one shape per family). `dist_key_size` is write-only
// (absent from `dpni_attr`, dpni-typestate design D4; DPNI-I12), so its
// probe records that it does not read back. Read-back is the only
// observation, and a mid-walk MC refusal is an answer, not a failure
// (dpni-typestate design Risks).
//
// Freeze command (`--n-traces` <= `--max-samples`; a directed .then run
// carries no nondet picks and is not usable generator input):
//
//   pnpm exec quint run models/board/V-DPNI-8/vdpni8.qnt --main=vdpni8 \
//     --mbt --max-steps=1 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPNI-8/vdpni8.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpni8 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  action init = m::init

  // 1. One scratch container — the walk's create/read-back/destroy cycles
  //    are the hook's, since each carries a different sizing value.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  action step = any { createContainer }
}
