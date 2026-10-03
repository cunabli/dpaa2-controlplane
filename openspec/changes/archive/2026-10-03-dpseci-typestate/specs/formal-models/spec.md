# formal-models — dpseci-typestate delta

## ADDED Requirements

### Requirement: The dpseci model carries the P2 configured-object shape

`models/families/dpseci.qnt` SHALL grow the ADR-0019 P2 shape on the
`dpni.qnt` reference: a create cfg block with the queue-pair count in
`1..DPSECI_MAX_QUEUE_NUM`, a priorities vector whose length equals the
queue count with each entry in `1..8`, and a closed options vocabulary
(`HAS_CG`, `HAS_OPR`, `OPR_SHARED`) plus a provenance-carrying raw
escape. The refusal surface SHALL replay the restool-layer validation
banked by V-DPSECI-1 rev 1 (priority 0, a priority above 8, a count ≠
queue number — each refused before any MC command). DPSECI-I1 SHALL
hold by construction: no mutating verb exists on the family surface.
Typecheck and simulate SHALL be green; invariants are named under their
baseline ids.

#### Scenario: A malformed cfg is unrepresentable or refused

- **WHEN** a cfg is built with 2 queues and priorities `[1]`, or any
  priority outside `1..8`
- **THEN** the model refuses exactly as the banked restool layer does,
  and no state carrying the malformed cfg is reachable

#### Scenario: The cfg never changes after create

- **WHEN** any sequence of modeled transitions runs after a dpseci create
- **THEN** the object's queue counts, priorities, and options are the
  create-time values in every reachable state (DPSECI-I1)

### Requirement: Congestion backpressure is a birth capability

The model SHALL type DPSECI-I4 as a capability fixed at create: the
congestion backstop exists iff `HAS_CG` is in the create options, and no
transition can add or remove it afterwards. The consumer SHALL appear in
the model only as consequences the cfg determines — sizing
(ADR-0012/0013), this birth capability, the hot-bind refusal
(`hotBindAvailable: false`, V-LIFE-DPSECI-1 rev 2), the modeled I5 dirt
law, and the I9 block-global counter law — and SHALL carry no
consumer-owned steering state machine.

#### Scenario: HAS_CG decides backpressure forever

- **WHEN** one dpseci is created with `HAS_CG` and another without
- **THEN** the first carries the congestion capability in every
  reachable state and the second in none, with no transition between
  the two conditions

### Requirement: COVERAGE dispositions move with the fences named

`models/COVERAGE.md` SHALL move DPSECI-I1 and DPSECI-I4 from deferred to
modeled with their rungs named, record DPSECI-I3 as implemented at the
adapter (the GET_ATTR observable), and re-anchor the unreachable faces
loudly with each fence stated: DPSECI-I2's MC layer → #10 (CREATE is
excluded from the read slice by construction), DPSECI-I5's board face →
#10 (`get_rx_queue`/`get_congestion` are on no userspace whitelist),
DPSECI-I9 → the block-global law (SEC counters are unreadable by every
userspace transport; per-object counter attribution is never built).
The ledger lint SHALL be green.

#### Scenario: No unreachable face hides behind this change

- **WHEN** the COVERAGE rows for DPSECI-I1..I9 are read after the model
  phase
- **THEN** every row is modeled, implemented, verified, or re-anchored
  to a named change with the blocking fence stated in the row
