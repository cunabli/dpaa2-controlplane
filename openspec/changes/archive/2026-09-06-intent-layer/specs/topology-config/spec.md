## MODIFIED Requirements

### Requirement: Declarative topology is keyed by stable DPMAC anchors
The system SHALL read a declarative topology that opens with an
`[intent]` table whose mandatory `schema` key names the version, in
which each port is identified by
its static DPMAC anchor and never by an MC-assigned DPNI index, and in
which every other object is derived from the constructs the operator
declares: `[tenant.<name>]` (keyed by tenant name — dataplane, `max_cores`,
an optional `isolation` of `public|restricted|isolated` defaulting to
`isolated`, and an optional `pool` naming a holder),
`[port.<name>]` (keyed by interface name — dpmac, rate, tenant, MAC and MAC
mode), `[link.<name>]` (keyed by link name — two tenant ends,
`interface_a`/`interface_b`), `[fabric.<name>]` (keyed by fabric name — switching
`hardware|software`, forwarded_by, members: ports, tenants, or fabrics), and `[[crypto]]` (tenant, flows).
A `[tenant.<name>]`, `[port.<name>]`, `[link.<name>]`, or `[fabric.<name>]` table
keys identity in the TOML structure (ADR-0015 decision 1), so a duplicate name
within a family is unrepresentable — a key redefinition — never validated; only a
name shared ACROSS families (a port and a link, or a fabric) is still a validated
`DuplicateName` refusal. `[[crypto]]` remains an ordered array (genuinely
anonymous). A `[[crypto]]`
array-of-tables is ordered, so a tenant's blocks are read in declaration
order: the Nth `[[crypto]]` block for a tenant sizes that tenant's Nth
dpseci (ordinal N), each dpseci by its own block's `flows`. A block's
`flows` is `1..16` — one dpseci carries at most 16 queue pairs, so a
larger demand is refused, not clamped, and the remedy is splitting it
across blocks. The config SHALL NOT require
or accept the operator naming a DPNI index or any dpio, dpbp, dpcon,
dpmcp, queue or worker count; `[extra.<tenant>]` tables of `family =
count` pairs are the only object-level numbers, they add on top of the
derived request, and they are accepted only for the four companion
families dpio/dpbp/dpmcp/dpcon with a `count` of at least 1. A duplicate
(tenant, family) is unrepresentable — a TOML key redefinition — never
validated. A nameable construct (`[tenant.<name>]`, `[port.<name>]`,
`[link.<name>]`, `[fabric.<name>]`) MAY carry `renamed = { from = "<old>" }`
declaring the construct's prior name (ADR-0015 decision 10) — a temporary,
self-neutralizing widening of the matcher's acceptance set consumed by the
converge, not the frontend.

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
- **WHEN** a tenant entry attempts `dpio = 10` outside an
  `[extra.<tenant>]` table, or any entry names a worker count
- **THEN** the system SHALL reject the config with a validation error
  naming the field and stating that the count is derived

#### Scenario: A port without a tenant belongs to the kernel
- **WHEN** a `[port.<name>]` entry names no tenant
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
references, construct names unique across families (an intra-family
duplicate being a TOML key redefinition the format refuses), well-formed MAC
addresses, tenant references that resolve — the reserved `kernel`
resolving at a `[link.<name>]` end without being declared — the reserved
`kernel` name not declared as a `[tenant.kernel]` table, link ends that name two
distinct tenants, and fabric members that exist. A `restricted` tenant
SHALL name a `pool`, and a `pool` SHALL be named only on a `restricted`
tenant. A declared construct name SHALL be a valid Linux interface name —
at most 15 characters, carrying no `/` and no whitespace, and neither `.`
nor `..` — and SHALL NOT match the reserved `family.N` pattern (any MC
family token dotted with digits, e.g. `dpni.4`), so a name serves losslessly
as its own restool label and can never be mistaken for a runtime handle
(ADR-0015 decision 13). A `renamed = { from = "<old>" }` clause's `from` value
SHALL satisfy that same interface-name rule, and a `from` naming a
currently-declared construct that is not itself renamed away SHALL be refused —
so no object is claimed twice (ADR-0015 decision 10), while a swap (each end
renamed to the other) and a chain (each link's target renamed onward) are
admitted. Validation failures SHALL be reported with actionable
messages and SHALL prevent compilation.

#### Scenario: Duplicate interface name
- **WHEN** two ports request the same interface name
- **THEN** validation fails and no compilation is attempted

#### Scenario: Malformed MAC
- **WHEN** a port declares a syntactically invalid MAC address
- **THEN** validation fails with a message identifying the offending
  port

#### Scenario: Unknown tenant reference
- **WHEN** a port names `tenant = "router"` and no `[tenant.router]`
  table exists
- **THEN** validation fails naming the port and the missing tenant

#### Scenario: Reserved name declared
- **WHEN** a `[tenant.kernel]` table is declared
- **THEN** validation fails stating the name is reserved for the root
  dataplane

#### Scenario: Name is not a valid interface name
- **WHEN** a declared construct name exceeds 15 characters, contains `/`
  or whitespace, is `.` or `..`, or matches the reserved `family.N`
  pattern (e.g. `dpni.4`)
- **THEN** validation fails naming the construct and the violated rule,
  and no compilation is attempted

#### Scenario: Rename from a currently-declared construct is rejected
- **WHEN** a construct declares `renamed = { from = "<old>" }` and `<old>`
  is itself currently declared and carries no `renamed` clause of its own
- **THEN** validation fails, naming the `from`-target and stating a
  construct cannot be claimed twice (ADR-0015 decision 10)

#### Scenario: A rename swap is accepted
- **WHEN** two constructs in one edit each declare `renamed` naming the
  other (`wan0` from `eth0` and `eth0` from `wan0`)
- **THEN** the config is accepted, since every `from`-target is itself
  renamed away and no object is claimed twice
