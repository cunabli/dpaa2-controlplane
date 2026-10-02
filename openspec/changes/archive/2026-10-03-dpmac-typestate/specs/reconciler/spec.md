# reconciler — dpmac-typestate delta

## ADDED Requirements

### Requirement: The dpmac arbitration surface is typestated and observation-judged

`dpaa2-api` SHALL carry `families/dpmac.rs` structurally isomorphic to
the model (ADR-0002): the phase sum `Offered | KernelOwned |
RemoteOwned` with typed transitions, judged purely from read-backs
(connection endpoint plus kernel driver-face observation). The family
SHALL expose no constructor for board objects (P4, DPMAC-I1): values
enter only through observation.

#### Scenario: Arbitration state follows the recorded equation

- **WHEN** observation reads an unconnected dpmac with the standalone
  driver bound
- **THEN** the judged state is `Offered`; a same-container kernel peer
  yields `KernelOwned`; a cross-container peer with the standalone
  driver holding the PHY yields `RemoteOwned` (DPMAC-I6)

### Requirement: The dpni–dpmac edge teardown law holds by construct

The connection surface SHALL type the sever-then-unbind law for the
dpni–dpmac edge kind: `sever` consumes `KernelOwned` and yields
`Offered` plus a severed witness, and the kernel-face unbind of a
dpni whose edge faces a dpmac SHALL require that witness. The demand
SHALL apply to no other edge kind. The planner consumes these types;
it does not own the ordering.

#### Scenario: The illegal order does not typecheck

- **WHEN** a library consumer attempts to express unbind-before-sever
  for a dpmac-connected dpni
- **THEN** the program is rejected at compile time — the sequence that
  strands a driverless port (ADR-0008 §8) is unrepresentable

#### Scenario: Non-dpmac edges are unaffected

- **WHEN** plans tear down dpdmux-, dpci-, or dpni-peered objects
- **THEN** no severed witness is demanded and existing plan behavior is
  unchanged

### Requirement: The port MAC relation is a typed judgment

Observation SHALL classify the relation between a dpmac-connected
dpni's primary MAC and the port as `Inherited` (equals the dpmac's
burned-in address), `Overridden` (equals an intent-declared value),
`Pending` (all-zeros, consumer not yet bound), or `Mismatched`.
`Pending` SHALL NOT classify as drift. `Mismatched` SHALL plan a
mutation only when intent declares a MAC; otherwise it surfaces as an
observation. Intent gains no new field: an absent MAC on a
dpmac-connected port means inherit, and the reconciler's write path is
unchanged from dpni-typestate.

#### Scenario: Bind-timing zeros do not churn

- **WHEN** a dpni is connected to a dpmac but its consumer has not yet
  bound and the primary MAC reads all-zeros
- **THEN** the judgment is `Pending`, no drift action is planned, and a
  later re-observation after bind judges `Inherited` (DPNI-I3 value
  semantics)

### Requirement: Counter reads are vocabulary-typed and never reconcile

Counter observation SHALL return `Known(value)` only for counters in
the firmware version's vocabulary and a typed `NotInVocabulary`
otherwise — a missing counter is unrepresentable as zero (DPMAC-I7).
Counters SHALL NOT participate in reconcile, drift, or convergence
judgments.

#### Scenario: Absence is not zero

- **WHEN** a counter outside the 10.39 vocabulary is requested
- **THEN** the result is `NotInVocabulary`, distinguishable by type
  from a zero reading, and no plan consumes it
