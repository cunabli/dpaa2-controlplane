// Scenario module for board suite V-DPRC-7 (dprc-encapsulation task 5.1;
// bead dpaa2-controlplane-cd3.10): the container lifecycle sweep on scratch
// children, rev 2 — grounded in the rev-1 board sitting (2026-09-13).
// Wraps the core machine (models/core/machine.qnt), like V-DPRC-1/3/6.
//
// Rev 1 ran all 8 trace steps PASS and its hook's lock accepts PASS, but the
// destroy-with-plugged-resident it asserted REFUSED was ACCEPTED by the
// board: `dprc destroy` of a scratch container holding a plugged owned dpbp
// exited 0, cascade-destroyed the container and its plugged resident, and
// raised no MC or restool refusal. The verdict stands as recorded (the
// refusal prediction was inverted by the board; the lock accepts held). The
// finding extends the model's conservative destroy fence rather than
// contradicting it: an OWNED resident may now be plugged at destroy time
// (models/core/machine.qnt destroyAt), so rev 2 makes that a judged trace
// step instead of a hook refusal.
//
// The 9-step positive lifecycle over two scratch children under the Linux
// root, default option mask:
//   1-2. create dprc.2 (A) and dprc.3 (B) under the root;
//   3-4. create a dpbp resident in each (dpbp.0 in A, dpbp.1 in B) —
//        CreatedIn residents, owned by their container;
//   5-6. plug both (dprc assign --plugged=1);
//   7.   unplug A's resident (dpbp.0);
//   8.   destroy A while it holds its UNPLUGGED owned dpbp.0 — owned-resident
//        release / evict-on-destroy (ADR-0007 §3, the anchor V-DPRC-1 rev3
//        closed with a dpbp);
//   9.   destroy B while it holds its STILL-PLUGGED owned dpbp.1 — the
//        widened face this rev exercises: the board cascade-destroys the
//        container and its plugged owned resident (V-DPRC-7 rev 1 anchor).
//
// No hook in rev 2. The lock/unlock accept faces rev 1 carried are
// double-anchored — V-DPRC-3 rev 2 (2/2 + hook 6/6) and this suite's rev-1
// hook (3/3) — so they are not re-run here; refusals.sh is removed.
//
// This rev is regenerated with `--pool-record` (ADR-0011's per-step pool
// instrument): rev 1 lost 2 MC portals (mcp 203 -> 201) with zero dpmcp
// created or destroyed, a datum for ADR-0011's "listing lags or uncounted
// reserve" open question; the per-step `dprc show mc.global --resources`
// captures resolve which step the reserve moves at.
//
// Refusal-shape coverage for this suite's permission matrix is already
// board-settled and is NOT re-probed here: 0x6 Configuration error (SPAWN
// absent) — V-DPRC-2-NOSPAWN-1 rev2 (2/2); 0x8 No resources (ALLOC absent) —
// V-DPRC-2-NOALLOC-1 rev2 (2/2); 0x4 No privilege under lock — V-DPRC-3 rev2
// (2/2 + hook 6/6); plugged-move Error::RestoolGuard client-guard pre-check
// and sibling-move 0x4 No privilege — V-DPRC-6 rev1 (4/4 + hook 3/3).
//
// Guards leave exactly one action enabled per state, so any seed freezes the
// same nine-step trace:
//
//   pnpm exec quint run models/board/V-DPRC-7/vdprc7.qnt --main=vdprc7 \
//     --mbt --max-steps=9 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPRC-7/vdprc7.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdprc7 {
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
  pure val scratchA: ObjId = { fam: Dprc, num: 2 }
  pure val scratchB: ObjId = { fam: Dprc, num: 3 }
  pure val dpbpA: ObjId = { fam: Dpbp, num: 0 }
  pure val dpbpB: ObjId = { fam: Dpbp, num: 1 }

  action init = m::init

  // 1-2. Two scratch containers under the Linux root; the counter guard
  //      stops the action after the second (dprc.2 then dprc.3).
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) < 4,
      m::createContainerAt(parent),
    }
  }

  // 3-4. One dpbp in each container, only once both containers exist. The
  //      dpbp counter forces A first (dpbp.0), then B (dpbp.1), so exactly
  //      one pick is enabled per state.
  action createObject = {
    nondet fam = Set(Dpbp).oneOf()
    nondet c = Set(scratchA, scratchB).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchB),
      m::s.nextNum.get(Dpbp) < 2,
      if (m::s.nextNum.get(Dpbp) == 0) c == scratchA else c == scratchB,
      m::createObjectAt(fam, c),
    }
  }

  // 5-6. Plug both residents, only once both exist. A is plugged first
  //      (while B is still unplugged), then B — the ordering keeps one pick
  //      enabled per state, and plug-A requires B unplugged so it never
  //      re-fires after A is later unplugged.
  action plug = {
    nondet o = Set(dpbpA, dpbpB).oneOf()
    all {
      // Both must exist before the guard dereferences either — after the
      // step-8 destroy releases dpbp.0, the counter still reads 2, so the
      // `contains` checks (not the counter) are what keep this safe.
      m::s.objs.keys().contains(dpbpA),
      m::s.objs.keys().contains(dpbpB),
      if (o == dpbpA)
        and { not(m::s.objs.get(dpbpA).plugged), not(m::s.objs.get(dpbpB).plugged) }
      else
        and { m::s.objs.get(dpbpA).plugged, not(m::s.objs.get(dpbpB).plugged) },
      m::plugAt(o),
    }
  }

  // 7. Unplug A's resident only once both are plugged; B's resident stays
  //    plugged for the step-9 destroy.
  action unplug = {
    nondet o = Set(dpbpA).oneOf()
    all {
      m::s.objs.keys().contains(dpbpB),
      m::s.objs.get(dpbpB).plugged,
      m::unplugAt(o),
    }
  }

  // 8-9. Destroy both containers, each holding its owned dpbp — the
  //      evict-on-destroy face (ADR-0007 §3). A goes first, holding its
  //      UNPLUGGED dpbp.0; then B, holding its STILL-PLUGGED dpbp.1 (the
  //      widened owned-plugged face, V-DPRC-7 rev 1 anchor). The end phase
  //      is keyed on dpbp.1 plugged so neither destroy fires before the
  //      lifecycle reaches it, and B waits for A to be gone.
  action destroy = {
    nondet o = Set(scratchA, scratchB).oneOf()
    all {
      m::s.objs.keys().contains(dpbpB),
      m::s.objs.get(dpbpB).plugged,
      if (o == scratchA) and {
        m::s.objs.keys().contains(dpbpA),
        not(m::s.objs.get(dpbpA).plugged),
        m::s.dprcs.keys().contains(scratchB),
      } else
        not(m::s.dprcs.keys().contains(scratchA)),
      m::destroyAt(o),
    }
  }

  action step = any { createContainer, createObject, plug, unplug, destroy }
}
