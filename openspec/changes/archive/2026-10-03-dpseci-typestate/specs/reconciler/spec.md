# reconciler — dpseci-typestate delta

## ADDED Requirements

### Requirement: The dpseci family surface is typed and isomorphic

`dpaa2-api` SHALL carry `families/dpseci.rs` structurally isomorphic to
the model (ADR-0002): refined newtypes for the queue count (`1..=16`)
and the priorities vector (length-coupled to the count, entries
`1..=8`, full range so the kernel profile's all-1 observes as `Known`),
the closed options vocabulary with provenance-carrying raw escape
(the `OptionMask` idiom), and the cfg immutable by privacy — a
`compile_fail` doctest SHALL witness that no field of a built cfg can
be mutated. The congestion capability SHALL be a type-level consequence
of `HAS_CG` at construction (DPSECI-I4).

#### Scenario: A malformed cfg is a type error, not a board rejection

- **WHEN** code attempts a cfg with 2 queues and one priority, or a
  priority of 0 or 9
- **THEN** the fallible constructor refuses before any plan exists,
  with the refusal naming the violated bound

### Requirement: Options drift is judged from the observed mask, and cfg repair is destroy+create

The reconciler SHALL judge `HAS_CG` drift only from the raw GET_ATTR
observation (DPSECI-I3); when that read is unavailable the outcome is a
typed unobservable-this-run result that convergence treats as absence
of evidence, never as drift. Because no setter exists (DPSECI-I1), any
queue-shape, priorities, or options mismatch on an existing object
SHALL plan as immutable-cfg repair — the destroy+create disruption
class — never as a live mutation.

#### Scenario: Unobservable options never fabricate drift

- **WHEN** convergence runs without access to the ioctl read path
- **THEN** the dpseci options judgment is the typed unobservable
  outcome and the plan contains no options-driven repair

#### Scenario: A cfg mismatch plans a replacement

- **WHEN** an observed dpseci's queue count differs from the compiled
  intent
- **THEN** the plan carries destroy+create with the disruption class
  stated, and no step mutates the live object's cfg
