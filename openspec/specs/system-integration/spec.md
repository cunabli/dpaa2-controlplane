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

