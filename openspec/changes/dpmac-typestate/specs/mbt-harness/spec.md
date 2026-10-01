# mbt-harness — dpmac-typestate delta

## ADDED Requirements

### Requirement: Conformance twins bind the dpmac typestates to the model

`dpaa2-verify` SHALL carry MBT twins for the dpmac surface: frozen-
trace replay of the arbitration transitions and the dpni–dpmac edge
teardown law against the Rust types, and property twins for the
vocabulary and MAC-relation judgments. The twins SHALL quantify over
the shared model/Rust shape (ADR-0002 isomorphism), not over rendered
strings.

#### Scenario: The edge-law trace replays through the Rust surface

- **WHEN** the frozen sever-then-unbind trace replays against
  `dpaa2-api`
- **THEN** every step conforms, and the mutated trace with the order
  inverted is rejected by construction (it cannot be expressed against
  the typed surface, which the twin asserts as its negative face)

### Requirement: The board program renders Suite A and Suite B inside the envelope

Suite generation SHALL render two suites under the ADR-0003 safety
envelope and the fixed cabling roles. **Suite A** (end-to-end,
dpaa2ctl-driven in the V-DPRC-9/V-MVP-1 shape): intent anchored on
dpmac.7 converges the kernel regime; hooks read arbitration state, MAC
immutability and the inheritance positive face (dpni primary equals
dpmac burned-in after bind), attribute constancy across the cycle
(DPMAC-I3), the 28-row counter read, and the carrier; teardown runs the
typed sever-then-unbind path; a RemoteOwned leg reads the child/VFIO
arrangement. **Suite B** (V-DPMAC-2): the phantom create
(`--mac-id` with no DPC port entry) contained in a scratch child so the
object is never bus-visible (DPRC-I6) and no kernel driver can reach
it; both outcomes are recorded findings; the suite runs late in the
sitting with its own teardown and census; the root-container face is
recorded as deliberately untaken. Banked verdicts (V-LINK-2, V-LINK-4,
V-DPMAC-1) SHALL be cited, never re-run.

#### Scenario: Suite B contains the phantom

- **WHEN** Suite B issues the phantom create
- **THEN** the create targets the scratch child only, a refusal is
  recorded as the DPC-gated answer to baseline unknown #1, an
  acceptance is followed by attribute read-back and in-child destroy,
  and the closing census reads zero deltas

#### Scenario: Suite A witnesses the typed teardown

- **WHEN** Suite A tears down the converged kernel-regime port
- **THEN** the disconnect is issued while the dpni is bound, the unbind
  follows, the dpmac reads back with the standalone driver re-attached
  (no driverless interval in the read-backs), and the post-suite
  census is clean
