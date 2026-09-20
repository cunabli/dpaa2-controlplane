# mc-backend delta — dpni-hardening

## MODIFIED Requirements

### Requirement: Every restool exit on the create chain is classified through the typed funnel
The restool shim SHALL route `stamp_label` and `read_inventory` through
`run_verb` so their refusals carry the typed McStatus/RestoolGuard
classification instead of an untyped backend error; no shim verb SHALL
call the raw `Runner::run` directly. The dpni observation mapping SHALL
capture `wriop_version` from the read-back (the board emits it and the
tile #10 num_queues-ceiling question anchors on it). (Review synthesis
rows 7-8.)

#### Scenario: A set-label refusal on the create chain is attributable
- **WHEN** the MC refuses the `dprc set-label` step of a dpni create
- **THEN** the rollback fires on a typed classification naming the
  refusing verb and status, not on an untyped backend error

#### Scenario: The read-back keeps the WRIOP revision
- **WHEN** the shim maps a `dpni info` read-back
- **THEN** `wriop_version` is captured on the raw attribute struct as an
  informational field, outside the pure core's equality
