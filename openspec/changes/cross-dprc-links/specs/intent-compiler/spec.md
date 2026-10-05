# intent-compiler delta: cross-dprc-links

## MODIFIED Requirements

### Requirement: dpni options derive purely from Dataplane and interface construct
The compiler SHALL choose each derived dpni's option set from the owning
tenant's `Dataplane` and the interface construct, using the two
board-verified profiles: the PMD profile (`DPNI_OPT_SINGLE_SENDER,
DPNI_OPT_CUSTOM_CG, DPNI_OPT_HAS_KEY_MASKING, DPNI_OPT_HAS_OPR,
DPNI_OPT_OPR_PER_TC` plus raw `0x80000000`) for `userspace-poll`
consumers and the kernel profile (`DPNI_OPT_HAS_KEY_MASKING` only) for
`kernel-netlink` consumers. The one exception SHALL be the additive
`single_sender` knob (`DPNI_OPT_SINGLE_SENDER`, baseline dpni unknown #7,
design D9): an interface MAY declare it to add that single option on top
of its profile-derived set, and when it is absent the derived option set
is exactly the profile's (defaults preserve the current derivation). No
other `DPNI_OPT_*` SHALL be nameable in the intent vocabulary or the TOML
schema and no other per-interface override SHALL exist; dry-run
provenance SHALL attribute each chosen option to its profile rule or to
the `single_sender` knob.

#### Scenario: No option token but the single_sender knob appears in intent
- **WHEN** an operator declares any valid intent without `single_sender`
- **THEN** no construct accepts a `DPNI_OPT_*` token, and the derived plan
  carries the profile-chosen option set with per-object rule provenance

#### Scenario: Profile follows the binding consumer
- **WHEN** two tenants differing only in dataplane declare the same
  interface construct
- **THEN** the `userspace-poll` tenant's dpni derives the PMD profile and
  the `kernel-netlink` tenant's dpni derives the kernel profile

#### Scenario: The single_sender knob adds exactly one option
- **WHEN** an interface declares the `single_sender` knob on a dpni whose
  profile does not already carry it
- **THEN** the derived option set is the profile's plus
  `DPNI_OPT_SINGLE_SENDER`, that option's provenance names the knob, and
  no other option is added

## ADDED Requirements

### Requirement: Compiled link edges are actuatable plan output
The compiler SHALL emit the dpni↔dpni link edge it derives (rules
`link-edge` and `fabric-wire`, ADR-0013 §link) as a first-class
actuatable edge in the compiled plan — the edge the reconciler issues a
`dprc connect` for — rather than a plan-only residue that derives,
dry-runs, and stays unactuated. The edge SHALL be constructed through the
same witness-taking plan constructors the port edge uses, so it carries
the deriving construct and the common-ancestor resolution the reconciler
consumes. Derivation of the edge itself SHALL be unchanged from
`intent-layer`: an intent that compiled before this change yields the
same edge.

#### Scenario: A declared link compiles to an actuatable edge
- **WHEN** an intent declares a dpni↔dpni link
- **THEN** the compiled plan carries the link as an actuatable edge with
  its deriving construct and ancestor resolution, and the same intent
  compiled before this change derived the identical edge

### Requirement: The dpcon priority knob derives additively
The compiler SHALL accept an additive dpcon priority knob (DPCON-I3,
design D9) and carry it onto the derived dpcon's create surface, with
per-object provenance naming the knob. The knob SHALL be additive only:
when it is absent, the derived dpcon is exactly what the current
companion derivation produces (defaults preserve the current
derivation).

#### Scenario: The priority knob reaches the derived dpcon
- **WHEN** a tenant declares the dpcon priority knob
- **THEN** the derived dpcon carries that priority with provenance naming
  the knob and the DPCON-I3 anchor

#### Scenario: Absent knob preserves current derivation
- **WHEN** no dpcon priority knob is declared
- **THEN** the derived dpcon is identical to the pre-change companion
  derivation, with no priority field forced
