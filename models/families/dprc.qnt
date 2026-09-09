// dprc family parameters (dprc.md). The container family itself: child
// containers are created under SPAWN and bound by fsl_mc_dprc
// automatically; identity (id, icid, portal) is pool-assigned at create.
//
// Named invariants (modeled; dispositions in models/COVERAGE.md):
//   DPRC-I1 core/invariants.qnt DPRC_I1; DPRC-I2 core/invariants.qnt DPRC_I2;
//   DPRC-I3 main.qnt DPRC_I3Test; DPRC-I4 core/invariants.qnt DPRC_I4;
//   DPRC-I5 core/connect.qnt canConnect + main.qnt DPNI_I9Test;
//   DPRC-I6 main.qnt DPRC_I6Test/DPRC_I6RescanRefusedTest;
//   DPRC-I7 main.qnt DPRC_I7Test; DPRC-I10 main.qnt DPRC_I10Test;
//   DPRC-I11 main.qnt DPRC_I11Test + DPRC_I11SpawnTest/DPRC_I11UnlockTest.
// Apalache-marked subset: DPRC-I1, DPRC-I2, DPRC-I4.
module dprc {
  import types.* from "../core/types"

  pure val dprcParams: FamilyParams = {
    creatable: true,
    placement: AnyContainer,
    singleton: false,
    pooled: false,
    resetOnBind: false,
    hasKernelDriver: true,        // fsl_mc_dprc binds automatically (§3)
    hotBindAvailable: true,       // fsl_mc_dprc takes new children
    endpointPorts: 0,             // dprc owns connections but has no endpoints (§2)
    createTriggersRescan: true,   // dprc create does trigger a rescan (§5 step 6)
    // Probe draws nothing; the /dev/dprc.N-opener dpmcp holds are the
    // post-probe allocate path (§3 census, last row), not a probe draw.
    draw: { dpmcp: 0, dpbp: 0, dpcon: 0 },
  }
}

