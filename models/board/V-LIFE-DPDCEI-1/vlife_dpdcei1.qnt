// Scenario module for board suite V-LIFE-DPDCEI-1 (task 5.2; design D7
// step 2's "per-family lifecycle scenarios"): the dpdcei lifecycle face
// — create in a scratch container, plugged, then destroyed
// while plugged and the container after it. dpdcei has no kernel
// consumer in the corpus (dpdcei.qnt: hasKernelDriver false), so there
// is no bind to await — presence and plugged state are the whole
// observable surface, which is why this face fits a scratch container.
//
// There is no bare dpdcei create to probe: restool rejects a create that
// omits --engine or --priority (restool dpdcei_commands.c), so the suite
// creates with DPDCEI_ENGINE_DECOMPRESSION at priority 1. Hardware is
// present per the manual's Table 2-1, so the create succeeding is still
// the positive half of DPDCEI-I1; the MC-defaults question the read-back
// answers now covers only the attributes those two flags leave unset.
//
// Guards leave exactly one action enabled per state, so any seed
// freezes the same five-step trace:
//
//   pnpm exec quint run models/board/V-LIFE-DPDCEI-1/vlife_dpdcei1.qnt --main=vlife_dpdcei1 \
//     --mbt --max-steps=5 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-LIFE-DPDCEI-1/vlife_dpdcei1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vlife_dpdcei1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }
  pure val theDpdcei: ObjId = { fam: Dpdcei, num: 0 }

  action init = m::init

  // 1. One scratch container under the Linux root.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // 2. The dpdcei create (engine and priority are mandatory in restool;
  //    every other attribute is left to MC defaults).
  action createObject = {
    nondet fam = Set(Dpdcei).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.nextNum.get(Dpdcei) == 0,
      m::createObjectAt(fam, c),
    }
  }

  // 3. Plug it (no kernel probe follows: a child-container object is
  //    never bus-visible, DPRC-I6 — and dpdcei has no driver anyway).
  action plug = {
    nondet o = Set(theDpdcei).oneOf()
    m::plugAt(o)
  }

  // 4. Destroy while plugged — the in_use blindness holds for any
  //    child-container family (V-DPNI-1 anchored it); enabled only
  //    after the plug by the plugged guard.
  // 5. Destroy the emptied container.
  action destroy = {
    nondet o = Set(scratchDprc, theDpdcei).oneOf()
    all {
      if (o == theDpdcei) and {
        m::s.objs.keys().contains(theDpdcei),
        m::s.objs.get(theDpdcei).plugged,
      } else and {
        // Only after the dpdcei existed and is gone again — the counter
        // is the monotone witness that step 4 already happened.
        m::s.nextNum.get(Dpdcei) == 1,
        not(m::s.objs.keys().contains(theDpdcei)),
      },
      m::destroyAt(o),
    }
  }

  action step = any { createContainer, createObject, plug, destroy }
}
