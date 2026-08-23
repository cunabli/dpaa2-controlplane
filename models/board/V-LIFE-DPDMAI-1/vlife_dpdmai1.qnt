// Scenario module for board suite V-LIFE-DPDMAI-1 (task 5.2; design D7
// step 2's "per-family lifecycle scenarios"): the §5 canonical forward
// order for a dpdmai — its census companion (1 dpmcp, dpdmai.qnt draw)
// created and plugged in the Linux root, bus rescan, then the kernel's
// probe (fsl-dpaa2-qdma) awaited and judged through the sysfs driver
// link. The create is bare — the V-DPDMAI-1 inventory row's default
// question (num_queues on the wire, DPDMAI-I3/I5) — so the bind
// read-back here is a genuine unknown, Read evidence: a missing driver
// link is a finding about what the kernel does with a default dpdmai,
// not automatically a model error.
//
// The objects live in dprc.1 by necessity, not preference: kernel binds
// happen in the Linux root only (DPRC-I6). The judged trace ends at the
// bind; the suite's unconditional teardown trap restores the root
// container.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same six-step trace:
//
//   pnpm exec quint run models/board/V-LIFE-DPDMAI-1/vlife_dpdmai1.qnt --main=vlife_dpdmai1 \
//     --mbt --max-steps=6 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-LIFE-DPDMAI-1/vlife_dpdmai1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vlife_dpdmai1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val theDpmcp: ObjId = { fam: Dpmcp, num: 0 }
  pure val theDpdmai: ObjId = { fam: Dpdmai, num: 0 }

  action init = m::init

  // 1–2. Companion first, consumer last (§5 order), serialized by the
  //      per-family counters.
  action createObject = {
    nondet fam = Set(Dpmcp, Dpdmai).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      if (fam == Dpmcp) m::s.nextNum.get(Dpmcp) == 0
      else and {
        m::s.nextNum.get(Dpmcp) == 1,
        m::s.nextNum.get(Dpdmai) == 0,
      },
      m::createObjectAt(fam, c),
    }
  }

  // 3–4. Plug in the same order, only once both exist.
  action plug = {
    nondet o = Set(theDpmcp, theDpdmai).oneOf()
    all {
      m::s.nextNum.get(Dpdmai) == 1,
      if (o == theDpmcp) true else m::s.objs.get(theDpmcp).plugged,
      m::plugAt(o),
    }
  }

  // 5. Root bus rescan makes both visible; disabled once the dpdmai is,
  //    so the run cannot rescan twice.
  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(theDpdmai),
      m::s.objs.get(theDpdmai).plugged,
      not(m::s.objs.get(theDpdmai).busVisible),
      m::rescanAt(c),
    }
  }

  // 6. The kernel's probe, awaited: the census draws exactly the
  //    companion (dpdmai draw 1/0/0).
  action kernelBind = {
    nondet o = Set(theDpdmai).oneOf()
    nondet mcp = Set(theDpmcp).oneOf()
    m::kernelBindAt(o, Set(mcp), Set(), Set())
  }

  action step = any { createObject, plug, rescan, kernelBind }
}
