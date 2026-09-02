// The intent-replay trace source (openspec change `intent-layer`, task
// 3.2, design D9): freezes one ITF trace per scenario/twin so dpaa2-verify
// can replay it through the Rust `compile` and diff the outcome against the
// model's. It mirrors the retro trace source (models/retro/reconciler.qnt):
// named `run`s that `quint test --out-itf='{test}.itf.json'` writes one
// frozen file per — here a single state whose three variables carry the two
// `compile` inputs (`intent`, `inv`) and the model's own `outcome`, so the
// replayer parses the inputs, re-derives with the Rust compiler, and asserts
// it equals the frozen `outcome`. The model is the oracle (design D9): the
// intents are not redeclared here but imported from the accepted scenarios,
// so a scenario amendment reaches the frozen traces the moment they are
// regenerated (`pnpm model:freeze-intent`), never drifting a second copy.
//
// Coverage: one Ok per scenario family and every refused twin, so both arms
// of `compile` replay — fabric (Ok / CoreBudgetExceeded), vfabric (Ok /
// Infeasible over its own twinInventory), router (Ok / DoubleClaimed),
// vwire (Ok / PoolDataplaneMismatch), reference (Ok).
//
// Named invariants: none — conformance is the replayer's diff (dpaa2-verify
// crates/dpaa2-verify/tests/intent_replay.rs). Apalache-marked subset: none.
module intent_replay {
  import intent_types.* from "./types"           // Intent, Inventory
  import intent_inventory.* from "./inventory"   // REF_INVENTORY
  import intent_refuse.* from "./refuse"         // Compiled, compile

  import fabric as fabric from "./scenarios/fabric"
  import vfabric as vfabric from "./scenarios/vfabric"
  import router as router from "./scenarios/router"
  import vwire as vwire from "./scenarios/vwire"
  import reference as reference from "./scenarios/reference"

  // The two `compile` inputs and the model's frozen outcome. One state per
  // trace suffices — `compile` is pure, so the replayer needs no transitions.
  var intent: Intent
  var inv: Inventory
  var outcome: Compiled

  // Binds a scenario's intent and inventory into the frozen state alongside
  // the model's own `compile` result. This is the run's single (initial)
  // action; the ITF trace it emits has exactly this one state.
  action feed(i: Intent, v: Inventory): bool = all {
    intent' = i,
    inv' = v,
    outcome' = compile(i, v),
  }

  run fabricAcceptedTrace    = feed(fabric::accepted, REF_INVENTORY)
  run fabricRefusedTrace     = feed(fabric::twin, REF_INVENTORY)
  run vfabricAcceptedTrace   = feed(vfabric::accepted, REF_INVENTORY)
  run vfabricRefusedTrace    = feed(vfabric::twin, vfabric::twinInventory)
  run routerAcceptedTrace    = feed(router::accepted, REF_INVENTORY)
  run routerRefusedTrace     = feed(router::twin, REF_INVENTORY)
  run vwireAcceptedTrace     = feed(vwire::accepted, REF_INVENTORY)
  run vwireRefusedTrace      = feed(vwire::twin, REF_INVENTORY)
  run referenceAcceptedTrace = feed(reference::accepted, REF_INVENTORY)
}
