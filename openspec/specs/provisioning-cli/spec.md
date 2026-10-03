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

### Requirement: The CLI exposes a read-only port-detail view

`dpaa2ctl` SHALL render a read-only per-port detail view over the
observed dpmac surface: arbitration state (Offered / KernelOwned /
RemoteOwned), the MAC relation judgment (Inherited / Overridden /
Pending / Mismatched), the link carrier reading (up / down /
NoObservable), and the vocabulary counters. The view SHALL be
display-only: no field in it gates convergence, plans actions, or
participates in drift (link readings lag PHY reality per V-LINK-2;
counters are traffic-dependent). `NoObservable` carrier SHALL render as
the driverless-port diagnosis, not as down.

#### Scenario: A converged port renders its full surface

- **WHEN** the operator requests port detail on a converged
  kernel-regime port
- **THEN** the view shows `KernelOwned`, the MAC relation `Inherited`,
  the carrier reading, and the 28 vocabulary counters, each sourced
  from read-backs, and a repeated `status` run plans zero actions
  regardless of any value shown

### Requirement: Counter rows render under their verbatim board names
The port-detail view SHALL label each counter value with the verbatim
restool row name carried in the readout. Positional pairing of board
values against the model's representative name slice is forbidden — it
showed the operator wrong labels on real values (review synthesis
MERGED-2: "IngressByteCount" against the rx-all-frames row).

#### Scenario: A non-uniform readout renders truthfully
- **WHEN** the scripted readout carries distinct values on the pause and byte rows
- **THEN** the rendered row for each name shows that row's value, asserted by test at the pause position

### Requirement: Port-detail inference is a pure family judgment
The port-detail view's derivations — the root-peer observation alphabet
and the handling of an absent MAC — SHALL live as pure judgments in the
dpmac family module, consumed by the shell, so no display consumer
re-derives them and no shell code manufactures an observation (the
`MacAddr::ZERO` coercion) on the surface whose law is absence ≠ zero
(review synthesis MERGED-5; display-only discipline of dpmac-typestate D7
unchanged).

#### Scenario: The shell holds no judgment
- **WHEN** the status path builds port details from root-scoped observation
- **THEN** the peer-observation draw and absent-MAC handling are calls into `families/dpmac.rs`, and no alphabet constructor or zero-MAC sentinel appears in the tools crate

### Requirement: Status detail shows the crypto surface read-only

`dpaa2ctl status --detail` SHALL render a dpseci row: queue count,
per-queue tx priorities, observed options (`Known` names plus raw-hex
escapes), API version, and binding state. When the ioctl read path is
unavailable the options and version render as explicitly unknown, never
as empty or zero. The view is display-only: no field in it gates
convergence.

#### Scenario: The detail row reads through both transports

- **WHEN** an operator runs `status --detail` with access to
  `/dev/dprc.N`
- **THEN** the dpseci row shows queues and priorities from the restool
  parse and options plus API version from the GET_ATTR read

#### Scenario: Unprivileged status stays honest

- **WHEN** `status --detail` runs without ioctl access
- **THEN** the options and API-version cells state that the read was
  unavailable, and the command exits successfully
