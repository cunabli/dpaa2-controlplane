# Design

## Context

The ensure pipeline is the imperative shell over pure plans (ADR-0018
sans-io split): `main.rs` sequences pool grow → dpni loop → consumer
containers → population → links → prunes → shrink, each pass idempotent
and judged by re-observation (ADR-0008 level-triggered law). The sequence
and its coverage are prose: nothing refuses a compiled family without an
executor (the dpsw-typestate discovery, bead dpaa2-controlplane-dbf.19),
and nothing admits an intent against the board's finite budgets before
dispatch. The family models (ADR-0019 patterns) stop at their own
boundary; the layer above them is unmodeled.

## Goals / Non-Goals

Goals:

- One Quint model of the pass pipeline whose invariants make a silently
  dropped family and a guard-violating order unrepresentable in the
  checked model and non-compiling in the Rust twin.
- A capacity face of the census: finite budgets as typed observations,
  admission as a planning-time refusal.
- The model is the frame #12 dpdmux (and later #10) incorporates into by
  adding rows, never by reworking the layer.

Non-Goals:

- No dedicated Petri-net tooling or any second formalism; Quint is the
  only design model (ADR-0023).
- No persisted state of any kind: no allocation ledger, no capacity
  bookkeeping between loops (ADR-0008 holds).
- No migration of the existing pool grow/shrink passes onto the capacity
  face in this change; they keep their delivered shape and join the
  registry as-is.
- No concurrency: dispatch stays serialized; concurrent portal
  scheduling is #10+ territory and an ADR-0023 revisit trigger.
- No new budgets beyond the two board-proven ones (container pools, dpsw
  FDB); bpid space, qbman seats, and portal counts join with the changes
  that prove them.

## Decisions

- **D1 — guarded-action semantics, Quint carrier.** The pipeline is
  modeled as actions (passes) with enabling guards over (observed census,
  compiled plan), nondeterministically scheduled — the same idiom as the
  family models, one level up. A Petri net was evaluated: its tokens are
  consumable, our convergence facts are persistent predicates many passes
  read, so the net degenerates to guard syntax; the gate vocabulary stays
  as documentation. Rejected alternatives: Petri-net tooling (second
  toolchain, no twin path), a pure lattice/monotone framing (destroy is
  not monotone), per-pass statecharts (megastate explosion).
- **D2 — two guard kinds and only two.** Convergence predicates
  (persistent census facts) and capacity counts (finite budgets). The
  counts are re-minted from observation each loop — Petri-net counting
  semantics without bookkeeping, so the no-persisted-state tenet is
  untouched. Convergence predicates read from two observation sources —
  the MC identity census and the fsl-mc bus read-back of driver
  occupancy (which driver holds an object); the bus source adds a
  predicate, not a third kind (ADR-0023). Any proposed third guard kind
  is a design smell to resolve in review, not extend silently.
- **D3 — completeness is a type law in Rust and an invariant in Quint.**
  The engine's executor registry matches exhaustively over the compiled
  construct families: adding a family without a pass fails compilation;
  the model checks "no dead family" (every emittable family has an
  enabled-or-converged transition from every reachable state). The
  interim guard dpsw-typestate ships (task 6.4) is this law's seed and is
  absorbed, not duplicated.
- **D4 — order lives in guards, the sequence is an optimization.** The
  `main.rs` order remains as a scheduler choice, but every true
  dependency (companions ≺ object ≺ edges; reverse on teardown; sever ≺
  rewire; unbind ≺ unplug, the V-LIFE-DPIO-1 law; a switch port wired
  only over a driver-free dpmac) is a modeled guard. The dpmac handover
  window is the model's checked order-as-guards example: sever ≺ wire
  ordered so the port never passes through a standalone-`fsl_dpaa2_mac`-
  held state into the `fsl_dpaa2_switch` take-over, the vendor-kernel
  use-after-free of docs/upstream/phylink-dpni-churn-crash.md (ADR-0023).
  Confluence — census independent of scheduler order where guards allow
  — is a named follow-on task: stated in the model now, bounded pairwise
  check lands inside this change, full treatment is a revisit trigger.
- **D5 — admission refuses at planning, in operator terms.** The derived
  draw of an intent is compared against the capacity face before any verb
  dispatch; the refusal names budget, draw, and headroom as data
  (the established refusal voice: one-line, data-only, no law IDs).
  Admission never substitutes for re-observation: a race between
  admission and dispatch still resolves level-triggered, by the next
  loop's census.
