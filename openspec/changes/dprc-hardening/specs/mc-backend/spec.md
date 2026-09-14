# mc-backend delta: dprc-hardening

## MODIFIED Requirements

### Requirement: The shim observes; the core judges
The restool shim SHALL report container observations raw — resident
rows, plug bits, labels — and SHALL NOT decide lifecycle state or
resident origin: state classification is the core's
(`ContainerState::classify`, beside the existing `VfioBind::classify`
precedent), and an unobservable resident origin is reported as absent
(`Option<ResidentKind>`), never guessed (review M2: PASS3-F1/F2).

#### Scenario: Origin is not invented for an undeclared container
- **WHEN** the shim observes a resident whose create-vs-assign origin restool cannot show
- **THEN** the observation carries no origin claim and the core predicts the eviction post-state conservatively

### Requirement: Every verb exits classified
Every `McControl` verb SHALL return through the single classifying error
exit (typed MC status / client-guard discrimination); raw `Runner::run`
is transport only. A runner outcome with no exit code (signal death) is
a backend error, never a client-guard refusal (review M10: PASS3-F7/F8).

#### Scenario: A killed restool is not a refusal
- **WHEN** a verb's restool process dies to a signal
- **THEN** the error is `Backend`, and no drift report attributes a client guard
