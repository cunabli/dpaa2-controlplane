# mc-backend — dpseci-typestate delta

## ADDED Requirements

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
