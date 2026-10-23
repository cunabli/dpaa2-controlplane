# orchestration-model — resource-aware-orchestration delta

## ADDED Requirements

### Requirement: The ensure pipeline is a guarded-action convergence model

`models/orchestration/` SHALL model the ensure pipeline as a
guarded-action system in Quint (ADR-0023): actions are the engine's
passes plus an observation action and an adversarial environment
action; state is the census schema projection (design D11) — per-
construct status, edge status, dpmac driver occupancy, the budget
counts, and pending plan multisets — parametric over the family table
shared with `models/core/types.qnt`. Family internals SHALL NOT be
imported; a construct's convergence is one atomic status transition.

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

### Requirement: The model checks the named orchestration invariants

The model SHALL state and check ORCH-I1 (completeness: every family
with pending plan entries has an enabled-or-converged transition or the
intent is refused typed), ORCH-I2 (order as guards, including the
live-read dpmac occupancy law), ORCH-I3 (admission soundness and stable
refusal), ORCH-I4 (bounded convergence under eventual environment
quiescence), and ORCH-I5 (quiescence: no enabled pass without
observable drift); ORCH-I1–I3 and ORCH-I5 SHALL be Apalache-marked
within ladder bounds. ORCH-I6 (confluence) SHALL be stated, with a
bounded pairwise simulator check.

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

### Requirement: Orchestration traces replay against the engine

Frozen ITF traces of the model's pass scheduling SHALL replay against
the engine's dispatch through the `dpaa2-verify` twin convention, and a
pass present in the engine without a model action (or the reverse)
SHALL fail the twin.

#### Scenario: A pass added without a model row fails the twin

- **WHEN** the engine gains a pass the model's transition set lacks
- **THEN** the `dpaa2-verify` registry twin fails
