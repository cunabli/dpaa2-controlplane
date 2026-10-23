# Spec Delta

## ADDED Requirements

### Requirement: Dry-run previews the switch pass
`dpaa2ctl ensure --dry-run` SHALL render a read-only preview of the
switch pass — the same verdict loop the ensure pass judges, composed
without dispatch (the plan_links idiom) — so a switched intent never
previews as hitless while ensure would do Disruptive work. Preview
composition only: the census stays the sole convergence judgment and
no preview field gates it.

#### Scenario: A switched intent previews its disruption
- **WHEN** dry-run runs over a config declaring a switched domain that
  does not yet exist (or exists with cfg drift)
- **THEN** the output renders the switch section with its Disruptive
  headline and the planned moves, and dispatch count stays zero
