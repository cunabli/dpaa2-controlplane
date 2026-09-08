# dprc-encapsulation

## Why

Roadmap tile #4. The intent layer (#3) can declare a consumer runtime and derive
its object set, but nothing yet realizes the container that encapsulates it: the
child DPRC — the MC's unit of delegation, isolation (ICID/IOMMU), and pooling —
has no lifecycle in the reconciler, so the intent layer cannot be used against
the board end-to-end. Containment and pool semantics are also the most opaque MC
behavior left (roadmap: the first online-MBT discovery target), and every later
tile (#5–8 populate containers, #9 links across them, #10 migrates them off
restool) stands on this one.

## What Changes

- The reconciler's object graph gains the child-DPRC family: lifecycle
  typestates (create → populate-while-unplugged → plug → lock/unlock → empty →
  destroy) that make invalid container transitions unrepresentable, mirroring
  the Quint lifecycle sum (structural-isomorphism law, ADR-0002).
- Board-settled containment laws become model invariants and plan guards: the
  per-option-bit permission matrix with its three distinct refusal statuses
  (Configuration error 0x6 / No resources 0x8 / No privilege 0x4), the eviction
  law (a resident the container created dies with it; an assigned-in resident is
  evicted unplugged to the parent, ADR-0007 §3), and the visibility law (sync
  never implies visibility; child-container residents are root-invisible at
  runtime, DPRC-I6).
- The intent layer wires end-to-end: a declared consumer converges to a
  correctly-optioned, correctly-placed, VFIO-bindable child DPRC on the board,
  idempotent and level-triggered, with dry-run provenance. **Container-only
  population**: residents are exercised only as far as containment semantics
  require; companion sizing convergence stays with tile #6, the dpni surface
  with tile #5.
- KernelControl grows the VFIO binding face: `driver_override` + bind/unbind of
  a child DPRC as typestates, including override propagation to
  subsequently-added children.
- The restool shim grows the dprc verb surface the reconciler needs (create,
  destroy, assign/unassign, set-label, set-locked) at MC-command granularity.
- The MBT harness runs its first online discovery sessions, targeting
  containment/pool semantics (DPRC-I1 pool-boundary, lock-face completion of
  DPRC-I11, teardown liveness DPRC-I9 under the reconciler).
- Everything stays behind the restool shim. The restool-unreachable faces —
  child-portal unlock (DPRC-I11), the `OBJ_CREATE_ALLOWED` gate, batch
  plug→probe ordering (DPRC-I8) — are recorded as explicit deferrals to tile
  #10; no ioctl/portal work lands here. The sans-io split is deliberately
  emphasized so the pure reconcile core (ITF-replayable off-board) carries as
  much of the change as possible, pre-shaping tiles #5–8.

## Capabilities

### New Capabilities

None — the change lands entirely as deltas to existing capabilities.

### Modified Capabilities

- `reconciler`: the object graph and plan vocabulary gain child-DPRC lifecycle
  states, containment guards (permission matrix, plugged-move precondition),
  and the eviction/visibility laws as planning semantics.
- `formal-models`: `models/families/dprc.qnt` grows the lifecycle sum,
  permission-matrix guards, and promotes DPRC-I1/I5/I7/I9/I10/I11 dispositions;
  invariants named and Apalache-marked per the DoD model gate.
- `intent-compiler`: the consumer/runtime construct derives its container
  (restool-default options mask, placement under the root, label) — the
  container half of ADR-0005's consumer realization.
- `mc-backend`: `McControl` (restool shim) adds the dprc verb surface;
  `KernelControl` adds VFIO `driver_override`/bind/unbind observation and
  actuation for child DPRCs.
- `mbt-harness`: the online driver gains discovery-session scenarios for
  containment/pool semantics — the first online-MBT discovery target — under
  the existing operator-supervised envelope.
- `system-integration`: board milestone suites for the container lifecycle,
  VFIO bind/unbind on scratch children, and end-to-end consumer convergence;
  scratch-first and self-cleaning, with deliberate use of the live VPP
  container allowed where a face requires it.

## Impact

- Crates: `dpaa2-api` (typestates, pure reconcile, plan guards), `dpaa2-mc`
  (restool dprc verbs, VFIO kernel control), `dpaa2-config` (consumer→container
  derivation), `dpaa2-verify` (discovery sessions, frozen traces),
  `dpaa2-tools` (convergence path exercised, minor surface).
- Models: `models/families/dprc.qnt`, `models/COVERAGE.md`, board suite index.
- Docs: `docs/baseline/dprc.md` amendments for anything the board settles or
  refutes; ADR for decisions that solidify (per DoD gate 5); roadmap row #4
  status updates.
- Deferred (recorded, not built): child-portal faces → tile #10; companion
  population → tile #6; dpni option surface → tile #5.
