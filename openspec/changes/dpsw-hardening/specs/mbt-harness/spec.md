# Spec Delta

## ADDED Requirements

### Requirement: The capture preamble names its failing link
The capture-tap preamble SHALL keep refusing loudly before any
mutating step, and its refusal SHALL name the actual failing link in
the chain — a restool spawn failure, an unreadable capture file, or a
genuinely dead tap — never fold distinct causes into one "dead tap"
message. Deliberate timing heuristics in the tap (the flush sleep)
carry a `ponytail:` marker naming the ceiling and upgrade path.

#### Scenario: A missing restool is not a dead tap
- **WHEN** the preamble's read-only self-query cannot spawn restool
- **THEN** the sitting refuses before any step with a message naming
  the spawn failure, distinct from the dead-tap refusal
