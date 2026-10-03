# provisioning-cli — dpseci-hardening delta

## ADDED Requirements

### Requirement: The detail row shows an attributed raw bit by value

The status-detail dpseci row SHALL render an attributed raw-escape bit by
its value beside the named options (never a bare `options=[unknown]` when
the bit's identity is known), closing the asymmetry between the desired
and observed option renders. A genuinely unobservable options face stays
the honest-unknown row this surface already guarantees.

#### Scenario: An escaped bit renders beside the named options

- **WHEN** the portal read decodes a named option plus a raw escape
- **THEN** the detail row shows the named option and the escaped bit's
  value, and a repeated `status` run still plans zero actions

## MODIFIED Requirements

None.
