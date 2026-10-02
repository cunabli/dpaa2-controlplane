# provisioning-cli — dpmac-typestate delta

## ADDED Requirements

### Requirement: The CLI exposes a read-only port-detail view

`dpaa2ctl` SHALL render a read-only per-port detail view over the
observed dpmac surface: arbitration state (Offered / KernelOwned /
RemoteOwned), the MAC relation judgment (Inherited / Overridden /
Pending / Mismatched), the link carrier reading (up / down /
NoObservable), and the vocabulary counters. The view SHALL be
display-only: no field in it gates convergence, plans actions, or
participates in drift (link readings lag PHY reality per V-LINK-2;
counters are traffic-dependent). `NoObservable` carrier SHALL render as
the driverless-port diagnosis, not as down.

#### Scenario: A converged port renders its full surface

- **WHEN** the operator requests port detail on a converged
  kernel-regime port
- **THEN** the view shows `KernelOwned`, the MAC relation `Inherited`,
  the carrier reading, and the 28 vocabulary counters, each sourced
  from read-backs, and a repeated `status` run plans zero actions
  regardless of any value shown
