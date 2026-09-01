## MODIFIED Requirements

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
