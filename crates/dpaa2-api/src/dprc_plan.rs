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
use crate::dprc::{ContainerState, Options, Refusal, Resident, ResidentId, ResidentKind};
use crate::family::Permission;
use crate::plan::Class;
use crate::types::{ConstructName, TenantName};

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
    /// (`dprc.qnt` `unplugResidentAt`, F-ebusy).
    UnplugResident {
        /// The resident to unplug.
        id: ResidentId,
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
    /// restool rejected the operation with its own client-side guard, before the MC
    /// saw it (design D4). A distinct attribution: it is not one of the three MC
    /// statuses. The southbound adapter assigns this (`dpaa2-mc`); the core carries it.
    RestoolClientGuard {
        /// The restool guard message, verbatim for the operator.
        detail: String,
    },
}

/// Attributes an MC-status refusal (`dprc.qnt` `type Refusal`) to a typed cause, given
/// the container's option mask (design D4).
///
/// The discrimination the reconciler must not lose: 0x8 (`AllocViolation`) is a
/// [`Attribution::PermissionGap`] when the mask lacks `ALLOC_ALLOWED`, and only a
/// [`Attribution::PoolExhaustion`] when the bit is present. 0x6 is always a spawn
/// permission gap; 0x4 (`TopologyLockGate`) is read as a topology permission gap when
/// the mask lacks the bit, else a [`Attribution::LockGate`] (a plugged-move is
/// attributed by the plan that refuses it, [`plan_move_out`], not by status alone).
#[must_use]
pub fn attribute_mc(refusal: Refusal, options: Options) -> Attribution {
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
            if options.topology_changes {
                Attribution::LockGate
            } else {
                Attribution::PermissionGap {
                    bit: OptionBit::TopologyChanges,
                }
            }
        }
    }
}

/// The predicted post-state of a teardown, computed before any MC command (design D2;
/// the reconciler predicts, it does not discover).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PredictedPostState {
    /// The container's phase after the teardown.
    pub final_state: ContainerState,
    /// The residents the container still holds afterwards (empty after a destroy).
    pub container_residents: BTreeMap<ResidentId, Resident>,
    /// The residents the parent gains — the assigned-in evictees, unplugged.
    pub parent_gained: BTreeMap<ResidentId, Resident>,
}

