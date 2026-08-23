// Scenario module for board suite V-RECOVERY-1 (task 5.1; design D7
// step 1; ADR-0003 §7): the smallest scratch-DPRC mutation set a reboot
// must erase — one scratch container under the Linux root, one dpbp
// created inside it, plugged. Nothing here touches a boot-born object.
//
// Board suites are generated from `--mbt` traces (the adapter needs
// `mbt::actionTaken` + `mbt::nondetPicks`), so this module wraps the
// machine's parameterized actions in nondet forms whose names and
// binder names match the machine's own — but with the picks restricted
// to the scenario and guards that leave exactly one action enabled per
// state. Any seed therefore freezes the same three-step trace:
//
//   pnpm exec quint run models/board/V-RECOVERY-1/recovery.qnt --main=recovery \
//     --mbt --max-steps=3 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-RECOVERY-1/recovery.itf.json
//
// Raw main.qnt simulation is not usable here: unconstrained picks reach
// total-deny objects and restool-unreachable containers, which the
// generator refuses (correctly).
//
// Named invariants: none — the machine's guards carry the model gate;
// the suite's read-back carries conformance. TLA+ escalation: none.
module recovery {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  // DPC-born dpmacs of the reference environment (as models/main.qnt).
  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  // Model ids the run creates, deterministic from the per-family
  // counters (dprc numbering starts at 2, dpbp at 0).
  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }
  pure val scratchDpbp: ObjId = { fam: Dpbp, num: 0 }

  action init = m::init

  // 1. One scratch container under the Linux root; disabled once it
  //    exists, so the run never creates a second.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      not(m::s.dprcs.keys().contains(scratchDprc)),
      m::createContainerAt(parent),
    }
  }

  // 2. One dpbp inside the scratch container (bare create on MC
  //    defaults); createObjectAt's own guard keeps this disabled until
  //    the container exists.
  action createObject = {
    nondet fam = Set(Dpbp).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      not(m::s.objs.keys().contains(scratchDpbp)),
      m::createObjectAt(fam, c),
    }
  }

  // 3. Plug it — the plugged-state bit is board state beyond mere
  //    existence that the reboot must also erase.
  action plug = {
    nondet o = Set(scratchDpbp).oneOf()
    m::plugAt(o)
  }

  action step = any { createContainer, createObject, plug }
}
