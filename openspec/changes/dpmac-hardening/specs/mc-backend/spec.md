# mc-backend — delta

## ADDED Requirements

### Requirement: The counter readout carries its verbatim row names
The dpmac counter readout SHALL carry the verbatim restool row name beside
each value instead of dropping the names at the shim: the names are
observed data riding `CounterReadout::Vocabulary`, not a second vocabulary
table. The D4 split is unchanged — the adapter's 28-row verbatim vocabulary
remains the only observable and the model keeps its representative slice —
but no consumer may re-pair values against any other name source (review
synthesis MERGED-2; the row count stays the typed version signal).

#### Scenario: Names survive the shim
- **WHEN** `observe_dpmac` reads the 28-row counter block on MC 10.39
- **THEN** the returned readout pairs each value with the verbatim restool row name it was read under

#### Scenario: The deviation signal is unchanged
- **WHEN** the row count deviates from the firmware vocabulary
- **THEN** the readout still types the deviation as the version signal, names included for the rows that were read

### Requirement: The dpmac info text has one raw scanner
The `dpmac info` rendered text SHALL be scanned by one raw parser from
which the offer/observation/info projections derive, with one token→enum
table per spelling oracle (the `OPTION_BITS` precedent). Three parallel
scanners of the same text are retired (review synthesis MERGED-6a; the
model↔Rust lockstep protection of ADR-0014 does not extend to intra-adapter
copies).

#### Scenario: One spelling oracle
- **WHEN** restool renders a link-type or eth-if token
- **THEN** exactly one table in the adapter maps it, and every projection consumes that mapping
