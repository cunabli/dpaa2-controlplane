# mc-backend — dpsw-typestate delta

## ADDED Requirements

### Requirement: The restool shim drives the dpsw verb set verbatim

The shim SHALL drive the three dpsw restool verbs the plan needs —
create, destroy, info — rendering the compiled create block verbatim,
never re-deriving an option or a size (the `create_dpni` contract
extended to the family). The taildrop `update` verb SHALL NOT be driven
(no intent knob, no in-corpus caller; taildrop is read-only in
observation). Each driven verb SHALL stay inside the `/dev/dprc.N`
whitelist (ADR-0004 §7/§8); no runtime switchdev verb is driven. A dead
or aborting spawn (restool's `assert` path on out-of-enum values) SHALL
surface as a typed observation failure, never as inherited partial
state.

#### Scenario: The create renders the compiled block exactly

- **WHEN** a plan dispatches a dpsw create
- **THEN** the rendered options, counts, and modes equal the compiled
  block field-for-field, and the object is labeled at create (ADR-0010
  §4, ADR-0015)

#### Scenario: The verbs resolve inside the whitelist

- **WHEN** the ioctl-policy test resolves the verbs this family drives
- **THEN** every emitted command id matches an accepted whitelist entry
  in `docs/baseline/mc-ioctl-policy.md`

### Requirement: The shim reads the dpsw observation surface typed

The shim SHALL parse `dpsw info` into the typed observation: decoded
attributes and options (unknown bits as attributed raw escapes,
display-only), the per-interface block (endpoint, link, taildrop, max
frame length, counters) with each absent line an honest unknown — a
port whose `connection:` line is missing is `Unobservable`, a blanked
option list is a typed decode escape, and a mid-list counter truncation
never reads as zeros (the DPMAC-I7 absence-≠-zero law extended to the
family).

#### Scenario: A truncated counter list stays honest

- **WHEN** `info` output truncates mid-counter-list
- **THEN** the parsed observation carries the read rows and types the
  rest unread; no zero is invented
