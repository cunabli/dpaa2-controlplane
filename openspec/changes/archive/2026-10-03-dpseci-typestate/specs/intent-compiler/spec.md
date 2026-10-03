# intent-compiler — dpseci-typestate delta

## ADDED Requirements

### Requirement: The compiled dpseci carries its complete create surface

The compiler SHALL complete the planned dpseci so the southbound can
actually create it: priorities derive as the uniform constant
`[2; num_queues]` (the verified deployed profile; the SEC scheduler
semantics of the value are baseline unknown #4, so no intent knob
exists), and options derive as `HAS_CG` only (ADR-0013 unamended — the
safety bit, nothing else; `HAS_OPR`/`OPR_SHARED` are never derived).
The provenance tree on both values SHALL name the rule and its
citation, like every derived value.

#### Scenario: A crypto block compiles to a creatable dpseci

- **WHEN** a `[[crypto]]` block with `flows = 3` compiles
- **THEN** the planned dpseci carries `num_queues = 3`, priorities
  `[2, 2, 2]`, and the option set `{HAS_CG}`, each with rule provenance

#### Scenario: No intent field reaches priorities or OPR bits

- **WHEN** any accepted intent compiles
- **THEN** every planned dpseci's priorities are uniform 2 and its
  options exactly `{HAS_CG}` — no intent vocabulary exists to vary
  either
