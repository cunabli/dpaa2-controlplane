# provisioning-cli Specification

## Purpose
Define the `dpaa2ctl` binary and the imperative shell that drives observe →
reconcile → act → wait → re-observe to convergence, exposing state and the
desired-vs-actual delta as a first-class surface.
## Requirements
### Requirement: CLI exposes scan, ensure, status, and dry-run
The `dpaa2-tools` binary (`dpaa2ctl`) SHALL provide subcommands to
observe the system (`scan`), reconcile it toward the desired topology
(`ensure`), report status (`status`), and preview actions without
applying them (`dry-run`). It SHALL also expose the MC-readiness probe
(`wait-ready`) the boot unit gates on. `ensure` and `dry-run` SHALL
read the hardware inventory, compile the intent, and then reconcile;
`dry-run` SHALL print the derived plan with per-object provenance
trees — rule, inputs, source construct, evidence anchor, and
request/extra values — before
the transitions it would execute and the plan-only report, and SHALL be
the exact plan that `ensure` would execute. When compilation is
refused, both SHALL print every refusal with its named rule and the
offending construct, exit non-zero, and change nothing.

#### Scenario: Dry-run applies nothing
- **WHEN** the operator runs the dry-run subcommand
- **THEN** the derived plan, its provenance, the planned transitions,
  and the plan-only report are printed and no MC or kernel state
  changes

#### Scenario: Dry-run shows a refusal
- **WHEN** the intent is infeasible against the inventory
- **THEN** dry-run prints the refusal naming the rule, the family, the
  amount needed and the amount available, and exits non-zero

#### Scenario: Provenance names the unmeasured row
- **WHEN** dry-run prints a userspace-poll tenant's dpios
- **THEN** each entry's tree names the rule, the tenant, ADR-0012,
  and the rate-table row marked `unmeasured` that produced T, down to
  the ports whose rates fed the row

#### Scenario: Status exposes observed state and delta
- **WHEN** the operator runs the status subcommand
- **THEN** it prints each managed object's lifecycle state and the
  delta from desired, and exits non-zero if the system has diverged
  from desired

### Requirement: The imperative shell converges asynchronously
The shell SHALL drive an observe → reconcile → act → wait → re-observe loop until
the system converges or a deadline is reached, accounting for the asynchronous
appearance of netdevs after connection/binding. A single invocation SHALL run to
completion.

#### Scenario: Waits for netdev after connect
- **WHEN** a connect transition is applied and the netdev has not yet appeared
- **THEN** the shell re-observes until the netdev appears or the deadline elapses,
  rather than reporting premature success

#### Scenario: Deadline reached without convergence
- **WHEN** the deadline elapses before convergence
- **THEN** the shell exits non-zero and reports which objects did not converge

### Requirement: Runs are idempotent and retry-safe
Re-running the CLI against an already-converged system SHALL make no changes and
SHALL succeed. A run interrupted after partial progress SHALL, on re-run, complete
correctly by re-observing actual state.

#### Scenario: Second run is a no-op
- **WHEN** reconcile is run twice against an unchanged system
- **THEN** the second run applies no transitions and exits zero

### Requirement: Structured, debuggable logging
The CLI SHALL emit structured logs describing observed state, planned transitions,
and applied actions, sufficient to diagnose divergence between desired and actual
state.

#### Scenario: Applied actions are logged
- **WHEN** reconcile applies transitions
- **THEN** each applied transition is logged with its target object and outcome

### Requirement: Dry-run reports the disruption headline and converge gates on `--allow`
`dry-run` SHALL print each planned transition with its disruption class
and the plan's headline — the maximum class over its transitions
(ADR-0015 decision 12). `ensure` SHALL accept `--allow=<hitless|boundary
|disruptive>`, defaulting to `hitless`, and SHALL proceed only when the
plan's headline is within the allowed class; a plan whose headline
exceeds it SHALL be refused with a message naming the headline and the
`--allow` value needed, exit non-zero, and change nothing — no
transition and no `.link` file. Disruptive SHALL never be implied: it is
actuated only under an explicit `--allow=disruptive`.

#### Scenario: Dry-run shows per-transition classes and the headline
- **WHEN** the operator dry-runs a plan that would create and connect
  ports
- **THEN** each transition line carries its class and the block names
  the headline `disruptive`

#### Scenario: Converge refuses a plan above the allowed class
- **WHEN** `ensure` runs with the default allowance against a board that
  needs a disruptive plan to provision
- **THEN** it refuses, names the `disruptive` headline and the
  `--allow=disruptive` needed to proceed, changes nothing, and exits
  non-zero

