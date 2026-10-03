# provisioning-cli — dpseci-typestate delta

## ADDED Requirements

### Requirement: Status detail shows the crypto surface read-only

`dpaa2ctl status --detail` SHALL render a dpseci row: queue count,
per-queue tx priorities, observed options (`Known` names plus raw-hex
escapes), API version, and binding state. When the ioctl read path is
unavailable the options and version render as explicitly unknown, never
as empty or zero. The view is display-only: no field in it gates
convergence.

#### Scenario: The detail row reads through both transports

- **WHEN** an operator runs `status --detail` with access to
  `/dev/dprc.N`
- **THEN** the dpseci row shows queues and priorities from the restool
  parse and options plus API version from the GET_ATTR read

#### Scenario: Unprivileged status stays honest

- **WHEN** `status --detail` runs without ioctl access
- **THEN** the options and API-version cells state that the read was
  unavailable, and the command exits successfully
