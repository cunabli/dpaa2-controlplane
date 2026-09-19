# system-integration Specification

## Purpose
Define how the reconciler is triggered causally on MC readiness, ordered before
network configuration, and how stable interface names are applied via
`systemd.link`.
## Requirements
### Requirement: Reconciler is triggered causally on MC readiness
The reconciler SHALL be started by a systemd unit gated on the appearance of the MC
root container (`dprc.1`) and on a liveness probe that issues an MC command and
retries until it responds, rather than a timer or arbitrary temporal ordering. The
probe SHALL NOT depend on a `firmware_version` sysfs attribute, which is absent on
the target MC. The unit SHALL run to completion.

#### Scenario: Waits for MC readiness
- **WHEN** `dprc.1` is enumerated but not yet responsive
- **THEN** the reconciler does not begin provisioning until the readiness probe
  passes

### Requirement: Provisioning is ordered before network configuration
The provisioning unit SHALL complete before `networkd`/NetworkManager configures the
resulting interfaces, ordered before `network-pre.target`.

#### Scenario: Interfaces provisioned before network stack
- **WHEN** the system boots
- **THEN** DPNI provisioning completes before the network management stack attempts
  to configure the DPAA2 interfaces

### Requirement: Stable naming via MAC match, presentation-only udev
Stable interface names SHALL be applied by `systemd.link` files that match each
port's MAC address and set the desired name. These files SHALL be generated at
runtime from the topology into `/run/systemd/network/` (not shipped as static files
and not produced by a systemd generator). Because a port's matchable MAC lives on its
DPNI and does not exist until the DPNI is provisioned — by which point the kernel has
already named the netdev — the reconciler SHALL generate the `.link` files after
convergence and SHALL apply the rename to the existing interface via a per-interface
udev retrigger. udev/`systemd.link` SHALL be used only for renaming (presentation)
and SHALL NOT be part of the reconciliation trigger or fan-out.

#### Scenario: Netdev renamed by MAC match
- **WHEN** a provisioned DPNI's netdev carries the port's inherited MAC
- **THEN** it is renamed to that port's configured name via the generated
  `systemd.link` file in `/run/systemd/network/`

#### Scenario: Generated link config takes precedence
- **WHEN** a generated `10-dpaa2-<name>.link` and the stock `99-default.link` both
  exist
- **THEN** the generated file wins (first match) and applies the stable name

#### Scenario: udev does not drive reconciliation
- **WHEN** DPAA2 objects are created and emit uevents
- **THEN** no udev rule performs provisioning work in response

### Requirement: Fixed-link ports are handled without a rename stage
The system SHALL treat a fixed-link port as provisioned upon connection where the
connected DPMAC is fixed-link and `dpaa2-eth` does not create a netdev, and SHALL
NOT block or fail waiting for a netdev to rename.

#### Scenario: Fixed-link port needs no rename
- **WHEN** a fixed-link port is connected and produces no netdev
- **THEN** system integration reports it provisioned without attempting a rename

### Requirement: Board milestone covers lifecycle, VFIO, and end-to-end convergence
The change's board milestone SHALL comprise operator-launched suites for: the
full container lifecycle on scratch children (create, populate, plug, lock,
unlock, evict, destroy), an active VFIO bind/unbind on a scratch child
(driver_override, bind, override propagation to a subsequently-added child,
unbind, teardown), and end-to-end consumer convergence (declared intent →
`dpaa2-tools` converges the container; re-run converges to zero actions).
Suites are scratch-first and self-cleaning; use of the live VPP container is
permitted where a face requires it, is deliberate, and is recorded in the
suite's plan. Scripts assert the reference pair (MC 10.39.0 + Linux 6.6.52)
before running.

#### Scenario: End-to-end convergence diffs clean
- **WHEN** the operator runs the convergence suite with one declared consumer
- **THEN** the first run creates the container, the second run plans zero actions, and the read-back matches the derived model

#### Scenario: VFIO suite leaves no residue
- **WHEN** the VFIO suite completes (pass or fail)
- **THEN** its scratch containers and overrides are removed by the suite itself and the board object census matches the pre-suite baseline

### Requirement: The board milestone extends the V-DPNI series and amends the baseline
The change's board milestone SHALL be one operator sitting: a named
batch suite extending the V-DPNI series (option-profile creation walks,
sizing-field probes, `HAS_REPLICATION` accept/reject, unread-flag
probes, primary-MAC mutation) plus the online-MBT learning session.
Every probe outcome SHALL amend `docs/baseline/dpni.md` in the same
change — answers move items off the unknown-register list, silence is
recorded with a revisit trigger — and the restool-unreachable unknowns
SHALL be verified as deferral rows: the runtime `dpni_set_*` surface and
TX_CONFIRMATION_MODE to `mc-portal-backend` (#10) — emit v2 with an explicit
channel index, probe v1-handler retention (register #1) — `num_rx_tcs`-via-
DPL to `dpl-tape-out` (#14), table-write and traffic-dependent items to
their earliest reachable tile.

#### Scenario: Suite results close or defer every targeted unknown
- **WHEN** the board sitting completes and results are diffed
- **THEN** each targeted unknown-register item is either amended in the
  baseline with its evidence tag or carried as a named deferral row, and
  no targeted item is left unaccounted

#### Scenario: Suites leave the board clean
- **WHEN** any dpni suite finishes, pass or refuse
- **THEN** every scratch object it created is destroyed and the
  recovery baseline still verifies

