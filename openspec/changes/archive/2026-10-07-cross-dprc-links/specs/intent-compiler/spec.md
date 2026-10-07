# intent-compiler delta: cross-dprc-links

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
design D9) and record it as per-object provenance naming the knob; the
derived dpcon's create operand stays the companion default, since the WQ
priority binds only at consumer registration (the consumer-rig trigger
recorded on the COVERAGE DPCON-I3 row). The knob SHALL be additive only:
when it is absent, the derived dpcon is exactly what the current
companion derivation produces (defaults preserve the current
derivation).

#### Scenario: The priority knob records provenance
- **WHEN** a tenant declares the dpcon priority knob
- **THEN** the derivation emits exactly one provenance node naming the
  knob and the DPCON-I3 anchor, and the dpcon create operand is
  unchanged

#### Scenario: Absent knob preserves current derivation
- **WHEN** no dpcon priority knob is declared
- **THEN** the derived dpcon is identical to the pre-change companion
  derivation, with no priority field forced
