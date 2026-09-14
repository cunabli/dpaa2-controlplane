# formal-models delta: dprc-hardening

## MODIFIED Requirements

### Requirement: Every reachable guard arm has a witness
The frozen-trace corpus SHALL witness the accepted `moveResidentOutAt`
and `unplugResidentAt` transitions, the lock-strip refusal sweep (the
five `Container<Locked>` refusing faces without a trace today), the
`Unlocked::Empty` arm, and the three DPRC-I12 directed runs (states
only; ghost fields stay model-side) — existing runs frozen, no new model
surface, the D8 fingerprint untouched (review M9: PASS2-F7/F8/F9).

#### Scenario: The replay corpus is self-accounting
- **WHEN** the trace inventory test runs
- **THEN** every committed trace is listed, every listed trace replays green, and the corpus holds 17+ traces

### Requirement: The replay asserts the refusal vocabulary, not itself
The ITF replay SHALL map each `Attribution` arm to its expected MC
status and assert the observed status against that map — a mutated
attribution function fails the suite — and a phase/variant mismatch in
the refusal check is a loud finding, never a silent skip (review M6:
PASS2-F1).

#### Scenario: A wrong attribution fails replay
- **WHEN** `attribute_mc` is mutated to return one fixed arm
- **THEN** `cargo test -p dpaa2-verify --test dprc_replay` fails
