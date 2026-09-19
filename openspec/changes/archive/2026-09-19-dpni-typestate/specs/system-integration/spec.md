# system-integration delta — dpni-typestate

## ADDED Requirements

### Requirement: The board milestone extends the V-DPNI series and amends the baseline
The change's board milestone SHALL be one operator sitting: a named
batch suite extending the V-DPNI series (option-profile creation walks,
sizing-field probes, `HAS_REPLICATION` accept/reject, unread-flag
probes, primary-MAC mutation) plus the online-MBT learning session.
Every probe outcome SHALL amend `docs/baseline/dpni.md` in the same
change — answers move items off the unknown-register list, silence is
recorded with a revisit trigger — and the restool-unreachable unknowns
SHALL be verified as deferral rows: the runtime `dpni_set_*` surface and
TX_CONFIRMATION_MODE to `mc-portal-backend` (#10) — emit v2 with an explicit
channel index, probe v1-handler retention (register #1) — `num_rx_tcs`-via-
DPL to `dpl-tape-out` (#14), table-write and traffic-dependent items to
their earliest reachable tile.

#### Scenario: Suite results close or defer every targeted unknown
- **WHEN** the board sitting completes and results are diffed
- **THEN** each targeted unknown-register item is either amended in the
  baseline with its evidence tag or carried as a named deferral row, and
  no targeted item is left unaccounted

#### Scenario: Suites leave the board clean
- **WHEN** any dpni suite finishes, pass or refuse
- **THEN** every scratch object it created is destroyed and the
  recovery baseline still verifies
