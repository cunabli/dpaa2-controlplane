// Scenario module for board suite V-DPNI-6 (dpni-typestate task 5.1;
// dpni-typestate design D7, per-step learning session): the kernel
// option profile (docs/baseline/dpni.md production-profiles list,
// [verified in use]) — `DPNI_OPT_HAS_KEY_MASKING` only, 1 queue / 1 TC,
// PFDR_IN_PEB and SINGLE_SENDER deliberately absent so the kernel side
// tx's freely — created in a scratch container and read back. The option
// mask and the sizes ride the generator's `--create-args dpni=…`
// (options are not a lifecycle attribute the object machine tracks,
// adapter.rs `create_args`); the `dpni info` read-back is the hook
// readback.sh, the only observation (dpni-typestate design D7).
//
// Freeze command (`--n-traces` <= `--max-samples`; a directed .then run
// carries no nondet picks and is not usable generator input):
//
//   pnpm exec quint run models/board/V-DPNI-6/vdpni6.qnt --main=vdpni6 \
//     --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPNI-6/vdpni6.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpni6 {
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

  // 2. The kernel-profile dpni create (options and sizes via --create-args).
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
