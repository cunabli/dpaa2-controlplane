# mbt-harness — dpseci-typestate delta

## ADDED Requirements

### Requirement: Conformance twins bind the dpseci surface to the model

`dpaa2-verify` SHALL carry MBT conformance twins for the dpseci cfg and
refusal surface: frozen-trace replay of the model's create/refuse
transitions through the Rust constructors, and property twins for the
length-coupling and range judgments (every model-refused cfg is
constructor-refused, every model-accepted cfg constructs). ITF replay
SHALL be green against the Rust core.

#### Scenario: Refusals match class-for-class

- **WHEN** the frozen refusal traces replay
- **THEN** each model refusal lands on the same Rust refusal class,
  with no Rust-only or model-only refusal in the cfg surface

### Requirement: Suite A is generated inside the safety envelope

The harness SHALL generate one operator suite for the sitting: converge
a `[[crypto]]` intent on a scratch tenant, assert the child dprc and
dpseci read-backs (`info`: queues and all-2 priorities; raw GET_ATTR:
`HAS_CG` — V-DPSECI-2 rev 1), take the VFIO RemoteOwned leg, and tear
down typed with a clean census (V-DPSECI-3 rev 1). Read-only
boot-object hooks ride the same script: GET_API_VERSION (baseline
unknown #2), GET_ATTR on the kernel dpseci (unknown #3), and the boot
dpseci driver-link read. The suite SHALL be self-contained per the
operator-script standing rules, never touch the production VPP child
container, and never unbind the boot dpseci; banked verdicts
(V-LIFE-DPSECI-1 rev 2, V-DPSECI-1 rev 1) are cited, never re-run.

#### Scenario: The sitting mutates only what it created

- **WHEN** Suite A runs end to end
- **THEN** every created object lives under the scratch tenant and is
  destroyed by the suite's own spaced teardown, the boot and
  production dpsecis are only read, and the closing census is clean
