# mc-backend — dpmac-typestate delta

## ADDED Requirements

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
