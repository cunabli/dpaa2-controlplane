# mc-backend delta — dpni-typestate

## ADDED Requirements

### Requirement: The restool shim drives dpni create at full option granularity
The `dpaa2-mc` restool shim SHALL grow the dpni create verb carrying
every live create option from the typed create block, emitting the
options mask it computed itself as a raw numeric value — never
operator-supplied tokens — so restool's loose token parsing
(case-sensitive match with silent numeric fallback) is bypassed as an
input path. The shim SHALL also drive the primary-MAC mutation, the only
restool-reachable runtime setter.

#### Scenario: Create emits the computed mask
- **WHEN** the reconciler dispatches a dpni create with any typed option
  set
- **THEN** the shim passes one raw mask value it computed from the typed
  set, and every sizing field the block carries

#### Scenario: Primary MAC set round-trips
- **WHEN** the shim sets a dpni primary MAC and re-observes the object
- **THEN** the read-back reports the new MAC (exit status is not the
  observation)

### Requirement: dpni observation maps the read-back asymmetries
The shim's dpni observation SHALL map `dpni_attr`'s asymmetric read-back
into the domain observation type: the split `num_rx_tcs`/`num_tx_tcs`,
the added `qos_key_size`/`fs_key_size`/`wriop_version`, and the omission
of `dist_key_size` (write-only; never synthesized).

#### Scenario: Observation never invents dist_key_size
- **WHEN** a dpni is observed
- **THEN** the observation carries no `dist_key_size` value, and the
  read-back fields map to their domain names

### Requirement: McControl observes a single container by id
`McControl` SHALL provide `observe_container(id)` beside the enumerate
verb, returning the same observation shape for exactly that container,
so per-candidate re-observation does not rescan every root child. The
disposition SHALL cite the OI-3 dpmcp-budget measurement outcome (bead
am0.2): if per-ensure spawns draw the never-returned budget, the seam is
recorded as a leak fix.

#### Scenario: Single-container observation
- **WHEN** the reconciler calls `observe_container(id)` for one child
- **THEN** the shim spawns commands scoped to that container only and
  returns its observation
