# system-integration — dpsw-typestate delta

## ADDED Requirements

### Requirement: The switched domain is witnessed end to end on the board

One switched-domain intent SHALL converge the board to a kernel-bound
switch and be witnessed structurally end to end: companions drawn
exactly per DPSW-I3 (1 dpmcp + 1 dpbp from the dpsw's own container, 0
dpcon, 0 dpio), the dpsw created bindable (kernel probe succeeds with no
`-EOPNOTSUPP` refusal), an existing face interposed sever-first, ports
connected per the planned edges, idempotent re-convergence, and the
typed observation read back matching the intent. Teardown SHALL converge
to empty through the typed destroy path with read-back proof. This
structural witness SHALL be recorded as the suite verdict with
divergences fed back to the model (the ROADMAP standing DoD,
restool-baseline design D11). The reachability-traffic witness — a
counted broadcast burst across the kernel-owned member port — SHALL NOT
form part of this change's verdict; it SHALL be recorded under its named
revisit trigger (the change #15 driver-occupancy ordering guard of
ADR-0023 plus the kernel refresh; adjudication on bead
dpaa2-controlplane-dbf.19).

#### Scenario: The intent converges and the kernel binds

- **WHEN** the operator runs the witness suite from the committed
  pre-run record
- **THEN** convergence reports done, the `dpaa2-switch` probe is judged
  settled through the bounded-eventually bind discipline (the V-TRAF-1
  rev 3 `bind_settle` pattern — poll with `drivers_probe` / `dprc sync`
  kicks, never an immediate read), and the read-back census equals the
  compiled plan

#### Scenario: The reachability-traffic leg is deferred under its named trigger

- **WHEN** the suite reaches the reachability leg — the counted
  broadcast burst across the member dpni, where the member is
  kernel-owned (kernel-netlink) and switch-port forwarding is
  established by VLAN-aware bridge enslavement performed by the suite,
  not by the convergence engine
- **THEN** that leg SHALL NOT form part of this change's verdict but
  SHALL be recorded as deferred under its named revisit trigger (the
  change #15 driver-occupancy ordering guard of ADR-0023 plus the kernel
  refresh; bead dpaa2-controlplane-dbf.19), because the dpmac handover
  window carries a vendor-kernel UAF
  (docs/upstream/phylink-dpni-churn-crash.md) and the member bind awaits
  the kernel refresh

#### Scenario: An existing port interposes into the domain

- **WHEN** the suite converges a switched-domain over a dpmac that an
  existing dpni already faces
- **THEN** the plan severs the dpni–dpmac edge through the typed
  severed-witness path (the dpmac-typestate edge law) before wiring
  both ends into the switch, and no driverless interval is observed

#### Scenario: Teardown leaves no residue

- **WHEN** the teardown half runs
- **THEN** the container census returns to its pre-suite baseline and
  no typed residue remains
