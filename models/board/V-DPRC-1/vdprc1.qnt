// Scenario module for board suite V-DPRC-1 (task 5.2; traffic-inventory
// §1): the scratch-container lifecycle sweep's positive faces — two
// scratch containers under the Linux root, a dpni created in the first,
// moved to the second the only way the board allows (ADR-0007 §1: one
// hop up via `dprc unassign`, one hop down via `dprc assign --child`),
// plugged and unplugged there, then repatriated the same two-hop way and
// destroyed at home. Rev 1 tried the sibling move as a single assign and
// the board refused it (exit 255); rev 2 anchored the two-hop route and
// the unassign rendering, but its standalone `dpni destroy` after the
// move hit MC "No privilege" — destroy authority is creator-bound
// (ADR-0007 §2), so this revision repatriates before destroying, which
// is the positive anchor for authority restored. The refusal faces stay
// with the online driver (models/board/README.md).
//
// The plug/unplug round trip leaves no monotone state to phase-key on,
// so step 7 creates a scratch dpbp in A as the phase marker separating
// the outbound trip from the return. It earns its keep at step 13:
// destroying A while it still holds the unplugged dpbp it created is the
// second board anchor for owned-resident release (rev 1 proved it with a
// dpni).
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same thirteen-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-1/vdprc1.qnt --main=vdprc1 \
//     --mbt --max-steps=13 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-1/vdprc1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  // Model ids the run creates, deterministic from the per-family
  // counters (dprc numbering starts at 2, dpni/dpbp at 0 — the model
  // ids are symbolic; the board names creates itself).
  pure val scratchA: ObjId = { fam: Dprc, num: 2 }
  pure val scratchB: ObjId = { fam: Dprc, num: 3 }
  pure val theDpni: ObjId = { fam: Dpni, num: 0 }
  pure val marker: ObjId = { fam: Dpbp, num: 0 }

  action init = m::init

  // 1–2. Two scratch containers under the Linux root; the counter guard
  //      stops the action after the second.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) < 4,
      m::createContainerAt(parent),
    }
  }

  // 3. One dpni in scratch A, only once both containers exist.
  // 7. The dpbp phase marker in A, only once the dpni is plugged in B —
  //    which is what separates the outbound trip from the return.
  action createObject = {
    nondet fam = Set(Dpni, Dpbp).oneOf()
    nondet c = Set(scratchA).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchB),
      if (fam == Dpni) m::s.nextNum.get(Dpni) == 0
      else and {
        m::s.nextNum.get(Dpbp) == 0,
        m::s.objs.keys().contains(theDpni),
        m::s.objs.get(theDpni).plugged,
      },
      m::createObjectAt(fam, c),
    }
  }

  // 4–5 outbound (A → dprc.1 → B, marker absent) and 8–9 return
  // (B → dprc.1 → A, marker present). Each hop is pinned to the dpni's
  // pre-state parent and the trip its marker key selects, so exactly one
  // fires per state.
  action assignChild = {
    nondet o = Set(theDpni).oneOf()
    nondet dst = Set(m::dprc1, scratchA, scratchB).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      if (m::s.objs.keys().contains(marker))
        // return trip
        if (dst == m::dprc1) m::s.objs.get(theDpni).parent == Some(scratchB)
        else if (dst == scratchA) m::s.objs.get(theDpni).parent == Some(m::dprc1)
        else false
      else
        // outbound trip
        if (dst == m::dprc1) m::s.objs.get(theDpni).parent == Some(scratchA)
        else if (dst == scratchB) m::s.objs.get(theDpni).parent == Some(m::dprc1)
        else false,
      m::assignChildAt(o, dst),
    }
  }

  // 6. Plug in B, only before the marker exists — the marker is created
  //    next, and its presence keeps this from re-firing after unplug.
  action plug = {
    nondet o = Set(theDpni).oneOf()
    all {
      m::s.objs.keys().contains(theDpni),
      m::s.objs.get(theDpni).parent == Some(scratchB),
      not(m::s.objs.keys().contains(marker)),
      m::plugAt(o),
    }
  }

  // 8. Unplug, only once the marker exists.
  action unplug = {
    nondet o = Set(theDpni).oneOf()
    all {
      m::s.objs.keys().contains(marker),
      m::unplugAt(o),
    }
  }

  // 10/12/13. Destroys, phase-keyed: the repatriated dpni at home in A
  //           (owner == parent, ADR-0007 §2); the empty B; then A still
  //           holding the dpbp it created — owned-resident release.
  action destroy = {
    nondet o = Set(scratchA, scratchB, theDpni).oneOf()
    all {
      if (o == theDpni) and {
        m::s.objs.keys().contains(theDpni),
        m::s.objs.keys().contains(marker),
        m::s.objs.get(theDpni).parent == Some(scratchA),
      } else if (o == scratchB) and {
        m::s.objs.keys().contains(marker),
        not(m::s.objs.keys().contains(theDpni)),
      } else and {
        // A goes last: only after the marker exists (the run reached its
        // second half) and B is already gone.
        m::s.objs.keys().contains(marker),
        not(m::s.dprcs.keys().contains(scratchB)),
      },
      m::destroyAt(o),
    }
  }

  action step = any { createContainer, createObject, assignChild, plug, unplug, destroy }
}
