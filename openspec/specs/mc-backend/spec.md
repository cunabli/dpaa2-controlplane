# mc-backend Specification

## Purpose
TBD - created by archiving change add-dpaa2-provisioning. Update Purpose after archive.
## Requirements
### Requirement: Southbound is split into MC control and kernel control ports
The system SHALL define two southbound ports as traits in `dpaa2-api`:
`McControl` for fsl-mc object operations (observe, create, connect, set MAC,
disconnect, destroy) and `KernelControl` for kernel-side concerns (driver bind and
observing netdev appearance). The reconciler core SHALL depend only on these traits,
not on any concrete implementation.

#### Scenario: Core depends only on traits
- **WHEN** the reconciler is compiled
- **THEN** it references `McControl`/`KernelControl` and no `restool` or ioctl types

### Requirement: restool shim implements McControl
The `dpaa2-mc` crate SHALL provide a `restool`-backed implementation of `McControl`
that shells out to the `restool` binary and parses its output. This implementation
SHALL introduce no `unsafe` code and SHALL keep the workspace `unsafe_code = forbid`
lint intact.

#### Scenario: Observe reflects real MC state
- **WHEN** `observe` is called against a live MC
- **THEN** it returns the current objects and connection edges as an
  `ObservedTopology`, sourced from `restool`

#### Scenario: No unsafe in phase 1
- **WHEN** `dpaa2-mc` is built
- **THEN** it compiles under `unsafe_code = "forbid"`

### Requirement: MC operations are expressed at MC-command granularity
The `McControl` trait SHALL expose operations at the granularity of individual MC
commands (create one object, connect one edge, etc.), not at "provision a whole
port" granularity, so that a future ioctl implementation maps one-to-one onto MC
firmware commands behind the same trait.

#### Scenario: Connect is a single-edge operation
- **WHEN** the executor connects a DPNI to a DPMAC
- **THEN** it issues one `McControl` connect call for that single edge

### Requirement: Binding and netdev observation live in KernelControl
`KernelControl` SHALL perform driver binding via the kernel's sysfs bind interface
where required, and SHALL observe netdev appearance for a given DPNI. Where a
connected DPMAC is fixed-link and `dpaa2-eth` does not bind, `KernelControl` SHALL
report that no netdev exists rather than fail.

#### Scenario: Fixed-link port reports no netdev
- **WHEN** a DPNI is connected to a fixed-link DPMAC that `dpaa2-eth` does not bind
- **THEN** `KernelControl` reports the absence of a netdev without erroring