// The child-container lifecycle as a sum type — the isomorphic twin (ADR-0002
// §3, quint-is-the-spec) of the Rust typestates dprc-encapsulation will grow.
// Authored model-first; sums/payloads/phases are what the Rust encoding must
// mirror, names converging on readable English both sides.
//
// The phase sum (design D3):
//   Declared -> Created -> Populated -> Plugged | Locked -> Emptied -> Destroyed
// with the kernel-side VFIO bind state carried ONLY on the Plugged face — a
// DPRC's plugged state is not restool-mutable (dprc.md "Lifecycle ordering",
// V-POOL-1 rev 2); binding to vfio-fsl-mc is the kernel lever, meaningful only
// while the container is Plugged, so bind/unbind live inside Plugged(_).
//
// Guarded transitions carry the board-settled containment laws, each anchored:
//   - Permission matrix (dprc.md unknown-register #3, V-DPRC-2/6): three
//     DISTINCT refusal statuses, never one generic denial (design D4). Each
//     option bit maps to its own refusal — SPAWN absent => SpawnViolation
//     (0x6, Configuration error), ALLOC absent => AllocViolation (0x8, No
//     resources), TOPOLOGY_CHANGES absent / a lock / a plugged-move =>
//     TopologyLockGate (0x4, No privilege). The refusal is a first-class
//     recorded outcome (lastOutcome), a taken transition that leaves the
//     container unchanged — not a dropped guard — so the trace shows WHICH bit
//     refused (spec scenario "Refusal statuses are distinguishable").
//   - Eviction law (ADR-0007 §3): destroy of a NON-EMPTY container is a valid
//     transition, not an error (dprc.md unknown-register #1 Answered). The two
//     resident kinds behave differently: CreatedIn residents die with the
//     container (release); AssignedIn residents are evicted one hop up into the
//     parent, unplugged (re-parented). Both are encoded (ResidentKind).
//   - Plugged-move precondition (DPRC-I3): assign/move of a resident is refused
//     while the resident is plugged (TopologyLockGate/0x4); assign is valid only
//     while unplugged; membership is unchanged after the refusal.
//   - Label-under-lock (V-DPRC-3, design Open Question resolved to its default):
//     set-label IS accepted on a locked child — label drift on a locked
//     container is repairable — so setLabel is permitted in the Locked phase
//     while create/destroy/assign/plug are stripped (0x4).
//   - Visibility law (DPRC-I6, Breaking): honored by construction — no action
//     claims a mutation is bus-visible and no `sync` transition exists; the only
//     visibility path is re-observing `world`. The named DPRC_I6Test property
//     stays owned by the core machine (main.qnt).
//
// Identity (icid, portal id) and the option mask are pool-assigned at create and
// never rewritten by any transition — the DPRC-I10 immutability hook.
//
// Scope: this module defines the sum, the guards, and directed `*Test` runs that
// pin the four acceptance shapes, plus (task 1.2) the named baseline-invariant
// properties and their COVERAGE.md dispositions. Named invariants and witnesses:
//   DPRC-I1  eviction boundary — `evictionBothKindsTest` / `DPRC_I1Test`
//            (a created resident never crosses into the parent);
//   DPRC-I5  connect precondition (restool face) — `connectWithTopologyTest` /
//            `connectNoTopologyRefused0x4Test` (the TOPOLOGY_CHANGES gate);
//   DPRC-I7  Linux removal ≠ MC destroy (Breaking) — `DPRC_I7Test` over the
//            no-op `busRemoveEvent`;
//   DPRC-I9  teardown reachability (liveness) — `teardownReachableTest`;
//   DPRC-I10 identity/options immutability — the `DPRC_I10` state invariant;
//   DPRC-I11 remainder (restool-reachable lock strip) — `lockStripsCreateTest` /
//            `lockRefusesPlugTest` / `unlockRestoresTest`, label-under-lock by
//            `labelUnderLockTest`.
// Apalache-marked subset (task 1.2): the `stateInvariants` conjunction — only
// DPRC-I10, the one state-expressible property here; I1/I5/I7/I9/I11 are
// action-guard, liveness or Breaking-absence properties carried as directed
// runs (simulate-only), exactly as main.qnt splits its own candidates. The
// restool-unreachable faces (the I11 child-portal unlock, the OBJ_CREATE gate,
// the I8 batch-scan ordering) are deferral rows to `mc-portal-backend` (#10).
module dprc_lifecycle {
  // The kernel-side bind state, carried only on the Plugged face. Two cases,
  // not a bool — the Rust twin is an enum (ADR-0002 §3).
  type VfioBind =
    | Unbound
    | BoundVfioFslMc

  // The container lifecycle phase (design D3).
  type ContainerState =
    | Declared            // intent exists; no MC object yet
    | Created             // dprc_create done; empty; unplugged
    | Populated           // holds residents; unplugged; the assign-only face
    | Plugged(VfioBind)   // kernel-active; VFIO bind state carried here
    | Locked              // set-locked 1: the sub-hierarchy is locked
    | Emptied             // residents evicted/released; ready to destroy
    | Destroyed

  // A resident's origin decides its fate under the eviction law (ADR-0007 §3).
  type ResidentKind =
    | CreatedIn           // created by this container: released (destroyed) with it
    | AssignedIn          // moved in from elsewhere: evicted one hop up, unplugged

  type Resident = { id: int, kind: ResidentKind, plugged: bool }

  // The per-option-bit permission mask (dprc.md create details; DPRC-I4 gives
  // the child default). objCreate is modeled for completeness though restool
  // never issues on the child's own portal (unknown-register #3: the bit gates
  // creates restool cannot reach, so it never refuses through this project).
  type Options = { spawn: bool, alloc: bool, objCreate: bool, topologyChanges: bool }

  // Pool-assigned at create, immutable thereafter (DPRC-I10).
  type Identity = { icid: int, portalId: int }

  type Container = {
    phase: ContainerState,
    options: Options,
    identity: Identity,
    label: str,
    residents: Set[Resident],
  }

  // The three distinct refusals of the permission matrix (dprc.md
  // unknown-register #3). Each names its MC status in mcStatus below.
  type Refusal =
    | SpawnViolation      // MC 0x6 Configuration error — SPAWN_ALLOWED absent
    | AllocViolation      // MC 0x8 No resources        — ALLOC_ALLOWED absent
    | TopologyLockGate    // MC 0x4 No privilege — TOPOLOGY_CHANGES / lock / plugged-move

  type Outcome =
    | Accepted
    | Refused(Refusal)

