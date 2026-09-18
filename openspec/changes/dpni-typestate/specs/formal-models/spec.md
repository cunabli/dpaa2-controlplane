# formal-models delta — dpni-typestate

## ADDED Requirements

### Requirement: The dpni family model carries the create-option surface with named invariants
`models/families/dpni.qnt` SHALL grow the create-option surface — the
twelve live options with their ranges, the typed flag vocabulary with the
raw-mask escape, and the two consumer profiles — as model state and
actions, with named invariants covering at minimum: create-range refusal
(no accepted create outside the verified envelope), profile totality
(every `Dataplane` + interface construct maps to exactly one option set),
parity of unrepresentable options (dead options and `num_rx_tcs` never
appear in an accepted create), and the write-only field law
(`dist_key_size` never participates in observation). Invariants SHALL be
Apalache-marked per the DoD model gate, keeping the structural-isomorphism
law (ADR-0002) between the Quint sums and the Rust typestates.

#### Scenario: Model gate runs green before Rust lands
- **WHEN** the CI ladder runs typecheck, simulate, and Apalache on the
  marked dpni invariants
- **THEN** all pass before the corresponding Rust typestates merge

#### Scenario: Profile derivation is total in the model
- **WHEN** the simulator explores intents over every dataplane and
  interface construct combination
- **THEN** every reachable create action carries exactly one derived
  option set and no action carries an operator-supplied option
