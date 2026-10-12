# topology-config — dpsw-typestate delta

## ADDED Requirements

### Requirement: The northbound schema parses the switched-domain construct

The declarative topology schema SHALL parse the switched-domain
construct into the backend-neutral model: a name-keyed construct
(ADR-0015) declaring its steering tenant, its ports (dpmac uplinks and
member dpnis by name), and the optional MAC-budget sizing input. Field
enumerations SHALL be linted against the model vocabulary (ADR-0014).
The schema SHALL NOT expose `flooding_cfg`, `broadcast_cfg`, or the
control-interface flag — those are derivation outputs of the
bindability law, not intent.

#### Scenario: A switched-domain round-trips

- **WHEN** a topology file declaring a switched-domain is parsed and
  re-rendered
- **THEN** the backend-neutral construct carries the declared name,
  tenant, ports, and sizing, unchanged

#### Scenario: A forced field in the file is refused

- **WHEN** a topology file sets a bindability-law field (for example a
  flooding mode) on a switched-domain
- **THEN** parsing refuses with a typed unknown-field error naming the
  construct
