# reconciler — delta

## ADDED Requirements

### Requirement: The severed witness is bound to the edge it severs
The proof `sever` yields SHALL identify the dpni whose edge it severed and
SHALL NOT be duplicable or forgeable by consumers: it carries its `DpniId`
privately, implements neither `Clone` nor `Copy`, and cannot be extracted
from the plan surface and re-used. `unbind` SHALL derive its target from
the proof it consumes, so a proof minted for one edge cannot release
another edge's kernel face. This makes the Rust side match the model's
consume-and-yield law (`severAt` consumes KernelOwned ∧ KernelFaceBound
and yields Offered plus the witness, ADR-0002/ADR-0008 §8) instead of
demanding only that some sever ran (review synthesis MERGED-1).

#### Scenario: A proof cannot cross edges
- **WHEN** a consumer holds the proof minted by severing dpni A and attempts to unbind dpni B's kernel face with it
- **THEN** the program does not typecheck (pinned compile-fail face), and the runtime replay rejects any trace attempting the inverted or crossed order

#### Scenario: A proof cannot be reused
- **WHEN** a consumer attempts to copy a severed proof or unbind twice from one sever
- **THEN** the program does not typecheck — the proof is consumed by the unbind that spends it

### Requirement: A zero MAC read-back is a bind-window transient, not drift
The reconciler's MAC comparison SHALL treat an observed all-zeros MAC as
unobserved (the bind-window transient the kernel reports before the dpni
binds), using the same pure family predicate the display path's Pending
judgment uses. An Assert intent SHALL NOT report a mismatch against a zero
read-back, and an Actuate intent SHALL NOT plan a MAC write targeting the
transient (review synthesis MERGED-4; D5's Pending-never-drift extended
structurally to the plan path).

#### Scenario: Assert does not flag the transient
- **WHEN** the observed dpni MAC parses to all zeros while an Assert intent names a MAC
- **THEN** no assert mismatch is reported and the status exit code is unaffected

#### Scenario: Actuate does not write against the transient
- **WHEN** the observed dpni MAC parses to all zeros while an Actuate intent names a MAC
- **THEN** the plan takes the decided posture (skip or defer, decided model-side in the bead) and never emits a SetMac justified solely by the zero read-back
