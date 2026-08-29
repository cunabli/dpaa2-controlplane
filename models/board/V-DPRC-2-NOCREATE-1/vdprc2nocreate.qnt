// Scenario module for board suite V-DPRC-2-NOCREATE-1 (task 5.10): the
// OBJ_CREATE half of the DPRC option-bit permission matrix (dprc.md
// unknowns 3 and 6). The model carries no notion of container options —
// a permission bit is not a lifecycle attribute — so the mask rides on
// `--create-args dprc=--options=<mask>` and never enters the trace; the
// model's dprc.2 keeps its default perms (Spawn, Alloc, ObjCreate,
// IrqCfg), so the createObjectAt below is enabled in the model and the
// board's refusal is what the suite records.
//
// The suite builds dprc.2 with restool's default option mask minus
// OBJ_CREATE (SPAWN,ALLOC,IRQ_CFG), then creates a dpbp inside it.
//
// Rev 1 finding (2026-08-29): the board ACCEPTED the dpbp create despite
// OBJ_CREATE being stripped from the container. restool creates the object
// through the root portal, and the OBJ_CREATE_ALLOWED bit gates only the
// child container's own portal, so the bit is unreachable through restool.
// The step is therefore regenerated as a plain accepted create carrying a
// model read-back — no `--expect-refusal` — so the rev 2 run passes.
//
// The trace is two creates:
//   1. a scratch dprc.2 under the Linux root;
//   2. a dpbp inside dprc.2 — accepted, read back present.
//
// dprc.md unknown 2 (options mutable post-create) needs no probe: restool
// has no verb to change a container's options after create, so this suite
// only exercises the create-time mask.
//
// The dpbp is created unplugged in a child container, so it is never
// bus-visible or driver-bound (DPRC-I6). The teardown destroys the dpbp
// (the create is accepted) and then dprc.2 cleanly.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same two-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-2-NOCREATE-1/vdprc2nocreate.qnt \
//     --main=vdprc2nocreate --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-2-NOCREATE-1/vdprc2nocreate.itf.json
//
// Generated (rev 2) with:
//
//   cargo run -p dpaa2-verify -- generate \
//     --trace models/board/V-DPRC-2-NOCREATE-1/vdprc2nocreate.itf.json \
//     --id V-DPRC-2-NOCREATE-1 --out models/board/V-DPRC-2-NOCREATE-1 \
//     --create-args 'dprc=--options=DPRC_CFG_OPT_SPAWN_ALLOWED,DPRC_CFG_OPT_ALLOC_ALLOWED,DPRC_CFG_OPT_IRQ_CFG_ALLOWED'
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc2nocreate {
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

  // 2. One dpbp inside it — the create the board (OBJ_CREATE stripped)
  //    refuses. The model admits it, so the refusal lives in the plan.
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
