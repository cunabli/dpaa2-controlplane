// Scenario module for board suite V-DPNI-5 (dpni-typestate task 5.1;
// dpni-typestate design D7, per-step learning session): the PMD option profile
// (docs/baseline/dpni.md production-profiles list, [verified in use]) —
// `DPNI_OPT_SINGLE_SENDER, CUSTOM_CG, HAS_KEY_MASKING, HAS_OPR,
// OPR_PER_TC, 0x80000000` at 16 queues / 16 TCs, num_cgs = queues + 8 =
// 24 — created in a scratch container and read back. The option mask and
// the sizes ride the generator's `--create-args dpni=…`, not the core
// machine (options are not a lifecycle attribute the object machine
// tracks, adapter.rs `create_args`); the `dpni info` read-back is the
// hook readback.sh, the only observation (dpni-typestate design D7). A child-container
// dpni never binds (DPRC-I6), so presence plus the attribute read-back
// are the whole observable surface.
//
// Freeze command (`--n-traces` <= `--max-samples`; a directed .then run
// carries no nondet picks and is not usable generator input):
//
//   pnpm exec quint run models/board/V-DPNI-5/vdpni5.qnt --main=vdpni5 \
//     --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPNI-5/vdpni5.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpni5 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }

  action init = m::init

  // 1. One scratch container under the Linux root.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // 2. The PMD-profile dpni create (options and sizes via --create-args).
  action createObject = {
    nondet fam = Set(Dpni).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.nextNum.get(Dpni) == 0,
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createContainer, createObject }
}
