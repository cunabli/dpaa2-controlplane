// Scenario module for board suite V-GENDPL-1 (task 5.10): DPDCEI-I2 and
// DPDMAI-I4 (and DPAIOP-I3 by re-anchoring) — `generate-dpl` is not a
// round-trip. It re-emits live MC state as a DPL device tree, but
// write-only create fields are dropped and dpdmai priorities are mangled,
// so the emitted tree does not reconstruct the objects it was generated
// from. The model has no DPL surface and no create-attribute state, so the
// trace only stands the objects (created with non-default attributes) and
// the hook captures the emit; the by-eye diff of emitted-vs-requested is
// done offline in the fold, not asserted on the board.
//
// The trace is four steps:
//   1. a scratch dprc.2 under the Linux root;
//   2. a dpdcei in it, engine DECOMPRESSION at priority 2 (non-default);
//   3. a dpdmai in it, priorities 2,4 (non-default);
//   4. a dpci in it, num-priorities 2.
// The create attributes ride `--create-args` per family — they are not
// model state — so the trace is attribute-blind and any seed freezes the
// same shape.
//
// The hook dpl.sh then generates the DPL of dprc.2, reads each object back
// with `info`, and RECORDs the emitted node block for each (the dpdcei@,
// dpdmai@ and dpci@ nodes grepped from the dts). generate-dpl is a
// read-only query walk of live MC state, so it is a legal hook verb; the
// hook mutates nothing. It PASSes on every command exiting 0 and leaves
// the diff to the fold.
//
// No dpaiop can exist on this AIOP-less silicon (V-DPAIOP-1: `dpaiop
// create` is refused Configuration error), so DPAIOP-I3's write-only-drop
// face has no object to emit and is re-anchored in the fold from the
// dpdcei/dpdmai evidence, not probed here.
//
// None of these families has a kernel driver that binds a runtime object
// (dpci none; dpdcei none; the reference kernel registers no qdma driver
// for dpdmai), so nothing binds and the generated teardown destroys all
// three and then dprc.2 cleanly, spaced per ADR-0008.
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same four-step trace:
//
//   pnpm exec quint run models/board/V-GENDPL-1/vgendpl1.qnt \
//     --main=vgendpl1 --mbt --max-steps=4 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-GENDPL-1/vgendpl1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vgendpl1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  pure val scratchDprc: ObjId = { fam: Dprc, num: 2 }

  action init = m::init

  // 1. One scratch container under the Linux root.
  action createContainer = {
    nondet parent = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dprc) == 2,
      m::createContainerAt(parent),
    }
  }

  // 2-4. Three objects in dprc.2, serialized by their per-family
  //      counters into dpdcei, then dpdmai, then dpci — one `createObject`
  //      action, the family forced per counter so exactly one pick is
  //      enabled per state and the action disables once all three exist.
  action createObject = {
    nondet fam = Set(Dpdcei, Dpdmai, Dpci).oneOf()
    nondet c = Set(scratchDprc).oneOf()
    all {
      m::s.dprcs.keys().contains(scratchDprc),
      if (m::s.nextNum.get(Dpdcei) == 0) fam == Dpdcei
      else if (m::s.nextNum.get(Dpdmai) == 0) fam == Dpdmai
      else and { m::s.nextNum.get(Dpci) == 0, fam == Dpci },
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createContainer, createObject }
}
