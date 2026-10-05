# provisioning-cli delta: cross-dprc-links

## ADDED Requirements

### Requirement: Dry-run renders link transitions with rule provenance
`dry-run` SHALL render each link connect and disconnect transition with
its disruption class and its rule provenance — the rule, source
construct, and evidence anchor — exactly as it renders every other
transition (design D10). The rendered link transition SHALL be the exact
one `ensure` would execute.

#### Scenario: Dry-run shows a link connect with provenance
- **WHEN** the operator dry-runs a plan that connects a dpni↔dpni link
- **THEN** the connect transition line carries its disruption class and a
  provenance tree naming the deriving rule, construct, and anchor, and it
  matches what `ensure` would execute

### Requirement: The Disruptive rebind cycle is consented, never silent
The `DeferredVisibility` discharge SHALL be classed `Disruptive` — the
unbind → bind → re-observe rebind cycle — and actuated only under an
explicit `--allow=disruptive` (ADR-0015 consent machinery, design
D5/D10). A declined consent SHALL report the typed standing residue and
change nothing; the rebind SHALL never fire silently.

#### Scenario: The rebind is refused without the disruptive allowance
- **WHEN** `ensure` runs with the default allowance against a plan whose
  only discharge is the rebind cycle
- **THEN** it refuses, names the `disruptive` headline and the
  `--allow=disruptive` needed, reports the standing `DeferredVisibility`
  residue, changes nothing, and exits non-zero

#### Scenario: Consented rebind discharges the obligation
- **WHEN** `ensure` runs with `--allow=disruptive` against the same plan
- **THEN** the rebind cycle is applied and re-observation after the scan
  reports the object visible, with the applied transitions logged

### Requirement: Status detail shows link and obligation rows
`dpaa2ctl status --detail` SHALL render a row per dpni↔dpni link — its
endpoints and connection state judged via `dprc_get_connection` from the
issuing ancestor — and a row per standing `DeferredVisibility`
obligation. The rows SHALL be display-only: no field in them gates
convergence, plans an action, or participates in drift. Where the
connection or obligation observation is unavailable, the cell SHALL
render as explicitly unknown (the honest-unknown idiom), never as empty,
down, or zero.

#### Scenario: A converged link renders from dprc_get_connection
- **WHEN** the operator runs `status --detail` over a converged link
- **THEN** the link row shows its endpoints and connection state sourced
  from `dprc_get_connection`, and a repeated `status` run plans zero
  actions regardless of any value shown

#### Scenario: An unobservable link state stays honest
- **WHEN** the connection read for a link is unavailable
- **THEN** the link row renders the state as explicitly unknown and the
  command exits successfully

#### Scenario: A standing obligation shows as a row
- **WHEN** a `DeferredVisibility` obligation stands undischarged
- **THEN** `status --detail` renders it as an obligation row, display-only,
  that gates no convergence verdict
