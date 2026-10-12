# mbt-harness — dpsw-typestate delta

## ADDED Requirements

### Requirement: Conformance twins bind the dpsw typestates to the model

`dpaa2-verify` SHALL carry MBT twins for the dpsw lifecycle and the two
edge kinds: property twins over the pure operators (bindability
predicate, census judgment, port-bound connects) and frozen-trace
replay of the committed ITF traces through the Rust core. Every
committed trace's `lastVerbs` SHALL resolve against the ioctl-policy
module as the existing conformance gate requires.

#### Scenario: Frozen traces replay green

- **WHEN** the ITF replayer runs the dpsw traces against the Rust core
- **THEN** every step's state and verb set match the model's

### Requirement: The board program settles the dpsw unknowns inside the envelope

The suite generator SHALL render the dpsw program (continuing the
V-DPSW series): a discovery sitting probing what `dpsw_reset` preserves
(DPSW-I4), the bpid-vs-object-id law with the live switch consumer
(DPSW-I5, extending DPBP-I5), and the FDB-budget question
(driver convention vs MC-enforced; `docs/baseline/dpsw.md` unknowns);
and a witness sitting for the end-to-end typed lifecycle. Suites SHALL
declare traffic class per ADR-0003, assert the reference pair before
running, and leave the banked V-DPSW-1 / V-DPSW-4 verdicts standing.

#### Scenario: A divergence feeds the model, not the harness

- **WHEN** a discovery step's read-back diverges from the model's
  prediction
- **THEN** the run records the divergence as a finding against the
  model or baseline, and no harness-side tolerance hides it

### Requirement: Board sittings carry the passive ioctl-payload capture tap

Every suite this change renders SHALL include the capture tap
(ADR-0004 §8): the restool-emitted MC command and response bytes are
recorded per step, observation-only, and committed as the
`mc-portal-backend` (#10) encode-fixture corpus beside the suite
results. The tap SHALL NOT alter the driven steps: a suite rendered
with the tap drives the identical verb sequence as one rendered
without it, and a tap failure fails loudly rather than silently
yielding an empty corpus.

#### Scenario: The tap banks fixtures without touching the run

- **WHEN** a dpsw suite executes with the capture tap
- **THEN** the step outcomes equal an untapped render's, and the
  committed corpus holds one payload record per MC-command-bearing step

#### Scenario: A dead tap is loud

- **WHEN** the tap cannot record (missing tooling or permission)
- **THEN** the suite preamble fails the sitting setup with a named
  error before any mutating step runs
