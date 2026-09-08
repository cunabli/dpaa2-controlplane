# formal-models delta: dprc-encapsulation

## ADDED Requirements

### Requirement: The dprc model carries the container lifecycle and its laws
`models/families/dprc.qnt` SHALL model the child-DPRC lifecycle as a sum type
(the Rust typestates' isomorphic twin), with the board-settled containment laws
as guarded transitions: the per-option-bit permission matrix with its three
distinct refusal statuses, the eviction law (ADR-0007 §3), the visibility law
(DPRC-I6), and the plugged-move precondition (DPRC-I3). Invariants DPRC-I1,
I5, I7, I9, I10 and the remaining face of I11 SHALL have named, simulate-green
properties, with Apalache marks per the DoD model gate, and COVERAGE.md
dispositions updated.

#### Scenario: Model gate green before Rust
- **WHEN** the model changes land
- **THEN** typecheck, simulate, and marked-Apalache runs are green with the invariants named, before dependent Rust merges

#### Scenario: Refusal statuses are distinguishable in traces
- **WHEN** a simulated action violates SPAWN, ALLOC, or a topology/lock gate
- **THEN** the trace records the matching distinct refusal (0x6, 0x8, 0x4 respectively), not a single generic denial

#### Scenario: Eviction law is a transition, not an error
- **WHEN** a modeled destroy hits a non-empty container
- **THEN** the next state removes created residents and re-parents assigned-in residents unplugged, and DPRC-I9 (teardown reachability) still holds
