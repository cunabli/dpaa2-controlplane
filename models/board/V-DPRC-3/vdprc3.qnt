// Scenario module for board suite V-DPRC-3 (task 5.10): DPRC-I11 and
// dprc.md unknown 4 — set-locked strips the create/destroy/assign/
// unassign/lock class from a container's whole sub-hierarchy, and who may
// unlock it. Lock/unlock round-trips leave no monotone state a
// deterministic trace can phase, and the interesting half is the refusal
// under lock anyway, so the lock cycle lives entirely in the hook. The
// trace only stands the state the hook locks and probes.
//
// The trace is two creates:
//   1. a scratch dprc.2 under the Linux root;
//   2. a dpbp inside dprc.2, left UNPLUGGED (no plug step) — the object
//      whose plug and set-label the hook tries under lock, and whose
//      read-back proves the strip.
//
// The hook lock.sh then, in order: (a) locks dprc.2 from the root; (b)
// tries to plug the dpbp — expected refused, status recorded, the pass is
// read back from the dpbp still unplugged (not asserted on the status,
// which the register predicts but this suite confirms); (c) tries
// set-label on the dpbp — accepted under the lock (the rev 1 sitting
// showed labels are not stripped, only assign is), status recorded, label
// read back present; (d) shows dprc.2 and reads the dpbp back — reads survive
// under lock; (e) unlocks, re-issues the plug (now it succeeds), then
// unplugs so the trace's final unplugged state stands; (f) unconditionally
// unlocks again (idempotent) so the teardown's destroy is never blocked by
// a left-set lock.
//
// Who may unlock: restool's `dprc set-locked` opens the target child's
// PARENT-container portal (dprc_commands.c, cmd_dprc_set_locked — it
// resolves the child's parent id and uses the root handle when that parent
// is the root). dprc.2 is a top-level scratch child, so both the lock and
// the unlock are issued through the root's portal — the only "who may
// unlock" this tool can exercise, and it agrees with the model's
// setLockedAt (only an ancestor strictly above the container gates
// toggling it). The create class under lock is NOT probed here: a create
// is the very operation the strip removes, and a hook does not create for
// the thing under test — that face stays with the mc-status register.
//
// The dpbp is created unplugged and lives in a child container, so it is
// never bus-visible or driver-bound (DPRC-I6); the generated teardown
// destroys it and then dprc.2 cleanly once the hook has left the lock off.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same two-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-3/vdprc3.qnt --main=vdprc3 \
//     --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-3/vdprc3.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc3 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }
  pure val theDpbp: ObjId = { fam: Dpbp, num: 0 }

  action init = m::init

  // 1. One scratch container under the Linux root.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // 2. One dpbp inside it, left unplugged (no plug action in this
  //    module) — the object the hook locks around.
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
