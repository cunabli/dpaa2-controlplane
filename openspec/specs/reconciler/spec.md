# reconciler Specification

## Purpose
TBD - created by archiving change add-dpaa2-provisioning. Update Purpose after archive.
## Requirements
### Requirement: Topology is modeled as an object graph with lifecycle state
The `dpaa2-api` crate SHALL model the topology as a graph of typed MC objects, each
carrying a provisioning lifecycle state (at minimum: Absent, Created, Connected,
Bound), plus connection edges. The model SHALL be backend- and frontend-neutral
(no ioctl, no serde) and SHALL be general enough to admit additional object kinds
(e.g. DPSW) without redefinition.

#### Scenario: Object carries lifecycle state
- **WHEN** observed state is constructed for a managed DPNI connected to its DPMAC
  but not yet bound
- **THEN** the object's lifecycle state is Connected

### Requirement: Reconciliation is a pure function
The core SHALL expose `reconcile(desired, observed) -> Plan` as a pure function that
performs no I/O and is deterministic for a given input. The plan SHALL be an ordered
list of transitions (Create, Connect, Bind, Reconfigure, Disconnect, Unbind,
Destroy) sufficient to move observed toward desired.

#### Scenario: Absent port yields create-then-connect
- **WHEN** desired declares a port whose DPMAC has no connected DPNI in observed
- **THEN** the plan contains a Create followed by a Connect for that port

#### Scenario: Converged state yields empty plan
- **WHEN** observed already satisfies desired
- **THEN** the plan is empty (idempotence)

### Requirement: Level-triggered, hardware-authoritative matching
Reconciliation SHALL treat observed MC state as authoritative and SHALL NOT rely on
any persisted association, marker file, or in-memory memory of prior runs. Managed
DPNIs SHALL be matched to desired ports by their connection edge to the configured
DPMAC, not by index.

#### Scenario: Renumbered DPNI still matches
- **WHEN** a managed DPNI appears at a different index after reboot but is connected
  to the same configured DPMAC
- **THEN** reconcile matches it to the same desired port and plans no change

### Requirement: Ownership is limited to the configured subgraph
Reconciliation SHALL only plan changes to objects reachable from a DPMAC named in
the desired topology. It SHALL NOT enumerate all MC objects and delete those absent
from desired. Objects outside the configured subgraph (e.g. DPL-provisioned or
foreign objects) SHALL be left untouched.

#### Scenario: Foreign object preserved
- **WHEN** the MC contains a DPNI connected to a DPMAC not present in desired
- **THEN** the plan contains no operation affecting that object

#### Scenario: Teardown is opt-in
- **WHEN** a previously-configured port is removed from desired and prune is not
  enabled
- **THEN** the plan does not destroy the corresponding DPNI

### Requirement: Unsafe drift is reported, not silently repaired
Reconciliation SHALL report drift and refuse the change when observed differs from
desired on an immutable (create-time-only) attribute, rather than plan a
destroy-and-recreate of a live interface.

#### Scenario: Immutable attribute mismatch
- **WHEN** desired requires an immutable DPNI attribute value that differs from the
  live object
- **THEN** reconcile reports drift for that object and plans no destructive change

### Requirement: Assert-only intent is verified, not actuated
Fields declared assert-only (e.g. link speed, board-burned MAC) SHALL be compared
against observed reality and reported on mismatch, and SHALL never produce an
actuating transition.

#### Scenario: Asserted MAC mismatch
- **WHEN** a port's MAC is assert-mode and the live DPNI MAC differs
- **THEN** reconcile reports a mismatch and plans no MAC write

