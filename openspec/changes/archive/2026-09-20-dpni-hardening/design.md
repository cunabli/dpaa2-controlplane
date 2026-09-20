# dpni-hardening design

## Context

The change executes the follow-up ledger of the dpni-typestate epic review
(`openspec/changes/dpni-typestate/review/synthesis.md`, sections 1 and 4).
The synthesis is the design authority: each task below cites its ledger rows
and the pass evidence behind them. No new decisions are taken here beyond
sequencing; where a task carries a decision point, the synthesis records the
options and the task records the outcome when it lands.

## Goals / Non-Goals

**Goals:**

- The dpni cfg-drift decision the reconciler spec promises is consumed in
  production, without false drift on unsized port-only creates.
- Every board-verified dpni law is reachable by the frozen-trace replay rung.
- The create hot path classifies every restool exit through the typed funnel.

**Non-Goals:**

- No re-review of the review (the synthesis stands as delivered).
- No CHANGELOG work — release-time concern at the end of the roadmap.
- No new dpni surface: this change only closes gaps the review named.

## Decisions

### D1 — The drift wiring lands as one unit (synthesis rows 1-3)

`plan_present` consuming `drift_disposition`, the unsized-sizing resolution
(core-side from `Inventory.cpus` OR an unsized fence — recorded at landing,
amending `contract/mc.rs:42-45` in the same commit, never before), and the
fake's `Some(DpniObservation::project(cfg))` projection ship together with a
reconcile test proving an unsized-created dpni does not drift. Landing them
separately reintroduces the false-drift misfire the review predicted.

### D2 — Replay faces land parser-arm-first (synthesis row 6)

`raw_escape()` gains the `HasReplication` arm before any trace that carries
it is frozen; then the McClearedFlags scenario, the queue-envelope intent
RefusedTrace, and the Unpriced freeze follow. A trace frozen before the
parser arm fails decode and mis-reports as model↔core divergence.

### D3 — The gate task seals the review (synthesis §4 doc pass)

The amend-at-archive pass (deferral rows, `NUM_QUEUES_HI` model constant,
guu.4a revisit trigger, wording amends, trait-doc + ADR-0018 chain-policy
note) lands on the dpni-typestate record before it archives with its review
directory, mirroring the dprc-hardening gate precedent. The accepted rule
amendment ("a shared bound is named once per artifact and referenced, never
restated as a literal in a twin") joins the review rule set.

## Risks / Trade-offs

- [The sizing-seam decision (core-side vs fence) changes a recorded shim
  contract] → the decision and the contract amend are one commit; the
  synthesis row 2 evidence stays citable either way.
- [Freezing new traces regenerates the corpus] → the bidirectional
  `every_committed_trace_is_listed` guard already fails CI on a mismatch.

## Migration Plan

Gate task first (this session, seals the review and archives
dpni-typestate). Beads A → B → C afterwards in any order except D2's
parser-arm-first ordering inside B; each keeps the quality floor green.

## Open Questions

- Whether `Tenant::dpni`/`Port::terminate` take `NumQueues` in-signature
  (synthesis row 11 optional hardening) — decided inside bead A if the
  sizing seam lands core-side.
