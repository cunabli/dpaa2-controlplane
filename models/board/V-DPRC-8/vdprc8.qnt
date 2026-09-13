// Scenario module for board suite V-DPRC-8 (dprc-encapsulation task 5.2;
// bead dpaa2-controlplane-cd3.11): the VFIO face on a scratch child dprc.
// Wraps the core machine (models/core/machine.qnt), like V-DPRC-1/3/6/7.
//
// The two-step trace is only the fixture: create a scratch child dprc.2
// under the Linux root dprc.1 with the default option mask, and create one
// dpbp.0 resident in it. The VFIO program itself is carried entirely by the
// suite hook (vfio.sh, sourced after the last trace step) — it is not
// traceable here for two intentional reasons:
//   - the core machine's vfioBindAt guard refuses a Dprc target
//     (models/core/machine.qnt:340, `o.fam != Dprc`): VFIO in the model is a
//     per-resident claim, never a container bind, so `driver_override` +
//     `bind` on the child dprc has no action to render; and
//   - the generator has no `driver_override` read-back — the bus-only state
//     the hook writes and reads (the child's driver symlink, its residents'
//     override) is outside the adapter's present/plugged/endpoint/bound
//     observation surface — so the propagation answer must be a hook oracle.
// Both are by design for this task and are not "fixed" here.
//
// The trace never destroys: the generated teardown trap self-cleans, and
// container cascade-destroy over the hook's extra post-bind resident is
// board-verified law (V-DPRC-7 rev 1). The post-bind resident create lives
// in the hook (it settles the design open question "post-bind resident
// creation").
//
// bind/unbind in the hook mirror models/families/dprc.qnt `bindVfio` /
// `unbindVfio` (the 1.1 model gate): the family model carries the kernel
// VFIO bind state ONLY on the `Plugged(_)` face, where `Plugged` means
// kernel-active (dprc.md "Lifecycle ordering": a DPRC's plugged state is not
// restool-mutable; the board's own dprc.2/VPP runs unplugged and
// VFIO-driven). This suite settles the restool/sysfs rendering of that face:
// `driver_override` + `bind`/`unbind` on the child, and override propagation
// to a subsequently added resident (dprc.md:230-234).
//
// Guards leave exactly one action enabled per state, so any seed freezes the
// same two-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-8/vdprc8.qnt --main=vdprc8 \
//     --mbt --max-steps=2 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-8/vdprc8.itf.json
//
// Named invariants: DPRC-I7 (hook-judged) — a VFIO bind is a bus-only event,
// so the MC census is unmoved across bind (the hook's Census A == B / C == D
// checks); the trace carries no model invariant (its two steps are the
// fixture). TLA+ escalation: none.
//
// Refusal-shape coverage for this suite's permission matrix is board-settled
// elsewhere and is NOT re-probed here (see V-DPRC-7's header for the roster).
module vdprc8 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  // Model ids the run creates, deterministic from the per-family counters
  // (dprc numbering starts at 2, dpbp at 0 — the model ids are symbolic; the
  // board names creates itself).
  pure val scratch: ObjId = { fam: Dprc, num: 2 }
  pure val dpbp0: ObjId = { fam: Dpbp, num: 0 }

  action init = m::init

  // 1. One scratch container under the Linux root, default option mask; the
  //    counter guard stops the action after dprc.2.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) < 3,
      m::createContainerAt(parent),
    }
  }

  // 2. One dpbp resident in the child, only once the child exists; the dpbp
  //    counter stops the action after dpbp.0, so exactly one action is
  //    enabled per state.
  action createObject = {
    nondet fam = Set(Dpbp).oneOf()
    nondet c = Set(scratch).oneOf()
    all {
      m::s.dprcs.keys().contains(scratch),
      m::s.nextNum.get(Dpbp) < 1,
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createContainer, createObject }
}
