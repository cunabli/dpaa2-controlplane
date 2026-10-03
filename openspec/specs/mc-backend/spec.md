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
OBJ_CREATE_ALLOWED gate) are explicitly out of scope, deferred to tile #10.
The `pool-objects` (#6) DPL-defined-child window closed NOT FIRED (bead
dpaa2-controlplane-960.13, 2026-09-27), so DPRC-I8 batch ordering rides bead
dpaa2-controlplane-5y7 on the raw command path (`mc-portal-backend`, #10).

#### Scenario: Bind a scratch child to VFIO
- **WHEN** KernelControl sets `driver_override` to vfio-fsl-mc on a plugged scratch child and binds it
- **THEN** the container is observed bound to vfio-fsl-mc and its IOMMU group exists

#### Scenario: Unbind restores the unbound state
- **WHEN** KernelControl unbinds the scratch child and clears the override
- **THEN** the container is observed unbound and eligible for fsl_mc_dprc again


### Requirement: Every restool exit on the create chain is classified through the typed funnel
The restool shim SHALL route `stamp_label` and `read_inventory` through
`run_verb` so their refusals carry the typed McStatus/RestoolGuard
classification instead of an untyped backend error; no shim verb SHALL
call the raw `Runner::run` directly. The dpni observation mapping SHALL
capture `wriop_version` from the read-back (the board emits it and the
tile #10 num_queues-ceiling question anchors on it). (Review synthesis
rows 7-8.)

#### Scenario: A set-label refusal on the create chain is attributable
- **WHEN** the MC refuses the `dprc set-label` step of a dpni create
- **THEN** the rollback fires on a typed classification naming the
  refusing verb and status, not on an untyped backend error

#### Scenario: The read-back keeps the WRIOP revision
- **WHEN** the shim maps a `dpni info` read-back
- **THEN** `wriop_version` is captured on the raw attribute struct as an
  informational field, outside the pure core's equality

### Requirement: dpni observation maps the read-back asymmetries
The shim's dpni observation SHALL map `dpni_attr`'s asymmetric read-back
into the domain observation type: the split `num_rx_tcs`/`num_tx_tcs`,
the added `qos_key_size`/`fs_key_size`/`wriop_version`, and the omission
of `dist_key_size` (write-only; never synthesized).

#### Scenario: Observation never invents dist_key_size
- **WHEN** a dpni is observed
- **THEN** the observation carries no `dist_key_size` value, and the
  read-back fields map to their domain names

### Requirement: McControl observes a single container by id
`McControl` SHALL provide `observe_container(id)` beside the enumerate
verb, returning the same observation shape for exactly that container,
so per-candidate re-observation does not rescan every root child. The
disposition SHALL cite the OI-3 dpmcp-budget measurement outcome (bead
am0.2): if per-ensure spawns draw the never-returned budget, the seam is
recorded as a leak fix.

#### Scenario: Single-container observation
- **WHEN** the reconciler calls `observe_container(id)` for one child
- **THEN** the shim spawns commands scoped to that container only and
  returns its observation

### Requirement: The restool shim drives the four pool families
The `dpaa2-mc` restool shim SHALL grow create and destroy verbs for
dpmcp, dpbp, dpcon, and dpio, resolving the disposition's count deltas
to concrete object ids: creates carry the family's cfg (dpcon/dpio
priorities, dpio channel mode) computed from the typed block, destroys
target the free individual the adapter selected — reclaim inside a
child container, or a never-plugged undeclared root object under
prune; plugged root capacity is never a destroy target (ADR-0020) —
and read-back is the only observation (exit status never is).
Order-sensitive sequencing that the typed surface does not carry (the
dpio→dpmcp probe draw) SHALL live procedurally in the adapter, per the
dpni set-MAC-before-plug precedent.

#### Scenario: A grow delta becomes N creates
- **WHEN** the reconciler dispatches a deficit of 2 dpcon in a
  container
