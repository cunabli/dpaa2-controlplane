// Scenario module for board suite V-DPRC-2-NOSPAWN-1 (task 5.10): the
// SPAWN half of the DPRC option-bit permission matrix (dprc.md unknowns 3
// and 6). The model carries no notion of container options, so the mask
// rides on `--create-args dprc=--options=<mask>` and never enters the
// trace. Create-args are per family, so BOTH dprc creates below render the
// same mask.
//
// The suite builds dprc.2 with restool's default option mask minus SPAWN
// (ALLOC,OBJ_CREATE,IRQ_CFG). Child-container creation is thus stripped
// from dprc.2, so creating a nested dprc.3 under it is refused — the
// finding is that DPRC_CFG_OPT_SPAWN_ALLOWED is the gate on
// `dprc create <child>`.
//
// The trace is two container creates:
//   1. a scratch dprc.2 under the Linux root (which holds SPAWN, so it is
//      created; the mask it is created WITH lacks SPAWN);
//   2. a nested dprc.3 under dprc.2 — the create the board refuses;
//      generated with `--expect-refusal 1="Configuration error"` (0-based
//      step index; the refused createContainer is step 1), which drops that
//      step's model read-back and judges it by the nonzero exit and the
//      captured MC status. Rev 1 finding (2026-08-29): the board refused
//      with "Configuration error" (0x6), NOT "No privilege" — the SPAWN
//      option-bit refusal surfaces as a configuration error, not a privilege
//      one. "Configuration error" is the exact status name in
//      docs/baseline/mc-status.md (0x6).
//
// The model admits the nesting: a newly created dprc keeps default perms
// (Spawn, Alloc, ObjCreate, IrqCfg), so `createContainerAt(dprc.2)` is
// enabled in the model even though the board's dprc.2 was built without
// SPAWN. That mismatch is exactly what the board refusal records.
//
// dprc.md unknown 2 (options mutable post-create) needs no probe: restool
// has no verb to change a container's options after create.
//
// Nothing here is plugged or bus-visible (DPRC-I6). On the board the
// refused create binds no object, so the teardown's guarded destroy is a
// no-op for dprc.3 and then destroys dprc.2 cleanly.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same two-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-2-NOSPAWN-1/vdprc2nospawn.qnt \
//     --main=vdprc2nospawn --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-2-NOSPAWN-1/vdprc2nospawn.itf.json
//
// Generated (rev 2) with:
//
//   cargo run -p dpaa2-verify -- generate \
//     --trace models/board/V-DPRC-2-NOSPAWN-1/vdprc2nospawn.itf.json \
//     --id V-DPRC-2-NOSPAWN-1 --out models/board/V-DPRC-2-NOSPAWN-1 \
//     --create-args 'dprc=--options=DPRC_CFG_OPT_ALLOC_ALLOWED,DPRC_CFG_OPT_OBJ_CREATE_ALLOWED,DPRC_CFG_OPT_IRQ_CFG_ALLOWED' \
//     --expect-refusal '1=Configuration error'
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc2nospawn {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }

  action init = m::init

  // Both container creates, serialized by the dprc counter into one
  // `createContainer` action so the MBT trace names the machine's own
  // wrapper: dprc.2 under the root first (step 0), then the nested dprc.3
  // under dprc.2 (step 1) — the create the board (SPAWN stripped)
  // refuses. The parent is forced per counter value so exactly one pick
  // is enabled per state. The model admits the nesting because dprc.2
  // keeps default Spawn; the board refusal is the whole finding.
  action createContainer = {
    nondet parent = Set(m::dprc1, scratchDprc).oneOf()
    all {
      m::s.nextNum.get(Dprc) < 4,
      if (m::s.nextNum.get(Dprc) == 2) parent == m::dprc1 else parent == scratchDprc,
      m::createContainerAt(parent),
    }
  }

  action step = any { createContainer }
}
