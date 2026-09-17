//! Plan semantics for the child-DPRC lifecycle — the pure functions that PREDICT
//! containment outcomes rather than discover them (design D2/D4; ADR-0007 §3; DPRC-I6).
//!
//! This layers on the [`crate::dprc`] typestates (task 2.1): the phase sum and its
//! guards decide what a plan may emit, and the eviction law computes a teardown's
//! post-state before any MC command runs. It reuses the port reconciler's disruption
//! [`Class`] and mirrors the [`crate::plan::Plan`] idioms (`is_converged`, `headline`),
//! but keeps its own step and observation vocabulary: a container/resident op does not
//! fit the DPNI-anchored [`crate::plan::Transition`] (different id domain), so this is
//! the "extend where they don't fit" half of the reuse contract, not a parallel plan.
//!
//! # Guarantees this module encodes
//!
//! - **Assign-only-while-unplugged / plug-then-assign unrepresentable** (reconciler
//!   delta): [`plan_create_resident`]/[`plan_assign_in`] refuse to emit a step against a
//!   plugged or locked face, and [`plan_population`] orders every resident step before
//!   [`ContainerStep::PlugContainer`]. The type-level witness that plug-then-assign
//!   cannot even be written is the `compile_fail` doctest on
//!   [`crate::dprc::Container::plug`] (task 2.1); this module carries the runtime plan
//!   half (F3/F4: a plugged face is *absent* at the type level and *not emitted* at the
//!   plan level; a locked face is *enabled-but-refusing* at the type level and likewise
//!   *not emitted* at the plan level).
//! - **Plugged-move never planned** (DPRC-I3): [`plan_move_out`] refuses a plugged
//!   resident, attributing [`Attribution::PluggedMove`]; no move step is emitted.
//! - **Eviction-aware teardown with predicted post-state** (ADR-0007 §3;
//!   `docs/baseline/dprc.md` unknown-register #1 Answered): [`plan_teardown`] gates the
//!   MC destroy on all-residents-unplugged (F-ebusy) and predicts the post-state —
//!   created residents released, assigned-in residents evicted unplugged into the
//!   parent — via [`predict_eviction`], the twin of `dprc.qnt` `evictInto` /
//!   [`crate::dprc`]'s private `evict_into`.
//! - **Convergence by re-observation only** (DPRC-I6; `docs/baseline/dprc.md`
//!   "Silent-failure notes": a `sync` after mutating a child refreshes nothing):
//!   [`verdict`] judges an [`ObservedContainer`] freshly re-queried from the MC, never a
//!   dispatched step's assumed success. There is no `sync`-trust path anywhere in this
//!   module.
//! - **Typed refusal attribution** (design D4): [`attribute_mc`] discriminates the three
//!   MC statuses (0x6 spawn / 0x8 alloc / 0x4 topology-lock) and never collapses 0x8 to
//!   pool exhaustion when the option mask is the cause.

use std::collections::{BTreeMap, BTreeSet};

use crate::compiled::{
    Attributes, CompiledPlan, Container as Placement, PlannedObject, ProvenanceKey,
};
use crate::core::error::Error;
use crate::core::family::Permission;
use crate::core::model::{DprcId, ObjectRef};
use crate::core::types::{ConstructName, TenantName};
use crate::dprc::{
    ContainerState, ObservedResident, Options, Refusal, Resident, ResidentId, ResidentKind,
};
use crate::plan::Class;

/// The child-DPRC option mask derived from a [`PlannedObject`]'s
/// [`Attributes::Dprc`] permission set (`compiled::dprc_default_options`).
///
/// Bridges the two twin representations of the same mask: the derivation carries a
/// [`BTreeSet<Permission>`] (`{Spawn, Alloc, ObjCreate, IrqCfg}` by default), while the
/// lifecycle model ([`crate::dprc::Options`], `dprc.qnt` `type Options`) tracks the four
/// *refusal-gating* bits `{spawn, alloc, obj_create, topology_changes}`. `IrqCfg` gates
/// no containment refusal and has no lifecycle field, so it is dropped; `TopologyChanges`
/// is absent from the child default (DPRC-I4), leaving [`Options::DEFAULT`].
#[must_use]
pub fn options_from_permissions(perms: &BTreeSet<Permission>) -> Options {
    Options {
        spawn: perms.contains(&Permission::Spawn),
        alloc: perms.contains(&Permission::Alloc),
        obj_create: perms.contains(&Permission::ObjCreate),
        topology_changes: perms.contains(&Permission::TopologyChanges),
    }
}

/// One MC-granularity step in a container plan — the container analog of
/// [`crate::plan::Transition`], one MC command per variant so a backend maps each
/// one-to-one (mc-backend spec).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ContainerStep {
    /// `dprc create`: mint the child container with its derived mask, label and
    /// placement (`dprc.qnt` `createContainerWith`).
    CreateContainer {
        /// The MC label — the owning construct's name (ADR-0015 decisions 9 + 13).
        label: ConstructName,
        /// The create-time option mask (DPRC-I10, create-time-immutable).
        options: Options,
        /// Where the container lives (a consumer child DPRC lives in [`Placement::Root`]).
        placement: Placement,
    },
    /// Create a resident IN the container (a [`ResidentKind::CreatedIn`] object;
    /// `dprc.qnt` `createResidentAt`). Emitted only against an unplugged face.
    CreateResident {
        /// The resident id, unique within this container.
        id: ResidentId,
    },
    /// Assign a foreign object IN (a [`ResidentKind::AssignedIn`] resident;
    /// `dprc.qnt` `assignResidentInAt`). Emitted only against an unplugged face.
    AssignResident {
        /// The resident id, unique within this container.
        id: ResidentId,
    },
    /// `assign --plugged=0`: unplug a resident so a destroy is not `-EBUSY`
    /// (`dprc.qnt` `unplugResidentAt`, F-ebusy). Names the resident by its
    /// family-qualified [`ObjectRef`] — the operand `dprc_assign`/`dprc_unassign` need
    /// (review M1; the observation keys residents by `ObjectRef`, PASS3-F14).
    UnplugResident {
        /// The resident to unplug.
        object: ObjectRef,
    },
    /// Move a resident OUT, one hop up into the parent (`dprc.qnt` `moveResidentOutAt`).
    /// Emitted only for an unplugged resident (DPRC-I3).
    MoveResidentOut {
        /// The resident to move out.
        id: ResidentId,
    },
    /// Hand the container to the kernel/VFIO: `Populated -> Plugged` (`dprc.qnt`
    /// `plugContainer`). Ordered after every resident step (assign-before-plug).
    PlugContainer,
    /// `dprc destroy` (`dprc.qnt` `destroyContainer`; ADR-0007 §3): valid on a
    /// container whose residents are all unplugged, non-empty included.
    Destroy,
}

impl ContainerStep {
    /// The disruption class of this step (ADR-0015 decision 12), reusing the port
    /// reconciler's [`Class`] so a mixed plan has one comparable headline.
    // Distinct verbs share a class deliberately; each keeps its own arm and rationale.
    #[allow(clippy::match_same_arms)]
    #[must_use]
    pub fn class(&self) -> Class {
        match self {
            // Minting the container or placing/creating a resident perturbs topology.
            Self::CreateContainer { .. } => Class::Disruptive,
            Self::CreateResident { .. } | Self::AssignResident { .. } => Class::Disruptive,
            // Unplug drops a driver binding; move re-parents; destroy removes — disruptive.
            Self::UnplugResident { .. } | Self::MoveResidentOut { .. } | Self::Destroy => {
                Class::Disruptive
            }
            // Plugging a container makes it kernel-active where nothing yet carried
            // traffic through it: a wait-to-observe nudge, hitless (decision 12).
            Self::PlugContainer => Class::Hitless,
        }
    }
}

/// The refusal-relevant option-mask bit whose absence a [`Attribution::PermissionGap`]
/// names (the three bits that gate a containment refusal; `dprc.qnt` permission matrix).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionBit {
    /// `SPAWN_ALLOWED` — absent => MC 0x6.
    Spawn,
    /// `ALLOC_ALLOWED` — absent => MC 0x8.
    Alloc,
    /// `TOPOLOGY_CHANGES_ALLOWED` — absent => MC 0x4.
    TopologyChanges,
}