  // MC status codes (0x4/0x6/0x8 == decimal 4/6/8): a reconciler must read
  // three distinct values, never collapse them to one denial (design D4).
  pure def mcStatus(o: Outcome): int =
    match o {
      | Accepted => 0
      | Refused(r) =>
        match r {
          | SpawnViolation => 6
          | AllocViolation => 8
          | TopologyLockGate => 4
        }
    }

  // The child-DPRC default option mask (DPRC-I4): the four restool defaults;
  // TOPOLOGY_CHANGES belongs to the root, not a child.
  pure val DEFAULT_OPTIONS: Options =
    { spawn: true, alloc: true, objCreate: true, topologyChanges: false }

  // Pool-assigned identity (icid, portal id) — a constant stand-in; the point
  // is that no transition rewrites it (DPRC-I10).
  pure val POOL_IDENTITY: Identity = { icid: 100, portalId: 5 }

  // ---- state ----

  type World = {
    child: Container,             // the child container under test
    parentResidents: Set[Resident], // the parent — the eviction destination
    lastOutcome: Outcome,         // the recorded outcome of the last transition
  }

  var world: World

  // ---- phase predicates ----

  pure def isPlugged(c: Container): bool =
    match c.phase { | Plugged(_) => true | _ => false }

  pure def isLocked(c: Container): bool = c.phase == Locked

  // The two unplugged, assignable faces.
  pure def unpluggedFace(c: Container): bool =
    c.phase == Created or c.phase == Populated

  // Active = the container exists and is not gone.
  pure def isActive(c: Container): bool =
    not(c.phase == Declared) and not(c.phase == Destroyed)

  // ---- eviction (ADR-0007 §3) ----

  // Move to `target`, applying the eviction law: CreatedIn residents are
  // released (dropped) with the container; AssignedIn residents are evicted one
  // hop up into the parent, unplugged. Used by both the non-empty destroy and
  // the clean empty-first teardown.
  pure def evictInto(w: World, target: ContainerState): World =
    w.with("child", w.child.with("residents", Set()).with("phase", target))
     .with("parentResidents",
        w.parentResidents.union(
          w.child.residents
            .filter(r => r.kind == AssignedIn)
            .map(r => r.with("plugged", false))))
     .with("lastOutcome", Accepted)

  // ---- init ----

