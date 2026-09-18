# reconciler delta — dpni-typestate

## ADDED Requirements

### Requirement: The dpni create surface is typed and invalid configurations are unrepresentable
The `dpaa2-api` crate SHALL model the dpni family in
`families::dpni` with the validated create block (`dpni_cfg`) as the
immutable type parameter of a DPNI: the twelve live create options
(baseline `docs/baseline/dpni.md` option inventory) SHALL carry refined
range types bounded by the restool-verified envelope, and the options
mask SHALL be a typed flag set over the 14-flag MC 10.39 vocabulary plus
a provenance-carrying raw-mask constructor for values outside the named
map (`0x80000000` PFDR_IN_PEB). No `dpni_cfg` field SHALL be mutable
after creation; changing one SHALL plan destroy + create, never repair
(ADR-0001 §4).

#### Scenario: Out-of-range option has no constructor
- **WHEN** code attempts to build a dpni create block with a value
  outside a live option's verified range (e.g. `num_queues` 0 or 33)
- **THEN** the value does not construct — the refusal is a type error or
  a refused fallible constructor, not a board rejection

#### Scenario: Cfg drift plans destroy-and-create
- **WHEN** an observed dpni differs from the desired one on any
  `dpni_cfg` field
- **THEN** the plan refuses repair and reports the drift with the
  destroy + create disposition

### Requirement: Dead options and num_rx_tcs are unrepresentable with parity refusals
The eleven dead restool options (create-time `--mac-addr` and the ten
v9-era `--max-*` flags) and the never-settable `num_rx_tcs` SHALL have
no constructor in the typed surface, and each SHALL be named by a
programmatic refusal so the design-D11 rows stay two-sided
(vocabulary-v2 precedent).

#### Scenario: A dead option is refused by name
- **WHEN** a caller asks the programmatic surface whether a dead option
  is expressible
- **THEN** the answer is a named refusal identifying the option, not a
  silent absence

### Requirement: The runtime surface is state within the DPNI type, limited to the primary MAC
The DPNI typestate SHALL carry its runtime surface as state within the
type parameterized by the immutable create block. In this change the
runtime surface SHALL expose exactly one mutation — the primary MAC —
because it is the only setter the restool transport can drive; the
remaining `dpni_set_*` surface SHALL be a named deferral to the
mc-portal backend and SHALL slot into the same state position without
reshaping the family.

#### Scenario: Primary MAC mutation plans without touching cfg
- **WHEN** desired and observed dpnis differ only in primary MAC
- **THEN** the plan carries a MAC mutation and no destroy + create

### Requirement: dist_key_size is write-only by construct
Because `dpni_attr` omits `dist_key_size`, the reconciler SHALL treat it
as write-only: it participates in create and never in observation
comparison, so it can never produce a drift report.

#### Scenario: dist_key_size never reports drift
- **WHEN** a dpni is observed after creation with any `dist_key_size`
- **THEN** the comparison excludes the field and no drift is reported

### Requirement: Planning re-observes per candidate container
The planning surface SHALL provide an `observe_container(id)` seam
beside the enumerate verb, and per-candidate re-observation SHALL use it
instead of rescanning every root child.

#### Scenario: Re-observation targets one container
- **WHEN** the reconciler re-checks a single affected container during
  an ensure
- **THEN** exactly that container is re-observed through
  `observe_container(id)`
