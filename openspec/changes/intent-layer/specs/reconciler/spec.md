## MODIFIED Requirements

### Requirement: Topology is modeled as an object graph with lifecycle state
The `dpaa2-api` crate SHALL model the topology as a graph of typed MC
objects, each carrying a provisioning lifecycle state (at minimum:
Absent, Created, Connected, Bound), plus connection edges and container
memberships. The desired side of the graph SHALL be the compiled plan:
every derived object with its provenance — produced by `compile` from
an `Intent` and an `Inventory`, or built programmatically through the
plan's public witness-taking constructors; `reconcile` SHALL NOT depend
on `Intent`. The model SHALL
be backend- and frontend-neutral (no ioctl, no serde) and SHALL admit
every object family the plan derives (dprc, dpni, dpmac, dpio, dpbp,
dpcon, dpmcp, dpseci, dpsw) without redefinition.

#### Scenario: Object carries lifecycle state
- **WHEN** observed state is constructed for a managed DPNI connected
  to its DPMAC but not yet bound
- **THEN** the object's lifecycle state is Connected

#### Scenario: A plan built by hand reconciles
- **WHEN** a library user builds a `DesiredTopology` through the plan
  constructors without an `Intent`
- **THEN** `reconcile` accepts it and the relationship locks still hold

#### Scenario: Desired is a compiled plan
- **WHEN** a `DesiredTopology` value exists
- **THEN** every object in it carries the rule, construct, and evidence
  anchor that derived it

## ADDED Requirements

### Requirement: Plan-only objects are reported, not reconciled as drift
`reconcile(desired, observed)` SHALL execute transitions only for the
object families it has executors for, and SHALL report every other
derived object as plan-only — present in the desired plan, awaiting
the change that adds its executor — never as drift, never as an error,
and never as a reason to refuse the executable subset.

#### Scenario: A userspace-poll plan against today's executors
- **WHEN** a compiled plan holds a child DPRC, dpios, dpbps, dpmcps,
  and two dpni↔dpmac ports, and only the dpni↔dpmac executor exists
- **THEN** the plan contains transitions for the two ports and a
  plan-only report listing the remaining objects by family and count

#### Scenario: Plan-only objects do not block convergence
- **WHEN** the executable subset is converged and plan-only objects
  remain
- **THEN** `is_converged` is true and the plan-only report is still
  emitted