  action init = world' = {
    child: {
      phase: Declared,
      options: DEFAULT_OPTIONS,
      identity: { icid: 0, portalId: 0 },   // unassigned until create
      label: "",
      residents: Set(),
    },
    parentResidents: Set(),
    lastOutcome: Accepted,
  }

  // ---- transitions: each <name>[At] directed, with a nondet wrapper below ----

  // dprc create: Declared -> Created. Identity/options pool-assigned once and
  // never rewritten (DPRC-I10). The parent's SPAWN gate is the spawnGrandchild
  // guard below; this action creates the child under test itself.
  action createContainerWith(opts: Options): bool = all {
    world.child.phase == Declared,
    world' = world
      .with("child", world.child
        .with("phase", Created)
        .with("options", opts)
        .with("identity", POOL_IDENTITY))
      .with("lastOutcome", Accepted),
  }

  // Spawn a grandchild container. SPAWN_ALLOWED absent => SpawnViolation (0x6);
  // under lock the create class is stripped => TopologyLockGate (0x4). The
  // grandchild itself is elided (design D5, container-only population).
  action spawnGrandchild: bool = all {
    unpluggedFace(world.child) or isLocked(world.child),
    if (isLocked(world.child))
      world' = world.with("lastOutcome", Refused(TopologyLockGate))
    else if (not(world.child.options.spawn))
      world' = world.with("lastOutcome", Refused(SpawnViolation))
    else
      world' = world.with("lastOutcome", Accepted),
  }

  // Create a resident IN the child (a CreatedIn object). ALLOC_ALLOWED absent =>
  // AllocViolation (0x8: the child cannot draw pool ids); under lock the create
  // class is stripped => TopologyLockGate (0x4).
  action createResidentAt(id: int): bool = all {
    unpluggedFace(world.child) or isLocked(world.child),
    not(world.child.residents.exists(r => r.id == id)),
    if (isLocked(world.child))
      world' = world.with("lastOutcome", Refused(TopologyLockGate))
    else if (not(world.child.options.alloc))
      world' = world.with("lastOutcome", Refused(AllocViolation))
    else
      world' = world
        .with("child", world.child
          .with("residents", world.child.residents
            .union(Set({ id: id, kind: CreatedIn, plugged: false })))
          .with("phase", Populated))
        .with("lastOutcome", Accepted),
  }

  // Assign a foreign object IN (an AssignedIn resident) — valid only on the
  // unplugged face; under lock the assign class is stripped => 0x4.
  action assignResidentInAt(id: int): bool = all {
    unpluggedFace(world.child) or isLocked(world.child),
    not(world.child.residents.exists(r => r.id == id)),
    if (isLocked(world.child))
      world' = world.with("lastOutcome", Refused(TopologyLockGate))
    else
      world' = world
        .with("child", world.child
          .with("residents", world.child.residents
            .union(Set({ id: id, kind: AssignedIn, plugged: false })))
          .with("phase", Populated))
        .with("lastOutcome", Accepted),
  }

  // Move a resident OUT (assign one hop up). Plugged-move precondition
  // (DPRC-I3): a plugged resident cannot move => TopologyLockGate (0x4), and its
  // membership is unchanged after the refusal. Under lock the assign class is
  // stripped => 0x4.
  action moveResidentOutAt(id: int): bool =
    val theOne = world.child.residents.filter(r => r.id == id)
    all {
      theOne.size() == 1,
      isActive(world.child),
      if (isLocked(world.child) or theOne.exists(r => r.plugged))
        world' = world.with("lastOutcome", Refused(TopologyLockGate))
      else
        world' = world
          .with("child", world.child
            .with("residents", world.child.residents.filter(r => r.id != id)))
          .with("parentResidents",
            world.parentResidents.union(theOne.map(r => r.with("plugged", false))))
          .with("lastOutcome", Accepted),
    }

  // Plug a resident (assign --plugged=1) — the kernel bind/unbind lever. Under
  // lock, assign --plugged is refused No privilege (0x4) and the resident reads
  // back unplugged (V-DPRC-3 rev 1).
  action plugResidentAt(id: int): bool = all {
    world.child.residents.exists(r => r.id == id and not(r.plugged)),
    if (isLocked(world.child))
      world' = world.with("lastOutcome", Refused(TopologyLockGate))
    else
      world' = world
        .with("child", world.child
          .with("residents", world.child.residents
            .map(r => if (r.id == id) r.with("plugged", true) else r)))
        .with("lastOutcome", Accepted),
  }

  // Unplug a resident (assign --plugged=0). Symmetric lock strip (0x4).
  action unplugResidentAt(id: int): bool = all {
    world.child.residents.exists(r => r.id == id and r.plugged),
    if (isLocked(world.child))
      world' = world.with("lastOutcome", Refused(TopologyLockGate))
    else
      world' = world
        .with("child", world.child
          .with("residents", world.child.residents
            .map(r => if (r.id == id) r.with("plugged", false) else r)))
        .with("lastOutcome", Accepted),
  }

  // connect/disconnect issued ON the child. TOPOLOGY_CHANGES_ALLOWED absent (the
  // child default) => No privilege (0x4) while the same connect on the root
  // ancestor is accepted (V-DPRC-2-TOPO-1). Under lock, also 0x4.
  action connectEndpoints: bool = all {
    unpluggedFace(world.child) or isPlugged(world.child) or isLocked(world.child),
    if (isLocked(world.child) or not(world.child.options.topologyChanges))
      world' = world.with("lastOutcome", Refused(TopologyLockGate))
    else
      world' = world.with("lastOutcome", Accepted),
  }

  // Hand the container to the kernel/VFIO: Populated -> Plugged(Unbound).
  action plugContainer: bool = all {
    world.child.phase == Populated,
    world' = world
      .with("child", world.child.with("phase", Plugged(Unbound)))
      .with("lastOutcome", Accepted),
  }

  // Bind/unbind vfio-fsl-mc — meaningful only on the Plugged face.
  action bindVfio: bool = all {
    world.child.phase == Plugged(Unbound),
    world' = world
      .with("child", world.child.with("phase", Plugged(BoundVfioFslMc)))
      .with("lastOutcome", Accepted),
  }

  action unbindVfio: bool = all {
    world.child.phase == Plugged(BoundVfioFslMc),
    world' = world
      .with("child", world.child.with("phase", Plugged(Unbound)))
      .with("lastOutcome", Accepted),
  }

  // set-locked 1: lock the child and its hierarchy.
  action lockHierarchy: bool = all {
    unpluggedFace(world.child),
    world' = world
      .with("child", world.child.with("phase", Locked))
      .with("lastOutcome", Accepted),
  }

  // set-locked 0 (from the root): restore the unlocked face.
  action unlock: bool = all {
    isLocked(world.child),
    world' = world
      .with("child", world.child.with("phase",
        if (world.child.residents == Set()) Created else Populated))
      .with("lastOutcome", Accepted),
  }

  // set-label — accepted on a locked child (V-DPRC-3): label drift on a locked
  // container is repairable (design Open Question, default yes). Permitted in
  // every active phase, Locked included.
  action setLabelAt(newLabel: str): bool = all {
    isActive(world.child),
    world' = world
      .with("child", world.child.with("label", newLabel))
      .with("lastOutcome", Accepted),
  }

  // Clean empty-first teardown -> Emptied (the NXP destroy_dynamic_dpl order,
  // ADR-0007 context). Requires residents unplugged; applies eviction.
  action emptyContainer: bool = all {
    unpluggedFace(world.child) or isPlugged(world.child),
    world.child.residents.forall(r => not(r.plugged)),
    world' = evictInto(world, Emptied),
  }

  // dprc destroy. Under lock the destroy class is stripped => 0x4. Otherwise a
  // non-empty destroy is a VALID transition (ADR-0007 §3, dprc.md
  // unknown-register #1): CreatedIn residents released with the container,
  // AssignedIn residents evicted unplugged into the parent. Residents must be
  // unplugged (the observed shape; a plugged owned resident makes destroy
  // -EBUSY, modeled here as a disabled guard, not one of the three matrix
  // refusals).
  action destroyContainer: bool = all {
    unpluggedFace(world.child) or isPlugged(world.child)
      or world.child.phase == Emptied or isLocked(world.child),
    if (isLocked(world.child))
      world' = world.with("lastOutcome", Refused(TopologyLockGate))
    else all {
      world.child.residents.forall(r => not(r.plugged)),
      world' = evictInto(world, Destroyed),
    },
  }

  // A Linux bus event on a kernel-active container: a device_del, a driver
  // unbind, or a stray rescan notification. DPRC-I7 (Breaking, dprc.md
  // "Kernel-side behavior"): object removal is Linux-side only — the MC
  // object and its residents survive untouched — so the event leaves `world`
  // unchanged but for the recorded outcome. No transition maps a bus event to
  // Destroyed; the action exists so DPRC_I7Test can witness that absence.
  action busRemoveEvent: bool = all {
    isActive(world.child),
    world' = world.with("lastOutcome", Accepted),
  }

  action stutter: bool = world' = world

  // ---- nondet wrappers: draw args from finite constant sets ----

  pure val OPTION_DRAWS: Set[Options] = Set(
    DEFAULT_OPTIONS,                              // child default (DPRC-I4)
    DEFAULT_OPTIONS.with("spawn", false),         // SPAWN absent  => 0x6 on spawn
    DEFAULT_OPTIONS.with("alloc", false),         // ALLOC absent  => 0x8 on create-in
    DEFAULT_OPTIONS.with("topologyChanges", true) // topology permitted
  )
  pure val RESIDENT_IDS: Set[int] = Set(1, 2, 3)
  pure val LABELS: Set[str] = Set("scratch", "vpp", "consumer")

  action doCreateContainer = { nondet opts = OPTION_DRAWS.oneOf() createContainerWith(opts) }
  action doCreateResident = { nondet id = RESIDENT_IDS.oneOf() createResidentAt(id) }
  action doAssignIn = { nondet id = RESIDENT_IDS.oneOf() assignResidentInAt(id) }
  action doMoveOut = { nondet id = RESIDENT_IDS.oneOf() moveResidentOutAt(id) }
  action doPlugResident = { nondet id = RESIDENT_IDS.oneOf() plugResidentAt(id) }
  action doUnplugResident = { nondet id = RESIDENT_IDS.oneOf() unplugResidentAt(id) }
  action doSetLabel = { nondet s = LABELS.oneOf() setLabelAt(s) }

  action step = any {
    doCreateContainer,
    doCreateResident,
    doAssignIn,
    doMoveOut,
    doPlugResident,
    doUnplugResident,
    doSetLabel,
    spawnGrandchild,
    connectEndpoints,
    plugContainer,
    bindVfio,
    unbindVfio,
    lockHierarchy,
    unlock,
    emptyContainer,
    destroyContainer,
    busRemoveEvent,
    stutter,
  }

  // ---- named baseline-invariant properties (task 1.2) ----

  // DPRC-I10 — container identity is pool-assigned once and never rewritten,
  // and the option mask never drifts off the create menu: whenever the child
  // is active its identity is POOL_IDENTITY, and its options always sit in
  // OPTION_DRAWS. The immutability face of LAW 1 for dprc (dprc.md
  // "Attribute mutability": icid/portal_id/options are create-time only). The
  // one state-expressible property here, hence the only Apalache-marked one.
  val DPRC_I10: bool = and {
    isActive(world.child) implies world.child.identity == POOL_IDENTITY,
    OPTION_DRAWS.contains(world.child.options),
  }

  // The simulator/Apalache conjunction (`pnpm model:simulate` / `model:verify`
  // pass --invariant=stateInvariants): the state invariants dprc_lifecycle
  // enforces on every reachable state. Only DPRC-I10 is state-expressible;
  // I1/I5/I7/I9/I11 are directed runs (COVERAGE.md records the simulate-only
  // disposition), the same split main.qnt applies to its candidates.
  val stateInvariants: bool = and {
    DPRC_I10,
  }

  // A stutter-check step for directed runs (local, as main.qnt defines its own).
  action check(p: bool): bool = all { p, world' = world }

  // ---- directed runs (rung 2a): pin the four acceptance shapes ----

  // The canonical lifecycle, through the Plugged/VFIO face.
  run lifecycleTest =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(createResidentAt(1))
      .then(plugContainer)
      .then(bindVfio)
      .then(unbindVfio)
      .expect(world.child.phase == Plugged(Unbound))

  // Acceptance 2 — the three refusal statuses are distinct values.
  run refusalStatusesDistinctTest =
    init.then(check(Set(
      mcStatus(Refused(SpawnViolation)),
      mcStatus(Refused(AllocViolation)),
      mcStatus(Refused(TopologyLockGate))).size() == 3))

  // SPAWN violation => 0x6, recorded, container empty (dprc.md #3).
  run spawnRefused0x6Test =
    init
      .then(createContainerWith(DEFAULT_OPTIONS.with("spawn", false)))
      .then(spawnGrandchild)
      .expect(and {
        world.lastOutcome == Refused(SpawnViolation),
        mcStatus(world.lastOutcome) == 6,
      })

  // ALLOC violation => 0x8, recorded, nothing created (dprc.md #3).
  run allocRefused0x8Test =
    init
      .then(createContainerWith(DEFAULT_OPTIONS.with("alloc", false)))
      .then(createResidentAt(1))
      .expect(and {
        world.lastOutcome == Refused(AllocViolation),
        mcStatus(world.lastOutcome) == 8,
        world.child.residents == Set(),
      })

  // TOPOLOGY_CHANGES absent on the child => 0x4 for connect (V-DPRC-2-TOPO-1)…
  run connectNoTopologyRefused0x4Test =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(connectEndpoints)
      .expect(and {
        world.lastOutcome == Refused(TopologyLockGate),
        mcStatus(world.lastOutcome) == 4,
      })

  // …and present => accepted.
  run connectWithTopologyTest =
    init
      .then(createContainerWith(DEFAULT_OPTIONS.with("topologyChanges", true)))
      .then(connectEndpoints)
      .expect(world.lastOutcome == Accepted)

  // Plugged-move precondition (DPRC-I3): moving a plugged resident => 0x4, and
  // the resident stays put.
  run pluggedMoveRefused0x4Test =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(createResidentAt(1))
      .then(plugResidentAt(1))
      .then(moveResidentOutAt(1))
      .expect(and {
        world.lastOutcome == Refused(TopologyLockGate),
        mcStatus(world.lastOutcome) == 4,
        world.child.residents.exists(r => r.id == 1),
      })

  // Acceptance 3 — non-empty destroy is a transition, both resident kinds
  // handled: CreatedIn (1) released with the container, AssignedIn (2)
  // re-parented unplugged (ADR-0007 §3).
  run evictionBothKindsTest =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(createResidentAt(1))       // CreatedIn
      .then(assignResidentInAt(2))     // AssignedIn
      .then(destroyContainer)          // non-empty destroy
      .expect(and {
        world.child.phase == Destroyed,
        not(world.child.residents.exists(r => r.id == 1)),
        world.parentResidents.exists(r => r.id == 2 and not(r.plugged)),
        not(world.parentResidents.exists(r => r.id == 1)),
      })

  // Teardown reachability (DPRC-I9 hook): empty-first then destroy reaches
  // Destroyed from a populated, mixed-kind container.
  run teardownReachableTest =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(createResidentAt(1))
      .then(assignResidentInAt(2))
      .then(emptyContainer)
      .then(destroyContainer)
      .expect(world.child.phase == Destroyed)

  // Acceptance 4 — set-label is accepted on a locked child; the label changes
  // while the phase stays Locked (V-DPRC-3, repairable drift).
  run labelUnderLockTest =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(createResidentAt(1))
      .then(lockHierarchy)
      .then(setLabelAt("repaired"))
      .expect(and {
        world.child.phase == Locked,
        world.child.label == "repaired",
        world.lastOutcome == Accepted,
      })

  // Lock strips the create class (DPRC-I11 remainder) => 0x4.
  run lockStripsCreateTest =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(lockHierarchy)
      .then(createResidentAt(2))
      .expect(and {
        world.lastOutcome == Refused(TopologyLockGate),
        mcStatus(world.lastOutcome) == 4,
      })

  // Lock refuses assign --plugged (V-DPRC-3) => 0x4, resident stays unplugged.
  run lockRefusesPlugTest =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(createResidentAt(1))
      .then(lockHierarchy)
      .then(plugResidentAt(1))
      .expect(and {
        world.lastOutcome == Refused(TopologyLockGate),
        mcStatus(world.lastOutcome) == 4,
        not(world.child.residents.exists(r => r.id == 1 and r.plugged)),
      })

  // Unlock (set-locked 0 from the root) restores the create class.
  run unlockRestoresTest =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(createResidentAt(1))
      .then(lockHierarchy)
      .then(unlock)
      .then(createResidentAt(2))
      .expect(and {
        world.child.phase == Populated,
        world.lastOutcome == Accepted,
        world.child.residents.exists(r => r.id == 2),
      })

  // ---- task 1.2 named baseline-invariant runs (simulate-only) ----

  // DPRC-I1 — allocation never crosses the container boundary: the resident
  // the child created (CreatedIn) is released with the container, never
  // re-homed into the parent (a different container); only the assigned-in
  // resident evicts one hop up. A CreatedIn object surfacing in the parent
  // would be a cross-boundary allocation (dprc.md DPRC-I1, ADR-0007 §3).
  run DPRC_I1Test =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(createResidentAt(1))       // CreatedIn — the child's own
      .then(assignResidentInAt(2))     // AssignedIn — on loan from the parent
      .then(destroyContainer)
      .expect(and {
        not(world.parentResidents.exists(r => r.id == 1)),  // stayed in-container
        world.parentResidents.exists(r => r.id == 2),        // returned home
      })

  // DPRC-I7 (Breaking) — a Linux bus event is not an MC destroy: after a
  // bus-remove event on a plugged, VFIO-bound container, the container object
  // and its residents survive unchanged (dprc.md: object removal is
  // Linux-side only). Mirrors main.qnt `DPRC_I7Test` on the lifecycle sum.
  run DPRC_I7Test =
    init
      .then(createContainerWith(DEFAULT_OPTIONS))
      .then(createResidentAt(1))
      .then(plugContainer)
      .then(bindVfio)
      .then(busRemoveEvent)
      .expect(and {
        world.child.phase == Plugged(BoundVfioFslMc),
        world.child.residents.exists(r => r.id == 1),
      })
}
