# topology-config delta: cross-dprc-links

## ADDED Requirements

### Requirement: The config surface carries the dpcon priority and dpni single_sender knobs
The `dpaa2-config` schema SHALL surface two additive intent knobs
(design D9): a dpcon priority knob (DPCON-I3) and a dpni `single_sender`
option (`DPNI_OPT_SINGLE_SENDER`, baseline dpni unknown #7). Both SHALL
be optional and additive — a config that omits them SHALL parse to the
same `Intent` as before this change (defaults preserve current
derivation) — and SHALL convert into the backend-neutral `Intent`
without leaking TOML/serde types into `dpaa2-api`. A malformed or
out-of-range knob value SHALL be refused with an actionable message
consistent with the existing refusal idiom, before any compilation.

#### Scenario: The knobs parse and convert to neutral intent
- **WHEN** a config declares the dpcon priority knob and a dpni
  `single_sender`
- **THEN** it yields an `Intent` carrying both knobs with no
  TOML/serde-specific types reaching `dpaa2-api`

#### Scenario: A malformed knob is refused at parse
- **WHEN** a knob carries an out-of-range or malformed value
- **THEN** validation fails with an actionable message naming the knob,
  and no compilation is attempted

#### Scenario: Omitted knobs preserve the current meaning
- **WHEN** a config declares neither knob
- **THEN** it parses to the identical `Intent` it produced before this
  change

### Requirement: Link tables carry no attributes
A `[link.<name>]` table SHALL admit only its two tenant ends
(`interface_a`/`interface_b`); it SHALL carry no rate and no other
attribute (design non-goal: no NXP script ever used link rates, so rates
stay unexpressed by refusal-by-omission at the schema). An unknown key on
a link table SHALL be refused by the parser, consistent with the
existing validation idiom.

#### Scenario: A rate on a link is refused by omission
- **WHEN** a `[link.<name>]` table declares a `rate` or any attribute
  beyond its two ends
- **THEN** validation fails naming the link and the unexpected key, and
  no compilation is attempted
