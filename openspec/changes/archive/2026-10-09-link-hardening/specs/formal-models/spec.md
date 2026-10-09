# Spec Delta

## ADDED Requirements

### Requirement: The link replay witnesses the refusal against the prior world
The link ITF replay SHALL witness each model-refused action against the
code, not merely assert the sentinel terminal: at a Refused sentinel the
replayer SHALL drive the wire planner with the **prior** world's
observations and assert a non-Connect verdict, binding the Rust refusal
to the model guard (ADR-0002 isomorphism). Replay and surface docs SHALL
pair `LINK_I1` only with its hardware-anchored state face — no edge
outlives its endpoints — never with the retired disconnect-before-destroy
refusal face, which survives only as engine typestate policy. The model
SHALL record the stricter-Rust refinement at the connect guard (the
visible-endpoint witness, design D4) in the recorded-refinement idiom.

#### Scenario: The Refused sentinel drives the planner
- **WHEN** the replay reaches the Refused sentinel of the
  already-connected refusal trace
- **THEN** the planner, driven with the prior world's observed peer,
  returns a non-Connect verdict and the replay asserts it

#### Scenario: No doc pairs LINK_I1 with the retired refusal face
- **WHEN** the connection surface and replay sources are swept for
  `LINK_I1` citations
- **THEN** every citation names the state face, and no site claims a
  model guard that `destroyEndAt` no longer has
