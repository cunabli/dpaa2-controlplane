# formal-models — dpsw-typestate delta

## ADDED Requirements

### Requirement: The dpsw family model carries the P1 phase-machine shape with the reset facet

`models/families/dpsw.qnt` SHALL model the created-configured switch as
an ADR-0019 P1 phase machine with the reset facet (`resetOnBind`): a
create block whose twelve `dpsw_cfg` fields are create-time immutable,
carrying the kernel-bindability predicate as type law (DPSW-I1: ctrl-if
enabled ∧ flooding `PER_FDB` ∧ broadcast `PER_FDB` ∧
`max_fdbs ≥ num_ifs`) so a kernel-steered switch that the driver would
refuse at probe is unrepresentable, never repaired. The model SHALL NOT
treat the vendor recipe as an oracle (DPSW-I2): `ls-addsw` defaults
(`PER_VLAN`/`PER_OBJECT`) are a refused shape for kernel-steered intent.
Convergence SHALL be a multiset census on the observable cfg signature
(the dpseci-typestate design D9 class).

#### Scenario: The ladder is green on the grown model

- **WHEN** the model CI ladder runs on `families/dpsw.qnt` and its
  instantiations
- **THEN** typecheck and simulate pass, and every invariant marked for
  Apalache (the DPSW-I1 predicate encoding and the census law) checks
  within the ladder's bounds

#### Scenario: The unbindable switch does not exist in the state space

- **WHEN** Apalache checks the bindability invariant
- **THEN** no reachable state holds a kernel-steered dpsw whose cfg
  violates the DPSW-I1 predicate

### Requirement: The dpsw invariant dispositions are named at the model gate

The change SHALL dispose every baseline candidate by name: **modeled
with Apalache encodings** — DPSW-I1 (bindability predicate), DPSW-I2
(vendor-recipe refusal shape), DPSW-I3's draw law (the probe-consumer
requirement below), DPSW-I7 (census-only convergence as the judge
operator); **modeled as the reset facet with the conservative
assumption** — DPSW-I4 (`dpsw_reset` treated as total until the
discovery sitting refines it; the facet's scope is a model parameter,
not a blank); **board-settled** — DPSW-I3's defer face, DPSW-I4's
answer, DPSW-I5 (bpid ≠ object id, extending DPBP-I5 with the live
consumer); **enforced at compile** — DPSW-I6 (regime refusal,
intent-compiler delta); **consumer-owned anchor** — DPSW-I8
(ADR-0004 §8). Two new laws are minted with ids and added to the
baseline candidate table at close-out: **DPSW-I9** (the probe is a pool
consumer; defer-not-fail on starvation) and **DPSW-I10** (switch-port
membership is bounded by the immutable `num_ifs`). Pure operators
follow the dpseci-hardening convention: hand-twin-sufficient recorded
in COVERAGE; world-mutating surfaces trace-frozen and replayed.

#### Scenario: Every candidate row has a named disposition

- **WHEN** the model gate closes (before Rust lands)
- **THEN** each of DPSW-I1…I10 appears in COVERAGE with its rung —
  encoding, facet parameter, suite id, compile law, or anchor — and no
  row is implicit

### Requirement: The dpsw probe is a modeled pool consumer

The model SHALL represent the kernel bind as a resource consumer
drawing 1 dpmcp + 1 dpbp from the container's pool census (DPSW-I9,
grounding DPSW-I3), with the missing-dpbp face a silent probe defer,
never a failure — the V-TRAF-1 rev 1 root-cause class (a bus-visible
consumer the model did not represent stole the census) made
unrepeatable for this family.

#### Scenario: A census competitor is visible to the model

- **WHEN** the model simulates a dpsw bind in a container whose pool a
  second consumer has drained
- **THEN** the bind sits in the deferred state and no invariant reads
  the switch as bound; restoring the pool lets the bind settle

### Requirement: The two dpsw edge kinds fill their waiting table rows

`core/connect.qnt` SHALL give the two ADR-0022 placeholder rows their
laws: `dpni ↔ dpsw.N.M` (switch-port membership) and
`dpsw.N.M ↔ dpmac` (switch uplink), each with its reification policy
and connect/disconnect faces, port index bounded by the create-time
`num_ifs` (DPSW-I10). Neither kind SHALL gain a teardown-witness demand — the
baseline records no teardown hazard for them — and no other edge kind's
law SHALL change.

#### Scenario: Port membership is bounded by the create block

- **WHEN** any action connects an endpoint to `dpsw.N.M`
- **THEN** `M < num_ifs` holds in every reachable state, and a connect
  beyond the created port count is refused in the model, matching the
  immutable `num_ifs`

#### Scenario: Existing edge kinds replay unchanged

- **WHEN** the frozen connection-surface traces for dpni–dpmac,
  dpni–dpni, and dpdmux edges replay
- **THEN** they pass unchanged

### Requirement: The dpsw coverage rows move from candidate to disposed

`models/COVERAGE.md` SHALL carry a disposition for DPSW-I1 through
DPSW-I10: modeled rows name their checking rung; board-settled rows
(DPSW-I3 verified; the discovery answers for DPSW-I4 reset scope and
DPSW-I5 bpid-vs-id) name their suite verdicts; consumer-owned rows
(DPSW-I8 class) anchor to ADR-0004 §8, and out-of-scope faces anchor
loudly. Frozen traces for the new behaviors SHALL be committed beside
the model.

#### Scenario: The ledger lint holds after the sync

- **WHEN** the ledger lint cross-checks COVERAGE rows against model
  annotations and recorded verdicts
- **THEN** every DPSW row names an existing encoding or an explicit
  anchor, and no row claims a verdict `VERDICTS.json` does not hold
