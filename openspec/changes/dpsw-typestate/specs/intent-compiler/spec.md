# intent-compiler — dpsw-typestate delta

## ADDED Requirements

### Requirement: The switched-domain construct joins the accepted vocabulary

The intent vocabulary (ADR-0013, amended by this change) SHALL accept
the **switched-domain** construct: N ports — dpmac uplinks and member
dpnis — forming one kernel-steered L2 domain. The pure derivation
compiler SHALL derive, with per-object rule provenance: `num_ifs` =
declared port count; `max_fdbs ≥ num_ifs` (the kernel runs
FDB-per-port); `max_fdb_entries` scaled by ports × the declared MAC
budget; flooding and broadcast forced `PER_FDB` and the control
interface enabled (DPSW-I1 — these are derivation outputs, never user
knobs); companions exactly 1 dpmcp + 1 dpbp placed in the dpsw's own
container (DPSW-I3).

#### Scenario: Dry-run shows the provenance of every forced field

- **WHEN** a switched-domain intent is compiled with dry-run
- **THEN** the plan names each derived field with the rule that set it,
  and the `PER_FDB`/ctrl-if fields cite the bindability law, not user
  input

#### Scenario: The derivation invariants hold

- **WHEN** the derivation invariants run over generated intents
- **THEN** every compiled switched-domain satisfies the DPSW-I1
  predicate and the companion count law

### Requirement: Non-kernel steering of a switched domain is refused typed

The compiler SHALL refuse, with a typed refusal naming the regime law,
any switched-domain whose steering owner is not the kernel: DPDK/VPP
cannot bind a dpsw (DPSW-I6 — the scan discards it silently), so a
VPP-steered switched-domain is an invalid topology, not a deferred one.
A VPP/DPDK tenant SHALL be expressible only as a dpni member of the
domain.

#### Scenario: A VPP-steered domain is refused at compile

- **WHEN** an intent declares a switched-domain steered by a non-kernel
  tenant
- **THEN** compilation returns the typed regime refusal and no plan is
  produced

#### Scenario: A VPP member port compiles

- **WHEN** an intent declares a kernel-steered switched-domain with one
  member dpni owned by a VPP tenant
- **THEN** compilation succeeds and the member's edge is planned as
  switch-port membership
