# mbt-harness delta: dprc-encapsulation

## ADDED Requirements

### Requirement: Online discovery sessions target containment semantics
The online driver SHALL run this change's discovery sessions — the roadmap's
first online-MBT discovery target — against containment/pool semantics on
scratch containers: the DPRC-I1 pool boundary, teardown liveness (DPRC-I9)
under reconciler-generated plans, and the restool-reachable remainder of the
lock face (DPRC-I11). Sessions run under the existing operator-supervised
safety envelope; a model/board divergence amends the model and
`docs/baseline/dprc.md` in the same change.

#### Scenario: Pool boundary session
- **WHEN** a session allocates from a container whose local pool is exhausted while a sibling has surplus
- **THEN** the observed refusal is local (-ENXIO shape) and the trace confirms allocation never crossed the container boundary (DPRC-I1)

#### Scenario: Divergence feeds back
- **WHEN** an observed outcome contradicts the model's predicted transition
- **THEN** the session halts that face, the model and baseline are amended, and the corrected prediction is re-verified before the invariant's disposition advances

#### Scenario: Deferred faces are recorded, not probed
- **WHEN** a session plan would require the child's own portal (I11 unlock face, OBJ_CREATE gate, DPRC-I8)
- **THEN** the face is emitted as a deferral row pointing at tile #10 and no probe is attempted