/// Predicts the eviction law over `residents` for a teardown reaching `final_state`
/// (ADR-0007 §3; `dprc.qnt` `evictInto`, the twin of [`crate::dprc`]'s private
/// `evict_into`, F-evict): [`ResidentKind::CreatedIn`] residents are released with the
/// container (absent afterwards), [`ResidentKind::AssignedIn`] residents move one hop up
/// into the parent, **unplugged** — the plug bit cleared so the parent never tracks a
/// resident as bound that VFIO does not hold. DPRC-I1: a created resident never crosses
/// the boundary.
#[must_use]
pub fn predict_eviction(
    residents: &BTreeMap<ResidentId, Resident>,
    final_state: ContainerState,
) -> PredictedPostState {
    let parent_gained = residents
        .iter()
        .filter(|(_, r)| r.kind == ResidentKind::AssignedIn)
        .map(|(id, r)| {
            (
                *id,
                Resident {
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

/// Plans a move-out step for a resident (`dprc.qnt` `moveResidentOutAt`). The
/// plugged-move precondition (DPRC-I3): a plugged resident is refused
/// [`Attribution::PluggedMove`] and no move is emitted; an unplugged resident yields the
/// [`ContainerStep::MoveResidentOut`]. A locked container is [`Attribution::LockGate`].
pub fn plan_move_out(state: ContainerState, resident: &Resident, id: ResidentId) -> PlanOutcome {
    if state == ContainerState::Locked {
        return PlanOutcome::Refused(Attribution::LockGate);
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
/// rather than discovering it, and re-observation (DPRC-I6) later confirms it.
#[must_use]
pub fn plan_teardown(observed: &ObservedContainer) -> ContainerPlan {
    let mut plan = ContainerPlan::new();
    for (id, resident) in &observed.residents {
        if resident.plugged {
            plan.steps.push(ContainerStep::UnplugResident { id: *id });
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
    /// The residents observed in the container, keyed by id.
    pub residents: BTreeMap<ResidentId, Resident>,
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

#[cfg(test)]
mod tests {
    //! The reconciler-delta scenarios (`specs/reconciler/spec.md`) covered as pure unit
    //! tests, plus the parity of [`predict_eviction`] with the task-2.1 typestate.

    use super::*;
    use crate::dprc::{Container, Parent, Teardown, VfioBind};
    use crate::intent::{Dataplane, Isolation, Tenant};

    fn res(kind: ResidentKind, plugged: bool) -> Resident {
        Resident { kind, plugged }
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
        let plugged = res(ResidentKind::AssignedIn, true);
        let out = plan_move_out(ContainerState::Populated, &plugged, ResidentId::new(1));
        assert_eq!(out, PlanOutcome::Refused(Attribution::PluggedMove));

        // An unplugged resident does yield the move step.
        let unplugged = res(ResidentKind::AssignedIn, false);
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
        let attr = attribute_mc(Refusal::AllocViolation, no_alloc);
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
            attribute_mc(Refusal::AllocViolation, Options::DEFAULT),
            Attribution::PoolExhaustion
        );
    }

    #[test]
    fn the_three_statuses_map_to_distinct_attributions() {
        // 0x6/0x8/0x4 are never collapsed to one shape (design D4).
        let no_topo = Options::DEFAULT; // topology_changes is false by default (DPRC-I4)
        assert_eq!(
            attribute_mc(Refusal::SpawnViolation, Options::DEFAULT),
            Attribution::PermissionGap {
                bit: OptionBit::Spawn
            }
        );
        assert_eq!(
            attribute_mc(Refusal::TopologyLockGate, no_topo),
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
                }
            ),
            Attribution::LockGate
        );
    }

    // ---- Requirement: Destroy planning encodes the eviction law ----

    #[test]
    fn scenario_non_empty_teardown_predicts_post_state() {
        // "Non-empty scratch container teardown": created resident absent, assigned-in
        // present unplugged in the parent (ADR-0007 §3).
        let mut residents = BTreeMap::new();
        residents.insert(ResidentId::new(1), res(ResidentKind::CreatedIn, false));
        residents.insert(ResidentId::new(2), res(ResidentKind::AssignedIn, false));
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
        assert!(!predicted.parent_gained.contains_key(&ResidentId::new(1)));
        let evictee = &predicted.parent_gained[&ResidentId::new(2)];
        assert!(!evictee.plugged);
        assert!(predicted.container_residents.is_empty());
        assert_eq!(plan.steps, vec![ContainerStep::Destroy]);
    }

    #[test]
    fn teardown_gates_destroy_on_all_residents_unplugged() {
        // F-ebusy: a plugged resident is unplugged first, then destroy — never a destroy
        // against a plugged resident (MC -EBUSY).
        let mut residents = BTreeMap::new();
        residents.insert(ResidentId::new(1), res(ResidentKind::CreatedIn, true));
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
                ContainerStep::UnplugResident {
                    id: ResidentId::new(1)
                },
                ContainerStep::Destroy,
            ]
        );
    }

    #[test]
    fn predict_eviction_matches_the_task_2_1_typestate() {
        // Parity: the plan-layer prediction equals what the 2.1 typestate's destroy does
        // (the canonical evict_into), binding the duplicated law to its owner (F-evict).
        let mut residents = BTreeMap::new();
        residents.insert(ResidentId::new(1), res(ResidentKind::CreatedIn, false));
        residents.insert(ResidentId::new(2), res(ResidentKind::AssignedIn, false));
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
            predicted
                .parent_gained
                .get(&ResidentId::new(2))
                .map(|r| r.plugged)
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
}
