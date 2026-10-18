# ADR-0023: Orchestration is a guarded-action convergence model with observed capacity

Status: accepted

## Context

The reconciler is layered: pure per-family plans (ADR-0018) under
per-family formal models (ADR-0002, ADR-0019), executed by an imperative
shell that sequences idempotent passes under level-triggered
re-observation (ADR-0008). The layer that sequences the passes has no
formal account of itself: pass coverage over the compiled construct
vocabulary is unchecked (a family the compiler emits and no pass consumes
is dropped silently), ordering constraints live in comment prose, and the
board's finite budgets (per-container object pools, the dpsw FDB table,
dpio seats, portal counts) are counted ad hoc with no admission judgment,
so an unsatisfiable intent surfaces as a mid-convergence errno rather
than a typed refusal. The gap class is proven by occurrence: the
dpsw-typestate change delivered a complete plan surface whose executor
did not exist, and no gate could notice
(openspec change `dpsw-typestate`; bead `dpaa2-controlplane-dbf.19`).

Candidate formalisms for the layer: Petri nets (places/tokens/
transitions), a dedicated Petri-net toolchain, lattice/monotone
fixed-point semantics, hierarchical state machines, and guarded-action
systems (the UNITY/TLA tradition Quint natively expresses). The decisive
observations: the engine's gates are mostly *persistent predicates* over
the observed census (a converged companion is not consumed by the pass
that reads it), which bends token semantics into read-arc workarounds;
the genuinely token-like facts are the finite budgets, which hardware
re-reports on every observation; destroy is not monotone, which bends the
lattice framing; and the execution is serialized by design, which leaves
true concurrency semantics unexercised.

Prior art for the shape: Kubernetes (per-kind reconcilers; scheduling as
requests-vs-allocatable admission), Linux DRM atomic modeset (check
admits the whole desired state against finite hardware before commit)
and `-EPROBE_DEFER` (level-triggered retry on missing dependencies), and
real-time admission tests (schedulability refused before runtime). None
carries a checked formal model into its implementation, and all persist
scheduler state; this system refuses persisted state and already owns a
model-to-twin pipeline.

## Decision

The orchestration layer is modeled as a **guarded-action convergence
system in Quint**: actions are the engine's passes, state is the observed
census plus the compiled plan, guards are the facts a pass needs, and
convergence is the fixed point of nondeterministic fair scheduling.
Quint is the sole carrier; Petri-net vocabulary ("gates", "no dead
transition") is retained as explanatory reading only, and dedicated
Petri-net tooling is rejected.

Guards have exactly two kinds:

1. **Convergence predicates** — persistent facts read from observation,
   drawn from two sources: the MC identity census (existence, signature,
   wiring) and the fsl-mc **bus read-back** of driver occupancy — which
   driver, if any, holds an object.
2. **Capacity counts** — finite budgets read from a typed **capacity
   face** of the census, re-derived from observation on every loop and
   never ledgered between loops (ADR-0008's no-persisted-state law is
   unchanged; the board is the only database).

Driver occupancy is a third predicate *source*, not a third guard kind:
it reports whether a dpmac reads back driver-free, held by the
standalone `fsl_dpaa2_mac` driver, or taken by `fsl_dpaa2_switch`. Its
motivating hazard is the dpmac **handover window**: wiring a switch port
over a dpmac still held by the standalone mac driver can fire a
vendor-kernel use-after-free (docs/upstream/phylink-dpni-churn-crash.md),
so a switch port is wired only while its target dpmac reads back
driver-free, or after an explicit unbind settles.

The modeled obligations are:

- **Completeness (no dead family):** every construct family the compiler
  can emit has an enabled-or-converged executor transition from every
  reachable state. The Rust twin is an exhaustive executor registry over
  the construct families: a family without a pass does not compile, and
  an intent naming a construct the engine cannot reify is refused typed.
- **Order as guards:** every true dependency is a guard (companions
  precede their object, an object precedes its edges, teardown exactly
  reverses, sever precedes rewire, unbind precedes unplug, and a switch
  port is wired only over a driver-free dpmac). The shell's pass
  sequence is a scheduler choice, never a correctness carrier. The Quint
  guarded-action model SHALL carry the driver-occupancy guard, and this
  obligation SHALL check the dpmac handover example: sever and wire are
  ordered so the port never passes through a standalone-mac-held state
  into the `fsl_dpaa2_switch` take-over (the hazard of
  docs/upstream/phylink-dpni-churn-crash.md and the dpsw-typestate
  system-integration deferred reachability clause).
- **Bounded convergence:** from any reachable board state the loop
  reaches its fixed point within the ladder's bounded-eventually
  discipline, and refusals are stable across iterations.
- **Admission:** an intent whose derived draw exceeds observed headroom
  is refused at planning time, named in operator data terms; admission
  never substitutes for re-observation as the dispatch truth.

Confluence — the final census is independent of scheduler order wherever
guards do not force one — is stated in the model; its check is bounded
and pairwise, with full treatment deferred behind a revisit trigger.

## Consequences

- Adding a family to the control plane requires adding its model rows
  and its executor pass; omission is a compile failure and a red
  invariant, not a latent board finding.
- Budget vocabulary grows monotonically: a budget joins the capacity
  face only with the change that proves its observation on the board;
  initial rows are the per-container pools and the dpsw FDB budget.
- The existing pool grow/shrink passes are not re-derived onto the
  capacity face by this decision; any such migration is its own change.
- Dispatch stays serialized. Revisit triggers: concurrent MC-portal
  scheduling becoming real (mc-portal backend work and beyond)
  re-evaluates true concurrency semantics, including whether the
  Petri-net reading earns tooling; confluence checking cost forcing a
  weaker claim re-opens the obligation's scope.
- The dpsw-typestate reachability-traffic witness is deferred to the
  driver-occupancy ordering guard plus the kernel refresh from the
  porting effort; its revisit trigger is both landing (the deferred
  reachability clause in the dpsw-typestate system-integration delta;
  bead `dpaa2-controlplane-dbf.19`).

## References

- openspec change `resource-aware-orchestration` (proposal, design)
- openspec change `dpsw-typestate` (the occurrence that proved the gap
  class; bead `dpaa2-controlplane-dbf.19`)
- ADR-0002 (Quint first), ADR-0008 (level-triggered, no persisted
  state), ADR-0018 (sans-io shell/plan split), ADR-0019 (family model
  patterns), ADR-0003 §7 (grow-only dpio seats)
- docs/upstream/phylink-dpni-churn-crash.md (the dpmac handover-window
  use-after-free the driver-occupancy guard refuses)
