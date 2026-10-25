# Spec Delta

## MODIFIED Requirements

### Requirement: The ensure pipeline is a guarded-action convergence model

`models/orchestration/` SHALL model the ensure pipeline as a
guarded-action system in Quint (ADR-0023): actions are the engine's
passes plus an observation action, an adversarial environment
action, and a rollback action over a run-journal projection (the
executed actuations of the current run); state is the census schema
projection (design D11) — per-
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

#### Scenario: The rollback action unwinds the journal projection

- **WHEN** the environment drains a budget after admission and a
  create's guard refuses mid-run under the rollback policy
- **THEN** the rollback action is enabled, replays the run-journal
  projection in reverse, and the reached state equals the pre-run
  state up to the irreversible rows' residue

### Requirement: The model checks the named orchestration invariants

The model SHALL state and check ORCH-I1 (completeness: every family
with pending plan entries has an enabled-or-converged transition or the
intent is refused typed), ORCH-I2 (order as guards, including the
live-read dpmac occupancy law), ORCH-I3 (admission soundness and stable
refusal, restated under rollback: a mid-run-refused run's net effect is
identity up to typed residue, and while the drained budget holds, the
next admission refuses at planning time — one rollback per drain event,
across both the per-container census ledger and the MC-global pool
ledger, a hardware-side drain judged against the global row of its
family), ORCH-I4 (bounded convergence under eventual environment
quiescence), ORCH-I5 (quiescence: no enabled pass without observable
drift), and ORCH-I7 (containment order: a child-homed family is
converged-or-drifted only while its container dprc is converged);
ORCH-I1–I3, ORCH-I5, and ORCH-I7 SHALL be Apalache-marked within ladder
bounds. ORCH-I6 (confluence) SHALL be stated, with a bounded pairwise
simulator check; the rollback action adds no scheduler choice — the
unwind order is the reverse journal, deterministic.

#### Scenario: Masking a family turns completeness red

- **WHEN** any family's executor transition is masked (its guard forced
  false) with plan entries pending for that family
- **THEN** ORCH-I1 is violated within the checked bound — the
  retro-oracle the change's origin requires

#### Scenario: The handover window is unreachable only via live-read

- **WHEN** the switch-port wire action re-samples dpmac occupancy at
  fire time (live-read, design D9)
- **THEN** no reachable state wires a port over a mac-driver-held dpmac
- **WHEN** the live-read is removed and the guard reads the census
  snapshot
- **THEN** the violating state is reachable — the discipline earns its
  cost inside the model

#### Scenario: Stable refusal survives rollback side effects

- **WHEN** a run rolls back on overcommit and the environment still
  holds the drained budget — whether the drain hit the census ledger or
  the MC-global pool ledger
- **THEN** every subsequent admission refuses at planning time and no
  apply/rollback oscillation is reachable within the checked bound

#### Scenario: Masking the global row re-opens the oscillation

- **WHEN** the MC-global pool ledger is masked so admission re-judges
  the census ledger alone, and the drain held the global ledger
- **THEN** the apply/rollback oscillation is reachable within the
  checked bound — the retro-oracle that the rev-3 blindness was real,
  closed when the global row is present

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
next admission refuses), a teardown trace through the prune lane, and
an overcommit-rollback trace (admission passes, the environment drains
the MC-global pool ledger, the run unwinds, the next admission refuses
on the global row).

#### Scenario: A pass added without a model row fails the twin

- **WHEN** the engine gains a pass the model's transition set lacks
- **THEN** the `dpaa2-verify` registry twin fails

#### Scenario: Budget-law drift fails the replay

- **WHEN** either side's budget arithmetic diverges — the model prices
  a draw the Rust admission does not, or the reverse
- **THEN** the verdict re-derivation disagrees with the frozen verdict
  on at least one trace and the replay fails

#### Scenario: The rollback trace re-derives both verdicts

- **WHEN** the overcommit-rollback trace replays
- **THEN** the mid-run overcommit verdict and the follow-on
  planning-time refusal are each re-derived through the Rust admission
  from the prior state's census and MC-global counts, never trusted
  from trace structure

## ADDED Requirements

### Requirement: The model carries the MC-global pool ledger beside the census

`models/orchestration/` SHALL carry a second budget ledger — an MC-global
pool counter — beside the per-container census budget (ADR-0011). The
environment-adversary action and
any container's creates SHALL draw the global counter, and a create's
mid-run guard SHALL refuse on the global row of its family even when the
census row shows headroom — the hardware-side drain the census cannot
see. The ledger SHALL add no scheduler choice — the rollback unwind
order stays the reverse journal.

#### Scenario: A hardware-side drain refuses on the global row

- **WHEN** a create in another container draws the global bp counter to
  exhaustion while the drawing container's census row still shows
  headroom
- **THEN** a pending bp create's mid-run guard refuses on the global
  row, and after the rollback the next admission refuses at planning
  time on that same global row while the drain holds — bp now (V-CEIL-1,
  V-DPRC-7 rev 2); dpcon and dpmcp join only with their own proving
  observations

#### Scenario: The census ledger keeps judging its own draws

- **WHEN** a draw or claim moves the per-container census without
  touching the global pool
- **THEN** the census row judges it as before — the global ledger adds
  the create-refusal vector, it does not replace census judgment
