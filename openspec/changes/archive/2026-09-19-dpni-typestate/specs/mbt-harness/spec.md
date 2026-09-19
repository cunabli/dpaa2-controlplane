# mbt-harness delta — dpni-typestate

## ADDED Requirements

### Requirement: Suite generation renders the dpni option walks from the model
The `dpaa2-verify` batch-suite generator SHALL render dpni suites from
the model's create actions: option-profile creation walks (PMD and
kernel profiles created and read back), sizing-field walks for the
unknown-register probes (`num_cgs`, `num_opr`, `dist_key_size`), the
`HAS_REPLICATION` (0x4000) accept/reject probe, and the unread-flag
probes (`TX_FRM_RELEASE`, `HAS_POLICING`, `SHARED_CONGESTION`), each
with expectations inline and read-back as the only observation.
Suites SHALL be scratch-first and self-cleaning inside the safety
envelope, asserting the reference pair before running.

#### Scenario: A probe refusal is an answer
- **WHEN** the MC refuses a probe step (e.g. rejects 0x4000)
- **THEN** the suite records the refusal as the observed verdict and
  continues cleanup; the run is not marked failed by the refusal itself

### Requirement: dpni enters per-step learning mode for one online session
The online driver SHALL run dpni in per-step learning mode for one
operator-supervised session targeting the restool-reachable
unknown-register items, under the existing envelope; frozen ITF traces
from the session SHALL replay in `cargo test` as the family's
conformance twins.

#### Scenario: Learning-mode divergence feeds the model
- **WHEN** an online step observes behavior diverging from the model
- **THEN** the divergence is recorded against the model and the baseline
  amendment queue, not patched in the harness
