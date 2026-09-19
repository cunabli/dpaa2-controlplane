// Scenario module for board suite V-DPNI-9 (dpni-typestate task 5.1;
// dpni-typestate design D7, per-step learning session): the
// `DPNI_OPT_HAS_REPLICATION` (0x4000) accept/reject probe
// (docs/baseline/dpni.md unknown-register #8) — restool's option map
// knows the flag, but the MC 10.39.0 flib's 14-flag header does not list
// it, so whether the firmware accepts a create carrying 0x4000 is
// unknown. The trace stands up one scratch container only; the probe
// create with the raw bit lives in the hook replication.sh, because a
// create whose outcome (accept or reject) is unknown cannot be traced —
// a refused action is a disabled action, not a step, and marking it an
// expected refusal would fail on acceptance and vice-versa. The hook
// records whichever the MC does as the observed verdict and continues
// cleanup: the run is not failed by the refusal itself (mbt-harness
// spec "A probe refusal is an answer"; dpni-typestate design D7).
//
// Freeze command (`--n-traces` <= `--max-samples`; a directed .then run
// carries no nondet picks and is not usable generator input):
//
//   pnpm exec quint run models/board/V-DPNI-9/vdpni9.qnt --main=vdpni9 \
//     --mbt --max-steps=1 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPNI-9/vdpni9.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpni9 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  action init = m::init

  // 1. One scratch container — the 0x4000 probe create/destroy is the
  //    hook's, since its outcome is unknown and cannot be a trace step.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  action step = any { createContainer }
}
