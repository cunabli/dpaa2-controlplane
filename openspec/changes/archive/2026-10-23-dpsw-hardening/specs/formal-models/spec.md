# Spec Delta

## ADDED Requirements

### Requirement: The intent derivation law carries the MAC-table envelope bound
`dpswDerivationLawful` and the compiled-attribute invariant SHALL carry
the `max_fdbs` field-envelope bound (0..=255) so the model refuses the
same fabric shapes the Rust compiler refuses — the bound lands on both
sides in one step, preserving the ADR-0002 isomorphism mid-change.
`fabricAcceptedTrace.itf.json` SHALL be re-frozen once with the synced
provenance anchor; the dpsw and dpmac family trace directories stay
byte-identical.

#### Scenario: The model refuses what the compiler refuses
- **WHEN** the intent model derives a fabric with a member count beyond
  the envelope
- **THEN** `dpswDerivationLawful` rejects it and the refusal action
  mirrors the Rust typed refusal

#### Scenario: The re-freeze is scoped
- **WHEN** the change completes
- **THEN** `git diff` over `models/traces/families/dpsw/` and
  `models/traces/families/dpmac/` is empty and only
  `fabricAcceptedTrace.itf.json` differs under `models/intent/traces/`
