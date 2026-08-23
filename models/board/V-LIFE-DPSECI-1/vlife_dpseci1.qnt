// Scenario module for board suite V-LIFE-DPSECI-1 (task 5.2; design D7
// step 2's "per-family lifecycle scenarios"): the §5 canonical forward
// order for a dpseci — its census companion (1 dpmcp, dpseci.qnt draw)
// created and plugged in the Linux root, bus rescan, then the kernel's
// own probe (dpaa2_caam) awaited and judged through the sysfs driver
// link.
//
// There is no bare dpseci create to probe: restool rejects a create that
// sets neither the queue count nor the priorities (dpseci_commands.c),
// so the suite creates with restool's own example pair — 2 queues at
// priorities 2,4. The read-back still shows what MC defaults look like
// for every attribute those two flags leave alone.
//
// The objects live in dprc.1 by necessity, not preference: kernel binds
// happen in the Linux root only (DPRC-I6). The judged trace ends at the
// bind; the suite's unconditional teardown trap restores the root
// container.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same six-step trace:
//
//   pnpm exec quint run models/board/V-LIFE-DPSECI-1/vlife_dpseci1.qnt --main=vlife_dpseci1 \
//     --mbt --max-steps=6 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-LIFE-DPSECI-1/vlife_dpseci1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vlife_dpseci1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val theDpmcp: ObjId = { fam: Dpmcp, num: 0 }
  pure val theDpseci: ObjId = { fam: Dpseci, num: 0 }

  action init = m::init

  // 1–2. Companion first, consumer last (§5 order), serialized by the
  //      per-family counters.
  action createObject = {
    nondet fam = Set(Dpmcp, Dpseci).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      if (fam == Dpmcp) m::s.nextNum.get(Dpmcp) == 0
      else and {
        m::s.nextNum.get(Dpmcp) == 1,
        m::s.nextNum.get(Dpseci) == 0,
      },
      m::createObjectAt(fam, c),
    }
  }

  // 3–4. Plug in the same order, only once both exist.
  action plug = {
    nondet o = Set(theDpmcp, theDpseci).oneOf()
    all {
      m::s.nextNum.get(Dpseci) == 1,
      if (o == theDpmcp) true else m::s.objs.get(theDpmcp).plugged,
      m::plugAt(o),
    }
  }

  // 5. Root bus rescan makes both visible; disabled once the dpseci is,
  //    so the run cannot rescan twice.
  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(theDpseci),
      m::s.objs.get(theDpseci).plugged,
      not(m::s.objs.get(theDpseci).busVisible),
      m::rescanAt(c),
    }
  }

  // 6. The kernel's probe, awaited: the census draws exactly the
  //    companion (dpseci draw 1/0/0).
  action kernelBind = {
    nondet o = Set(theDpseci).oneOf()
    nondet mcp = Set(theDpmcp).oneOf()
    m::kernelBindAt(o, Set(mcp), Set(), Set())
  }

  action step = any { createObject, plug, rescan, kernelBind }
}
