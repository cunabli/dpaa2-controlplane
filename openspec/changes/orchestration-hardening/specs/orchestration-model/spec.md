# orchestration-model (delta)

## MODIFIED Requirements

### Requirement: The ensure pipeline is a guarded-action convergence model

`models/orchestration/` SHALL model the ensure pipeline as a
guarded-action system in Quint (ADR-0023): actions are the engine's
passes plus an observation action and an adversarial environment
action; state is the census schema projection (design D11) — per-
construct status, edge status, dpmac driver occupancy, the budget
counts, and pending plan multisets — parametric over the family table
shared with `models/core/types.qnt`. Family internals SHALL NOT be
imported; a construct's convergence is one atomic status transition.
The model's budget arithmetic SHALL state the board-proven law and no
other: a switch-port wire draws no FDB unit (V-DPSW-2 — the bracket is
dpsw-object-granular), and dpio seats never enter a refusing fold
(they are spent headroom, ADR-0003 §7). Refusal scope SHALL be
whole-intent: no create fires while an `Unimplemented` family has
pending entries, matching the engine's dispatch-time refusal. Every
member of the status vocabulary SHALL be reachable — an environment
action orphans a construct so the prune lane carries weight. An engine
pass deliberately outside the model's alphabet SHALL be recorded,
with its reason, where the alphabet is stated.

#### Scenario: A new family is a table row

- **WHEN** a construct family joins the control plane
- **THEN** the orchestration model incorporates it by adding its family
  table row and pass action only, with no change to the model's state
  vocabulary or invariants

#### Scenario: The environment interferes between passes

- **WHEN** the environment action perturbs a census fact or budget
  after admission and before a pass fires
- **THEN** the run remains within the model's invariants: the stale
  pass outcome is a refused-verb transition, and the next observation
  re-judges

#### Scenario: A wire never draws the FDB budget

- **WHEN** a run admits an intent whose only pending work is wiring a
  switch port
- **THEN** the gross FDB draw is zero and the wire proceeds under an
  exhausted FDB bracket

#### Scenario: An orphaned construct is pruned

- **WHEN** the environment orphans a converged construct
- **THEN** the prune action is enabled, fires, and the construct
  returns to the vocabulary's missing state — the state is reachable,
  not vacuous

#### Scenario: An unimplemented family refuses the whole intent

- **WHEN** an intent carries pending entries for an `Unimplemented`
  family alongside implementable families
- **THEN** no create fires for any family — the refusal is
  whole-intent, as the engine dispatches it

### Requirement: Orchestration traces replay against the engine

Frozen ITF traces of the model's pass scheduling SHALL replay against
the engine's dispatch through the `dpaa2-verify` twin convention, and a
pass present in the engine without a model action (or the reverse)
SHALL fail the twin. On every verdict transition the replay SHALL
re-derive the verdict: the capacity face and gross draw rebuilt from
the prior state's pending and budget counts, passed through the Rust
admission, and asserted equal to the trace's verdict — structural
agreement alone is not conformance. The frozen set SHALL include an
FDB-side admission refusal (the environment eats the FDB bracket, the
next admission refuses) and a teardown trace through the prune lane.

#### Scenario: A pass added without a model row fails the twin

- **WHEN** the engine gains a pass the model's transition set lacks
- **THEN** the `dpaa2-verify` registry twin fails

#### Scenario: Budget-law drift fails the replay

- **WHEN** either side's budget arithmetic diverges — the model prices
  a draw the Rust admission does not, or the reverse
- **THEN** the verdict re-derivation disagrees with the frozen verdict
  on at least one trace and the replay fails
