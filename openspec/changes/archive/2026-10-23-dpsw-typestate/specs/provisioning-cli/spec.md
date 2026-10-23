# provisioning-cli — dpsw-typestate delta

## ADDED Requirements

### Requirement: The CLI exposes a read-only switch detail view

`dpaa2ctl status --detail` SHALL render the dpsw row: attributes,
decoded options (raw escapes rendered by value), and the
per-interface table — endpoint, link state, taildrop, counters — with
every honest-unknown face rendered as unknown, never blank or zero.
Link and counters are display-only and SHALL never gate convergence.

#### Scenario: The detail row renders a live switch

- **WHEN** `status --detail` runs against a container holding a dpsw
- **THEN** the row shows the created cfg and the per-port table, and an
  `Unobservable` port face prints as unknown
