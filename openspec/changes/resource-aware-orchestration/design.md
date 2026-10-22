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
  model-to-twin path. Not invention — synthesis; ADR-0023 carries the
  comparison so the choice is auditable.

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
invariants first (ladder green), registry twin second (absorbing the 6.4
interim guard), capacity face + admission third, CLI rendering last.
Each step lands behind the existing quality floor; no flag days — the
engine's observable behavior changes only by refusing earlier and typed.

## Open Questions

- Whether the registry is hand-written exhaustive match or macro-derived
  from the family list — decided by the first implementation parcel
  against ADR-0018's trait seams.
- Where the FDB budget's observed ceiling comes from on boards without a
  banked bracket (V-DPSW-2 is one board's answer) — characterize vs
  conservative constant, resolved when the admission oracle lands.
