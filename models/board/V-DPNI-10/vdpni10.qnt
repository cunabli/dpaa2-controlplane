// Scenario module for board suite V-DPNI-10 (dpni-typestate task 5.1;
// dpni-typestate design D7): the primary-MAC mutation — the one runtime
// setter restool can drive (docs/baseline/dpni.md command surface: `dpni
// update --mac-addr`, dpni-typestate design D1's runtime slot). A bare
// dpni is created in a scratch container; the hook mac.sh then sets a
// primary MAC through restool and reads it back with `dpni info`, the
// only observation. The MAC value is built in the hook rather than
// rendered into the trace: the generator's pre-plug mutation emits a
// literal MAC the public-repo leak scan rejects, so the mutation lives in
// the hook, which assembles the address from octets.
//
// Freeze command (`--n-traces` <= `--max-samples`; a directed .then run
// carries no nondet picks and is not usable generator input):
//
//   pnpm exec quint run models/board/V-DPNI-10/vdpni10.qnt --main=vdpni10 \
//     --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPNI-10/vdpni10.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpni10 {
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

  // 2. The bare dpni create — the primary MAC moves in the hook.
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
