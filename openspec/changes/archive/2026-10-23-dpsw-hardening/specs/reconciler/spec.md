# Spec Delta

## ADDED Requirements

### Requirement: The ensure switch pass executes the api-stated plan
The engine's switch dispatch SHALL consume `plan_switched_domain`'s
typed steps for the create → port-connects → plug ordering and SHALL
match every `InterposeStep` arm of the interposition plan with the
severed proof bound — the ordering law and the DPSW-I10 port-overflow
refusal live in `dpaa2-api` and guard the production path, never
re-stated shell-side. Disruption classing SHALL read
`DriftRepair::class()`, not a shell-side literal.

#### Scenario: An overflowing edge refuses typed before any dispatch
- **WHEN** a compiled switched domain carries a port edge whose
  interface index exceeds the created switch's `num_ifs`
- **THEN** the pass yields the typed port-overflow refusal and no verb
  is dispatched for that domain

#### Scenario: The witnessed recipe is unchanged
- **WHEN** a declared switched domain converges over the fake backend
- **THEN** the step order remains sever-first interposition, then
  create, port connects, and plug last — the V-DPSW-5 rev 6 witnessed
  behavior, now carried by the typed plan
