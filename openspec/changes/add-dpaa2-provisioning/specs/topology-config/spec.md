## ADDED Requirements

### Requirement: Declarative topology is keyed by stable DPMAC anchors
The system SHALL read a declarative topology in which each port is identified by its
static DPMAC anchor and never by an MC-assigned DPNI index. The config SHALL NOT
require the operator to name or predict DPNI indices.

#### Scenario: Port defined by DPMAC
- **WHEN** a topology entry specifies `dpmac = "dpmac.3"`, a target name, and a MAC
- **THEN** the config is accepted and the DPNI index is left unspecified

#### Scenario: DPNI index in config is rejected
- **WHEN** a topology entry attempts to pin a DPNI index (e.g. `dpni = "dpni.3"`)
- **THEN** the system SHALL reject the config with a validation error explaining
  that DPNI identity is derived from the DPMAC edge

### Requirement: Config parses into a backend-neutral desired-state model
The `dpaa2-config` crate SHALL deserialize the on-disk format (TOML) into its own
type and convert it into the backend-neutral `DesiredTopology` defined by
`dpaa2-api`. The neutral model SHALL NOT carry serialization derives, so that an
alternative frontend (e.g. gNMI) can produce the same neutral model.

#### Scenario: TOML converts to neutral model
- **WHEN** a valid `topology.toml` is loaded
- **THEN** it yields a `DesiredTopology` value with no TOML/serde-specific types
  leaking into `dpaa2-api`

### Requirement: Per-port MAC mode is actuate or assert
Each port SHALL declare whether its MAC is **actuated** (the reconciler sets the
DPNI primary MAC) or **asserted** (the MAC is board-provided and only verified).
The default SHALL be assert.

#### Scenario: Assert mode default
- **WHEN** a port entry provides a MAC without a mode
- **THEN** the port is treated as assert-mode (verify, warn on mismatch)

### Requirement: Configuration is validated before use
The system SHALL validate the topology for structural correctness before any
reconciliation, including well-formed DPMAC references, unique interface names, and
well-formed MAC addresses. Validation failures SHALL be reported with actionable
messages and SHALL prevent reconciliation.

#### Scenario: Duplicate interface name
- **WHEN** two ports request the same interface name
- **THEN** validation fails and no reconciliation is attempted

#### Scenario: Malformed MAC
- **WHEN** a port declares a syntactically invalid MAC address
- **THEN** validation fails with a message identifying the offending port
