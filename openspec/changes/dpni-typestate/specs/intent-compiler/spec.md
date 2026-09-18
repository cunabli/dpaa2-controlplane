# intent-compiler delta — dpni-typestate

## ADDED Requirements

### Requirement: dpni options derive purely from Dataplane and interface construct
The compiler SHALL choose each derived dpni's option set solely from the
owning tenant's `Dataplane` and the interface construct, using the two
board-verified profiles: the PMD profile (`DPNI_OPT_SINGLE_SENDER,
DPNI_OPT_CUSTOM_CG, DPNI_OPT_HAS_KEY_MASKING, DPNI_OPT_HAS_OPR,
DPNI_OPT_OPR_PER_TC` plus raw `0x80000000`) for `userspace-poll`
consumers and the kernel profile (`DPNI_OPT_HAS_KEY_MASKING` only) for
`kernel-netlink` consumers. The intent vocabulary and the TOML schema
SHALL NOT name a `DPNI_OPT_*` and SHALL NOT carry a per-interface
override; dry-run provenance SHALL attribute each chosen option to its
profile rule.

#### Scenario: Options never appear in intent
- **WHEN** an operator declares any valid intent
- **THEN** no construct accepts an option token, and the derived plan
  carries the profile-chosen option set with per-object rule provenance

#### Scenario: Profile follows the binding consumer
- **WHEN** two tenants differing only in dataplane declare the same
  interface construct
- **THEN** the `userspace-poll` tenant's dpni derives the PMD profile and
  the `kernel-netlink` tenant's dpni derives the kernel profile

### Requirement: Zero-value intent escape hatches are closed
The intent vocabulary SHALL NOT admit a zero-value construction path
that bypasses the `TenantRef::from_name` discipline: `Isolation` and
`Intent` SHALL NOT be constructible via `Default`, and an empty
`TenantName` SHALL be unconstructible outside its internal sentinel role.
Every intent that was valid before this change SHALL keep its meaning.

#### Scenario: Zero-value intent does not construct
- **WHEN** code attempts to build an `Intent` or `Isolation` from a
  default/zero value without routing tenant references through
  `TenantRef::from_name`
- **THEN** the construction is unrepresentable at the type level

#### Scenario: Frozen traces replay unchanged
- **WHEN** the pre-change frozen intent traces replay through the
  compiler after the hazard closure
- **THEN** every accepted intent compiles to the identical plan and every
  refusal keeps its variant