/// Why the planner refused to emit a step — the typed attribution the reconciler
/// reports instead of collapsing every denial into one shape (design D4). Covers both
/// MC-status-predicted causes and the plan-structural ones the typestate forbids.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Attribution {
    /// An option-mask bit is absent, so the create/spawn/connect would refuse
    /// (0x6/0x8/0x4). A *permission* gap — never read as pool exhaustion.
    PermissionGap {
        /// The absent bit.
        bit: OptionBit,
    },
    /// MC 0x8 with `ALLOC_ALLOWED` present: the child may draw, so 0x8 is a genuine
    /// pool exhaustion, not a permission gap — the reading the option mask rules out.
    PoolExhaustion,
    /// MC 0x4 from the lock strip: the create/destroy/assign class is stripped while
    /// the sub-hierarchy is locked (DPRC-I11 remainder).
    LockGate,
    /// MC 0x4 from the plugged-move precondition (DPRC-I3): a plugged resident cannot
    /// move; the plan does not emit the move.
    PluggedMove,
    /// The container face is not an unplugged, assignable one (plugged, declared,
    /// emptied or destroyed): the assign/create is unrepresentable at the type level
    /// (F4), so the plan declines to emit it — no MC command is issued.
    FaceNotAssignable,
    /// A destroy bounced `-EBUSY` (MC `0x10`) because a resident is still plugged — the
    /// plan-layer twin of the typestate [`crate::dprc::Teardown::ResidentPlugged`],
    /// outside the 0x4/0x6/0x8 matrix (review M1; `docs/baseline/dprc.md` DPRC-I2).
    ResidentPlugged,
    /// restool rejected the operation with its own client-side guard, before the MC
    /// saw it (design D4). A distinct attribution: it is not one of the three MC
    /// statuses. The southbound adapter assigns this (`dpaa2-mc`); the core carries it.
    RestoolClientGuard {
        /// The restool guard message, verbatim for the operator.
        detail: String,
    },
}

/// The refused MC operation; connect gates on topology, structural verbs on the lock (review M7; `docs/baseline/dprc.md` permission matrix).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verb {
    /// `dprc create` of a child container.
    SpawnChild,
    /// Create a created-in resident.
    CreateResident,
    /// `dprc connect` — the sole topology-changes-gated verb.
    Connect,
    /// `dprc destroy` of a child container — a structural verb, so its `0x4` reads the
    /// lock, never a topology gap (review M1).
    DestroyContainer,
}

/// Attributes an MC refusal to a typed cause from the mask and `verb`: a 0x4 is a topology gap only for a [`Verb::Connect`], else the lock (review M7; `docs/baseline/dprc.md` permission matrix).
#[must_use]
pub fn attribute_mc(refusal: Refusal, options: Options, verb: Verb) -> Attribution {
    match refusal {
        Refusal::SpawnViolation => Attribution::PermissionGap {
            bit: OptionBit::Spawn,
        },
        Refusal::AllocViolation => {
            if options.alloc {
                Attribution::PoolExhaustion
            } else {
                Attribution::PermissionGap {
                    bit: OptionBit::Alloc,
                }
            }
        }
        Refusal::TopologyLockGate => {
            if verb == Verb::Connect && !options.topology_changes {
                Attribution::PermissionGap {
                    bit: OptionBit::TopologyChanges,
                }
            } else {
                Attribution::LockGate
            }
        }
    }
}

/// Attributes a shim [`Error`] via [`attribute_mc`], or `None` when not a container refusal (review M7).
#[must_use]
pub fn attribute_refusal(error: &Error, options: Options, verb: Verb) -> Option<Attribution> {
    match error {
        // A destroy against a still-plugged resident bounces `-EBUSY` (MC `0x10`), the one
        // teardown refusal outside the 0x4/0x6/0x8 matrix (review M1; `docs/baseline/dprc.md` DPRC-I2).
        Error::McStatus { status: 0x10 } => Some(Attribution::ResidentPlugged),
        Error::McStatus { status } => {
            Refusal::from_status(*status).map(|r| attribute_mc(r, options, verb))
        }
        Error::RestoolGuard { detail } => Some(Attribution::RestoolClientGuard {
            detail: detail.clone(),
        }),
        _ => None,
    }
}

/// The predicted post-state of a teardown, computed before any MC command (design D2;
/// the reconciler predicts, it does not discover).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PredictedPostState {
    /// The container's phase after the teardown.
    pub final_state: ContainerState,
    /// The residents the container still holds afterwards (empty after a destroy).
    pub container_residents: BTreeMap<ObjectRef, ObservedResident>,
    /// The residents the parent gains — the assigned-in evictees plus any
    /// origin-unobservable resident kept conservatively, all unplugged (review M2, PASS3-F2).
    pub parent_gained: BTreeMap<ObjectRef, ObservedResident>,
}

/// Predicts the eviction law over `residents` for a teardown reaching `final_state`
/// (ADR-0007 §3; `dprc.qnt` `evictInto`, the twin of [`crate::dprc`]'s private
/// `evict_into`, F-evict): [`ResidentKind::CreatedIn`] residents are released with the
/// container (absent afterwards), [`ResidentKind::AssignedIn`] residents move one hop up
/// into the parent, **unplugged** — the plug bit cleared so the parent never tracks a
/// resident as bound that VFIO does not hold. DPRC-I1: a created resident never crosses
/// the boundary. Origin is `None` when the shim could not observe it, so the prediction
/// is conservative: only a *known* [`ResidentKind::CreatedIn`] is released; an unknown
/// origin is kept in `parent_gained` rather than lie that the parent gains nothing
/// (review M2, PASS3-F2; ADR-0007 note 2026-09-15).
#[must_use]
pub fn predict_eviction(
    residents: &BTreeMap<ObjectRef, ObservedResident>,
    final_state: ContainerState,
) -> PredictedPostState {
    let parent_gained = residents
        .iter()
        .filter(|(_, r)| r.origin != Some(ResidentKind::CreatedIn))
        .map(|(id, r)| {
            (
                *id,
                ObservedResident {
                    plugged: false,
                    ..*r
                },
            )
        })
        .collect();
    PredictedPostState {
        final_state,
        container_residents: BTreeMap::new(),
        parent_gained,
    }
}

/// The outcome of planning one guarded step: either the MC step to emit, or a typed
/// [`Attribution`] for why the planner refused to emit it (predicted, not discovered).
#[derive(Clone, PartialEq, Eq, Debug)]
#[must_use]
pub enum PlanOutcome {
    /// The step to emit.
    Step(ContainerStep),
    /// The planner refused; nothing is emitted, and this attributes why.
    Refused(Attribution),
}

/// Plans a create-resident step against an observed container `state` with mask
/// `options` (`dprc.qnt` `createResidentAt`). Emitted only on an unplugged face and
/// only when `ALLOC_ALLOWED` is present; a locked face is [`Attribution::LockGate`], a
/// plugged/torn-down face is [`Attribution::FaceNotAssignable`], and a missing alloc
/// bit is a predicted [`Attribution::PermissionGap`] — the doomed step is never emitted.
pub fn plan_create_resident(
    state: ContainerState,
    options: Options,
    id: ResidentId,
) -> PlanOutcome {
    match state {
        ContainerState::Created | ContainerState::Populated => {
            if options.alloc {
                PlanOutcome::Step(ContainerStep::CreateResident { id })
            } else {
                PlanOutcome::Refused(Attribution::PermissionGap {
                    bit: OptionBit::Alloc,
                })
            }
        }
        ContainerState::Locked => PlanOutcome::Refused(Attribution::LockGate),
        ContainerState::Plugged(_)
        | ContainerState::Declared
        | ContainerState::Emptied
        | ContainerState::Destroyed => PlanOutcome::Refused(Attribution::FaceNotAssignable),
    }
}

/// Plans an assign-in step against an observed container `state` (`dprc.qnt`
/// `assignResidentInAt`). Emitted only on an unplugged face; a locked face is
/// [`Attribution::LockGate`], any other face [`Attribution::FaceNotAssignable`]. No
/// `ALLOC` gate: assign draws no pool id.
pub fn plan_assign_in(state: ContainerState, id: ResidentId) -> PlanOutcome {
    match state {
        ContainerState::Created | ContainerState::Populated => {
            PlanOutcome::Step(ContainerStep::AssignResident { id })
        }
        ContainerState::Locked => PlanOutcome::Refused(Attribution::LockGate),
        ContainerState::Plugged(_)
        | ContainerState::Declared
        | ContainerState::Emptied
        | ContainerState::Destroyed => PlanOutcome::Refused(Attribution::FaceNotAssignable),
    }
}

