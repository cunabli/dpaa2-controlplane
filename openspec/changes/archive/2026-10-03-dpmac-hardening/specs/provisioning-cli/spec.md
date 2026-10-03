# provisioning-cli — delta

## ADDED Requirements

### Requirement: Counter rows render under their verbatim board names
The port-detail view SHALL label each counter value with the verbatim
restool row name carried in the readout. Positional pairing of board
values against the model's representative name slice is forbidden — it
showed the operator wrong labels on real values (review synthesis
MERGED-2: "IngressByteCount" against the rx-all-frames row).

#### Scenario: A non-uniform readout renders truthfully
- **WHEN** the scripted readout carries distinct values on the pause and byte rows
- **THEN** the rendered row for each name shows that row's value, asserted by test at the pause position

### Requirement: Port-detail inference is a pure family judgment
The port-detail view's derivations — the root-peer observation alphabet
and the handling of an absent MAC — SHALL live as pure judgments in the
dpmac family module, consumed by the shell, so no display consumer
re-derives them and no shell code manufactures an observation (the
`MacAddr::ZERO` coercion) on the surface whose law is absence ≠ zero
(review synthesis MERGED-5; display-only discipline of dpmac-typestate D7
unchanged).

#### Scenario: The shell holds no judgment
- **WHEN** the status path builds port details from root-scoped observation
- **THEN** the peer-observation draw and absent-MAC handling are calls into `families/dpmac.rs`, and no alphabet constructor or zero-MAC sentinel appears in the tools crate
