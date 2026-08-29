// Scenario module for board suite V-DPRC-2-PL-1 (task 5.10): the
// PL_ALLOWED bit of the DPRC option-bit permission matrix (dprc.md
// unknowns 3 and 6). The model carries no notion of container options, so
// the mask rides on `--create-args dprc=--options=<mask>` and never enters
// the trace.
//
// The suite builds dprc.2 with restool's default option mask PLUS
// PL_ALLOWED (SPAWN,ALLOC,OBJ_CREATE,IRQ_CFG,PL_ALLOWED). PL_ALLOWED
// (physical-layer/policer access) neither adds nor removes a create-class
// privilege, so the trace is an ordinary accepted lifecycle: dprc.2 and a
// dpbp in it both created without complaint. The finding is not in the
// trace but in what the container's decoded options read back as — the
// hook's job.
//
// The trace is two creates:
//   1. a scratch dprc.2 under the Linux root;
//   2. a dpbp inside dprc.2 — a bare, accepted create (PL_ALLOWED does not
//      gate it); no `--expect-refusal`, so its read-back is kept.
//
// The hook pl.sh then reads `dprc info dprc.2 --verbose` and RECORDs the
// decoded options line: whether PL_ALLOWED reads back set, and whether any
// other bit changed. Purely a capture — the by-eye judgement of the option
// echo is done offline, so the hook only records and PASSes on the read
// exiting 0.
//
// dprc.md unknown 2 (options mutable post-create) needs no probe: restool
// has no verb to change a container's options after create, so this suite
// only observes the create-time mask.
//
// The dpbp is created unplugged in a child container, so it is never
// bus-visible or driver-bound (DPRC-I6); the teardown destroys it and then
// dprc.2 cleanly.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same two-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-2-PL-1/vdprc2pl.qnt \
//     --main=vdprc2pl --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-2-PL-1/vdprc2pl.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc2pl {
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

  // 2. One dpbp inside dprc.2 — an ordinary accepted create (PL_ALLOWED
  //    gates nothing here); the hook reads the container's options back.
  action createObject = {
    nondet fam = Set(Dpbp).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchDprc),
      m::s.nextNum.get(Dpbp) == 0,
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createContainer, createObject }
}