/// Plans a move-out step for a resident (`dprc.qnt` `moveResidentOutAt`). The model's
/// guard requires an active, unlocked face and an unplugged resident (review M1; PASS2-F4
/// folds the missing active-face check the sibling planners already had): a locked
/// container is [`Attribution::LockGate`]; a non-active face (plugged, declared, emptied,
/// destroyed) is [`Attribution::FaceNotAssignable`]; a plugged resident is
/// [`Attribution::PluggedMove`] (DPRC-I3); else the [`ContainerStep::MoveResidentOut`].
pub fn plan_move_out(state: ContainerState, resident: &Resident, id: ResidentId) -> PlanOutcome {
    match state {
        ContainerState::Locked => return PlanOutcome::Refused(Attribution::LockGate),
        ContainerState::Created | ContainerState::Populated => {}
        ContainerState::Plugged(_)
        | ContainerState::Declared
        | ContainerState::Emptied
        | ContainerState::Destroyed => {
            return PlanOutcome::Refused(Attribution::FaceNotAssignable);
        }
    }
    if resident.plugged {
        return PlanOutcome::Refused(Attribution::PluggedMove);
    }
    PlanOutcome::Step(ContainerStep::MoveResidentOut { id })
}

/// A container plan — the container analog of [`crate::plan::Plan`]: an ordered list of
/// [`ContainerStep`]s plus the typed refusal [`Attribution`]s the planner recorded and,
/// for a teardown, the predicted post-state. Reuses [`Class`] for its headline.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ContainerPlan {
    /// Ordered MC steps to converge the container.
    pub steps: Vec<ContainerStep>,
    /// Typed refusals the planner recorded instead of emitting a doomed step (design D4).
    pub gaps: Vec<Attribution>,
    /// The teardown's predicted post-state, when this plan tears down (design D2).
    pub predicted: Option<PredictedPostState>,
}

impl ContainerPlan {
    /// An empty (converged) plan.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` when there is nothing to actuate.
    #[must_use]
    pub fn is_converged(&self) -> bool {
        self.steps.is_empty()
    }

    /// The plan's headline disruption class (ADR-0015 decision 12): the maximum over
    /// its steps, or [`Class::Hitless`] for an empty plan. Gaps and the prediction are
    /// non-actuating reports and never raise the headline.
    #[must_use]
    pub fn headline(&self) -> Class {
        self.steps
            .iter()
            .map(ContainerStep::class)
            .max()
            .unwrap_or(Class::Hitless)
    }
}

/// Plans the population of a fresh container with `residents`, in the create-time
/// order the lifecycle requires: every resident step first, then a single
/// [`ContainerStep::PlugContainer`] when `plug` is asked and residents were placed
/// (`dprc.qnt`: `plugContainer` needs the [`ContainerState::Populated`] face). The
/// assign-before-plug ordering is by construction — no resident step follows the plug —
/// which is the plan half of "plug-then-assign is unrepresentable"; the type half is the
/// `compile_fail` witness on [`crate::dprc::Container::plug`].
///
/// A [`ResidentKind::CreatedIn`] resident under a mask lacking `ALLOC_ALLOWED` is
/// recorded as a predicted [`Attribution::PermissionGap`] gap and its step skipped
/// (design D4); an assign-in draws no pool id and is always emitted on the unplugged
/// face.
#[must_use]
pub fn plan_population(
    label: ConstructName,
    options: Options,
    residents: &[(ResidentId, ResidentKind)],
    plug: bool,
) -> ContainerPlan {
    let mut plan = ContainerPlan::new();
    plan.steps.push(ContainerStep::CreateContainer {
        label,
        options,
        placement: Placement::Root,
    });

    // After the create the container is on an unplugged face; every resident step is
    // planned here, ahead of any plug — the ordering guarantee.
    let mut placed = 0usize;
    for &(id, kind) in residents {
        let face = if placed == 0 {
            ContainerState::Created
        } else {
            ContainerState::Populated
        };
        let outcome = match kind {
            ResidentKind::CreatedIn => plan_create_resident(face, options, id),
            ResidentKind::AssignedIn => plan_assign_in(face, id),
        };
        match outcome {
            PlanOutcome::Step(step) => {
                plan.steps.push(step);
                placed += 1;
            }
            PlanOutcome::Refused(gap) => plan.gaps.push(gap),
        }
    }

    // Plug only a populated container (residents present): plugging an empty container
    // is not a valid transition (`dprc.qnt` `plugContainer` requires Populated).
    if plug && placed > 0 {
        plan.steps.push(ContainerStep::PlugContainer);
    }
    plan
}

/// Plans the teardown of an observed container (ADR-0007 §3; `docs/baseline/dprc.md`
/// unknown-register #1 Answered: a non-empty destroy is valid). The MC destroy is gated
/// on all-residents-unplugged (F-ebusy): every plugged resident is unplugged first, then
/// a single [`ContainerStep::Destroy`]. The [`ContainerPlan::predicted`] post-state is
/// computed up front by [`predict_eviction`] — the reconciler predicts the outcome
/// rather than discovering it, and re-observation (DPRC-I6) later confirms it. A
/// [`ContainerState::Locked`] container yields an empty plan plus an
/// [`Attribution::LockGate`] gap: the lock strips every teardown step (review M1).
#[must_use]
pub fn plan_teardown(observed: &ObservedContainer) -> ContainerPlan {
    let mut plan = ContainerPlan::new();
    // A locked container has no enabled teardown step: record the gap, emit nothing (review M1; PASS2-F3; `docs/baseline/dprc.md` DPRC-I11 lock face).
    if observed.state == ContainerState::Locked {
        plan.gaps.push(Attribution::LockGate);
        return plan;
    }
    for (object, resident) in &observed.residents {
        if resident.plugged {
            plan.steps
                .push(ContainerStep::UnplugResident { object: *object });
        }
    }
    plan.steps.push(ContainerStep::Destroy);
    plan.predicted = Some(predict_eviction(
        &observed.residents,
        ContainerState::Destroyed,
    ));
    plan
}

/// A freshly re-observed child container — the input a convergence verdict is judged
/// against (DPRC-I6). Read by re-querying the MC, never assumed from a dispatched step.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ObservedContainer {
    /// The observed lifecycle phase.
    pub state: ContainerState,
    /// The observed option mask.
    pub options: Options,
    /// The observed MC label.
    pub label: ConstructName,
    /// Where the container was observed to live.
    pub placement: Placement,
    /// The residents observed in the container, keyed by family-qualified [`ObjectRef`]
    /// so two same-ordinal residents of different families (e.g. `dpbp.0` and `dpmcp.0`)
    /// both count — the [`crate::dprc::Container`] model twin keeps its [`ResidentId`] key
    /// untouched (review M1; PASS3-F14; ADR-0014). Each carries [`ObservedResident`], whose
    /// origin is `None` when unobservable (review M2, PASS3-F2).
    pub residents: BTreeMap<ObjectRef, ObservedResident>,
}

/// One way an observed container diverges from its declared intent (the reasons a
/// [`ContainerVerdict::Diverged`] carries).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Divergence {
    /// The container does not exist on the board.
    Missing,
    /// The observed label differs from the derived construct name.
    LabelDrift,
    /// The observed option mask differs from the derived mask.
    OptionsDrift,
    /// The observed placement differs from the derived placement.
    PlacementDrift,
}

/// The verdict of a convergence check.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ContainerVerdict {
    /// The observation matches intent — converged.
    Converged,
    /// The observation diverges; each reason is named.
    Diverged(Vec<Divergence>),
}

