// Entry point of the intent corpus (openspec change `intent-layer`): the
// directed `<what>Test` runs that pin the vocabulary, the reference
// inventory, and — from task 1.2 on — the derivation and its refusals.
//
//   pnpm exec quint test models/intent/main.qnt --main=intent_main
//
// Named invariants: none yet (task 1.4 adds intent/invariants.qnt).
// Apalache-marked subset: none yet (task 1.5).
module intent_main {
  import types.* from "../core/types"
  import companions.* from "../core/companions"
  import intent_types.* from "./types"
  import intent_inventory.* from "./inventory"

  // The reference inventory reads as the spec's "Reserved and foreign
  // anchors are inventory facts" scenario says: dpmac.3 and dpmac.17
  // Reserved, dpni.0 Foreign, dpbp Counted, dpni Observed, dpcon Unknown.
  run refInventoryTest = all {
    match availabilityOf(REF_INVENTORY, { fam: Dpmac, num: 3 }) {
      | Reserved(_) => true
      | _ => false
    },
    match availabilityOf(REF_INVENTORY, { fam: Dpmac, num: 17 }) {
      | Reserved(_) => true
      | _ => false
    },
    availabilityOf(REF_INVENTORY, { fam: Dpmac, num: 7 }) == Free,
    availabilityOf(REF_INVENTORY, { fam: Dpni, num: 0 }) == Foreign("dpl"),
    availabilityOf(REF_INVENTORY, { fam: Dpni, num: 5 }) == Free,
    REF_INVENTORY.ceilings.get(Dpbp) == Counted(63),
    match REF_INVENTORY.ceilings.get(Dpni) {
      | Observed(o) => o.n == 18
      | _ => false
    },
    REF_INVENTORY.ceilings.get(Dpcon) == Unknown,
    REF_INVENTORY.ceilings.keys() == DERIVED_FAMILIES,
    // the DPC-born dpmacs of the reference environment, 3–10 and 17
    REF_INVENTORY.dpmacs.keys() == Set(3, 4, 5, 6, 7, 8, 9, 10, 17),
    REF_INVENTORY.dpmacs.get(7).maxRate == 10000,
    REF_INVENTORY.dpmacs.get(3).maxRate == 25000,
    kernelConsumer(16).regime == Kernel,
  }
}
