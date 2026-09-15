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
lint intact. The shim SHALL report container observations raw — resident rows, plug
bits, labels — and SHALL NOT decide lifecycle state or resident origin: state
classification is the core's (`ContainerState::classify`, beside the existing
`VfioBind::classify` precedent), and an unobservable resident origin is reported as
absent (`Option<ResidentKind>`), never guessed (review M2: PASS3-F1/F2).

#### Scenario: Observe reflects real MC state
- **WHEN** `observe` is called against a live MC
- **THEN** it returns the current objects and connection edges as an
  `ObservedTopology`, sourced from `restool`

#### Scenario: No unsafe in phase 1
- **WHEN** `dpaa2-mc` is built
- **THEN** it compiles under `unsafe_code = "forbid"`

#### Scenario: Origin is not invented for an undeclared container
- **WHEN** the shim observes a resident whose create-vs-assign origin restool cannot show
- **THEN** the observation carries no origin claim and the core predicts the eviction post-state conservatively

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

### Requirement: The restool shim exposes the dprc verb surface
`McControl` SHALL expose the dprc verbs the reconciler plans — create, destroy,
assign (child placement and plugged state), unassign, set-label, set-locked —
at MC-command granularity, surfacing MC status codes distinctly (0x4/0x6/0x8
refusal shapes preserved) and surfacing restool's own client-side refusals
(e.g. the plugged-move guard) as typed errors, never as scraped text. Every
`McControl` verb SHALL return through the single classifying error exit (typed
MC status / client-guard discrimination); raw `Runner::run` is transport only.
A runner outcome with no exit code (signal death) is a backend error, never a
client-guard refusal (review M10: PASS3-F7/F8).

#### Scenario: Create returns the created identity
- **WHEN** the shim creates a child DPRC under a parent
- **THEN** the result carries the child's id for re-observation, and the new child reads back unplugged (board-verified default)

#### Scenario: Client-side refusal is typed
- **WHEN** restool refuses a plugged-object move before issuing any MC command
- **THEN** the shim returns a typed refusal distinguishable from an MC status refusal

#### Scenario: A killed restool is not a refusal
- **WHEN** a verb's restool process dies to a signal
- **THEN** the error is `Backend`, and no drift report attributes a client guard

### Requirement: KernelControl actuates VFIO binding for child DPRCs
`KernelControl` SHALL observe and actuate the vfio-fsl-mc binding path for a
child DPRC — `driver_override` write, bind, unbind — and SHALL expose the
observed propagation of the override to subsequently-added children of a bound
container. Restool-unreachable portal faces (child-portal unlock, the
OBJ_CREATE_ALLOWED gate) are explicitly out of scope, deferred to tile #10;
DPRC-I8 batch ordering defers to `pool-objects` (#6), where a DPL-defined child
first reaches pool machinery — earliest reachability wins (review PASS4-F4).

#### Scenario: Bind a scratch child to VFIO
- **WHEN** KernelControl sets `driver_override` to vfio-fsl-mc on a plugged scratch child and binds it
- **THEN** the container is observed bound to vfio-fsl-mc and its IOMMU group exists

#### Scenario: Unbind restores the unbound state
- **WHEN** KernelControl unbinds the scratch child and clears the override
- **THEN** the container is observed unbound and eligible for fsl_mc_dprc again

