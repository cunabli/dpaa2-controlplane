# mbt-harness delta — dpni-hardening

## MODIFIED Requirements

### Requirement: The frozen-trace rung reaches every board-verified dpni refusal face
The ITF parser SHALL decode the full `RawEscape` vocabulary
(`HasReplication` included), and the frozen corpus SHALL carry: a
McClearedFlags create/read-back scenario (projection strips both cleared
escapes), a queue-envelope RefusedTrace twin for the guu.4a intent arm
(`QueueEnvelopeExceeded`), and the `UnpricedDataplane` refusal run. The
parser arm SHALL land before any trace that carries its tag is frozen.
(Review synthesis row 6.)

#### Scenario: A regenerated cleared-escape trace decodes
- **WHEN** `model:freeze-dpni` emits a trace whose option mask carries
  `HasReplication`
- **THEN** the replay decodes it and asserts the projection strips the
  MC-cleared bits, rather than failing at decode

#### Scenario: The intent-side envelope fence is replayed
- **WHEN** the frozen intent corpus is replayed
- **THEN** at least one trace drives the `QueueEnvelopeExceeded` decoder
  arm end-to-end