- **THEN** the shim issues two dpcon creates in that container and the
  post-dispatch census reads back the new count

#### Scenario: A destroy targets only the adapter-selected free object
- **WHEN** the reconciler dispatches a surplus destroy for dpbp in a
  child container
- **THEN** the shim destroys exactly one free dpbp id and re-observes
  the census

### Requirement: The kernel root-bind face brings a dpni alive
The adapter SHALL drive the dpaa2-eth bind of a root-container dpni
whose pool companions are in place, and observe the outcome through
read-back (bus binding state, interface presence): probe success is
judged per-target, never inferred from the bind write alone (DPIO-I5).
A probe deferral for want of companions (-EPROBE_DEFER, the C1 silent
exhaustion class) SHALL map to a typed observation, not an error
swallow.

#### Scenario: Bind with satisfied draw produces a live interface
- **WHEN** a root dpni with its derived companions present is bound to
  dpaa2-eth
- **THEN** the observation reports the binding and the kernel network
  interface exists

#### Scenario: Bind with dry pool is observed as deferral
- **WHEN** a root dpni is bound while a companion family's free pool
  cannot satisfy the draw
- **THEN** the adapter observes and reports the deferred probe with
  the shortfall family, and no object is destroyed or created in
  response

### Requirement: Child-container population serves the VFIO handoff
The adapter SHALL populate a child dprc with a dpni and its derived
pool companions and drive the child's VFIO binding through the
typestates dprc-encapsulation delivered, so the container is
consumable by a userspace dataplane; the population SHALL be
observable as a census of the child matching the derived counts.

#### Scenario: A populated child binds to VFIO
- **WHEN** the reconciler converges an intent declaring a userspace
  tenant's container
- **THEN** the child dprc holds the dpni and the regime-derived
  companion counts and is bound to vfio-fsl-mc, read back from the bus

### Requirement: The restool shim reads the dpmac observation surface

