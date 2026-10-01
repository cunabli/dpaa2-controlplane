# formal-models — dpmac-typestate delta

## ADDED Requirements

### Requirement: The dpmac family model carries the P4 offer reference shape

`models/families/dpmac.qnt` SHALL model the boot-born offer (ADR-0019
P4) as the reference implementation: the driver-arbitration phase sum
`Offered | KernelOwned | RemoteOwned` (DPMAC-I6) with typed transitions
judged from observation (endpoint + driver-face read-backs), never
commanded; the two directional MC link channels as distinct named types
(DPMAC-I4) with the requests-down channel typed `Unreadable` on the
restool transport; a firmware-version-indexed counter vocabulary
(DPMAC-I7: the 28 rows of MC 10.39 readable, the 10.40 extension named
but unread); and the MAC address as an immutable value (DPMAC-I2). The
model SHALL NOT add a create action for the family
(`creatable: false`, DPMAC-I1) and the core machine SHALL be unchanged.

#### Scenario: The ladder is green on the grown model

- **WHEN** the model CI ladder runs on `families/dpmac.qnt` and its
  instantiations
- **THEN** typecheck and simulate pass, and every invariant marked for
  Apalache (DPMAC-I2, I3, I6, I7 encodings) checks within the ladder's
  bounds

#### Scenario: The two link channels cannot be conflated

- **WHEN** any action writes or reads link information in the model
- **THEN** it names exactly one of the two channel types, and no
  expression exists that reads the requests-down channel's value on the
  current transport

### Requirement: The connection surface types per-edge-kind teardown laws

The connection surface SHALL carry a teardown-order law per edge kind
(the ADR-0019 edge facet, beside the ADR-0009 `legalPorts` guard). For
the dpni–dpmac edge kind: `sever` is enabled only from `KernelOwned`,
consumes it, and yields `Offered` plus a severed witness; the dpni
kernel-face unbind of a dpmac-connected dpni requires that witness. No
other edge kind SHALL gain a law in this change.

#### Scenario: The driverless-port sequence is unreachable

- **WHEN** Apalache checks the sever-order invariant on the dpni–dpmac
  edge
- **THEN** no reachable state holds an unbound dpni whose dpmac edge
  was severed after the unbind — the ADR-0008 §8 hazard sequence does
  not exist in the state space

#### Scenario: Other edge kinds are untouched

- **WHEN** the existing connection-surface tests and frozen traces for
  dpdmux, dpci, and dpsw edges replay
- **THEN** they pass unchanged — no witness demand and no new guard
  applies to any non-dpmac edge

### Requirement: The dpmac coverage rows move from deferred to modeled

`models/COVERAGE.md` SHALL account for DPMAC-I2, I3, I4 (reachable
half), I6, and I7 as modeled with their checking rung named, carry the
phantom-create face's disposition from the V-DPMAC-2 verdict, and
re-anchor the out-of-scope rows loudly: the requests-down channel and
MC-view link read to `mc-portal-backend` (#10), DPRTC-I4 off this
change. Frozen traces for the new behaviors SHALL be committed beside
the model.

#### Scenario: The ledger lint holds after the sync

- **WHEN** the ledger lint cross-checks COVERAGE rows against model
  annotations and recorded verdicts
- **THEN** every DPMAC row names an existing encoding or an explicit
  anchor, and no row claims a verdict VERDICTS.json does not hold
