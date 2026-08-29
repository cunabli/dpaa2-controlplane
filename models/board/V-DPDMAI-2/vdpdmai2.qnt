// Scenario module for board suite V-DPDMAI-2 (task 5.11; sitting-5.11
// note "a runtime dpdmai across a reboot"): dpdmai.md unknown 4 — whether
// a created dpdmai survives a kernel shutdown. On this BSP no qdma driver
// binds a dpdmai at all (V-LIFE-DPDMAI-1, ADR-0008), so the shutdown path
// never runs and the wrong-token question is unanswerable here; what the
// board can answer is the persistence half — whether a runtime-created
// root resident of this family outlives a reboot, which V-RECOVERY-1
// answered for a container and a dpbp and this suite answers for a
// dpdmai.
//
// The suite is generated with --reboot-persistence: V-RECOVERY-1's
// two-script shape (pre-state capture, no teardown trap — the reboot is
// the teardown — and a post-boot diff that here also asserts the created
// object is gone). The trace is one bare dpdmai created in the root,
// unplugged: nothing for the bus to scan.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same one-step trace:
//
//   pnpm exec quint run models/board/V-DPDMAI-2/vdpdmai2.qnt --main=vdpdmai2 \
//     --mbt --max-steps=1 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPDMAI-2/vdpdmai2.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpdmai2 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val theDpdmai: ObjId = { fam: Dpdmai, num: 0 }

  action init = m::init

  // One bare dpdmai in the Linux root; disabled once it exists, so the
  // trace is exactly one step. Left unplugged (the reboot is its
  // teardown).
  action createObject = {
    nondet fam = Set(Dpdmai).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      not(m::s.objs.keys().contains(theDpdmai)),
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createObject }
}
