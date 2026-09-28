// Scenario module for board suite V-DPIO-1 (pool-objects task 4.1;
// pool-objects design D5 root-bind floor): the dpio seat/priority surface —
// DPIO-I3's kernel half as far as it reaches at ROOT scope. Reads the boot
// kernel dpios' seat/priority surface, creates one scratch DPIO_NO_CHANNEL
// dpio at 8 priorities in dprc.1, reads `dpio info` back (num_priorities is
// reported mode-independently — the V-READBACK-1 settled half), and observes
// what the kernel face reports for it.
//
// DPIO-I3's kernel half (what the fsl_mc_dpio seat driver does with a
// NO_CHANNEL dpio's priorities) is expected UNREACHABLE at root: the boot
// layout fills every per-CPU seat, so a dpio created at runtime never binds
// (ADR-0008; V-LIFE-DPIO-1). The hook names that unreachable face explicitly
// (the loud-not-silent rule, pool-objects spec) — it does not take the
// DPL-child path (bead dpaa2-controlplane-960.13, separately gated).
//
// Channel mode and priority count are not lifecycle attributes the object
// machine tracks (V-READBACK-1), so they ride the generator's `--create-args`
// (`dpio=--channel-mode=DPIO_NO_CHANNEL --num-priorities=8`), not the model.
// The read-back and boot-dpio surface are the hook seats.sh; the trace is one
// create, so the suite has a teardown and a results directory.
//
// Guards leave exactly one action enabled per state, so any seed freezes the
// same one-step trace:
//
//   pnpm exec quint run models/board/V-DPIO-1/vdpio1.qnt --main=vdpio1 \
//     --mbt --max-steps=1 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPIO-1/vdpio1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpio1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  action init = m::init

  // One dpio in the Linux root (dprc.1). Singleton picks (fam, c) so the mbt
  // records them; disabled once it exists, so the trace is one step.
  action createObject = {
    nondet fam = Set(Dpio).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dpio) == 0,
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createObject }
}
