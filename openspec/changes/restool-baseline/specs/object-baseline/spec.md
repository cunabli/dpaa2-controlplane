## ADDED Requirements

### Requirement: Every restool object family has a baseline document
The baseline SHALL contain one document per restool object family — dprc,
dpni, dpmac, dpbp, dpio, dpcon, dpmcp, dpseci, dpdmux, dpsw, dpaiop, dpci,
dpdcei, dpdmai, dprtc, dpdbg — under `docs/baseline/`, each following the
same template: command surface, option inventory, attribute mutability,
MC API notes, kernel-side behavior, lifecycle ordering and dependencies,
intent mapping, silent-failure notes, and an unknown/unverified register.

#### Scenario: Family document exists and is complete
- **WHEN** a reader opens `docs/baseline/<family>.md` for any of the 16
  families
- **THEN** every template section is present, and sections with no content
  state explicitly that nothing is known or applicable rather than being
  absent

#### Scenario: Unknowns are findings, not gaps
- **WHEN** a claim about MC or kernel behavior cannot be established from
  the corpus (restool C, ls-main, mc-utils, kernel 6.6.52 source, manual)
- **THEN** the document records it in the unknown/unverified register as a
  candidate for board validation instead of omitting or guessing it

### Requirement: Option inventory separates used from available
Each family document SHALL list every command and option the restool source
exposes for that family, and SHALL mark each option as used by
`ls-main`/`ls-debug`/`ls-append-dpl` (with the invoking script and default
value) or available-but-unused.

#### Scenario: Unused option is still documented
- **WHEN** a restool option for the family is never referenced by any ls-*
  script
- **THEN** the document still lists it, marked available-but-unused, with
  its semantics from source or manual

### Requirement: Attribute mutability is classified per family
Each family document SHALL classify the family's attributes as
create-time-immutable or mutable-at-runtime, since immutable-attribute
drift is refused (never repaired) by the reconciler and the classification
feeds the typestate design.

#### Scenario: Immutable attribute identified
- **WHEN** an attribute can only be set at object creation per the restool
  source or MC documentation
- **THEN** the document lists it as create-time-immutable

### Requirement: Kernel-side behavior is documented against the pinned kernel
Each family document SHALL contain a kernel-side section describing how
Linux 6.6.52 interacts with the family (driver binding, allocation from
container pools, sysfs/netdev surfaces, udev reactions), sourced from the
pinned kernel tree, and SHALL note where kernel behavior — not restool —
defines the observable semantics.

#### Scenario: Driver allocation behavior captured
- **WHEN** a family's objects are allocated (not created) by a kernel driver
  at probe time
- **THEN** the document states which driver, what it allocates, and what
  must pre-exist in the container for the probe to succeed

### Requirement: A cross-object relationship map exists
The baseline SHALL contain `docs/baseline/object-model.md` describing the
relationships across all families: containment (DPRC parent/child),
connect edges, create-vs-allocate semantics, allocation pools, and
lifecycle ordering constraints, in a form precise enough to seed typestate
and Quint lifecycle models without re-reading source.

#### Scenario: Create-vs-allocate is unambiguous
- **WHEN** the map describes how an object reaches a consumer
- **THEN** it distinguishes creation (an actor makes a new object) from
  allocation (a driver claims an existing pooled object) and names the pool

### Requirement: Validation scenarios are traffic-classified against the port matrix
The baseline SHALL contain `docs/baseline/traffic-inventory.md` classifying
every planned validation scenario as object-lifecycle-only, link-signaling,
or traffic-bearing, together with the board port safety matrix: dpmac.3 and
dpmac.17/dpni.0 total-deny, dpmac.4–6 and dpmac.8/10 lifecycle-only (no
link-up possible), dpmac.7/9 link-up and traffic-bearing only when
explicitly flagged.

#### Scenario: dpmac.3 admits no scenario class
- **WHEN** any scenario in the inventory is checked against the port matrix
- **THEN** no scenario of any class names dpmac.3

### Requirement: The reference environment is pinned and captured
The baseline SHALL record the validation pair — MC firmware 10.39.0, Linux
6.6.52, restool v2.4 — in `docs/baseline/reference-environment.md`,
together with a DPC/DPL snapshot of the board, captured by a user-run,
read-only board script; board evidence anywhere in the series is only valid
against this stamped pair.

#### Scenario: Capture script is read-only
- **WHEN** the reference-environment capture script runs on the board
- **THEN** it reads versions and snapshots DPC/DPL state without creating,
  destroying, connecting, or binding any object

### Requirement: A master roadmap sequences the change series
The baseline SHALL contain `docs/ROADMAP.md` naming every planned change in
the port series with its tier, dependencies, board-session sync points, and
named decision points (at minimum: the Mellanox device-tree revert before
the first traffic-bearing phase, and the DPL tape-out), and SHALL be
amendable as tiles are reached.

#### Scenario: Decision point is visible before its phase
- **WHEN** a reader inspects the roadmap entry for the first traffic-bearing
  change
- **THEN** the Mellanox-revert decision point is attached to it, with the
  alternatives recorded

### Requirement: Process decisions are codified as ADRs
The scoping-session decisions SHALL be recorded as numbered ADRs under
`docs/adr/`: formal-methods process (Quint primary, TLA+ escalation,
dual-mode MBT, model-before-code), board interaction protocol and safety
envelope, southbound evolution (staged dual-backend, differential gate,
MC v10 pin, single unsafe module), intent layer (network-construct
vocabulary, intent-compiles-to-objects), and the single-initiating-writer
assumption with the kernel as reactive environment.

#### Scenario: Later change cites the process ADR
- **WHEN** a later change in the series applies a process rule (e.g. the
  board safety envelope)
- **THEN** the rule resolves to a numbered ADR in `docs/adr/`, not to
  conversation history

### Requirement: Upstream findings are seeded
The baseline SHALL seed `docs/upstream/findings.md` where undocumented
MC/restool semantics discovered during the series are captured for sharing
upstream, without device-identifying information.

#### Scenario: Divergence lands upstream-shareable
- **WHEN** a baseline or board finding contradicts NXP documentation or
  restool behavior
- **THEN** it is recorded in the findings file in generic terms (object
  types and versions only)
