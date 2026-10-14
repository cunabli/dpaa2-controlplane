# intent-compiler — dpsw-typestate delta

## ADDED Requirements

### Requirement: The switched-domain construct joins the accepted vocabulary

The intent vocabulary (ADR-0013, amended by this change) SHALL accept
the **switched-domain** construct: N ports — dpmac uplinks and member
dpnis — forming one kernel-steered L2 domain. The pure derivation
compiler SHALL derive, with per-object rule provenance: `num_ifs` =
declared port count; `max_fdbs ≥ num_ifs` (the kernel runs
FDB-per-port); `max_fdb_entries` = declared `mac_table_size`
(learnable MACs per switched domain) × port count — compensating the
kernel's even per-port FDB split — or an explicit 1024 (the reference
manual's family default) when undeclared; the 0 let-MC-decide sentinel
SHALL never be emitted (DPAA2UM self-contradicts on its meaning: 64 on
p. 487 vs 1024 in Table 101/p. 505); flooding and broadcast forced
`PER_FDB` and the control interface enabled (DPSW-I1 — these are
derivation outputs, never user knobs); companions exactly 1 dpmcp +
1 dpbp placed in the dpsw's own container (DPSW-I3).

#### Scenario: Dry-run shows the provenance of every forced field

- **WHEN** a switched-domain intent is compiled with dry-run
- **THEN** the plan names each derived field with the rule that set it,
  and the `PER_FDB`/ctrl-if fields cite the bindability law, not user
  input

#### Scenario: The sizing rule distinguishes declared from defaulted

- **WHEN** a switched-domain is compiled with dry-run
- **THEN** the MAC-table sizing provenance is named `mac-table` when
  `mac_table_size` was declared and `mac-table-default` when it was
  not, and all rendered provenance text is single-line data carrying
  no internal law or decision identifiers

#### Scenario: An overdrawn MAC table is refused typed

- **WHEN** a switched-domain declares a `mac_table_size` whose product
  with its port count exceeds the 16-bit `max_fdb_entries` bound
- **THEN** compilation returns a typed refusal whose message states
  the hardware limit and the operator's own numbers, with no process
  identifiers

#### Scenario: MAC-table sizing on a software fabric is refused

- **WHEN** a software-switched fabric declares `mac_table_size`
- **THEN** compilation returns a typed refusal naming the field as
  hardware-switch sizing the software forwarder owns itself

#### Scenario: An empty switched-domain is refused

- **WHEN** a switched-domain declares zero member ports
- **THEN** compilation returns a typed refusal stating the hardware
  bound (the MC accepts 1–65535 interfaces), closing the only path
  that could derive a 0 `max_fdb_entries`

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
