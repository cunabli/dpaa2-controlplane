# system-integration delta: dprc-encapsulation

## ADDED Requirements

### Requirement: Board milestone covers lifecycle, VFIO, and end-to-end convergence
The change's board milestone SHALL comprise operator-launched suites for: the
full container lifecycle on scratch children (create, populate, plug, lock,
unlock, evict, destroy), an active VFIO bind/unbind on a scratch child
(driver_override, bind, override propagation to a subsequently-added child,
unbind, teardown), and end-to-end consumer convergence (declared intent →
`dpaa2-tools` converges the container; re-run converges to zero actions).
Suites are scratch-first and self-cleaning; use of the live VPP container is
permitted where a face requires it, is deliberate, and is recorded in the
suite's plan. Scripts assert the reference pair (MC 10.39.0 + Linux 6.6.52)
before running.

#### Scenario: End-to-end convergence diffs clean
- **WHEN** the operator runs the convergence suite with one declared consumer
- **THEN** the first run creates the container, the second run plans zero actions, and the read-back matches the derived model

#### Scenario: VFIO suite leaves no residue
- **WHEN** the VFIO suite completes (pass or fail)
- **THEN** its scratch containers and overrides are removed by the suite itself and the board object census matches the pre-suite baseline
