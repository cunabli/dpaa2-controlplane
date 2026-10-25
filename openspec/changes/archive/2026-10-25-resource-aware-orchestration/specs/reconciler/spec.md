# reconciler — resource-aware-orchestration delta

## ADDED Requirements

### Requirement: Family dispatch is compile-checked total

Every plan-entry family SHALL flow through one total dispatch over the
construct-family vocabulary (design D10): a visitor-shaped trait
without default implementations or an exhaustive match, such that a
family without a row does not compile; a runtime registry map is
refused. A family's row is `Execute` — the pass that consumes its plan
entries — or `Unimplemented` — a typed whole-intent refusal dispatched
before any verb, naming the retirement trigger that ships the pass.
The interim reify guard (dpsw-typestate task 6.4) is absorbed into this
dispatch, not duplicated. A unit test SHALL iterate the family
vocabulary and assert every `Unimplemented` row names its retirement
trigger.

#### Scenario: dpseci is the first Unimplemented row

- **WHEN** an intent names a crypto block (a compiled dpseci construct)
- **THEN** the whole intent is refused typed before any verb, and the
  refusal row names the change that retires it

#### Scenario: A new family without a pass does not compile

- **WHEN** the compiled construct vocabulary gains a family with no
  dispatch row
- **THEN** the workspace fails to build — the gap class is a compile
  error, never a silent drop

### Requirement: Ordering constraints are guards, the sequence is a scheduler choice

Every true dependency between passes SHALL be a guard over the observed
census (design D4/D9): companions precede their object, the object
precedes its edges, teardown exactly reverses, sever precedes rewire,
and unbind precedes unplug. The model additionally carries the
switch-port live-read guard — a port wires only over a dpmac whose
occupancy is re-sampled driver-free at fire time (design D9; ORCH-I2) —
and its engine landing is named, not yet shipped: ROADMAP #11 plus the
kernel refresh (the dpsw-typestate deferred reachability clause, bead
`dpaa2-controlplane-dbf.19` / witness V-DPSW-5). The `main.rs` pass
order remains a scheduler optimization; removing it may cost loops,
never correctness.

#### Scenario: The model's live-read never wires the handover window

- **WHEN** the census snapshot reads a dpmac driver-free but the kernel
  has since bound it
- **THEN** the model's live-read ahead of the wire verb refuses the
  pass this loop and the next observation re-judges — checked in the
  Quint model (ORCH-I2); the engine implementation lands with ROADMAP
  #11 and the kernel refresh
