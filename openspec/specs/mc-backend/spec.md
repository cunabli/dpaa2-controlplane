# mc-backend Specification

## Purpose
Define the southbound `McControl`/`KernelControl` ports and the phase-1 `restool`
shim that observes and actuates fsl-mc objects behind them, so the core stays free
of any transport.
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
The `McControl` trait SHALL expose object operations at MC-command granularity
(connect one edge, set one MAC, disconnect, destroy) so that a future ioctl
implementation maps one-to-one onto MC firmware commands behind the same trait.
Creating a DPNI is the one coarse exception discovered on the board: `dpaa2-eth`
*allocates* a DPBP, a DPMCP, and one DPCON per queue from a container pool that must
already exist, so `create_dpni` SHALL provision those private dependencies (and top
up the per-core DPIO pool), mirroring `ls-addni`, rather than leave a bare DPNI that
fails at probe.

#### Scenario: Connect is a single-edge operation
- **WHEN** the executor connects a DPNI to a DPMAC
- **THEN** it issues one `McControl` connect call for that single edge

#### Scenario: Creating a DPNI provisions its private dependencies
- **WHEN** `create_dpni` runs against a container missing the driver's pool objects
- **THEN** it provisions a DPBP, a DPMCP, and one DPCON per queue and tops up the
  per-core DPIO pool before the DPNI is plugged

### Requirement: Binding and netdev observation live in KernelControl
`KernelControl` SHALL perform driver binding via the kernel's sysfs bind interface
where required, and SHALL observe netdev appearance for a given DPNI. Where a
connected DPMAC is fixed-link and `dpaa2-eth` does not bind, `KernelControl` SHALL
report that no netdev exists rather than fail.

#### Scenario: Fixed-link port reports no netdev
- **WHEN** a DPNI is connected to a fixed-link DPMAC that `dpaa2-eth` does not bind
- **THEN** `KernelControl` reports the absence of a netdev without erroring