- **D6 — prior art anchors the shape, the ADR records it.** Kubernetes
  (per-kind reconcilers + allocatable-vs-requests scheduling), Linux DRM
  atomic check/commit and `-EPROBE_DEFER`, and real-time admission tests
  are the precedents; the distinctive constraints here are statelessness
  (tokens re-minted, no etcd/state-file analog) and the checked
  model-to-twin path. The precedent also bounds what it can teach:
  Kubernetes is open-world (kinds register at runtime), so a resource
  without a controller sits silently unreconciled and the ecosystem
  mitigates by convention (readiness conditions, GitOps health
  timeouts) — detection, late and optional. This control plane is
  closed-world (one binary, the family vocabulary a compile-time sum
  type), which is what makes prevention — compile-checked totality and
  typed admission — nearly free. Not invention — synthesis; ADR-0023
  carries the comparison so the choice is auditable.
- **D7 — admission is check-without-reservation; the environment is a
  modeled adversary.** No reservation exists between admission and
  dispatch at any layer: MC verbs are individually atomic with no
  multi-verb transaction, and the concurrent writers that matter are
  the kernel's own drivers and the firmware, which no userspace
  exclusion reaches. The correctness claim is therefore conditional —
  an admitted plan converges when the environment holds still; when it
  does not, the next loop re-observes and re-judges, and refusals are
  stable across iterations. The model carries an **environment action**
  that perturbs census facts and budgets between passes, so the claim
  is checked against interference rather than assumed; bounded
  convergence (ORCH-I4) is proven under an eventual-environment-
  quiescence fairness assumption, stated in the model. The loop is the
  validation phase of optimistic concurrency; scratch reservation state
  in any host (userspace file, kernel map) is rejected — it is a ledger
  with a scope problem, and ADR-0008 stands.
- **D8 — admission judges the gross draw of the pending creates.** An
  intent's derived draw is the set of creates the run would dispatch —
  the family passes' own convergence reads decide what is pending, so
  an already-converged object draws nothing and admission admits the
  fixpoint its own convergence produced — compared against currently
  observed headroom; frees the plan itself would produce do not count. The canonical consequence: a
  rebuild (destroy + create of an immutable-drift object) against an
  exhausted budget is refused, and resolves as two converge runs — the
  teardown run frees, the next run admits — matching the engine's
  same-run rebuild refusal (pool-objects design D12). Net admission
  (counting planned frees, which forces free-before-draw scheduling
  semantics) is a named revisit trigger, not a goal; the precedent is
  Kubernetes, whose scheduler is gross-admission with net arriving only
  as priority preemption — a deliberately scoped, complexity-dense
  late addition — while the kubelet re-validates admission at bind.
- **D9 — guard evaluation discipline derives from authority
  alignment.** Guards keep their two kinds (D2); they differ in when
  they are read, and the discipline is computed, never listed. A guard
  is **census-read** (the default: answered from the loop's census
  snapshot) when its observing authority is the verb's executing
  authority — the executor re-validates the fact atomically at
  dispatch, so a stale guard costs one refused verb, repaired by the
  next loop. A guard is **live-read** (re-sampled immediately before
  the dependent verb fires) when its observing authority differs from
  the verb's executing authority, because then no atomic backstop
  exists and the TOCTOU window is real damage, not a wasted verb. The
  derivation maps onto D2's two observation sources: bus-sourced
  predicates gating MC-executed verbs are cross-authority, hence
  live-read. Exactly one guard qualifies today — dpmac driver
  occupancy, where the MC wires a switch port without seeing kernel
  driver bindings and a stale read fires the phylink use-after-free
  (docs/upstream/phylink-dpni-churn-crash.md). All capacity counts are
  census-read: the firmware enforces the real budget at dispatch.
