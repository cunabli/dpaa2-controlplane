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

### Requirement: The shim binds a kernel-driven child container explicitly

The fsl-mc bus does not bind a runtime-created child dprc on its own, so
the shim SHALL NOT rely on it to do so. When a kernel-driven child
container is observed present on the bus but unbound (empty driver link,
residents unprobed), the shim SHALL bind it by writing the container name
to the `fsl_mc_dprc` driver's bind file, level-triggered: an `ENODEV`
return from the write racing the asynchronous post-sync scan SHALL be
treated as a deferral re-kicked the next pass, never as a terminal error
(finding 54, `docs/upstream/fsl-mc-no-autobind-child-dprc.md`; bead
`dpaa2-controlplane-dbf.19`).

#### Scenario: A sync-added child container binds on a later pass

- **WHEN** a pass observes a kernel-driven child dprc present on the bus
  but unbound
- **THEN** the shim writes the container name to the `fsl_mc_dprc` bind
  file, and an `ENODEV` from a write that races the post-sync scan is
  reported as pending and re-kicked, not failed

### Requirement: The disconnect verb issues no post-mutation bus sync

The restool shim's dprc disconnect verb SHALL NOT issue a post-mutation
bus sync. A sync after a disconnect points the kernel at a transiently
endpoint-free wired port, inviting the standalone `fsl_dpaa2_mac` driver
to attach and then be torn down under an armed phylink timer — the
teardown crash family of finding 49
(`docs/upstream/phylink-dpni-churn-crash.md`). The next mutating verb's
sync SHALL carry the disconnected state instead.

#### Scenario: A disconnect leaves the bus un-synced

- **WHEN** the shim drives a dprc disconnect that severs a dpni–dpmac
  edge
- **THEN** no bus sync is issued by the disconnect verb, and the kernel
  is never pointed at the endpoint-free wired port until a later mutating
  verb's sync
