# resource-aware-orchestration — the ensure pipeline gets the rigor the families already have

## Why

Every resource family carries a Quint model, typestates, and MBT twins —
but the orchestrator that sequences them has none. `dpaa2ctl ensure` is a
hand-ordered list of passes in `main.rs` whose coverage nothing checks: a
compiled construct family no pass consumes is dropped *silently*, not
refused. That is not hypothetical — the dpsw-typestate epic shipped four
sealed task groups, an offline operand pin, and a rendered board witness
before the gap was caught (the planner produced the switched-domain plan;
the executor never ran it; bead dpaa2-controlplane-dbf.19 records the
discovery). The same review found capacity is folklore: container pool
draws, the FDB budget, and dpio seats are counted ad hoc in scattered
arithmetic, so an intent that cannot fit the hardware is discovered
mid-convergence as an errno, never refused at admission.

The business outcome this change works backwards from: **adding a family
to the control plane cannot silently miss the executor again, and an
intent that exceeds the board's finite resources is refused typed at
dry-run, naming the headroom.** Roadmap #12 (dpdmux) is the first
consumer: it must incorporate by adding only its own rows and pass, with
the orchestration layer forcing that pass to exist.

## What Changes

- `models/` gains an **orchestration model**: the ensure pipeline as a
  guarded-action convergence system in Quint (the UNITY/TLA idiom the
  family models already use, lifted one level). Actions are the engine's
  passes; state is the observed census plus the compiled plan; guards are
  the gates a pass needs. Petri-net vocabulary (places/gates) is kept as
  the explanatory reading; dedicated Petri-net tooling is rejected
  (ADR-0023).
- Guards come in exactly **two kinds**: *convergence predicates*
  (persistent facts read from the census — "both endpoints of this edge
  read back converged") and *capacity counts* (token-like finite budgets
  — "this create's draw fits the free pool"). Capacity tokens are
  **re-minted from observation every loop, never ledgered** — the
  level-triggered no-persisted-state tenet is load-bearing and unchanged.
- Checked obligations, in scope: **completeness** (no dead family — every
  construct family the compiler can emit has an enabled-or-converged
  executor transition; Apalache-checked, and the Rust twin is an
  exhaustive match that fails to compile when a family lacks a pass),
  **order-as-guards** (companions ≺ object ≺ its edges on create, exact
  reverse on teardown, sever ≺ rewire, unbind ≺ unplug — the `main.rs`
  pass sequence stops being load-bearing), and **bounded convergence**
  (from any reachable board state the loop reaches the fixed point within
  the ladder's bounded-eventually pattern; refusals are stable, never
  oscillating).
- Named follow-on, stated now and checked later inside this change's own
  task list: **confluence** — the final census is independent of pass
  order wherever no guard forces one (bounded pairwise check over
  passes).
- The census gains a **capacity face**: a typed projection of remaining
  budgets, re-derived from the same observations the pool passes already
  parse. First budgets are the two board-proven ones only — per-container
  free pools (dpmcp/dpbp/dpio/dpcon) and the dpsw FDB budget (the
  V-DPSW-2 bracket). Other budgets (bpid space, qbman seats, #10's
  counted portals) join as rows when their changes land, not here.
- `dry-run`/`ensure` gain **typed admission**: an intent whose derived
  draw exceeds observed headroom is refused before any verb is
  dispatched, with the refusal naming the budget and the shortfall in
  operator terms (data-only, the established refusal voice).
- The engine's family dispatch is restructured as the model's Rust twin:
  an exhaustive registry of executor passes checked against the model's
  transition set by the usual `dpaa2-verify` twins and frozen-trace
  replay.

## Capabilities

### New Capabilities

- `orchestration-model`: the guarded-action pipeline model, its checked
  invariants, and the MBT twins against the engine's pass scheduling.
- `capacity-census`: the typed capacity projection and the admission
  refusal vocabulary.

### Modified Capabilities

- `reconciler`: pass dispatch becomes the exhaustive executor registry;
  ordering constraints move from comment prose into guards.
- `provisioning-cli`: dry-run renders admission verdicts (draw vs
  headroom) alongside the plan.

## Validation

The change is done when its own origin story cannot recur, witnessed by:

1. **The retro-oracle**: masking any family's executor transition turns
   the completeness invariant red in Apalache, and the Rust registry
   fails to compile — the dpsw-typestate gap class, pinned as a test
   forever.
2. **The admission oracle**: a committed over-budget intent (pool draw
   and FDB cases) is refused typed at dry-run against banked fixtures
   offline; one board suite row confirms it live on the next sitting
   that runs anyway.
3. **MBT continuity**: orchestration-model frozen traces replay against
   the engine's pass schedule green, the standard `dpaa2-verify` pattern.
4. **The forward proof (DoD)**: roadmap #12 (dpdmux) incorporates by
   adding only its rows and its pass; any orchestration-layer rework
   beyond that is this change's failure, fed back before #12 proceeds.

## Impact

- Affected: `models/` (new orchestration model), `crates/dpaa2-api`
  (capacity projection types, admission refusals), `crates/dpaa2-tools`
  (`engine.rs`/`main.rs` dispatch registry), `crates/dpaa2-verify`
  (twins, traces), docs (ADR-0023; ADR-0013 refusal vocabulary rows).
- Not affected: family models and their frozen traces; the identity
  census; `dpaa2-hal`/`dpaa2-mc` verb surfaces; the dpsw-typestate scope
  (that change ships only its own pass plus the interim exhaustive-match
  guard this model later absorbs).
- Sequencing: proposal/design/ADR are settled now; implementation tasks
  are generated and executed only after dpsw-typestate archives
  (ROADMAP just-in-time law).