/// Judges whether an observed consumer container has converged to its derived intent
/// (existence, options, label, placement — the container-only surface of this change;
/// companion sizing and dpni options are later tiles).
///
/// The verdict is a pure diff of `observed` against `desired`: success comes from a
/// fresh MC re-observation, never from assuming a dispatched step worked, and this
/// function issues no `sync` (DPRC-I6; `docs/baseline/dprc.md` "Silent-failure notes").
/// `desired` is the derived child-DPRC [`PlannedObject`]; `observed` is `None` when the
/// container was not found on the board.
#[must_use]
pub fn verdict(desired: &PlannedObject, observed: Option<&ObservedContainer>) -> ContainerVerdict {
    let Some(observed) = observed else {
        return ContainerVerdict::Diverged(vec![Divergence::Missing]);
    };
    let mut reasons = Vec::new();
    if observed.label != *desired.label() {
        reasons.push(Divergence::LabelDrift);
    }
    if let Attributes::Dprc { options } = desired.attributes()
        && observed.options != options_from_permissions(options)
    {
        reasons.push(Divergence::OptionsDrift);
    }
    if observed.placement != *desired.container() {
        reasons.push(Divergence::PlacementDrift);
    }
    if reasons.is_empty() {
        ContainerVerdict::Converged
    } else {
        ContainerVerdict::Diverged(reasons)
    }
}

/// The child-DPRC realization the intent compiler derives for one declared consumer
/// runtime (ADR-0005 §1 consumer/runtime construct; `docs/baseline/dprc.md` "Intent
/// mapping": one child DPRC per declared consumer).
///
/// A consumer runtime is a declared [`crate::intent::Tenant`] that owns its own
/// container — an isolated tenant or a public holder. This is the container ALONE: the
/// board-verified default option mask ([`Options::DEFAULT`], DPRC-I4 —
/// `{spawn, alloc, obj_create}` with `topology_changes` reserved to the root), root
/// placement ([`Placement::Root`], dprc.1), and the consumer's name-keyed label as a
/// [`ConstructName`] (ADR-0015 decisions 9+13 — never a `String` in a name slot).
///
/// Container-only (this change, tile #4): no companion (dpio/dpbp/dpcon/dpmcp) or dpni
/// is realized here — the sizing rules stay dormant until tiles #5/#6. The realization
/// carries the derived object's provenance key ([`ProvenanceKey`]) so a caller resolves
/// its rule node — the baseline anchor — in the same [`CompiledPlan`] provenance DAG the
/// intent layer already populates (design D6).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConsumerContainer {
    /// The consumer runtime the container belongs to.
    pub tenant: TenantName,
    /// The MC label — the consumer's name-keyed identity (ADR-0015 decisions 9+13).
    pub label: ConstructName,
    /// The create-time-immutable option mask, typed as the lifecycle [`Options`]
    /// (task 2.1) — the board-verified child default (DPRC-I4).
    pub options: Options,
    /// Where the container lives: a consumer's child DPRC sits under the root
    /// container ([`Placement::Root`], dprc.1).
    pub placement: Placement,
    /// The derived object's provenance key; its node in the plan's DAG carries the
    /// baseline anchor (`docs/baseline/dprc.md`).
    pub provenance: ProvenanceKey,
}

/// Derives, from a compiled plan, the child-DPRC realization of every declared consumer
/// runtime (intent-compiler spec: "A declared consumer derives its container").
///
/// The realization reads the child DPRC the intent compiler already emitted (one
/// `Family::Dprc` object per consumer, [`crate::intent::Tenant::child_dprc`]) and re-keys
/// it into the typestate vocabulary: the derivation's [`Permission`] mask becomes the
/// lifecycle [`Options`] via [`options_from_permissions`] (the one bridge — no second
/// mask representation). The reserved kernel tenant remains the root container and emits
/// no child DPRC, so it never appears here (ADR-0005; `docs/baseline/dprc.md` "Intent
/// mapping"). Container-only by construction: only the `Dprc` family is projected, so no
/// companion or dpni the full plan may carry for the consumer is realized (tiles #5/#6).
#[must_use]
pub fn derive_consumer_containers(plan: &CompiledPlan) -> BTreeMap<TenantName, ConsumerContainer> {
    plan.objects
        .iter()
        .filter_map(|o| {
            let Attributes::Dprc { options } = o.attributes() else {
                return None;
            };
            let tenant = o.key().tenant.clone();
            let realization = ConsumerContainer {
                tenant: tenant.clone(),
                label: o.label().clone(),
                options: options_from_permissions(options),
                placement: o.container().clone(),
                provenance: o.provenance().clone(),
            };
            Some((tenant, realization))
        })
        .collect()
}

/// Plans convergence for a declared consumer's child container against a fresh
/// observation (design D5: container-only — existence, options, label, placement; no
/// companion sizing (tile #6) or dpni option surface (tile #5)).
///
/// An absent container yields exactly one [`ContainerStep::CreateContainer`] with the
/// derived mask/label/placement — and because the function plans the single child DPRC
/// object, the plan contains no companion-population steps by construction. A present,
/// converged container yields an empty plan; divergence on the container-only surface is
/// left to a follow-on repair tile. `desired` must be the tenant's child-DPRC
/// [`PlannedObject`] ([`crate::intent::Tenant::child_dprc`]); a non-DPRC object yields
/// an empty plan.
#[must_use]
pub fn plan_consumer_container(
    desired: &PlannedObject,
    observed: Option<&ObservedContainer>,
) -> ContainerPlan {
    let mut plan = ContainerPlan::new();
    // An `Attributes::Dprc` object IS the tenant's child DPRC (`Family::Dprc`); a
    // non-DPRC object is not this planner's concern and yields an empty plan.
    let Attributes::Dprc { options } = desired.attributes() else {
        return plan;
    };
    if observed.is_none() {
        plan.steps.push(ContainerStep::CreateContainer {
            label: desired.label().clone(),
            options: options_from_permissions(options),
            placement: desired.container().clone(),
        });
    }
    plan
}

/// One declared consumer's container-only convergence: its 4.1 realization paired with
/// the plan to reach it from a fresh observation and the re-observation verdict
/// (design D2/D5; DPRC-I6). Container-only by construction — the realization comes from
/// [`derive_consumer_containers`], which projects the child DPRC alone, so no
/// companion/dpni step can appear (reconciler delta "Consumer convergence is
/// container-only"; carry-forward decision pinned on bead cd3.8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConsumerConvergence {
    /// The derived child-DPRC realization (task 4.1): label, options, placement, and the
    /// provenance key that resolves the baseline anchor in the plan's DAG.
    pub container: ConsumerContainer,
    /// The container-only plan to reach it from the observation (empty when converged).
    pub plan: ContainerPlan,
    /// The re-observation verdict against the derived intent (DPRC-I6).
    pub verdict: ContainerVerdict,
}

/// Plans container-only convergence for every declared consumer in `compiled` against a
/// fresh observation `observed` keyed by re-observation handle (design D2/D5; DPRC-I6:
/// the observation is re-queried, never a `sync`-assumed state).
///
/// The single seam the imperative shell drives: it projects the consumers via
/// [`derive_consumer_containers`] (container-only — the child DPRC alone), matches each
/// to its observed container by name-keyed label, and pairs the create-only
/// [`plan_consumer_container`] plan with the [`verdict`]. Because every plan is projected
/// from the `Family::Dprc` object alone, no companion-population step is representable —
/// the intent layer's dormant companion/dpni sizing (tiles #5/#6) never reaches this
/// path (carry-forward decision, bead cd3.8).
#[must_use]
pub fn plan_consumer_convergence(
    compiled: &CompiledPlan,
    observed: &BTreeMap<DprcId, ObservedContainer>,
) -> Vec<ConsumerConvergence> {
    let derived = derive_consumer_containers(compiled);
    compiled
        .objects
        .iter()
        .filter_map(|object| {
            if !matches!(object.attributes(), Attributes::Dprc { .. }) {
                return None;
            }
            let container = derived.get(&object.key().tenant)?.clone();
            // Match the observed container by its name-keyed label (ADR-0015 decisions
            // 9+13); the id domain is the MC's, so identity rides on the label, not the
            // ordinal (`derive_consumer_containers` keys intent by name).
            let seen = observed.values().find(|c| c.label == container.label);
            Some(ConsumerConvergence {
                container,
                plan: plan_consumer_container(object, seen),
                verdict: verdict(object, seen),
            })
        })
        .collect()
}

// ---- undeclared-consumer prune (reconciler spec) ----

