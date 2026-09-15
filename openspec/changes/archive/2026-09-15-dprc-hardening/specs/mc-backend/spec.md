# mc-backend delta: dprc-hardening

## MODIFIED Requirements

### Requirement: restool shim implements McControl
The `dpaa2-mc` crate SHALL provide a `restool`-backed implementation of `McControl`
that shells out to the `restool` binary and parses its output. This implementation
SHALL introduce no `unsafe` code and SHALL keep the workspace `unsafe_code = forbid`
lint intact. The shim SHALL report container observations raw — resident rows, plug
bits, labels — and SHALL NOT decide lifecycle state or resident origin: state
classification is the core's (`ContainerState::classify`, beside the existing
`VfioBind::classify` precedent), and an unobservable resident origin is reported as
absent (`Option<ResidentKind>`), never guessed (review M2: PASS3-F1/F2).

#### Scenario: Observe reflects real MC state
- **WHEN** `observe` is called against a live MC
- **THEN** it returns the current objects and connection edges as an
  `ObservedTopology`, sourced from `restool`

#### Scenario: No unsafe in phase 1
- **WHEN** `dpaa2-mc` is built
- **THEN** it compiles under `unsafe_code = "forbid"`

#### Scenario: Origin is not invented for an undeclared container
- **WHEN** the shim observes a resident whose create-vs-assign origin restool cannot show
- **THEN** the observation carries no origin claim and the core predicts the eviction post-state conservatively

### Requirement: The restool shim exposes the dprc verb surface
`McControl` SHALL expose the dprc verbs the reconciler plans — create, destroy,
assign (child placement and plugged state), unassign, set-label, set-locked —
at MC-command granularity, surfacing MC status codes distinctly (0x4/0x6/0x8
refusal shapes preserved) and surfacing restool's own client-side refusals
(e.g. the plugged-move guard) as typed errors, never as scraped text. Every
`McControl` verb SHALL return through the single classifying error exit (typed
MC status / client-guard discrimination); raw `Runner::run` is transport only.
A runner outcome with no exit code (signal death) is a backend error, never a
client-guard refusal (review M10: PASS3-F7/F8).

#### Scenario: Create returns the created identity
- **WHEN** the shim creates a child DPRC under a parent
- **THEN** the result carries the child's id for re-observation, and the new child reads back unplugged (board-verified default)

#### Scenario: Client-side refusal is typed
- **WHEN** restool refuses a plugged-object move before issuing any MC command
- **THEN** the shim returns a typed refusal distinguishable from an MC status refusal

#### Scenario: A killed restool is not a refusal
- **WHEN** a verb's restool process dies to a signal
- **THEN** the error is `Backend`, and no drift report attributes a client guard
