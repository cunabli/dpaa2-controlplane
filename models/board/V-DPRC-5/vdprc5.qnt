// Scenario module for board suite V-DPRC-5 (task 5.10): DPRC-I6 (a bus
// rescan reaches root containers only; a child container's residents are
// never bus-visible) and DPCI-I3 (a dpci create performs no rescan, so
// sysfs lags the MC until an explicit rescan). Both are visibility facts,
// not lifecycle transitions — nothing in the model's state distinguishes
// a bus-visible object from an MC-only one at the granularity restool
// drives, so the faces live in the hook, which reads sysfs directly.
//
// The trace only builds the standing state the hook observes — three
// creates:
//   1. a scratch dprc.2 under the Linux root;
//   2. a dpci inside dprc.2 (the child resident, dpci.0) — never
//      bus-visible, because a rescan reaches root containers only;
//   3. a dpci inside dprc.1, the root (dpci.1) — MC-present at once but
//      not yet a sysfs node, because the create triggered no rescan.
// A dpci has no kernel driver, so no bind ever fires and the hook's one
// rescan (`dprc sync`) binds nothing — it only walks the bus. That is why
// the rescan is safe here (ADR-0008): nothing driver-bound is touched and
// no destroy precedes it.
//
// The hook visibility.sh then, in order: (a) lists the bus before any
// rescan, records whether the root dpci is a sysfs node yet (DPCI-I3's
// oracle — expected absent) and asserts the child dpci is absent
// (DPRC-I6); (b) issues the rescan; (c) lists again and asserts the root
// dpci is now present and the child dpci still absent; (d) shows dprc.2
// to prove the MC held the child dpci all along. It also records
// /sys/bus/fsl-mc/autorescan, the policy a child's own refresh path keys
// off (dprc.md unknown 12). A visibility read is not a model transition,
// so every one of these is a hook line, never a trace step.
//
// Both dpcis are created unplugged (post-create default) and driver-less,
// so the generated teardown's unplug of each is a harmless no-op refusal
// logged to teardown.log and the destroy that follows still reclaims it —
// the same boot-edge shape other suites' teardowns take.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same three-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-5/vdprc5.qnt --main=vdprc5 \
//     --mbt --max-steps=3 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-5/vdprc5.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc5 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  // Model ids the run creates, deterministic from the per-family counters
  // (dprc numbering starts at 2, dpci at 0; the model ids are symbolic —
  // the board names creates itself). dpci.0 is created first, in the
  // child; dpci.1 second, in the root.
  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }
  pure val childDpci: ObjId = { fam: Dpci, num: 0 }
  pure val rootDpci: ObjId = { fam: Dpci, num: 1 }

  action init = m::init

  // 1. One scratch container under the Linux root; the counter guard
  //    stops the action after the first.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // 2-3. Two dpci creates, serialized by the counter into the child
  //      first (dpci.0, the resident the rescan never reaches) then the
  //      root (dpci.1, MC-present but bus-absent until a rescan). One
  //      `createObject` action carries both — the adapter keys on the
  //      canonical name — with the container forced per counter value so
  //      exactly one (fam, c) pick is enabled per state.
  action createObject = {
    nondet fam = Set(Dpci).oneOf()
    nondet c = Set(scratchDprc, m::dprc1).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchDprc),
      m::s.nextNum.get(Dpci) < 2,
      if (m::s.nextNum.get(Dpci) == 0) c == scratchDprc else c == m::dprc1,
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createContainer, createObject }
}
