// Scenario module for board suite V-DPNI-7 (dpni-typestate task 5.1;
// dpni-typestate design D7, per-step learning session): the unread
// option flags (docs/baseline/dpni.md unknown-register #6) —
// `DPNI_OPT_TX_FRM_RELEASE, HAS_POLICING, SHARED_CONGESTION`, which no
// consumer in the corpus reads. A dpni carrying all three is created in a
// scratch container and read back: does the MC accept them and how does
// `dpni info` render them. The mask rides the generator's `--create-args
// dpni=…` (options are not a lifecycle attribute the object machine
// tracks, adapter.rs `create_args`); the `dpni info` read-back is the
// hook readback.sh, the only observation (dpni-typestate design D7).
//
// Freeze command (`--n-traces` <= `--max-samples`; a directed .then run
// carries no nondet picks and is not usable generator input):
//
//   pnpm exec quint run models/board/V-DPNI-7/vdpni7.qnt --main=vdpni7 \
//     --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPNI-7/vdpni7.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpni7 {
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

  // 2. The dpni carrying the three unread flags (mask via --create-args).
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