- **D10 — dispatch is compile-checked total; registry rows are
  `Execute` or `Unimplemented`.** Every plan-entry family flows through
  one total dispatch over the family vocabulary — a visitor-shaped
  trait with no default implementations or an exhaustive match (the
  shape is the first implementation parcel's call; a runtime map is
  refused because a missing insertion compiles). A family's row is
  either `Execute` (the pass that consumes it) or `Unimplemented` — a
  typed whole-intent refusal, dispatched before any verb, that names
  its **retirement trigger**: the roadmap item or bead that ships the
  pass. Completeness forbids silence, never unimplementedness; dpseci
  is the first `Unimplemented` row (its compiled create surface has no
  consuming pass; crypto provisioning rides the sibling VPP scripts
  until its own change). The #12 acceptance sharpens accordingly:
  dpdmux lands as `Execute`, so `Unimplemented` cannot become a
  loophole for landing a family without its executor.
- **D11 — the model state is the census schema (interface
  projection).** The orchestration model does not import the core
  machine: its state is the seam the engine can actually read —
  per-construct status (`Missing | Converged | Drifted | Orphan`), edge
  status, dpmac driver occupancy (`Free | MacHeld | SwitchHeld`), the
  budget counts, and the pending plan multisets — parametric over the
  family table so a new family is a table row, never model rework.
  Family internals are trusted as atomic verdicts: their own models and
  twins carry them (assume-guarantee layering; the family models'
  cfg-only precedent). Composition is validated where the layers meet:
  the MBT twins replay frozen traces against the one Rust
  implementation, which exercises both layers at once. The projection
  law: a guard may only need what the census shows, so growing the
  model means growing the census first — the model cannot drift from
  the interface because it is the interface.

## Named invariants

The model's checked obligations, numbered in the family convention:

- **ORCH-I1 — completeness (no dead family).** Every family with
  pending plan entries has an enabled-or-converged executor transition
  or the intent is refused typed; no reachable state drops a family
  silently. Apalache-marked; the Rust half is the D10 totality.
- **ORCH-I2 — order as guards.** Companions precede their object, the
  object precedes its edges, teardown exactly reverses, sever precedes
  rewire, unbind precedes unplug, and a switch port wires only over a
  dpmac whose live-read occupancy is driver-free. The checked example:
  the state "switch take-over fired while the standalone mac driver
  holds the dpmac" is unreachable with the D9 live-read in place and
  reachable with it removed.
- **ORCH-I3 — admission soundness and stable refusal.** An admitted
  plan converges without a capacity errno while the environment holds
  still; a refused intent is refused identically on every subsequent
  loop until the census or the intent changes — refusals never
  oscillate.
- **ORCH-I4 — bounded convergence.** From any reachable state, under
  eventual environment quiescence, the loop reaches its fixed point
  within the ladder's bounded-eventually discipline.
- **ORCH-I5 — quiescence (no churn).** At the fixed point no pass is
  enabled; an enabled pass implies observable drift. No verb ever fires
  whose effect leaves the final census unchanged.
- **ORCH-I6 — confluence (bounded).** The final census is independent
  of scheduler order wherever guards force none; checked pairwise over
  passes, simulator-first, full treatment behind the revisit trigger.

## Risks / Trade-offs

- [Model drift from the real pass list] → the registry is generated-or-
  checked against the model's transition set by a `dpaa2-verify` twin;
  a pass added without a model row fails the twin, mirroring D3.
- [Capacity reads lie or lag (allocator races, grow-only dpio seats)] →
  admission is advisory-refusal only; dispatch truth stays re-observation
  (D5). Known non-reclaimable budgets (dpio seats, ADR-0003 §7) render
  as headroom already spent, never as reclaimable.
- [Scope creep into pool-pass rewrites or all-budget modeling] → Non-
  Goals pin both; new budgets arrive only with the change that proves
  them on the board.
- [Confluence check cost explodes] → bounded, pairwise, simulator-first;
  full check is a revisit trigger, not a gate.

## Migration Plan

Implementation starts only after dpsw-typestate archives. Order: model +
invariants first (ladder green), then the mask-a-family retro-oracle,
total dispatch next (absorbing the 6.4 interim guard), capacity face +
admission, CLI rendering, MBT twins + frozen traces, confluence last.
Each step lands behind the existing quality floor; no flag days — the
engine's observable behavior changes only by refusing earlier and typed.

## Open Questions

- Whether the registry is hand-written exhaustive match or macro-derived
  from the family list — decided by the first implementation parcel
  against ADR-0018's trait seams.
- Where the FDB budget's observed ceiling comes from on boards without a
  banked bracket (V-DPSW-2 is one board's answer) — characterize vs
  conservative constant, resolved when the admission oracle lands.
