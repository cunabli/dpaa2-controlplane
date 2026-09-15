# intent-compiler delta: dprc-encapsulation

## ADDED Requirements

### Requirement: A declared consumer derives its container
The compiler SHALL derive, for each declared consumer runtime, its child-DPRC
realization: restool-default options mask ({SPAWN, ALLOC, OBJ_CREATE,
IRQ_CFG}_ALLOWED — the board-verified VPP-container mask), placement under the
root container, and a label carrying the consumer's name-keyed identity
(ADR-0015). The kernel tenant remains the root container and never derives a
child (ADR-0005).

#### Scenario: Consumer container derivation
- **WHEN** intent declares a consumer runtime
- **THEN** the derived model contains one child DPRC with exactly the default options mask, root placement, and the consumer's name as label, with per-object rule provenance citing the baseline anchor

#### Scenario: Kernel tenant derives no container
- **WHEN** intent declares the kernel tenant
- **THEN** the derived model contains no child DPRC for it

#### Scenario: Derivation is container-only
- **WHEN** a consumer is derived under this change
- **THEN** no companion objects (DPIO/DPBP/DPCON/DPMCP) or DPNIs are emitted for it; sizing rules remain dormant until tiles #5/#6
