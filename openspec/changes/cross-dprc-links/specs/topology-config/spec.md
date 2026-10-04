# topology-config delta: cross-dprc-links

## ADDED Requirements

### Requirement: The config surface carries the dpcon priority knob
The `dpaa2-config` schema SHALL surface one additive intent knob
(design D9): a dpcon priority knob (DPCON-I3). It SHALL be optional and
additive — a config that omits it SHALL parse to the same `Intent` as
before this change (defaults preserve current derivation) — and SHALL
convert into the backend-neutral `Intent` without leaking TOML/serde
types into `dpaa2-api`. A malformed or out-of-range value SHALL be
refused with an actionable message consistent with the existing refusal
idiom, before any compilation. There is no `single_sender` knob:
`DPNI_OPT_SINGLE_SENDER` is consumer-typed (already in `Profile::Pmd`,
dpni-typestate design D3) and settled board-side by the V-TRAF-1 wire
variant (cross-dprc-links design D9), never an intent field.

#### Scenario: The knob parses and converts to neutral intent
- **WHEN** a config declares the dpcon priority knob
- **THEN** it yields an `Intent` carrying the knob with no
  TOML/serde-specific types reaching `dpaa2-api`

#### Scenario: A malformed knob is refused at parse
- **WHEN** the knob carries an out-of-range or malformed value
- **THEN** validation fails with an actionable message naming the knob,
  and no compilation is attempted

#### Scenario: An omitted knob preserves the current meaning
- **WHEN** a config omits the knob
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
