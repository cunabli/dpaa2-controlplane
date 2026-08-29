// Scenario module for board suite V-DPRC-6 (task 5.9): DPRC-I3's refusal
// face (a plugged object cannot be moved between containers) plus the
// sibling-move status the MC status register still lists as unknown.
// V-DPRC-1 rev 1 tried a sibling move as a single `dprc assign --child`
// and the board refused it (exit 255), but that revision predated stderr
// capture, so the status text was never recorded — this suite records it.
//
// The trace only builds the standing state the hook probes: two scratch
// containers dprc.2 and dprc.3 under the Linux root, a dpbp created in
// dprc.2, and that dpbp plugged where it sits — four steps. Nothing is
// bus-visible (a child container restool creates is never kernel-bound,
// DPRC-I6), so no driver ever holds the dpbp; the refusals below are the
// MC's own gate, not a driver's. The hook moves.sh then, in order:
//   (a) tries to move the *plugged* dpbp one hop up to the root — refused
//       because it is plugged (DPRC-I3); the dpbp stays in dprc.2;
//   (b) unplugs it;
//   (c) tries the sibling move dprc.2 -> dprc.3 in one command (the exact
//       rendering rev 1 used) — refused with No privilege.
// A refused move is a disabled action, never a trace step, which is why
// all three live in the hook rather than the trace.
//
// The hook leaves the dpbp unplugged in dprc.2, so the generated
// teardown's unplug of it is a no-op refusal (already unplugged) logged
// to teardown.log, and the destroy that follows still finds it and
// reclaims it — the same harmless boot-edge shape other suites' teardowns
// take. The trace's own final state has the dpbp plugged; the hook is
// what leaves it unplugged, and the teardown copes with either.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same four-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-6/vdprc6.qnt --main=vdprc6 \
//     --mbt --max-steps=4 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-6/vdprc6.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc6 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  // Model ids the run creates, deterministic from the per-family counters
  // (dprc numbering starts at 2, dpbp at 0 — the model ids are symbolic;
  // the board names creates itself).
  pure val scratchA: ObjId = { fam: Dprc, num: 2 }
  pure val scratchB: ObjId = { fam: Dprc, num: 3 }
  pure val theDpbp: ObjId = { fam: Dpbp, num: 0 }

  action init = m::init

  // 1-2. Two scratch containers under the Linux root; the counter guard
  //      stops the action after the second.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) < 4,
      m::createContainerAt(parent),
    }
  }

  // 3. One dpbp in scratch A, only once both containers exist.
  action createObject = {
    nondet fam = Set(Dpbp).oneOf()
    nondet c = Set(scratchA).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchB),
      m::s.nextNum.get(Dpbp) == 0,
      m::createObjectAt(fam, c),
    }
  }

  // 4. Plug the dpbp where it sits, only once it exists — plug renders
  //    against its parent (dprc.2), the way V-DPRC-1 step 5 plugged in a
  //    child. This is the plugged state the hook's first move is refused
  //    from.
  action plug = {
    nondet o = Set(theDpbp).oneOf()
    all {
      m::s.objs.keys().contains(theDpbp),
      m::plugAt(o),
    }
  }

  action step = any { createContainer, createObject, plug }
}
