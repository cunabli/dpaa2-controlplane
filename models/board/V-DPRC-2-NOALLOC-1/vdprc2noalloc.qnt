// Scenario module for board suite V-DPRC-2-NOALLOC-1 (task 5.10): the
// ALLOC half of the DPRC option-bit permission matrix (dprc.md unknowns 3
// and 6). The model carries no notion of container options, so the mask
// rides on `--create-args dprc=--options=<mask>` and never enters the
// trace; the model's dprc.2 keeps its default perms, so the createObjectAt
// below is enabled in the model and the board is what settles the question.
//
// The suite builds dprc.2 with restool's default option mask minus ALLOC
// (SPAWN,OBJ_CREATE,IRQ_CFG). The dpbp created inside it is a pooled,
// buffer-pool-drawing family (dpbp.md), so ALLOC is the gate on its create.
//
// Rev 1 finding (2026-08-29): the board REFUSED the dpbp create with "No
// resources" (0x8) — a container without ALLOC cannot draw the object id
// from its parent's pool, so the option-bit refusal surfaces as a resource
// shortfall, not a privilege one. The step is regenerated with
// `--expect-refusal 1="No resources"`, which drops its model read-back and
// judges it by the nonzero exit and the captured MC status.
//
// The trace is two creates:
//   1. a scratch dprc.2 under the Linux root;
//   2. a dpbp inside dprc.2 — the refused create; step index 1 (0-based),
//      so `--expect-refusal 1="No resources"`. "No resources" is the exact
//      status name in docs/baseline/mc-status.md (0x8).
//
// dprc.md unknown 2 (options mutable post-create) needs no probe: restool
// has no verb to change a container's options after create.
//
// The dpbp is created unplugged in a child container, so it is never
// bus-visible or driver-bound (DPRC-I6). The refused create binds no
// object, so the teardown's guarded destroy is a no-op for it and then
// destroys dprc.2 cleanly.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same two-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-2-NOALLOC-1/vdprc2noalloc.qnt \
//     --main=vdprc2noalloc --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-2-NOALLOC-1/vdprc2noalloc.itf.json
//
// Generated (rev 2) with:
//
//   cargo run -p dpaa2-verify -- generate \
//     --trace models/board/V-DPRC-2-NOALLOC-1/vdprc2noalloc.itf.json \
//     --id V-DPRC-2-NOALLOC-1 --out models/board/V-DPRC-2-NOALLOC-1 \
//     --create-args 'dprc=--options=DPRC_CFG_OPT_SPAWN_ALLOWED,DPRC_CFG_OPT_OBJ_CREATE_ALLOWED,DPRC_CFG_OPT_IRQ_CFG_ALLOWED' \
//     --expect-refusal '1=No resources'
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc2noalloc {
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

  // 2. One dpbp (a pooled, buffer-pool-drawing family) inside dprc.2 —
  //    the create whose acceptance-or-refusal under a no-ALLOC mask is the
  //    finding. The model admits it, so the prediction lives in the plan.
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