/// The classification a drift report assigns each observed child container against the
/// declared-consumer set (reconciler spec "Undeclared consumer containers are pruned
/// under the double gate"; `dprc.qnt` `type PruneBucket`).
///
/// The Rust twin of the model sum, same declaration order. A container is [`Converged`]
/// when its label names a declared consumer, a prune candidate ([`PruneCandidateFull`] or
/// [`PruneCandidatePartial`]) when it matches no declared consumer but carries a non-empty
/// label, and [`ReportOnly`] when its label is empty (a bare-restool create, or the voided
/// label of the accepted DPRC-I12 escape) — the report-only fence never touches it
/// (ADR-0001 §4).
///
/// [`Converged`]: PruneBucket::Converged
/// [`PruneCandidateFull`]: PruneBucket::PruneCandidateFull
/// [`PruneCandidatePartial`]: PruneBucket::PruneCandidatePartial
/// [`ReportOnly`]: PruneBucket::ReportOnly
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PruneBucket {
    /// The label names a declared consumer — the intent owns it; drift repair on it is
    /// [`plan_consumer_convergence`]'s job, not the pruner's.
    Converged,
    /// No declared match; full fingerprint (non-empty label + default mask + root).
    PruneCandidateFull,
    /// No declared match; partial fingerprint (non-empty label + a matched subset).
    PruneCandidatePartial,
    /// Empty label (or zero overlap): unmanaged, never touched (ADR-0001 §4).
    ReportOnly,
}

/// The [`PruneBucket`] variant names, in declaration order — the Rust copy of the
/// `dprc.qnt` `type PruneBucket` cases (ADR-0014-style parity, tied by
/// [`PruneBucket::name`]).
pub const PRUNE_BUCKETS: [&str; 4] = [
    "Converged",
    "PruneCandidateFull",
    "PruneCandidatePartial",
    "ReportOnly",
];

impl PruneBucket {
    /// This variant's name, the token [`PRUNE_BUCKETS`] lists.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Converged => "Converged",
            Self::PruneCandidateFull => "PruneCandidateFull",
            Self::PruneCandidatePartial => "PruneCandidatePartial",
            Self::ReportOnly => "ReportOnly",
        }
    }
}

/// One field of the ownership fingerprint (reconciler spec): the typed vocabulary a
/// dry-run render names as matched or unmatched, so the operator sees WHY a container
/// landed in its bucket rather than a bare verdict.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum FingerprintField {
    /// The MC label — matched iff non-empty (`dpaa2ctl` always labels; bare restool
    /// creates do not).
    Label,
    /// The create-time option mask — matched iff it equals the derived child default.
    OptionsMask,
    /// The container placement — matched iff it sits under the root (dprc.1).
    Placement,
}

/// A container's prune classification: its [`PruneBucket`] plus the matched and unmatched
/// fingerprint fields the dry-run render surfaces (reconciler spec: every candidate
/// "rendered with its matched and unmatched fingerprint fields").
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PruneClassification {
    /// The bucket the observed container falls into.
    pub bucket: PruneBucket,
    /// The fingerprint fields that matched the ownership fingerprint.
    pub matched: BTreeSet<FingerprintField>,
    /// The fingerprint fields that did not.
    pub unmatched: BTreeSet<FingerprintField>,
}

/// Classifies one observed child container against the declared-consumer set by ownership
/// fingerprint (reconciler spec "Undeclared consumer containers are pruned under the
/// double gate"; `dprc.qnt` `pruneBucket`).
///
/// The bucket precedence mirrors the `dprc.qnt` `pruneBucket` arms exactly, in order:
/// a label that names a declared consumer is [`PruneBucket::Converged`] first of all —
/// repairing drift on a declared tenant stays [`plan_consumer_convergence`]'s job, not the
/// pruner's; else an empty label is [`PruneBucket::ReportOnly`] (the ADR-0001 §4
/// report-only fence, and the shape the accepted DPRC-I12 escape leaves behind — a voided
/// label reads as unmanaged, design D8 (dprc-encapsulation)); else the full
/// fingerprint (non-empty label + derived default mask + root) is
/// [`PruneBucket::PruneCandidateFull`]; else [`PruneBucket::PruneCandidatePartial`]. The
/// matched/unmatched sets are computed the same way for every bucket (report-only
/// included), so the renderer can show why nothing matched.
#[must_use]
pub fn classify_container(
    observed: &ObservedContainer,
    declared: &BTreeMap<TenantName, ConsumerContainer>,
) -> PruneClassification {
    let default_mask = options_from_permissions(&crate::compiled::dprc_default_options());

    let label_ok = !observed.label.is_empty();
    let mask_ok = observed.options == default_mask;
    let root_ok = observed.placement == Placement::Root;

    let mut matched = BTreeSet::new();
    let mut unmatched = BTreeSet::new();
    for (field, ok) in [
        (FingerprintField::Label, label_ok),
        (FingerprintField::OptionsMask, mask_ok),
        (FingerprintField::Placement, root_ok),
    ] {
        if ok {
            matched.insert(field);
        } else {
            unmatched.insert(field);
        }
    }

    // Precedence pinned to the `dprc.qnt` `pruneBucket` arm order: declared match, then the
    // empty-label fence, then full, then partial.
    let bucket = if declared.values().any(|c| c.label == observed.label) {
        PruneBucket::Converged
    } else if !label_ok {
        PruneBucket::ReportOnly
    } else if mask_ok && root_ok {
        PruneBucket::PruneCandidateFull
    } else {
        PruneBucket::PruneCandidatePartial
    };

    PruneClassification {
        bucket,
        matched,
        unmatched,
    }
}

/// One observed child container's prune disposition: its [`PruneClassification`] paired
/// with the eviction-law teardown plan, present only for a prune candidate.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PruneItem {
    /// The container's classification and matched/unmatched fingerprint fields.
    pub classification: PruneClassification,
    /// The teardown plan (ADR-0007 §3; [`plan_teardown`]) carrying the eviction-law
    /// predicted post-state — `Some` for the two prune-candidate buckets, `None` for
    /// [`PruneBucket::Converged`] and [`PruneBucket::ReportOnly`]. Even when present, the
    /// plan is only ever dispatched behind the `--prune` + `--allow disruptive` double
    /// gate (reconciler spec); the gate itself is the engine's, not this pure core's.
    pub plan: Option<ContainerPlan>,
}

