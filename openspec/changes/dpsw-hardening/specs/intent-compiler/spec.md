# Spec Delta

## ADDED Requirements

### Requirement: Fabric membership is bounded at compile
A hardware fabric whose member count drives `max_fdbs` beyond its field
envelope (0..=255) SHALL refuse at compile with a typed fabric refusal
(the MacTableOverflow shape) — never reach dispatch as a construction
error. The bound SHALL hold identically in the model
(`dpswDerivationLawful`) and the Rust refusal surface (ADR-0002).

#### Scenario: A 256-member fabric refuses typed
- **WHEN** a fabric intent declares 256 or more member ports
- **THEN** compilation yields the typed overflow refusal naming the
  fabric, and no `Attributes::Dpsw` is emitted

### Requirement: The bindability provenance anchor cites banked board evidence
The derivation's bindability provenance anchor SHALL cite the banked
board evidence — the V-DPSW-1 positive face and the V-DPSW-4 flooding
refusal face, with the remaining refusal faces named as
kernel-source-read — instead of "read-not-verified". The anchor stays
single-line data (suite ids are data; no law or decision identifiers),
and its Rust and Quint twins move in lockstep.

#### Scenario: The anchor names the suites
- **WHEN** a switched-domain dry-run renders its provenance
- **THEN** the bindability rule's anchor names V-DPSW-1 and V-DPSW-4
  and no anchor in the tree reads "read-not-verified"