The shim SHALL read dpmac attributes, the MAC address, and counters
through `restool dpmac info` (one spawn per dpmac), parsing against the
firmware-version-indexed vocabulary: exactly the rows the pinned
firmware answers are expected, a deviating row count is a typed
observation (firmware-version signal), and restool's silent counter
skips are never read as zeros (DPMAC-I7). A dead shim spawn — restool's
`assert(false)` on out-of-enum values is a recorded hazard — SHALL
surface as a typed observation failure, never as inherited state. The
shim SHALL NOT drive `set_protocol`, `set_params`, MDIO, or any dpmac
link command: `DPMAC_GET_LINK_CFG` and `DPMAC_SET_LINK_STATE` are
outside the `/dev/dprc.N` whitelist and remain `mc-portal-backend`
(#10) scope.

#### Scenario: The 10.39 counter read round-trips typed

- **WHEN** the shim reads counters on a port under MC 10.39.0
- **THEN** the 28 vocabulary counters return `Known` values, requests
  outside the vocabulary return `NotInVocabulary`, and a row count
  other than 28 is reported as a version-signal observation, not a
  parse error

### Requirement: KernelControl observes link carrier through one sysfs primitive

`dpaa2-hal` SHALL provide one policy-free carrier read resolved per
arbitration state: a KernelOwned port reads its peer dpni's netdev
carrier; Offered and RemoteOwned ports read the standalone driver's
`macN` netdev carrier. A port with neither netdev SHALL read
`NoObservable` — the diagnosis of a driverless port, not an error. The
`macN` route's availability (`CONFIG_FSL_DPAA2_MAC_NETDEVS`) SHALL be
treated as a reference-pair property (ADR-0008 class): asserted where
suites depend on it, degrading to `NoObservable` elsewhere. The
MC-propagated link view (`dpni_get_link_state`) is a distinct signal
deliberately not read by the product in this change; the deferral and
its trigger ride the #10 anchor.

#### Scenario: Carrier resolves per owner

- **WHEN** the carrier primitive reads a KernelOwned port, a
  RemoteOwned port, and a driverless port
- **THEN** it returns the dpni netdev's carrier, the macN carrier, and
  `NoObservable` respectively, with no restool spawn involved

### Requirement: The counter readout carries its verbatim row names
The dpmac counter readout SHALL carry the verbatim restool row name beside
each value instead of dropping the names at the shim: the names are
observed data riding `CounterReadout::Vocabulary`, not a second vocabulary
table. The D4 split is unchanged — the adapter's 28-row verbatim vocabulary
remains the only observable and the model keeps its representative slice —
but no consumer may re-pair values against any other name source (review
synthesis MERGED-2; the row count stays the typed version signal).

#### Scenario: Names survive the shim
- **WHEN** `observe_dpmac` reads the 28-row counter block on MC 10.39
- **THEN** the returned readout pairs each value with the verbatim restool row name it was read under

#### Scenario: The deviation signal is unchanged
- **WHEN** the row count deviates from the firmware vocabulary
- **THEN** the readout still types the deviation as the version signal, names included for the rows that were read

### Requirement: The dpmac info text has one raw scanner
The `dpmac info` rendered text SHALL be scanned by one raw parser from
which the offer/observation/info projections derive, with one token→enum
table per spelling oracle (the `OPTION_BITS` precedent). Three parallel
scanners of the same text are retired (review synthesis MERGED-6a; the
model↔Rust lockstep protection of ADR-0014 does not extend to intra-adapter
copies).

#### Scenario: One spelling oracle
- **WHEN** restool renders a link-type or eth-if token
- **THEN** exactly one table in the adapter maps it, and every projection consumes that mapping

### Requirement: The shim creates and destroys dpsecis over restool

`dpaa2-mc` SHALL dispatch dpseci create with `--num-queues`,
`--priorities` (comma-separated, length equal to the count — restool
refuses a mismatch), the computed options mask from the typed set, and
the target container; destroy SHALL ride the existing creator-bound
path. The `info` parse type for dpseci SHALL carry queue counts and
per-queue tx priorities and SHALL have no options field at all — the
wrong convergence observable does not typecheck (DPSECI-I3). restool's
child-container destroy result is unreliable (the error is overwritten
by `dprc_close`), so destroy verification SHALL be presence read-back,
never exit status alone.

#### Scenario: A planned dpseci renders the mandatory pair

- **WHEN** the shim creates a planned dpseci with 3 queues
- **THEN** the spawned command carries `--num-queues=3`
  `--priorities=2,2,2` `--options=0x20` and the container flag, and
  the result is re-observed, not trusted

### Requirement: The MC-ioctl read primitive is whitelist-read-only by construction

`dpaa2-hal` SHALL gain the `/dev/dprc.N` MC-command primitive as a
typed, policy-free module whose command vocabulary is a closed sum over
exactly OPEN, GET_ATTR, GET_API_VERSION, DPSECI_GET_TX_QUEUE, and
CLOSE — a write command id is unrepresentable. The primitive encodes
the portal command, performs the ioctl (the workspace's single unsafe
confinement, layouts asserted against the pinned kernel and restool
sources), and types three outcomes: a decoded response, an MC status
refusal, and a transport refusal (permission, missing node). All
policy — retry, tolerance, error mapping — SHALL stay in `dpaa2-mc`
(ADR-0018). `mc-portal-backend` (#10) owns all growth of the command
vocabulary.

#### Scenario: The options mask becomes observable

- **WHEN** `dpaa2-mc` reads a dpseci through the primitive
- **THEN** the typed attributes carry the options mask parsed through
  the closed vocabulary (an unknown bit lands as an attributed escape)
  and the API version, neither of which restool's print exposes

#### Scenario: The read path refuses gracefully

- **WHEN** the device node is absent or access is denied
- **THEN** the primitive returns the typed transport refusal and no
  caller can mistake it for an observed attribute value