/// Classifies every observed root child container against the declared-consumer set and
/// pairs each with its prune disposition (reconciler spec "Undeclared consumer containers
/// are pruned under the double gate").
///
/// Every observed root child yields a [`PruneItem`]: the [`classify_container`] verdict for
/// all, plus a [`plan_teardown`] plan (ADR-0007 §3, carrying the eviction-law predicted
/// post-state) for exactly the two prune-candidate buckets. Converged and report-only
/// containers carry no plan. This is pure prediction only — no `--prune`/`--allow
/// disruptive` gate logic lives here; the double gate that decides whether a candidate's
/// plan is actually dispatched is the engine's (the dispatch half, a separate bead), and
/// prune success is judged by re-observation only (DPRC-I6).
#[must_use]
pub fn plan_prune(
    observed: &BTreeMap<DprcId, ObservedContainer>,
    declared: &BTreeMap<TenantName, ConsumerContainer>,
) -> BTreeMap<DprcId, PruneItem> {
    observed
        .iter()
        .map(|(id, container)| {
            let classification = classify_container(container, declared);
            let plan = match classification.bucket {
                PruneBucket::PruneCandidateFull | PruneBucket::PruneCandidatePartial => {
                    Some(plan_teardown(container))
                }
                PruneBucket::Converged | PruneBucket::ReportOnly => None,
            };
            (
                *id,
                PruneItem {
                    classification,
                    plan,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! The reconciler-delta scenarios (`specs/reconciler/spec.md`) covered as pure unit
    //! tests, plus the parity of [`predict_eviction`] with the task-2.1 typestate.

    use super::*;
    use crate::dprc::{Container, Parent, Teardown, VfioBind};
    use crate::intent::{Dataplane, Isolation, Tenant};

    /// An observed resident with a known origin — the observation type the census and
    /// eviction prediction traffic in (review M2, PASS3-F2).
    fn res(kind: ResidentKind, plugged: bool) -> ObservedResident {
        ObservedResident {
            origin: Some(kind),
            plugged,
        }
    }

    /// A model-twin [`Resident`] for the typestate-facing planners (`plan_move_out`),
    /// whose origin is always known by construction.
    fn core_res(kind: ResidentKind, plugged: bool) -> Resident {
        Resident { kind, plugged }
    }

    /// A family-qualified resident key for the observation type (review M1; PASS3-F14).
    fn oref(ordinal: u32) -> ObjectRef {
        ObjectRef::new(crate::core::family::Family::Dpbp, ordinal)
    }

    fn consumer_dprc() -> PlannedObject {
        // A declared isolated consumer's child-DPRC PlannedObject (compiled::Tenant).
        Tenant {
            name: "vpp".into(),
            dataplane: Dataplane::UserspacePoll,
            max_cores: 4,
            isolation: Isolation::Isolated,
            renamed: None,
        }
        .child_dprc()
    }

    // ---- Requirement: Child DPRC lifecycle is typestated ----

    #[test]
    fn scenario_plugged_object_is_never_planned_into_a_move() {
        // "Plugged objects cannot be planned into a move": a plugged resident yields no
        // move step, attributed PluggedMove (DPRC-I3).
        let plugged = core_res(ResidentKind::AssignedIn, true);
        let out = plan_move_out(ContainerState::Populated, &plugged, ResidentId::new(1));
        assert_eq!(out, PlanOutcome::Refused(Attribution::PluggedMove));

        // An unplugged resident does yield the move step.
        let unplugged = core_res(ResidentKind::AssignedIn, false);
        assert_eq!(
            plan_move_out(ContainerState::Populated, &unplugged, ResidentId::new(1)),
            PlanOutcome::Step(ContainerStep::MoveResidentOut {
                id: ResidentId::new(1)
            })
        );
    }

    #[test]
    fn scenario_population_only_while_unplugged() {
        // "Population is representable only while unplugged": no assign/create step is
        // emitted against a plugged or locked face.
        for face in [
            ContainerState::Plugged(VfioBind::Unbound),
            ContainerState::Plugged(VfioBind::BoundVfioFslMc),
        ] {
            assert_eq!(
                plan_create_resident(face, Options::DEFAULT, ResidentId::new(1)),
                PlanOutcome::Refused(Attribution::FaceNotAssignable)
            );
            assert_eq!(
                plan_assign_in(face, ResidentId::new(1)),
                PlanOutcome::Refused(Attribution::FaceNotAssignable)
            );
        }
        assert_eq!(
            plan_create_resident(ContainerState::Locked, Options::DEFAULT, ResidentId::new(1)),
            PlanOutcome::Refused(Attribution::LockGate)
        );
    }

    #[test]
    fn scenario_population_orders_assign_before_plug() {
        // The plan half of "plug-then-assign is unrepresentable": every resident step
        // precedes the single PlugContainer step.
        let plan = plan_population(
            "vpp".into(),
            Options::DEFAULT,
            &[
                (ResidentId::new(1), ResidentKind::CreatedIn),
                (ResidentId::new(2), ResidentKind::AssignedIn),
            ],
            true,
        );
        let plug_at = plan
            .steps
            .iter()
            .position(|s| *s == ContainerStep::PlugContainer)
            .expect("a populated plan plugs");
        // Every resident step sits before the plug.
        for step in &plan.steps[..plug_at] {
            assert!(matches!(
                step,
                ContainerStep::CreateContainer { .. }
                    | ContainerStep::CreateResident { .. }
                    | ContainerStep::AssignResident { .. }
            ));
        }
        assert_eq!(plug_at, plan.steps.len() - 1, "plug is the last step");
    }

    #[test]
    fn empty_population_does_not_plug() {
        // Plugging needs the Populated face; an empty container is never plugged.
        let plan = plan_population("vpp".into(), Options::DEFAULT, &[], true);
        assert!(!plan.steps.contains(&ContainerStep::PlugContainer));
    }

    // ---- Requirement: Containment refusals are discriminated, not collapsed ----

    #[test]
    fn scenario_alloc_refusal_is_a_permission_gap_not_exhaustion() {
        // REQUIRED: an ALLOC-refused create on a mask lacking the alloc bit is attributed
        // to the option mask, never to pool exhaustion (design D4).
        let no_alloc = Options {
            alloc: false,
            ..Options::DEFAULT
        };
        let attr = attribute_mc(Refusal::AllocViolation, no_alloc, Verb::CreateResident);
        assert_eq!(
            attr,
            Attribution::PermissionGap {
                bit: OptionBit::Alloc
            }
        );
        assert_ne!(attr, Attribution::PoolExhaustion);

        // And the plan predicts it, refusing before emitting a doomed create step.
        assert_eq!(
            plan_create_resident(ContainerState::Created, no_alloc, ResidentId::new(1)),
            PlanOutcome::Refused(Attribution::PermissionGap {
                bit: OptionBit::Alloc
            })
        );
    }

    #[test]
    fn alloc_with_the_bit_present_reads_as_exhaustion() {
        // The counter-reading: 0x8 with ALLOC present is genuine exhaustion.
        assert_eq!(
            attribute_mc(
                Refusal::AllocViolation,
                Options::DEFAULT,
                Verb::CreateResident
            ),
            Attribution::PoolExhaustion
        );
    }

    #[test]
    fn the_three_statuses_map_to_distinct_attributions() {
        // 0x6/0x8/0x4 are never collapsed to one shape (design D4).
        let no_topo = Options::DEFAULT; // topology_changes is false by default (DPRC-I4)
        assert_eq!(
            attribute_mc(Refusal::SpawnViolation, Options::DEFAULT, Verb::SpawnChild),
            Attribution::PermissionGap {
                bit: OptionBit::Spawn
            }
        );
        assert_eq!(
            attribute_mc(Refusal::TopologyLockGate, no_topo, Verb::Connect),
            Attribution::PermissionGap {
                bit: OptionBit::TopologyChanges
            }
        );
        assert_eq!(
            attribute_mc(
                Refusal::TopologyLockGate,
                Options {
                    topology_changes: true,
                    ..Options::DEFAULT
                },
                Verb::Connect
            ),
            Attribution::LockGate
        );
    }

    #[test]
    fn create_verb_0x4_reads_the_lock_gate_not_a_topology_gap() {
        // A create verb's 0x4 is the lock, not the child's absent topology bit (review M7).
        assert_eq!(
            attribute_mc(
                Refusal::TopologyLockGate,
                Options::DEFAULT,
                Verb::CreateResident
            ),
            Attribution::LockGate
        );
    }

    #[test]
    fn attribution_covers_exactly_the_model_refusal_vocabulary() {
        // attribute_mc discriminates exactly dprc.qnt `type Refusal`, via REFUSAL_VARIANTS.
        let vocabulary = [
            Refusal::SpawnViolation,
            Refusal::AllocViolation,
            Refusal::TopologyLockGate,
        ];
        for (refusal, name) in vocabulary.iter().zip(crate::dprc::REFUSAL_VARIANTS) {
            assert_eq!(refusal.name(), name);
            let _: Attribution = attribute_mc(*refusal, Options::DEFAULT, Verb::Connect);
        }
        assert_eq!(
            crate::dprc::REFUSAL_VARIANTS,
            ["SpawnViolation", "AllocViolation", "TopologyLockGate"]
        );
    }

    // ---- Requirement: Destroy planning encodes the eviction law ----

    #[test]
    fn scenario_non_empty_teardown_predicts_post_state() {
        // "Non-empty scratch container teardown": created resident absent, assigned-in
        // present unplugged in the parent (ADR-0007 §3).
        let mut residents = BTreeMap::new();
        residents.insert(oref(1), res(ResidentKind::CreatedIn, false));
        residents.insert(oref(2), res(ResidentKind::AssignedIn, false));
        let observed = ObservedContainer {
            state: ContainerState::Populated,
            options: Options::DEFAULT,
            label: "vpp".into(),
            placement: Placement::Root,
            residents,
        };
        let plan = plan_teardown(&observed);
        let predicted = plan.predicted.expect("a teardown predicts its post-state");
        assert_eq!(predicted.final_state, ContainerState::Destroyed);
        // Created (1) released with the container; assigned-in (2) re-parented unplugged.
        assert!(!predicted.parent_gained.contains_key(&oref(1)));
        let evictee = &predicted.parent_gained[&oref(2)];
        assert!(!evictee.plugged);
        assert!(predicted.container_residents.is_empty());
        assert_eq!(plan.steps, vec![ContainerStep::Destroy]);
    }

    #[test]
    fn teardown_gates_destroy_on_all_residents_unplugged() {
        // F-ebusy: a plugged resident is unplugged first, then destroy — never a destroy
        // against a plugged resident (MC -EBUSY).
        let mut residents = BTreeMap::new();
        residents.insert(oref(1), res(ResidentKind::CreatedIn, true));
        let observed = ObservedContainer {
            state: ContainerState::Populated,
            options: Options::DEFAULT,
            label: "vpp".into(),
            placement: Placement::Root,
            residents,
        };
        let plan = plan_teardown(&observed);
        assert_eq!(
            plan.steps,
            vec![
                ContainerStep::UnplugResident { object: oref(1) },
                ContainerStep::Destroy,
            ]
        );
    }

    #[test]
    fn locked_teardown_emits_no_step_and_a_lock_gate_gap() {
        // PASS2-F3 (review M1): a Locked container plans zero steps + a LockGate gap.
        let observed = ObservedContainer {
            state: ContainerState::Locked,
            options: Options::DEFAULT,
            label: "locked".into(),
            placement: Placement::Root,
            residents: BTreeMap::from([(oref(1), res(ResidentKind::CreatedIn, false))]),
        };
        let plan = plan_teardown(&observed);
        assert!(plan.steps.is_empty(), "a locked teardown emits no step");
        assert_eq!(plan.gaps, vec![Attribution::LockGate]);
        assert!(plan.predicted.is_none());
    }

    #[test]
    fn two_same_ordinal_residents_of_different_families_both_count() {
        // PASS3-F14 (review M1): `dpbp.0` and `dpmcp.0` are distinct ObjectRef keys.
        use crate::core::family::Family;
        let dpbp0 = ObjectRef::new(Family::Dpbp, 0);
        let dpmcp0 = ObjectRef::new(Family::Dpmcp, 0);
        let observed = ObservedContainer {
            state: ContainerState::Populated,
            options: Options::DEFAULT,
            label: "twin".into(),
            placement: Placement::Root,
            residents: BTreeMap::from([
                (dpbp0, res(ResidentKind::CreatedIn, true)),
                (dpmcp0, res(ResidentKind::CreatedIn, false)),
            ]),
        };
        assert_eq!(
            observed.residents.len(),
            2,
            "same-ordinal families both count"
        );
        let plan = plan_teardown(&observed);
        assert!(
            plan.steps
                .contains(&ContainerStep::UnplugResident { object: dpbp0 }),
            "the plugged same-ordinal resident is unplugged by its ObjectRef"
        );
    }

    #[test]
    fn move_out_of_a_torn_down_face_is_face_not_assignable() {
        // PASS2-F4 (review M1): a non-active face refuses the move like its sibling planners.
        let r = core_res(ResidentKind::AssignedIn, false);
        assert_eq!(
            plan_move_out(ContainerState::Destroyed, &r, ResidentId::new(1)),
            PlanOutcome::Refused(Attribution::FaceNotAssignable)
        );
    }

    #[test]
    fn predict_eviction_matches_the_task_2_1_typestate() {
        // Parity: the plan-layer prediction equals what the 2.1 typestate's destroy does
        // (the canonical evict_into), binding the duplicated law to its owner (F-evict).
        // The prediction keys by ObjectRef (review M1); the typestate twin by ResidentId — same ordinals bind them.
        let mut residents = BTreeMap::new();
        residents.insert(oref(1), res(ResidentKind::CreatedIn, false));
        residents.insert(oref(2), res(ResidentKind::AssignedIn, false));
        let predicted = predict_eviction(&residents, ContainerState::Destroyed);

        // Drive the same residents through the real typestate destroy.
        let c = Container::declare().create(Options::DEFAULT);
        let crate::dprc::ResidentStep::Placed(c) = c.create_resident(ResidentId::new(1)) else {
            panic!("resident 1 placed");
        };
        let crate::dprc::ResidentStep::Placed(c) = c.assign_in(ResidentId::new(2)) else {
            panic!("resident 2 assigned in");
        };
        let mut parent = Parent::new();
        let Teardown::Done(done) = c.destroy(&mut parent) else {
            panic!("destroy should complete");
        };
        assert_eq!(done.phase(), predicted.final_state);
        assert_eq!(done.residents().len(), predicted.container_residents.len());
        // Both agree the assigned-in resident is re-parented unplugged, the created one not.
        assert!(parent.get(ResidentId::new(2)).is_some());
        assert!(parent.get(ResidentId::new(1)).is_none());
        assert_eq!(
            parent.get(ResidentId::new(2)).map(|r| r.plugged),
            predicted.parent_gained.get(&oref(2)).map(|r| r.plugged)
        );
    }

    #[test]
    fn unobservable_origin_is_kept_in_parent_gained_conservatively() {
        // review M2, PASS3-F2: a None-origin resident is conservatively kept in the
        // parent's gained set (never released as if known created-in), so the render
        // cannot lie that the parent gains nothing; only a known created-in is dropped.
        let residents = BTreeMap::from([
            (
                oref(1),
                ObservedResident {
                    origin: None,
                    plugged: true,
                },
            ),
            (oref(2), res(ResidentKind::CreatedIn, false)),
        ]);
        let predicted = predict_eviction(&residents, ContainerState::Destroyed);
        assert!(
            predicted.parent_gained.contains_key(&oref(1)),
            "unknown origin is kept"
        );
        assert!(
            !predicted.parent_gained[&oref(1)].plugged,
            "the evictee is unplugged"
        );
        assert!(
            !predicted.parent_gained.contains_key(&oref(2)),
            "a known created-in resident is released, not gained"
        );
    }

    // ---- Requirement: Mutation visibility only by re-observation ----

    #[test]
    fn scenario_converged_verdict_comes_from_a_fresh_observation() {
        // "Converged verdict after child mutation": the verdict is judged from a
        // re-observed container, never a sync assumption.
        let desired = consumer_dprc();
        let fresh = ObservedContainer {
            state: ContainerState::Created,
            options: options_from_permissions(&crate::compiled::dprc_default_options()),
            label: desired.label().clone(),
            placement: desired.container().clone(),
            residents: BTreeMap::new(),
        };
        assert_eq!(verdict(&desired, Some(&fresh)), ContainerVerdict::Converged);

        // A drifted label is caught only because we re-observed it.
        let drifted = ObservedContainer {
            label: "stale".into(),
            ..fresh
        };
        assert_eq!(
            verdict(&desired, Some(&drifted)),
            ContainerVerdict::Diverged(vec![Divergence::LabelDrift])
        );
    }

    #[test]
    fn absent_container_is_missing_not_converged() {
        assert_eq!(
            verdict(&consumer_dprc(), None),
            ContainerVerdict::Diverged(vec![Divergence::Missing])
        );
    }

    // ---- Requirement: Consumer convergence is container-only ----

    #[test]
    fn scenario_consumer_on_empty_board_creates_only_the_container() {
        // "Consumer declared on an empty board": exactly the child DPRC, derived
        // options/label/placement, no companion-population steps.
        let desired = consumer_dprc();
        let plan = plan_consumer_container(&desired, None);
        assert_eq!(
            plan.steps,
            vec![ContainerStep::CreateContainer {
                label: desired.label().clone(),
                options: Options::DEFAULT,
                placement: Placement::Root,
            }]
        );
        // No resident/companion steps.
        assert!(
            plan.steps
                .iter()
                .all(|s| matches!(s, ContainerStep::CreateContainer { .. }))
        );
    }

    fn plan_with(object: PlannedObject) -> CompiledPlan {
        let mut plan = CompiledPlan::default();
        plan.order.push(object.key().clone());
        plan.objects.insert(object);
        plan
    }

    fn observed(object: &PlannedObject) -> ObservedContainer {
        ObservedContainer {
            state: ContainerState::Created,
            options: Options::DEFAULT,
            label: object.label().clone(),
            placement: object.container().clone(),
            residents: BTreeMap::new(),
        }
    }

    #[test]
    fn convergence_creates_on_empty_and_is_idempotent_when_present() {
        // The whole-plan seam the shell drives: an empty board plans exactly one
        // CreateContainer per consumer, and a re-observation with the container present
        // plans zero steps and a Converged verdict (DPRC-I6; reconciler delta).
        let object = consumer_dprc();
        let plan = plan_with(object.clone());

        let empty = plan_consumer_convergence(&plan, &BTreeMap::new());
        assert_eq!(empty.len(), 1);
        assert_eq!(
            empty[0].plan.steps,
            vec![ContainerStep::CreateContainer {
                label: object.label().clone(),
                options: Options::DEFAULT,
                placement: Placement::Root,
            }]
        );
        assert_eq!(
            empty[0].verdict,
            ContainerVerdict::Diverged(vec![Divergence::Missing])
        );
        assert_eq!(empty[0].container.provenance, object.provenance().clone());

        let present = BTreeMap::from([(DprcId::new(2), observed(&object))]);
        let converged = plan_consumer_convergence(&plan, &present);
        assert_eq!(converged.len(), 1);
        assert!(converged[0].plan.is_converged());
        assert_eq!(converged[0].verdict, ContainerVerdict::Converged);
    }

    #[test]
    fn present_converged_container_yields_no_steps() {
        let desired = consumer_dprc();
        let observed = ObservedContainer {
            state: ContainerState::Created,
            options: Options::DEFAULT,
            label: desired.label().clone(),
            placement: desired.container().clone(),
            residents: BTreeMap::new(),
        };
        assert!(plan_consumer_container(&desired, Some(&observed)).is_converged());
    }

    #[test]
    fn container_plan_headline_is_the_max_step_class() {
        assert_eq!(ContainerPlan::new().headline(), Class::Hitless);
        let plan = plan_consumer_container(&consumer_dprc(), None);
        assert_eq!(plan.headline(), Class::Disruptive);
    }

    // ---- Requirement: Undeclared consumer containers are pruned under the double gate ----

    fn declared_consumers() -> BTreeMap<TenantName, ConsumerContainer> {
        // The single declared consumer ("vpp") every prune fixture classifies against.
        derive_consumer_containers(&plan_with(consumer_dprc()))
    }

    fn orphan(
        label: impl Into<ConstructName>,
        options: Options,
        placement: Placement,
    ) -> ObservedContainer {
        // Two residents (created + assigned-in) so a teardown predicts a real post-state.
        let mut residents = BTreeMap::new();
        residents.insert(oref(1), res(ResidentKind::CreatedIn, false));
        residents.insert(oref(2), res(ResidentKind::AssignedIn, false));
        ObservedContainer {
            state: ContainerState::Populated,
            options,
            label: label.into(),
            placement,
            residents,
        }
    }

    #[test]
    fn scenario_declared_consumer_container_is_converged_no_plan() {
        // Declared-label match is Converged even under a drifted mask (`dprc.qnt`
        // `pruneBucket` arm order wins over the fingerprint).
        let declared = declared_consumers();
        let drifted = orphan(
            "vpp",
            Options {
                spawn: false,
                ..Options::DEFAULT
            },
            Placement::Root,
        );
        assert_eq!(
            classify_container(&drifted, &declared).bucket,
            PruneBucket::Converged
        );
        let items = plan_prune(&BTreeMap::from([(DprcId::new(2), drifted)]), &declared);
        assert!(items[&DprcId::new(2)].plan.is_none());
    }

    #[test]
    fn scenario_full_fingerprint_orphan_is_a_prune_candidate() {
        // Foreign label + default mask + root => PruneCandidateFull, its plan carrying the
        // eviction-law predicted post-state (ADR-0007 §3): created released, assigned-in
        // re-parented unplugged.
        let declared = declared_consumers();
        let orphan = orphan("foreign", Options::DEFAULT, Placement::Root);
        let c = classify_container(&orphan, &declared);
        assert_eq!(c.bucket, PruneBucket::PruneCandidateFull);
        assert_eq!(
            c.matched,
            BTreeSet::from([
                FingerprintField::Label,
                FingerprintField::OptionsMask,
                FingerprintField::Placement,
            ])
        );
        assert!(c.unmatched.is_empty());

        let items = plan_prune(&BTreeMap::from([(DprcId::new(2), orphan)]), &declared);
        let plan = items[&DprcId::new(2)]
            .plan
            .as_ref()
            .expect("a candidate carries a plan");
        let predicted = plan
            .predicted
            .as_ref()
            .expect("a teardown predicts its post-state");
        assert_eq!(predicted.final_state, ContainerState::Destroyed);
        assert!(!predicted.parent_gained.contains_key(&oref(1)));
        assert!(!predicted.parent_gained[&oref(2)].plugged);
    }

    #[test]
    fn scenario_partial_fingerprint_names_matched_and_unmatched() {
        // Non-default mask (+ non-empty label + root) => PruneCandidatePartial: OptionsMask
        // unmatched, Label and Placement matched (the dry-run render vocabulary).
        let declared = declared_consumers();
        let orphan = orphan(
            "foreign",
            Options {
                spawn: false,
                ..Options::DEFAULT
            },
            Placement::Root,
        );
        let c = classify_container(&orphan, &declared);
        assert_eq!(c.bucket, PruneBucket::PruneCandidatePartial);
        assert!(c.unmatched.contains(&FingerprintField::OptionsMask));
        assert!(c.matched.contains(&FingerprintField::Label));
        assert!(c.matched.contains(&FingerprintField::Placement));

        let items = plan_prune(&BTreeMap::from([(DprcId::new(2), orphan)]), &declared);
        assert!(items[&DprcId::new(2)].plan.is_some());
    }

    #[test]
    fn scenario_empty_label_is_report_only_the_label_void_escape() {
        // `labelVoidEscapeTest` twin (DPRC-I12): a would-be full candidate drops to
        // ReportOnly with plan None when its label is voided (ADR-0001 §4 fence).
        let declared = declared_consumers();
        let voided = orphan("", Options::DEFAULT, Placement::Root);
        let c = classify_container(&voided, &declared);
        assert_eq!(c.bucket, PruneBucket::ReportOnly);
        assert!(c.unmatched.contains(&FingerprintField::Label));

        let items = plan_prune(&BTreeMap::from([(DprcId::new(2), voided)]), &declared);
        assert!(items[&DprcId::new(2)].plan.is_none());
    }

    #[test]
    fn scenario_relabel_remedy_re_enters_the_candidate_buckets() {
        // Re-label remedy (design D8, dprc-encapsulation): a non-empty label restored
        // re-enters the fingerprint buckets — a full candidate again.
        let declared = declared_consumers();
        let relabeled = orphan("foreign", Options::DEFAULT, Placement::Root);
        assert_eq!(
            classify_container(&relabeled, &declared).bucket,
            PruneBucket::PruneCandidateFull
        );
    }

    #[test]
    fn scenario_empty_label_beats_a_matching_fingerprint() {
        // Quint arm order: an empty label is ReportOnly even when mask and placement match —
        // the fence outranks the full fingerprint (ADR-0001 §4).
        let declared = declared_consumers();
        let voided = orphan("", Options::DEFAULT, Placement::Root);
        let c = classify_container(&voided, &declared);
        assert_eq!(c.bucket, PruneBucket::ReportOnly);
        assert_ne!(c.bucket, PruneBucket::PruneCandidateFull);
        assert!(c.matched.contains(&FingerprintField::OptionsMask));
        assert!(c.matched.contains(&FingerprintField::Placement));
        assert!(c.unmatched.contains(&FingerprintField::Label));
    }

    #[test]
    fn prune_buckets_match_the_enum_and_the_model() {
        // dprc.qnt `type PruneBucket`: the exact four cases, in order.
        let sample = [
            PruneBucket::Converged,
            PruneBucket::PruneCandidateFull,
            PruneBucket::PruneCandidatePartial,
            PruneBucket::ReportOnly,
        ];
        for b in sample {
            assert!(PRUNE_BUCKETS.contains(&b.name()), "{}", b.name());
        }
        assert_eq!(PRUNE_BUCKETS.len(), sample.len());
        let mut seen = PRUNE_BUCKETS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), PRUNE_BUCKETS.len(), "duplicate bucket name");
    }
}
