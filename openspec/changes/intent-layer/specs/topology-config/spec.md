## MODIFIED Requirements

### Requirement: Declarative topology is keyed by stable DPMAC anchors
The system SHALL read a declarative topology that opens with an
`[intent]` table whose mandatory `schema` key names the version, in
which each port is identified by
its static DPMAC anchor and never by an MC-assigned DPNI index, and in
which every other object is derived from the constructs the operator
declares: `[[tenant]]` (name, dataplane, `max_cores`,
`crypto_flows`), `[[port]]` (dpmac, name, rate, tenant, MAC and MAC
mode), `[[link]]` (two tenant ends), `[[fabric]]` (switching
`hardware|software`, forwarded_by, members: ports, tenants, or fabrics), and `[[crypto]]` (tenant). The config SHALL NOT require
or accept the operator naming a DPNI index or any dpio, dpbp, dpcon,
dpmcp, queue or worker count; per-family limits under a tenant are
the only object-level numbers and MUST NOT fall below the derived
request.

#### Scenario: Port defined by DPMAC
- **WHEN** a topology entry specifies `dpmac = "dpmac.7"`, a name, a
  rate, and an owning tenant
- **THEN** the config is accepted and the DPNI index is left
  unspecified

#### Scenario: DPNI index in config is rejected
- **WHEN** a topology entry attempts to pin a DPNI index (e.g. `dpni =
  "dpni.3"`)
- **THEN** the system SHALL reject the config with a validation error
  explaining that DPNI identity is derived from the DPMAC edge

#### Scenario: A count field is rejected
- **WHEN** a tenant entry attempts `dpio = 10` outside an override
  table, or any entry names a worker count
- **THEN** the system SHALL reject the config with a validation error
  naming the field and stating that the count is derived

#### Scenario: A port without a tenant belongs to the kernel
- **WHEN** a `[[port]]` entry names no tenant
- **THEN** it is owned by the reserved `kernel` tenant in the root
  container

#### Scenario: Missing or unknown schema version
- **WHEN** the file has no `[intent]` table, no `schema` key, or names a
  version this build does not know
- **THEN** the system SHALL reject the config naming the versions it
  accepts

### Requirement: Config parses into a backend-neutral desired-state model
The `dpaa2-config` crate SHALL deserialize the on-disk format (TOML)
into its own types and convert them into the backend-neutral `Intent`
defined by `dpaa2-api`. The neutral model SHALL NOT carry serialization
derives, so that an alternative frontend (e.g. a YANG data tree over
gNMI) can produce the same `Intent`. `ConfigSource::load` SHALL return
`Intent`; compilation to the desired object plan is not the frontend's.

#### Scenario: TOML converts to neutral intent
- **WHEN** a valid `topology.toml` is loaded
- **THEN** it yields an `Intent` value with no TOML/serde-specific
  types leaking into `dpaa2-api`

#### Scenario: The frontend does not compile
- **WHEN** `ConfigSource::load` returns
- **THEN** no derived object exists yet; `compile` in `dpaa2-api`
  produces the plan from the returned `Intent` and an `Inventory`

### Requirement: Configuration is validated before use
The system SHALL validate the topology for structural correctness
before any compilation or reconciliation, including well-formed DPMAC
references, unique interface and tenant names, well-formed MAC
addresses, tenant references that resolve, the reserved `kernel`
name not declared as a `[[tenant]]`, link ends that name two distinct
tenants, and fabric members that exist. Validation failures SHALL be
reported with actionable messages and SHALL prevent compilation.

#### Scenario: Duplicate interface name
- **WHEN** two ports request the same interface name
- **THEN** validation fails and no compilation is attempted

#### Scenario: Malformed MAC
- **WHEN** a port declares a syntactically invalid MAC address
- **THEN** validation fails with a message identifying the offending
  port

#### Scenario: Unknown tenant reference
- **WHEN** a port names `tenant = "router"` and no `[[tenant]]`
  named `router` exists
- **THEN** validation fails naming the port and the missing tenant

#### Scenario: Reserved name declared
- **WHEN** a `[[tenant]]` entry is named `kernel`
- **THEN** validation fails stating the name is reserved for the root
  dataplane
