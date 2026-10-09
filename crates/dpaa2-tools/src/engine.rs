//! The imperative shell: observe → reconcile → act → wait → re-observe (design D0; add-dpaa2-provisioning).
//!
//! This is the "imperative shell" wrapped around the pure core. It is generic over
//! the [`McControl`]/[`KernelControl`] trait seams so the whole convergence loop runs
//! against the in-memory fake with no board (design D10; restool-baseline). Actuation resolves the
//! DPNI index for a freshly-created port from the id the MC assigned this pass, and
//! for existing ports from the observed connection edge (design D1; restool-baseline).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::thread::sleep;
use std::time::{Duration, Instant};

use dpaa2_api::contract::{KernelControl, McControl};
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::inventory::{Ceiling, Inventory};
use dpaa2_api::core::model::{
    DesiredTopology, DpmacId, DpniId, DprcId, ObjectRef, ObservedTopology,
};
use dpaa2_api::core::types::{ConstructName, TenantName};
use dpaa2_api::families::dpio::derived_seats;
use dpaa2_api::families::dprc::Options;
use dpaa2_api::families::pool_lifecycle::{
    CustodyScope, ObservedPoolObject, PoolDeltas, PoolFamily, PoolMembership, RawLabel,
    ShrinkBelowDraw, census_of, derived_requirement, drift_disposition, label_membership,
};
use dpaa2_api::intent::KERNEL;
use dpaa2_api::intent::compiled::{
    Attributes, CompiledPlan, Container, ObjectKey, PlannedObject, ProvenanceKey,
};
use dpaa2_api::plan::connect::{
    CONNECT_ANCESTOR, ChildDeferredVisibility, LinkEndState, WireEnd, WireHeldRefusal, WirePlan,
    WireTransition, discharge_child, plan_wire,
};
use dpaa2_api::plan::dprc::{
    Attribution, ConsumerConvergence, ContainerPlan, ContainerStep, ContainerVerdict, PruneBucket,
    PruneItem, Verb, attribute_refusal, derive_consumer_containers, plan_consumer_convergence,
    plan_prune, verdict,
};
use dpaa2_api::plan::populate::ChildPlan;
use dpaa2_api::plan::reconcile::{ReconcileOptions, reconcile_with};
use dpaa2_api::plan::{Class, Plan, RebuildRefusal, Transition};
use dpaa2_mc::{
    default_dpio_cfg, dispatch_child_population, dispatch_pool_deltas, plan_child_population,
    vfio_handoff,
};

/// The root pool drift value types now live in [`dpaa2_api::plan::pool`]; re-exported so the
/// `dpaa2ctl` frontend keeps addressing them through `engine::` (bead H part 1).
pub use dpaa2_api::plan::pool::{PoolDrift, PoolFamilyDrift, PoolPass};

/// Policy for a convergence run.
#[derive(Clone, Copy, Debug)]
pub struct ConvergeConfig {
    /// Overall wall-clock budget before giving up.
    pub deadline: Duration,
    /// Delay between re-observation passes (accounts for async netdev appearance).
    pub poll_interval: Duration,
    /// Whether to tear down declared-absent ports and undeclared containers.
    /// dprc-hardening design D8 widened the intent-layer prune (D7) to
    /// containers behind `--allow disruptive` (PASS4-F5).
    pub prune: bool,
    /// The maximum disruption class the run may actuate (ADR-0015 decision 12). A
    /// plan whose headline exceeds this is refused, not applied — the default allows
    /// [`Class::Hitless`] only, and [`Class::Disruptive`] is never implied.
    pub allow: Class,
}

impl Default for ConvergeConfig {
    fn default() -> Self {
        Self {
            deadline: Duration::from_secs(30),
            poll_interval: Duration::from_millis(250),
            prune: false,
            allow: Class::Hitless,
        }
    }
}

/// The result of a convergence run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The system reached the desired state.
    Converged,
    /// The deadline elapsed with these DPMAC anchors still unconverged.
    DeadlineExceeded {
        /// Anchors whose ports had not converged when the deadline hit.
        unconverged: Vec<DpmacId>,
    },
    /// The plan's headline disruption class exceeded the run's `--allow` gate
    /// (ADR-0015 decision 12); nothing was actuated.
    DisruptionRefused {
        /// The headline class the plan would have actuated.
        headline: Class,
        /// The maximum class the run allowed.
        allowed: Class,
    },
    /// A same-run rebuild was refused so the converge loop cannot churn a just-created
    /// port dpni into the phylink crash — the loop-breaker exit (pool-objects design D12;
    /// ADR-0008 §9). Nothing was actuated for the refused ports; each refusal's field diff
    /// pins the mispredicted projection field.
    RebuildRefused {
        /// The refused same-run rebuilds, each naming its port, dpni, and field diff.
        refusals: Vec<RebuildRefusal>,
    },
}

/// The outcome of a child-DPRC (consumer container) convergence pass — the container
/// analog of [`Outcome`] (design D2; ADR-0002; reconciler delta). Kept distinct because a
/// container refusal is a typed [`Attribution`] (design D4; ADR-0003), not a DPMAC-anchored port
/// deadline: the two families do not share a failure vocabulary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContainerOutcome {
    /// Every declared consumer's container matches its derived intent by re-observation.
    Converged,
    /// A container plan's headline exceeded the run's `--allow` gate (ADR-0015
    /// decision 12); nothing was actuated.
    DisruptionRefused {
        /// The headline class the container plan would have actuated.
        headline: Class,
        /// The maximum class the run allowed.
        allowed: Class,
    },
    /// A dispatched container verb was refused; the typed cause is attributed
    /// (design D4; ADR-0003) and the refusal shapes (0x6/0x8/0x4, or a restool client guard) stay
    /// discriminated — never collapsed into one denial.
    Refused {
        /// The container whose create was refused.
        label: ConstructName,
        /// The discriminated cause the reconciler reports.
        attribution: Attribution,
    },
}

/// The outcome of the undeclared-consumer prune pass (reconciler spec "Undeclared
/// consumer containers are pruned under the double gate"). Every non-`Clean` variant
/// carries the [`PruneItem`] map so the shell renders each container's bucket,
/// fingerprint fields and predicted post-state before reporting the outcome. The
/// `Converged` bucket (a declared consumer) is convergence's job, so it is filtered out
/// of the carried map — only prune candidates and the report-only fence appear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PruneOutcome {
    /// Nothing to prune or report — no undeclared child container under the root.
    Clean,
    /// Candidates and/or report-only containers exist but nothing was dispatched:
    /// either `--prune` was withheld, or the only undeclared containers are the
    /// report-only fence (ADR-0001 §4). Items are carried for rendering.
    ReportOnly {
        /// Every undeclared container, keyed by re-observation handle.
        items: BTreeMap<DprcId, PruneItem>,
    },
    /// `--prune` was given but the run's `--allow` gate sits below the teardown's
    /// disruptive headline (ADR-0015 decision 12); nothing was dispatched.
    DisruptionRefused {
        /// The headline the candidate teardowns would actuate.
        headline: Class,
        /// The maximum class the run allowed.
        allowed: Class,
        /// Every undeclared container, keyed by re-observation handle.
        items: BTreeMap<DprcId, PruneItem>,
    },
    /// Both gates held: every candidate was torn down and its absence confirmed by
    /// re-observation (DPRC-I6).
    Pruned {
        /// Every undeclared container, keyed by re-observation handle.
        items: BTreeMap<DprcId, PruneItem>,
    },
    /// Both gates held but a candidate could not be torn down: a locked container (its
    /// destroy is lock-stripped) or a still-plugged resident (`-EBUSY`, MC `0x10`). The
    /// refusal is typed, never a pass abort, and the survivor is named
    /// (review M1; `docs/baseline/dprc.md` DPRC-I11 lock face / DPRC-I2).
    Refused {
        /// The container whose teardown was refused (it survives the prune).
        id: DprcId,
        /// The discriminated cause the reconciler reports.
        attribution: Attribution,
    },
}

/// Pairs each declared child-DPRC object with its convergence by position, guarding both
/// list lengths and per-pair label identity. The two lists filter `plan.objects`
/// independently, so a count drift means an undispatched declared container the bare
/// `zip` would silently truncate — a loud [`Error::Backend`], never a container that
/// passes as Converged (synthesis row 9; dpni-typestate design D5; ADR-0015).
fn pair_containers<'a>(
    desired: &[&'a PlannedObject],
    convergences: &'a [ConsumerConvergence],
) -> Result<Vec<(&'a PlannedObject, &'a ConsumerConvergence)>, Error> {
    if desired.len() != convergences.len() {
        return Err(Error::Backend(format!(
            "container convergence count {} does not match {} declared containers: \
             an undispatched declared container",
            convergences.len(),
            desired.len()
        )));
    }
    desired
        .iter()
        .copied()
        .zip(convergences)
        .map(|(object, c)| {
            if *object.label() != c.container.label {
                return Err(Error::Backend(format!(
                    "container pairing misaligned: object `{}` vs convergence `{}`",
                    object.label(),
                    c.container.label
                )));
            }
            Ok((object, c))
        })
        .collect()
}

/// Reconciles every declared consumer's child container toward the compiled intent
/// (design D2; ADR-0002; reconciler delta "Consumer convergence is container-only").
///
/// The container half of the product pipeline: it re-observes the board's containers
/// (DPRC-I6 — a fresh MC query, never `sync`), plans container-only via
/// [`plan_consumer_convergence`] (the child DPRC alone, so no companion/dpni step is
/// representable — bead cd3.8), gates the headline against `cfg.allow`, dispatches each
/// step to the task-3.1 verbs, and judges convergence by re-observing each dispatched
/// candidate alone (dpni-typestate design D5), never a full root rescan. A
/// second run against the post-converged state plans zero steps and returns
/// [`ContainerOutcome::Converged`] without dispatching. A typed shim refusal
/// (`Error::McStatus`/`Error::RestoolGuard`) becomes a discriminated
/// [`ContainerOutcome::Refused`]; any other error propagates.
///
/// # Errors
/// Propagates a backend read/dispatch error that is not a typed container refusal, and
/// reports a post-dispatch divergence (a container that failed to converge) as an error.
pub fn converge_containers<M: McControl>(
    plan: &CompiledPlan,
    mc: &M,
    cfg: ConvergeConfig,
) -> Result<ContainerOutcome, Error> {
    // Read: re-observe the root's child containers (DPRC-I6).
    let observed = mc.observe_containers()?;
    let convergences = plan_consumer_convergence(plan, &observed);
    if convergences.iter().all(|c| c.plan.is_converged()) {
        return Ok(ContainerOutcome::Converged);
    }

    // Gate on the combined headline before touching the board (ADR-0015 decision 12):
    // creating a container is disruptive, so a hitless-only run refuses it, unchanged.
    let headline = convergences
        .iter()
        .map(|c| c.plan.headline())
        .max()
        .unwrap_or(Class::Hitless);
    if headline > cfg.allow {
        tracing::error!(%headline, allowed = %cfg.allow, "container plan exceeds allowed disruption class");
        return Ok(ContainerOutcome::DisruptionRefused {
            headline,
            allowed: cfg.allow,
        });
    }

    // The declared child-DPRC objects, carried so each dispatched candidate keeps its `PlannedObject` for the per-candidate verdict (dpni-typestate design D5).
    let desired: Vec<&PlannedObject> = plan
        .objects
        .iter()
        .filter(|o| matches!(o.attributes(), Attributes::Dprc { .. }))
        .collect();

    let mut touched: Vec<(&PlannedObject, DprcId)> = Vec::new();
    for (object, c) in pair_containers(&desired, &convergences)? {
        for step in &c.plan.steps {
            match dispatch_container_step(step, mc) {
                Ok(Some(id)) => touched.push((object, id)),
                Ok(None) => {}
                Err(e) => {
                    return match attribute_refusal(&e, c.container.options, Verb::SpawnChild) {
                        Some(attribution) => Ok(ContainerOutcome::Refused {
                            label: c.container.label.clone(),
                            attribution,
                        }),
                        None => Err(e),
                    };
                }
            }
        }
    }

    // Verdict by per-candidate re-observation (dpni-typestate design D5; DPRC-I6): re-query exactly each touched container and judge it core-side, never a full root rescan.
    for (object, id) in touched {
        let observed = mc.observe_container(id)?;
        if let ContainerVerdict::Diverged(reasons) = verdict(object, observed.as_ref()) {
            return Err(Error::Backend(format!(
                "container `{}` did not converge after dispatch: {reasons:?}",
                object.label()
            )));
        }
    }
    Ok(ContainerOutcome::Converged)
}

/// Plans (without dispatching) container-only convergence for every declared consumer,
/// re-observing the board — the read seam `dry-run` renders (design D2/D6; ADR-0002, ADR-0004). Each
/// [`ConsumerConvergence`] carries the derived container's provenance key, which the
/// renderer resolves to the baseline anchor in the plan's DAG.
///
/// # Errors
/// Propagates the backend read failure.
pub fn plan_containers<M: McControl>(
    plan: &CompiledPlan,
    mc: &M,
) -> Result<Vec<ConsumerConvergence>, Error> {
    let observed = mc.observe_containers()?;
    Ok(plan_consumer_convergence(plan, &observed))
}

/// Classifies every observed root child container against the declared consumers and
/// returns the prune-relevant items — the [`plan_prune`] map minus the `Converged`
/// bucket, whose declared containers are [`converge_containers`]'s job (reconciler spec;
/// dprc-encapsulation task 4.3). The read seam `dry-run` renders read-only.
///
/// # Errors
/// Propagates the backend read failure.
pub fn plan_prune_report<M: McControl>(
    plan: &CompiledPlan,
    mc: &M,
) -> Result<BTreeMap<DprcId, PruneItem>, Error> {
    let observed = mc.observe_containers()?;
    let declared = derive_consumer_containers(plan);
    Ok(plan_prune(&observed, &declared)
        .into_iter()
        .filter(|(_, item)| item.classification.bucket != PruneBucket::Converged)
        .collect())
}

/// Prunes undeclared consumer containers under the double gate (dprc-encapsulation task 4.3).
/// Reconciler spec "Undeclared consumer containers are pruned under the double gate":
/// re-observes the root's children (DPRC-I6), classifies each against the
/// declared set via [`plan_prune_report`], and dispatches a candidate's eviction-law
/// teardown (ADR-0007 §3) only when `cfg.prune` AND `cfg.allow` reaches the disruptive
/// headline — the two gates. A report-only container is never dispatched regardless of
/// flags (ADR-0001 §4). The verdict comes from a second re-observation (DPRC-I6): a
/// pruned id that survives is an error, never assumed gone.
///
/// Before a candidate's residents are unplugged and destroyed, the container is released from
/// its consumer bindings (pool-objects design D11 teardown walk): its VFIO handoff is undone
/// ([`KernelControl::vfio_unbind`]) so restool can reach its residents, and each child dpni is
/// disconnected from its root peer (the DPNI-I9 disconnect from the common ancestor) so no
/// dangling endpoint survives the destroy — consumers before pools.
///
/// # Errors
/// Propagates a backend/kernel read/dispatch error, and reports a survivor (a pruned container
/// still observed) as an [`Error::Backend`].
pub fn prune_containers<M: McControl, K: KernelControl>(
    plan: &CompiledPlan,
    mc: &M,
    kernel: &K,
    cfg: ConvergeConfig,
) -> Result<PruneOutcome, Error> {
    let items = plan_prune_report(plan, mc)?;
    if items.is_empty() {
        return Ok(PruneOutcome::Clean);
    }

    // Report-only fence, or `--prune` withheld: nothing is dispatched (ADR-0001 §4).
    let has_candidates = items.values().any(|i| i.plan.is_some());
    if !has_candidates || !cfg.prune {
        return Ok(PruneOutcome::ReportOnly { items });
    }

    // Second gate (ADR-0015 decision 12): a teardown is disruptive, so a run allowing
    // less refuses it, dispatching nothing.
    let headline = items
        .values()
        .filter_map(|i| i.plan.as_ref())
        .map(ContainerPlan::headline)
        .max()
        .unwrap_or(Class::Hitless);
    if headline > cfg.allow {
        tracing::error!(%headline, allowed = %cfg.allow, "prune plan exceeds allowed disruption class");
        return Ok(PruneOutcome::DisruptionRefused {
            headline,
            allowed: cfg.allow,
            items,
        });
    }

    // A per-candidate refusal is a typed outcome, never a pass abort: record the first survivor, dispatch the rest, re-observe below (review M1; PASS3-F4/F5; DPRC-I6).
    let mut refused: Option<(DprcId, Attribution)> = None;
    let mut dispatched: BTreeSet<DprcId> = BTreeSet::new();
    for (id, item) in &items {
        let Some(candidate) = &item.plan else {
            continue;
        };
        // A locked candidate emitted only a LockGate gap and no step (`docs/baseline/dprc.md` DPRC-I11 lock face).
        if let Some(gap) = candidate.gaps.first() {
            refused.get_or_insert((*id, gap.clone()));
            continue;
        }
        // Release the consumer bindings BEFORE the residents are torn down (pool-objects design D11).
        release_child_bindings(mc, kernel, *id)?;
        match dispatch_candidate_teardown(candidate, *id, mc)? {
            Some(attribution) => {
                refused.get_or_insert((*id, attribution));
            }
            None => {
                dispatched.insert(*id);
            }
        }
    }

    // Verdict by per-candidate re-observation (dpni-typestate design D5; DPRC-I6): each dispatched id must read back absent (`None`) — a survivor is an error, a refused one expected.
    for id in &dispatched {
        if mc.observe_container(*id)?.is_some() {
            return Err(Error::Backend(format!(
                "container {id} survived prune dispatch"
            )));
        }
    }
    if let Some((id, attribution)) = refused {
        return Ok(PruneOutcome::Refused { id, attribution });
    }
    Ok(PruneOutcome::Pruned { items })
}

/// Dispatches every step of one candidate's teardown, returning the typed [`Attribution`]
/// when a step is refused (MC `0x10` -EBUSY plugged resident, or a `0x4` lock strip)
/// rather than aborting the whole prune pass (review M1; PASS3-F5). A non-attributable
/// backend error still propagates.
fn dispatch_candidate_teardown<M: McControl>(
    candidate: &ContainerPlan,
    id: DprcId,
    mc: &M,
) -> Result<Option<Attribution>, Error> {
    for step in &candidate.steps {
        if let Err(e) = dispatch_teardown_step(step, id, mc) {
            return match attribute_refusal(&e, Options::DEFAULT, Verb::DestroyContainer) {
                Some(attribution) => Ok(Some(attribution)),
                None => Err(e),
            };
        }
    }
    Ok(None)
}

/// The outcome of the undeclared root-dpni prune pass (pool-objects design D10/D11): the root
/// analog of [`PruneOutcome`] for kernel-interface dpnis. A root dpni is a consumer (it draws
/// the pool), so it is torn down BEFORE the shrink pass reclaims the pool it drew.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootDpniPruneOutcome {
    /// No undeclared managed-labelled root dpni — nothing to prune or report.
    Clean,
    /// Candidates exist but nothing was dispatched: `--prune` was withheld, or the run's gate
    /// sits below the disruptive teardown headline. Carried so the shell reports them.
    ReportOnly {
        /// The candidate root dpnis, by id.
        candidates: Vec<DpniId>,
    },
    /// `--prune` was given but `cfg.allow` sits below the disruptive teardown headline
    /// (ADR-0015 decision 12); nothing was dispatched.
    DisruptionRefused {
        /// The disruptive headline the teardowns would actuate.
        headline: Class,
        /// The maximum class the run allowed.
        allowed: Class,
        /// The candidate root dpnis, by id.
        candidates: Vec<DpniId>,
    },
    /// Both gates held: every candidate was disconnected and destroyed, its absence confirmed
    /// by re-observation.
    Pruned {
        /// The pruned root dpnis, by id.
        pruned: Vec<DpniId>,
    },
}

/// The undeclared managed-labelled root dpnis the prune targets (the one-label law of
/// pool-objects task 3.10; design D10/D11): a root dpni whose MC label is non-empty (managed-labelled —
/// the tool stamps its dpnis with construct names, ADR-0015) yet names no declared construct is
/// a candidate; an empty label is the DPL/foreign sentinel, structurally exempt (ADR-0001 §4),
/// so the DPL baseline and the management-plane dpnis are never touched. Root dpnis only: a
/// child dpni is a pool row under its own container, not a root topology dpni.
fn root_dpni_candidates(
    observed: &ObservedTopology,
    declared: &BTreeSet<ConstructName>,
) -> Vec<DpniId> {
    observed
        .dpnis
        .iter()
        .filter(|d| match &d.label {
            Some(label) => {
                label_membership(&RawLabel::from(label.as_str()), declared)
                    == PoolMembership::Foreign
            }
            None => false,
        })
        .map(|d| d.id)
        .collect()
}

/// Prunes undeclared managed-labelled root dpnis under the double gate (pool-objects design D10/D11).
/// The requirement: undeclared managed-labelled root dpnis become prune candidates.
/// Re-observes the root topology, classifies each dpni against the declared set by the
/// one-label law (empty-label/DPL exempt), and, only when
/// `cfg.prune` AND `cfg.allow` reaches the disruptive headline, tears each candidate down in the
/// ADR-0008 §8 order: disconnect while bound, then `kernel.unbind`, then `mc.destroy`. The
/// disconnect lets dpaa2-eth re-attach the standalone MAC driver; the unbind must precede the
/// destroy because restool refuses a destroy of a driver-bound dpni client-side. A consumer
/// released before the shrink pass reclaims the pool it drew. The verdict comes from a second
/// re-observation: a pruned dpni that survives is an error.
///
/// # Errors
/// Propagates a backend read/dispatch error, and reports a survivor as an [`Error::Backend`].
pub fn prune_root_dpnis<M: McControl, K: KernelControl>(
    plan: &CompiledPlan,
    mc: &M,
    kernel: &K,
    cfg: ConvergeConfig,
) -> Result<RootDpniPruneOutcome, Error> {
    let declared = root_declared(plan);
    let observed = mc.observe()?;
    let candidates = root_dpni_candidates(&observed, &declared);
    if candidates.is_empty() {
        return Ok(RootDpniPruneOutcome::Clean);
    }

    // First gate: `--prune` withheld ⇒ report only (the tool's most destructive act is opt-in).
    if !cfg.prune {
        return Ok(RootDpniPruneOutcome::ReportOnly { candidates });
    }
    // Second gate (ADR-0015 decision 12): a dpni teardown is disruptive.
    if Class::Disruptive > cfg.allow {
        tracing::error!(allowed = %cfg.allow, "root dpni prune exceeds allowed disruption class");
        return Ok(RootDpniPruneOutcome::DisruptionRefused {
            headline: Class::Disruptive,
            allowed: cfg.allow,
            candidates,
        });
    }

    // ADR-0008 §8 teardown order for each candidate: disconnect while bound, unbind, destroy.
    for dpni in &candidates {
        if observed
            .dpnis
            .iter()
            .any(|d| d.id == *dpni && d.connected_to.is_some())
        {
            mc.dprc_disconnect(CONNECT_ANCESTOR, *dpni)?;
        }
        kernel.unbind(*dpni)?;
        mc.destroy(*dpni)?;
        tracing::info!(%dpni, "pruned undeclared managed-labelled root dpni");
    }

    let after = mc.observe()?;
    if let Some(survivor) = candidates
        .iter()
        .find(|id| after.dpnis.iter().any(|d| d.id == **id))
    {
        return Err(Error::Backend(format!(
            "root dpni {survivor} survived prune dispatch"
        )));
    }
    Ok(RootDpniPruneOutcome::Pruned { pruned: candidates })
}

/// Releases one candidate container's consumer bindings before its residents are torn down
/// (pool-objects design D11 teardown walk): undoes the VFIO handoff so restool can reach the
/// residents, then disconnects each child dpni from its root peer (the DPNI-I9 disconnect from
/// the common ancestor). Consumers before pools — the residents' unplug/destroy follows.
///
/// # Errors
/// Propagates the first backend/kernel error the unbind, child-dpni observation, or a
/// disconnect raises.
fn release_child_bindings<M: McControl, K: KernelControl>(
    mc: &M,
    kernel: &K,
    id: DprcId,
) -> Result<(), Error> {
    kernel.vfio_unbind(id)?;
    for row in mc.observe_pool(Some(id), Family::Dpni)? {
        let dpni = DpniId::new(row.object.ordinal());
        mc.dprc_disconnect(CONNECT_ANCESTOR, dpni)?;
        tracing::info!(%id, %dpni, "disconnected child dpni before container teardown");
    }
    Ok(())
}

/// Dispatches one prune-teardown step against the target container `id`. A [`Destroy`]
/// maps to `dprc_destroy(id)`; an [`UnplugResident`] maps to `assign --plugged=0` on its
/// family-qualified [`ObjectRef`] — the operand the observation now carries, so the F-ebusy
/// unplug is dispatchable rather than a deferred read-back (review M1; PASS3-F3/F14).
///
/// [`Destroy`]: ContainerStep::Destroy
/// [`UnplugResident`]: ContainerStep::UnplugResident
fn dispatch_teardown_step<M: McControl>(
    step: &ContainerStep,
    id: DprcId,
    mc: &M,
) -> Result<(), Error> {
    match step {
        ContainerStep::Destroy => {
            mc.dprc_destroy(id)?;
            tracing::info!(%id, "destroyed undeclared child dprc container");
            Ok(())
        }
        ContainerStep::UnplugResident { object } => {
            mc.dprc_assign(id, *object, None, Some(false))?;
            tracing::info!(%id, %object, "unplugged resident before teardown");
            Ok(())
        }
        other => Err(Error::Backend(format!(
            "prune teardown emits only Destroy/UnplugResident, not {other:?}"
        ))),
    }
}

/// Dispatches one container-only step to its task-3.1 verb, returning the created child's
/// re-observation handle for [`ContainerStep::CreateContainer`] so the caller re-observes
/// exactly it (dpni-typestate design D5). The container-only convergence path emits only
/// that step (existence, options, label, placement); any other step is out of this
/// change's scope (companion sets converge in the pool passes, `converge_pools`; child
/// dpnis in the population pass, `converge_population`) and is an error, not a silent no-op.
fn dispatch_container_step<M: McControl>(
    step: &ContainerStep,
    mc: &M,
) -> Result<Option<DprcId>, Error> {
    match step {
        ContainerStep::CreateContainer {
            label,
            options,
            placement,
        } => {
            let parent = match placement {
                Container::Root => DprcId::ROOT,
                Container::Child(tenant) => {
                    return Err(Error::Backend(format!(
                        "consumer container places in root, not child:{tenant}"
                    )));
                }
            };
            let id = mc.dprc_create(parent, *options, label)?;
            tracing::info!(%id, %label, "created child dprc container");
            Ok(Some(id))
        }
        other => Err(Error::Backend(format!(
            "container-only convergence emits no {other:?} (companion sets ride converge_pools, child dpnis ride converge_population)"
        ))),
    }
}

/// The pool families a root convergence pass visits, in dependency order
/// (dpmcp→dpbp→dpcon; everything draws a dpmcp — pool-objects design D8). dpio is a seat,
/// converged after the trio (pool-objects design D4), so it is not in this array.
const POOL_TRIO: [PoolFamily; 3] = [PoolFamily::Dpmcp, PoolFamily::Dpbp, PoolFamily::Dpcon];

/// The outcome of the root-scope pool convergence pass (pool-objects task 3.4) — the pool
/// analog of [`ContainerOutcome`]. Kept distinct because a pool refusal is a typed
/// [`ShrinkBelowDraw`] (pool-objects design D3), not a container [`Attribution`] nor a DPMAC deadline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PoolOutcome {
    /// Every root pool family meets its derived requirement by post-dispatch read-back, and
    /// the dpio seats equal their derived count (the idempotence witness reproduces it).
    Converged,
    /// The pool deltas' headline exceeded the run's `--allow` gate (ADR-0015 decision 12);
    /// nothing was actuated. A pool create/destroy/prune is [`Class::Disruptive`].
    DisruptionRefused {
        /// The headline the pool convergence would have actuated.
        headline: Class,
        /// The maximum class the run allowed.
        allowed: Class,
    },
    /// A family's derived requirement fell below its drawn count: a free-only shrink cannot
    /// reach a live consumer, so it surfaces to the operator and nothing is torn down
    /// (pool-objects design D3). Never a forced teardown.
    ShrinkRefused {
        /// The typed below-draw refusal, naming the family and the two counts.
        refusal: ShrinkBelowDraw,
    },
}

/// The declared-name recognition set the root census and dispatch judge custody against —
/// every planned object's label (companions wear their consumer's name, ADR-0015). The same
/// label-fingerprint declaredness the inventory and the `plan::dprc` prune use (ADR-0010 §4
/// refined by ADR-0015), so a reconciler-grown companion reads back managed.
fn root_declared(plan: &CompiledPlan) -> BTreeSet<ConstructName> {
    plan.objects.iter().map(|o| o.label().clone()).collect()
}

/// The label a root grow of `family` stamps — the label of a planned root object of that
/// family (companions wear their consumer's name, ADR-0015; at root that is the kernel or a
/// restricted drawer pooling it). Anonymity is the pattern's truth (pool-objects design D2), so the pick
/// is not a policy surface as long as it is a declared name — the census recognizes any
/// declared name as managed. A grow only fires when the requirement is positive, so a
/// planned object always exists then; the [`KERNEL`] fallback covers the grow-free case
/// (destroy/prune ignore the label), root being the kernel's own container.
fn root_family_label(plan: &CompiledPlan, family: Family) -> ConstructName {
    plan.objects
        .iter()
        .find(|o| o.container() == &Container::Root && o.key().family == family)
        .map_or_else(|| ConstructName::from(KERNEL), |o| o.label().clone())
}

/// The runtime ceiling for `family`, threaded the way the fit-check reads it — the
/// inventory's listed ceiling, or [`Ceiling::Unknown`] (admit-and-warn) where the family's
/// ceiling is unlistable (ADR-0011; consistent with the `plan_child_population` gap, bead
/// dpaa2-controlplane-amt).
fn ceiling_of(inventory: &Inventory, family: PoolFamily) -> Ceiling {
    inventory
        .ceilings
        .get(&family.family())
        .cloned()
        .unwrap_or(Ceiling::Unknown)
}

/// Reads the root's pool drift without dispatching — the seam `dry-run` and `status` render
/// (pool-objects task 3.4). For each trio family it censuses the root
/// ([`McControl::observe_pool`] with `None` for the shim root), folds with [`census_of`]
/// against the plan's declared set, and takes the count-level [`drift_disposition`] against
/// [`derived_requirement`] and the inventory ceiling; it also reads the dpio seat count
/// versus [`derived_seats`]. Pure of any mutation: the adapter reads, the core judges.
///
/// # Errors
/// Propagates a backend read failure.
pub fn plan_pools<M: McControl>(plan: &CompiledPlan, mc: &M) -> Result<PoolDrift, Error> {
    let declared = root_declared(plan);
    let inventory = mc.read_inventory()?;
    let mut families = Vec::with_capacity(POOL_TRIO.len());
    for family in POOL_TRIO {
        let rows = mc.observe_pool(None, family.family())?;
        let census = census_of(&rows, &declared);
        let required = derived_requirement(plan, &Container::Root, family);
        // Root is grow-only: no managed-surplus destroy, prune only never-plugged, no refusal (ADR-0020).
        let disposition = drift_disposition(
            family,
            census,
            required,
            &ceiling_of(&inventory, family),
            CustodyScope::RootScope,
        );
        families.push(PoolFamilyDrift {
            family,
            census,
            required,
            disposition,
        });
    }
    let dpio_required = derived_seats(plan, &Container::Root);
    let dpio_observed =
        i64::try_from(mc.observe_pool(None, Family::Dpio)?.len()).unwrap_or(i64::MAX);
    Ok(PoolDrift {
        families,
        dpio_required,
        dpio_observed,
    })
}

/// Converges the root's pool families toward the compiled plan's derived counts (pool-objects
/// task 3.4; pool-objects design D3), mirroring [`dispatch_child_population`] at root scope:
/// census → disposition → dispatch → read-back verdict, composing only the phase-2/3 pure
/// functions and the [`dispatch_pool_deltas`] edge (no new policy).
///
/// The pass runs one half of the grow-first/shrink-last walk (`pass`; design D10 pool-objects),
/// in order:
/// - Reads the whole drift ([`plan_pools`]). On the [`PoolPass::Shrink`] half, a family whose
///   requirement fell below its drawn count surfaces as [`PoolOutcome::ShrinkRefused`] before
///   anything is dispatched — a free-only shrink never tears down a live consumer the teardown
///   could not release (pool-objects design D3). The [`PoolPass::Grow`] half defers it (the
///   consumer is torn down between the two passes — the teardown walk).
/// - Gates the pass-specific headline ([`PoolDrift::headline_for`]) against `cfg.allow`
///   (ADR-0015 decision 12): a pool create/destroy/prune is [`Class::Disruptive`], so a
///   hitless run refuses it, matching the container steps' class-gating.
/// - Dispatches each trio family in traversal order (dpmcp→dpbp→dpcon; pool-objects design D8)
///   through [`dispatch_pool_deltas`] with the deltas MASKED to the pass — the grow half only
///   creates, the shrink half only destroys and prunes — and judges by the post-dispatch
///   census read-back (grow: the deficit closed; shrink: the full
///   [`PoolCensus::converged`](dpaa2_api::families::pool_lifecycle::PoolCensus::converged)).
/// - On the grow half only, grows the dpio seat deficit plain (`dpio_create`, NOT
///   `create_dpio_seat`): the dpio→dpmcp probe pairing is the kernel driver's own draw, and the
///   dpmcp pool the trio just grew supplies it (pool-objects design D4). Seats are grown, never
///   shrunk — a surplus is the reboot-required residue, reported not reclaimed.
///
/// Idempotent and level-triggered: a converged root yields an empty pass headline and returns
/// [`PoolOutcome::Converged`] without dispatching, and a second pass over it does too.
///
/// # Errors
/// Propagates a backend read/dispatch error, and reports a family that failed to reach its
/// requirement after dispatch as an [`Error::Backend`] (the container-convergence precedent;
/// the operator re-runs, level-triggered — e.g. after a ceiling-capped grow).
pub fn converge_pools<M: McControl>(
    plan: &CompiledPlan,
    mc: &M,
    cfg: ConvergeConfig,
    pass: PoolPass,
) -> Result<PoolOutcome, Error> {
    let drift = plan_pools(plan, mc)?;

    // A below-draw requirement is the one refusal, surfaced by the shrink half before any
    // dispatch; the grow half defers it to the post-teardown shrink (pool-objects design D10).
    if pass == PoolPass::Shrink
        && let Some(refusal) = drift.shrink_refusal()
    {
        tracing::error!(
            family = refusal.family.name(),
            requirement = refusal.requirement,
            drawn = refusal.drawn,
            "root pool requirement below drawn count"
        );
        return Ok(PoolOutcome::ShrinkRefused { refusal });
    }

    // Gate on the pass headline before touching the board (ADR-0015 decision 12).
    let headline = drift.headline_for(pass);
    if headline > cfg.allow {
        tracing::error!(%headline, allowed = %cfg.allow, "root pool convergence exceeds allowed disruption class");
        return Ok(PoolOutcome::DisruptionRefused {
            headline,
            allowed: cfg.allow,
        });
    }
    if headline == Class::Hitless {
        // Nothing this half does: a converged root, or the other half's work.
        return Ok(PoolOutcome::Converged);
    }

    let declared = root_declared(plan);

    for f in &drift.families {
        // A below-draw Err family carries no grow and is ruled out of shrink, so skip it (pool-objects design D10).
        let Ok(deltas) = f.disposition else {
            continue;
        };
        let masked = match pass {
            PoolPass::Grow => PoolDeltas {
                create: deltas.create,
                destroy: 0,
                prune: 0,
            },
            PoolPass::Shrink => PoolDeltas {
                create: 0,
                destroy: deltas.destroy,
                prune: deltas.prune,
            },
        };
        if masked.is_empty() {
            continue;
        }
        let label = root_family_label(plan, f.family.family());
        let dispatch = dispatch_pool_deltas(
            mc,
            None,
            f.family,
            masked,
            f.required,
            &label,
            &declared,
            CustodyScope::RootScope,
        )?;
        if let Some(refusal) = dispatch.refusal {
            return Ok(PoolOutcome::ShrinkRefused { refusal });
        }
        let after = census_of(&dispatch.after, &declared);
        // Root shrink is grow-only: the verdict is the never-plugged prune cleared; surplus is residue (ADR-0020).
        let converged = match pass {
            PoolPass::Grow => after.managed() == f.required,
            PoolPass::Shrink => after.foreign_free_unplugged() == 0,
        };
        if !converged {
            return Err(Error::Backend(format!(
                "root {} pool did not converge after dispatch: managed {} of required {}",
                f.family.name(),
                after.managed(),
                f.required
            )));
        }
    }

    // dpio seats grow the deficit on the grow half only, grown never shrunk (pool-objects design D4).
    // `required = derived_seats` is the KernelSeat `seat_ceiling` (ADR-0012), so the deficit loop cannot pass it and the `admit_seat` gate stays count-level; a short board surfaces raw MC status.
    if pass == PoolPass::Grow {
        let deficit = drift.seat_deficit();
        if deficit > 0 {
            let label = root_family_label(plan, Family::Dpio);
            for _ in 0..deficit {
                mc.dpio_create(None, default_dpio_cfg(), &label)?;
            }
            let observed_after =
                i64::try_from(mc.observe_pool(None, Family::Dpio)?.len()).unwrap_or(i64::MAX);
            if observed_after != drift.dpio_required {
                return Err(Error::Backend(format!(
                    "root dpio seats did not converge after dispatch: {observed_after} of required {}",
                    drift.dpio_required
                )));
            }
        }
    }

    Ok(PoolOutcome::Converged)
}

/// The outcome of the child-population pass (pool-objects design D11) — the population analog
/// of [`ContainerOutcome`]. Kept distinct because a population refusal is either the disruption
/// gate or the bound-child drift refusal (ADR-0017), neither a container [`Attribution`] nor a
/// DPMAC deadline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PopulationOutcome {
    /// Every declared child is populated to its derived census, its dpnis connected, and
    /// bound to VFIO (the idempotence witness reproduces it).
    Converged,
    /// A child population's headline exceeded the run's `--allow` gate (ADR-0015 decision
    /// 12); nothing was actuated. Creating a resident is [`Class::Disruptive`].
    DisruptionRefused {
        /// The headline the population would have actuated.
        headline: Class,
        /// The maximum class the run allowed.
        allowed: Class,
    },
    /// A child is already VFIO-bound yet its plan still needs residents AND the rebind consent was
    /// declined (`cfg.allow` below [`Class::Disruptive`]): the post-bind residents stay
    /// kernel-invisible until a consented rebind cycle (ADR-0017 decision 3), so nothing is
    /// actuated and the typed standing residue is surfaced. Under a Disruptive allow the drift is
    /// healed in place (one rebind cycle), not refused.
    DriftRefused {
        /// The bound child whose plan still carries pending residents.
        label: ConstructName,
        /// The typed standing residue — rendered via its Display (the honest-residue idiom).
        residue: ChildDeferredVisibility,
    },
    /// A child pool family's derived requirement fell below its drawn count, at either
    /// discovery path — the pre-dispatch census read or the probe-discovered draw — so a
    /// free-only shrink cannot reach the live consumer and nothing is torn down. Mirrors the
    /// root [`PoolOutcome::ShrinkRefused`] (ADR-0020 decision 3; pool-objects design D10).
    ShrinkRefused {
        /// The child whose family requirement fell below its draw.
        label: ConstructName,
        /// The typed below-draw refusal, naming the family and the two counts.
        refusal: ShrinkBelowDraw,
    },
}

/// The outcome of the root-reconcile link pass (cross-dprc-links task 5.2 Half B) — the
/// dpni↔dpni analog of [`PopulationOutcome`]. Distinct because a link refusal is either the
/// disruption gate or the typed held-end (disconnect-before-reconnect) refusal, neither of which
/// the population outcomes carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkOutcome {
    /// Every declared dpni↔dpni link is wired to its planned peer (or waiting on a pending end a
    /// later level-triggered pass resolves); the idempotence witness reproduces it.
    Converged,
    /// A link's headline exceeded the run's `--allow` gate (ADR-0015 decision 12); nothing was
    /// actuated. A root-end create or a fresh connect is [`Class::Disruptive`].
    DisruptionRefused {
        /// The headline the pass would have actuated.
        headline: Class,
        /// The maximum class the run allowed.
        allowed: Class,
    },
    /// One or more link ends are held by a peer other than the plan names (DPRC-I5
    /// disconnect-before-reconnect); the pass refuses rather than silently rewire, and nothing is
    /// actuated (cross-dprc-links design D7).
    RewireRefused {
        /// The held-end refusals, each naming the end, its observed peer, and the planned peer.
        refusals: Vec<WireHeldRefusal>,
    },
}

/// The distinct child (non-root) containers the compiled plan places objects in, by tenant
/// name (pool-objects design D11). A child that owns a container appears here; the root and a
/// restricted drawer pooling the kernel do not.
fn child_tenants(plan: &CompiledPlan) -> BTreeSet<TenantName> {
    plan.objects
        .iter()
        .filter_map(|o| match o.container() {
            Container::Child(tenant) => Some(tenant.clone()),
            Container::Root => None,
        })
        .collect()
}

/// Reads every declared child's population plan (pool-objects design D11), resolving each
/// child's re-observation handle by its container label — the seam the `dry-run` and `status`
/// surfaces render and [`converge_population`] dispatches. A declared child whose container is
/// not yet observed (not created this pass) is skipped: it cannot be populated until it
/// exists, and the level-triggered re-run populates it once [`converge_containers`] has.
///
/// # Errors
/// Propagates a backend/kernel read failure.
pub fn plan_population<M: McControl, K: KernelControl>(
    plan: &CompiledPlan,
    mc: &M,
    kernel: &K,
) -> Result<Vec<ChildPlan>, Error> {
    let observed = mc.observe_containers()?;
    let declared = root_declared(plan);
    // The observed children by label, so a child dpni's link peer resolves against the peer's
    // own container's id (cross-dprc-links design D2).
    let children: BTreeMap<ConstructName, DprcId> = observed
        .iter()
        .map(|(&id, c)| (c.label.clone(), id))
        .collect();
    let mut plans = Vec::new();
    for tenant in child_tenants(plan) {
        let label = ConstructName::from(&tenant);
        let Some((&id, _)) = observed
            .iter()
            .find(|(_, c)| c.label.as_str() == label.as_str())
        else {
            continue;
        };
        let container = Container::Child(tenant);
        plans.push(plan_child_population(
            mc, kernel, id, plan, &container, &label, &declared, &children,
        )?);
    }
    Ok(plans)
}

/// Converges every declared child container's population toward the compiled plan
/// (pool-objects D11; system-integration req 1), run after [`converge_containers`] so each
/// child exists before it is populated. Mirrors [`converge_pools`] at child scope: plan → gate →
/// dispatch → read-back verdict, composing the `dpaa2_mc::populate` edge (no new policy).
///
/// The pass, in order:
/// - Reads every child's plan ([`plan_population`]). A child that is already VFIO-bound yet
///   still needs residents is [`PopulationOutcome::DriftRefused`] before any dispatch: a
///   resident added to a bound child stays invisible until a rebind (ADR-0017), so the drift
///   surfaces and nothing is healed here.
/// - Gates the combined headline against `cfg.allow` (ADR-0015 decision 12): creating a
///   resident (a dpni, a trio delta, a dpio seat) is [`Class::Disruptive`].
/// - Populates each unconverged child ([`dispatch_child_population`]) — per-port dpnis with
///   arity from the plan, their dpmac connect from the common ancestor, the trio deltas and
///   dpio seats — and judges convergence by the post-dispatch read-back census.
/// - Hands each not-yet-bound child to VFIO ([`vfio_handoff`]), guarded by the plan's
///   `bound_driver` read so a re-run over a bound child issues nothing.
///
/// Idempotent and level-triggered: a converged, bound child yields an empty plan and is
/// neither grown nor re-bound, and a second pass over it does too.
///
/// # Errors
/// Propagates a backend/kernel read/dispatch error, and reports a child that failed to reach
/// its derived census after dispatch as an [`Error::Backend`].
pub fn converge_population<M: McControl, K: KernelControl>(
    plan: &CompiledPlan,
    mc: &M,
    kernel: &K,
    cfg: ConvergeConfig,
) -> Result<PopulationOutcome, Error> {
    let plans = plan_population(plan, mc, kernel)?;

    // The pre-dispatch below-draw refusal, surfaced before any dispatch (mirroring root
    // `converge_pools`; pool-objects design D10).
    if let Some((label, refusal)) = plans
        .iter()
        .find_map(|cp| cp.shrink_refusal().map(|r| (cp.label.clone(), r)))
    {
        tracing::error!(
            %label,
            family = refusal.family.name(),
            requirement = refusal.requirement,
            drawn = refusal.drawn,
            "child pool requirement below drawn count"
        );
        return Ok(PopulationOutcome::ShrinkRefused { label, refusal });
    }

    // ADR-0017 decision 3: a bound child's pending residents are MC-accepted but kernel-invisible
    // until a rebind; consent is the run's Disruptive allow, judged through the pure
    // `discharge_child` (never an inline class check). Declined consent surfaces the typed standing
    // residue and actuates nothing; a granted one is healed in the dispatch loop below.
    for cp in plans.iter().filter(|c| c.bound && !c.is_converged()) {
        let obligation = ChildDeferredVisibility::new(cp.label.clone(), cp.child);
        if let Err(residue) = discharge_child(obligation, cfg.allow) {
            tracing::error!(label = %cp.label, "post-bind residents drift in a bound child; rebind consent declined (ADR-0017)");
            return Ok(PopulationOutcome::DriftRefused {
                label: cp.label.clone(),
                residue,
            });
        }
    }

    // Gate on the combined headline before touching the board (ADR-0015 decision 12).
    let headline = plans
        .iter()
        .map(ChildPlan::headline)
        .max()
        .unwrap_or(Class::Hitless);
    if headline > cfg.allow {
        tracing::error!(%headline, allowed = %cfg.allow, "child population exceeds allowed disruption class");
        return Ok(PopulationOutcome::DisruptionRefused {
            headline,
            allowed: cfg.allow,
        });
    }

    let declared = root_declared(plan);
    // The observed children by label, so a post-rebind re-plan resolves each child's id and any
    // cross-container link peer (cross-dprc-links design D2).
    let children: BTreeMap<ConstructName, DprcId> = mc
        .observe_containers()?
        .iter()
        .map(|(&id, c)| (c.label.clone(), id))
        .collect();
    for cp in &plans {
        if !cp.is_converged() {
            let pop = dispatch_child_population(mc, cp, &declared)?;
            if let Some(refusal) = pop.refusal {
                tracing::error!(
                    label = %cp.label,
                    family = refusal.family.name(),
                    requirement = refusal.requirement,
                    drawn = refusal.drawn,
                    "child pool requirement below drawn count discovered during dispatch"
                );
                return Ok(PopulationOutcome::ShrinkRefused {
                    label: cp.label.clone(),
                    refusal,
                });
            }
            // A not-yet-bound child's post-dispatch read-back is its verdict; a bound child is
            // judged only AFTER its rebind cycle below (ADR-0017 decision 2: never by create
            // acceptance, always by post-rebind re-observation).
            if !cp.bound && !pop.converged(cp.dpnis.len()) {
                return Err(Error::Backend(format!(
                    "child `{}` did not converge after population dispatch: {pop:?}",
                    cp.label
                )));
            }
        }
        if cp.bound {
            // ADR-0017 decision 2: a consent-cleared bound child heals with ONE rebind cycle
            // (unbind → bind), then the post-rebind re-observation is the sole convergence verdict.
            if !cp.is_converged() {
                kernel.vfio_unbind(cp.child)?;
                kernel.vfio_bind(cp.child)?;
                let container = Container::Child(TenantName::from(cp.label.as_str()));
                let healed = plan_child_population(
                    mc, kernel, cp.child, plan, &container, &cp.label, &declared, &children,
                )?;
                if !healed.is_converged() {
                    return Err(Error::Backend(format!(
                        "child `{}` stayed kernel-invisible after the post-bind rebind cycle: {healed:?}",
                        cp.label
                    )));
                }
                tracing::info!(label = %cp.label, child = %cp.child, "post-bind rebind cycle applied; residents now visible (ADR-0017)");
            }
        } else {
            // Populate, then bind (ADR-0017): the handoff fires only on a not-yet-bound child.
            vfio_handoff(kernel, cp.child)?;
        }
    }
    Ok(PopulationOutcome::Converged)
}

/// One resolved dpni↔dpni link end for the root link pass (cross-dprc-links design D2): already
/// on the board, or a root-resident end absent that this pass creates with the planned block.
enum LinkSlot<'a> {
    /// The end's dpni is already observed in its container.
    Have(WireEnd),
    /// A root-resident end absent from the board — created this pass with the planned block; a
    /// child deficit is never here (that stays population's job, level-triggered).
    CreateRoot {
        /// The construct (link-name) label the create stamps.
        label: &'a ConstructName,
        /// The planned create block.
        cfg: &'a dpaa2_api::families::dpni::DpniCfg,
    },
}

/// What the root pass resolved for one dpni↔dpni edge (cross-dprc-links design D2): its two
/// end slots, or a skip when an end is pending this pass (a child dpni not yet created, or a
/// child container not yet observed — a later level-triggered pass resolves it).
enum LinkEdgePlan<'a> {
    /// An end is pending; the edge plans nothing this pass.
    Skip,
    /// Both ends are resolvable (present or a root create).
    Wire {
        /// One end (unordered — a wire has no direction).
        a: LinkSlot<'a>,
        /// The other end.
        b: LinkSlot<'a>,
    },
}

/// The MC container id a planned [`Container`] resolves to, or `None` when a child container is
/// not yet observed (cross-dprc-links design D2): the root is [`DprcId::ROOT`]; a child is read
/// from the observed label→id map `plan_population` also builds.
fn link_container_id(
    container: &Container,
    children: &BTreeMap<ConstructName, DprcId>,
) -> Option<DprcId> {
    match container {
        Container::Root => Some(DprcId::ROOT),
        Container::Child(tenant) => children.get(&ConstructName::from(tenant)).copied(),
    }
}

/// The `observe_pool` scope for a container id — `None` for the root (the `dprc.1` sentinel),
/// `Some(id)` for a child, matching the child-population resolution (pool-objects design D11).
fn link_observe_scope(id: DprcId) -> Option<DprcId> {
    (id != DprcId::ROOT).then_some(id)
}

/// A per-pass dpni pool-row cache for the link pass (link-hardening task 5.1, S27): each
/// container's `observe_pool(scope, Dpni)` rows are read ONCE per pass, so the N link edges resolve
/// (and the root ends plug) against one read per container instead of one per edge — the same
/// per-ensure rescan prune the [`McControl::observe_container`] doc records. A create invalidates
/// the mutated container's entry, so a later edge re-reads it; the cache lives in the shell, never
/// as interior mutability behind the trait (the per-MC-command seam #10 drops into, cross-dprc-links design D6).
struct LinkPoolCache<'m, M: McControl> {
    mc: &'m M,
    rows: RefCell<BTreeMap<Option<DprcId>, Vec<ObservedPoolObject>>>,
}

impl<'m, M: McControl> LinkPoolCache<'m, M> {
    fn new(mc: &'m M) -> Self {
        Self {
            mc,
            rows: RefCell::new(BTreeMap::new()),
        }
    }

    /// The dpni pool rows for `scope`, observed once then served from the cache for the rest of the
    /// pass (one restool spawn per container per pass).
    fn dpni_rows(&self, scope: Option<DprcId>) -> Result<Vec<ObservedPoolObject>, Error> {
        if let Some(rows) = self.rows.borrow().get(&scope) {
            return Ok(rows.clone());
        }
        let rows = self.mc.observe_pool(scope, Family::Dpni)?;
        self.rows.borrow_mut().insert(scope, rows.clone());
        Ok(rows)
    }

    /// Drops `scope`'s cached rows after a create in it, so the next read re-observes (step 4): a
    /// freshly minted end would otherwise be absent from a later edge's resolution.
    fn invalidate(&self, scope: Option<DprcId>) {
        self.rows.borrow_mut().remove(&scope);
    }
}

/// Resolves one dpni↔dpni edge's two ends from the compiled plan and the board
/// (cross-dprc-links design D2). Two ends sharing one container are a multiset of rows bearing
/// the link-name label — position is not a hardware identity (ADR-0015 decision 5), so the link
/// wants exactly two such rows, the ROOT deficit is created, and the two distinct rows connect in
/// either order (a self-loop that would share a child container is refused at intent). Ends in
/// distinct containers each resolve by the per-end label match.
fn resolve_link_edge<'a, M: McControl>(
    cache: &LinkPoolCache<'_, M>,
    plan: &'a CompiledPlan,
    a_key: &dpaa2_api::intent::compiled::ObjectKey,
    b_key: &dpaa2_api::intent::compiled::ObjectKey,
    children: &BTreeMap<ConstructName, DprcId>,
) -> Result<LinkEdgePlan<'a>, Error> {
    let (Some(a_obj), Some(b_obj)) = (
        plan.objects.iter().find(|o| o.key() == a_key),
        plan.objects.iter().find(|o| o.key() == b_key),
    ) else {
        return Ok(LinkEdgePlan::Skip);
    };
    let (Attributes::Dpni { cfg: a_cfg }, Attributes::Dpni { cfg: b_cfg }) =
        (a_obj.attributes(), b_obj.attributes())
    else {
        return Ok(LinkEdgePlan::Skip);
    };
    // Both ends carry the link name as their label (derive link-end dpnis).
    let label = a_obj.label();
    let (Some(a_id), Some(b_id)) = (
        link_container_id(a_obj.container(), children),
        link_container_id(b_obj.container(), children),
    ) else {
        return Ok(LinkEdgePlan::Skip);
    };

    if a_id == b_id {
        let rows: Vec<_> = cache
            .dpni_rows(link_observe_scope(a_id))?
            .into_iter()
            .filter(|r| r.label.as_str() == label.as_str())
            .collect();
        let have = |r: &dpaa2_api::families::pool_lifecycle::ObservedPoolObject| {
            LinkSlot::Have(WireEnd {
                dpni: DpniId::new(r.object.ordinal()),
                container: a_id,
            })
        };
        return Ok(match (rows.len(), a_id == DprcId::ROOT) {
            (0, true) => LinkEdgePlan::Wire {
                a: LinkSlot::CreateRoot { label, cfg: a_cfg },
                b: LinkSlot::CreateRoot { label, cfg: b_cfg },
            },
            (1, true) => LinkEdgePlan::Wire {
                a: have(&rows[0]),
                b: LinkSlot::CreateRoot { label, cfg: b_cfg },
            },
            (n, _) if n >= 2 => LinkEdgePlan::Wire {
                a: have(&rows[0]),
                b: have(&rows[1]),
            },
            // A child same-container deficit is population's job (level-triggered).
            _ => LinkEdgePlan::Skip,
        });
    }

    let (Some(a), Some(b)) = (
        resolve_link_end(cache, a_obj.container(), a_id, label, a_cfg)?,
        resolve_link_end(cache, b_obj.container(), b_id, label, b_cfg)?,
    ) else {
        return Ok(LinkEdgePlan::Skip);
    };
    Ok(LinkEdgePlan::Wire { a, b })
}

/// Resolves one end of a distinct-container edge: the observed dpni row, else a root create, else
/// `None` when a child end is absent this pass (population's job; cross-dprc-links design D2).
fn resolve_link_end<'a, M: McControl>(
    cache: &LinkPoolCache<'_, M>,
    container: &Container,
    id: DprcId,
    label: &'a ConstructName,
    cfg: &'a dpaa2_api::families::dpni::DpniCfg,
) -> Result<Option<LinkSlot<'a>>, Error> {
    let row = cache
        .dpni_rows(link_observe_scope(id))?
        .into_iter()
        .find(|r| r.label.as_str() == label.as_str());
    Ok(match (row, container) {
        (Some(r), _) => Some(LinkSlot::Have(WireEnd {
            dpni: DpniId::new(r.object.ordinal()),
            container: id,
        })),
        (None, Container::Root) => Some(LinkSlot::CreateRoot { label, cfg }),
        (None, Container::Child(_)) => None,
    })
}

/// Creates a root-resident end absent from the board, else returns the already-resolved end
/// (cross-dprc-links design D2). A created root dpni reads back unplugged; its plug follows the
/// connect (the populate-connect-bind order, cross-dprc-links design D4).
// The slot is moved out of the per-edge plan, so it is taken by value though its fields are Copy.
#[allow(clippy::needless_pass_by_value)]
fn materialize_link_end<M: McControl>(
    mc: &M,
    slot: LinkSlot<'_>,
    cache: &LinkPoolCache<'_, M>,
) -> Result<WireEnd, Error> {
    match slot {
        LinkSlot::Have(end) => Ok(end),
        LinkSlot::CreateRoot { label, cfg } => {
            let dpni = mc.create_dpni(label, cfg)?;
            // The new root row lands in `dprc.1`, so the cached root rows are now stale (step 4).
            cache.invalidate(None);
            Ok(WireEnd {
                dpni,
                container: DprcId::ROOT,
            })
        }
    }
}

/// Plugs a root-resident end after its fresh connect, skipping an end already plugged
/// (cross-dprc-links design D4): a root kernel dpni is plugged with `dprc assign --plugged=1`; a
/// child end is never plugged here (the VFIO handoff owns a child's plug face).
///
/// The plug-check reads no board (link-hardening task 5.1, S27): a `fresh` end was created this
/// pass and reads back unplugged by construction (the fake and restool both create unplugged; see
/// [`materialize_link_end`]), so it is plugged unconditionally; an already-resident end is judged
/// from its cached row — plugged when the row is present and plugged, plugged otherwise.
fn plug_root_link_end<M: McControl>(
    mc: &M,
    end: WireEnd,
    cache: &LinkPoolCache<'_, M>,
    fresh: bool,
) -> Result<(), Error> {
    if end.container != DprcId::ROOT {
        return Ok(());
    }
    let object = ObjectRef::new(Family::Dpni, end.dpni.into_inner());
    let plugged = !fresh
        && cache
            .dpni_rows(None)?
            .iter()
            .any(|r| r.object == object && r.plugged);
    if !plugged {
        mc.dprc_assign(DprcId::ROOT, object, None, Some(true))?;
    }
    Ok(())
}

/// The pre-pass verdict for one link edge (link-hardening task 5.1): a both-present edge is judged
/// in the pre-pass (its two connection reads feed dispatch unchanged), a create/pending edge is
/// deferred so its absent root ends materialize before it is judged.
enum EdgeVerdict {
    /// A both-present edge already judged pre-pass; the dispatch arm reuses this `WirePlan` rather
    /// than re-reading both connections (S27).
    Judged(WirePlan),
    /// A create/pending edge judged at dispatch, after any absent root end is materialized.
    Deferred,
}

/// Resolves and actuates one link edge at dispatch (link-hardening task 5.1, S27): a both-present
/// edge reuses its pre-pass [`WirePlan`]; a deferred edge materializes its absent root ends, reads
/// only a present end's connection (a freshly created end is disconnected by construction), then
/// connects at the common ancestor and plugs the root ends. Returns the actuated `(dpni, peer)` for
/// the post-dispatch read-back, or `None` when nothing fired (an already-wired or pending edge).
///
/// # Errors
/// Propagates a backend create/connect/assign error.
fn dispatch_link_edge<M: McControl>(
    mc: &M,
    cache: &LinkPoolCache<'_, M>,
    plan: LinkEdgePlan<'_>,
    verdict: EdgeVerdict,
) -> Result<Option<(DpniId, ObjectRef)>, Error> {
    let (wp, a_fresh, b_fresh) = match verdict {
        EdgeVerdict::Judged(wp) => (wp, false, false),
        EdgeVerdict::Deferred => {
            let LinkEdgePlan::Wire { a, b } = plan else {
                return Ok(None);
            };
            let a_fresh = matches!(a, LinkSlot::CreateRoot { .. });
            let b_fresh = matches!(b, LinkSlot::CreateRoot { .. });
            let a = materialize_link_end(mc, a, cache)?;
            let b = materialize_link_end(mc, b, cache)?;
            let a_obs = if a_fresh {
                None
            } else {
                mc.dprc_get_connection(a.dpni)?
            };
            let b_obs = if b_fresh {
                None
            } else {
                mc.dprc_get_connection(b.dpni)?
            };
            let wp = plan_wire(
                LinkEndState::Resolved(a),
                LinkEndState::Resolved(b),
                a_obs,
                b_obs,
            );
            (wp, a_fresh, b_fresh)
        }
    };
    if let WirePlan::Connect(WireTransition::ConnectWire { a, b, ancestor }) = wp {
        let peer = ObjectRef::new(Family::Dpni, b.dpni.into_inner());
        mc.dprc_connect(ancestor, a.dpni, peer)?;
        plug_root_link_end(mc, a, cache, a_fresh)?;
        plug_root_link_end(mc, b, cache, b_fresh)?;
        return Ok(Some((a.dpni, peer)));
    }
    Ok(None)
}

/// Converges every declared dpni↔dpni link toward the compiled plan — the root-reconcile link
/// pass (cross-dprc-links task 5.2; reconciler reqs "representable in every container
/// arrangement" and "disconnect-before-reconnect"). Mirrors [`converge_population`]:
/// resolve → gate → dispatch → read-back verdict, the adapter drives and the pure
/// [`plan_wire`] judges.
///
/// The pass, in order:
/// - Resolves every link edge's two ends (reads only): present, a root end to create, or a
///   pending end (a child dpni not yet created — population's job — or a child container not yet
///   observed) that skips the edge this pass, level-triggered. Each container's dpni pool rows are
///   read ONCE per pass through a per-pass pool cache, so N edges cost one read per container, not
///   one per edge (link-hardening task 5.1, S27).
/// - Refuses before any mutation when a resolved end is held by a peer other than planned
///   ([`LinkOutcome::RewireRefused`]; DPRC-I5 disconnect-before-reconnect — never a silent
///   rewire), and gates the headline against `cfg.allow` (ADR-0015 decision 12): a root-end
///   create or a fresh connect is [`Class::Disruptive`].
/// - Creates each absent root end, connects each fresh pair at the common ancestor
///   ([`WireTransition::connect_wire`]'s resolved [`CONNECT_ANCESTOR`]), then plugs the root ends
///   — the populate-connect-bind order (cross-dprc-links design D4). A both-present edge reuses the
///   connection reads the pre-pass took, a freshly created end is known disconnected-and-unplugged
///   by construction, and the plug-check judges from the cached row — so the dispatch arm re-reads
///   neither the connection nor the plug state of an end the pass already knows (S27). An
///   already-wired edge connects nothing (idempotence in [`plan_wire`]).
/// - Judges convergence by re-reading each actuated edge's connection; a miss is an
///   [`Error::Backend`] (the [`converge_population`] read-back precedent).
///
/// Idempotent and level-triggered: a fully-wired set resolves to all-`Nothing` and actuates
/// nothing, and a second pass over it does too.
///
/// # Errors
/// Propagates a backend read/create/connect/assign error, and reports an actuated edge that did
/// not read back connected as an [`Error::Backend`].
pub fn converge_links<M: McControl>(
    plan: &CompiledPlan,
    mc: &M,
    cfg: ConvergeConfig,
) -> Result<LinkOutcome, Error> {
    let edges: Vec<(
        &dpaa2_api::intent::compiled::ObjectKey,
        &dpaa2_api::intent::compiled::ObjectKey,
    )> = plan
        .edges
        .iter()
        .filter_map(dpaa2_api::intent::compiled::Edge::link_edge_dpnis)
        .collect();
    if edges.is_empty() {
        return Ok(LinkOutcome::Converged);
    }
    let observed = mc.observe_containers()?;
    let children: BTreeMap<ConstructName, DprcId> = observed
        .iter()
        .map(|(&id, c)| (c.label.clone(), id))
        .collect();

    // Resolve first (reads only), so the held refusal and the gate are judged before any mutation.
    // One pool read per container feeds every edge's resolution through the pass cache (S27).
    let cache = LinkPoolCache::new(mc);
    let mut plans = Vec::new();
    for (a_key, b_key) in edges {
        plans.push(resolve_link_edge(&cache, plan, a_key, b_key, &children)?);
    }

    // A held-end refusal changes nothing (DPRC-I5), so it is judged before the gate; a create or a
    // fresh connect sets the Disruptive headline. Each both-present edge's `plan_wire` verdict is
    // kept so the dispatch arm reuses it instead of re-reading both connections (S27).
    let mut held = Vec::new();
    let mut any_work = false;
    let mut verdicts: Vec<EdgeVerdict> = Vec::with_capacity(plans.len());
    for p in &plans {
        match p {
            LinkEdgePlan::Wire {
                a: LinkSlot::Have(a),
                b: LinkSlot::Have(b),
            } => {
                let wp = plan_wire(
                    LinkEndState::Resolved(*a),
                    LinkEndState::Resolved(*b),
                    mc.dprc_get_connection(a.dpni)?,
                    mc.dprc_get_connection(b.dpni)?,
                );
                match &wp {
                    WirePlan::Connect(_) => any_work = true,
                    WirePlan::HeldByOtherPeer(r) => held.push(*r),
                    WirePlan::Nothing => {}
                }
                verdicts.push(EdgeVerdict::Judged(wp));
            }
            LinkEdgePlan::Wire { a, b } => {
                // A mixed edge's lone present end, judged against its still-to-create peer, so a
                // foreign-held root end refuses before the missing peer is minted (link-hardening
                // review PASS2-F1; DPRC-I5).
                let lone = match (a, b) {
                    (LinkSlot::Have(e), LinkSlot::CreateRoot { .. })
                    | (LinkSlot::CreateRoot { .. }, LinkSlot::Have(e)) => Some(*e),
                    _ => None,
                };
                if let Some(have) = lone
                    && let WirePlan::HeldByOtherPeer(r) = plan_wire(
                        LinkEndState::Resolved(have),
                        LinkEndState::Pending,
                        mc.dprc_get_connection(have.dpni)?,
                        None,
                    )
                {
                    held.push(r);
                }
                any_work = true;
                verdicts.push(EdgeVerdict::Deferred);
            }
            LinkEdgePlan::Skip => verdicts.push(EdgeVerdict::Deferred),
        }
    }
    if !held.is_empty() {
        for r in &held {
            tracing::error!(refusal = %r, "link end held by a different peer; disconnect before reconnect (DPRC-I5)");
        }
        return Ok(LinkOutcome::RewireRefused { refusals: held });
    }
    let headline = if any_work {
        Class::Disruptive
    } else {
        Class::Hitless
    };
    if headline > cfg.allow {
        tracing::error!(%headline, allowed = %cfg.allow, "link convergence exceeds allowed disruption class");
        return Ok(LinkOutcome::DisruptionRefused {
            headline,
            allowed: cfg.allow,
        });
    }

    let mut actuated: Vec<(DpniId, ObjectRef)> = Vec::new();
    for (p, verdict) in plans.into_iter().zip(verdicts) {
        if let Some(edge) = dispatch_link_edge(mc, &cache, p, verdict)? {
            actuated.push(edge);
        }
    }
    for (dpni, peer) in actuated {
        if mc.dprc_get_connection(dpni)? != Some(peer) {
            return Err(Error::Backend(format!(
                "link {dpni} did not read back connected to {peer} after dispatch"
            )));
        }
    }
    Ok(LinkOutcome::Converged)
}

/// The read-only plan for one dpni↔dpni link edge, the dry-run twin of a [`converge_links`]
/// dispatch step (cross-dprc-links task 5.5): the two plan ends, the issuing ancestor, the
/// link-edge provenance key, and the action the same resolution + [`plan_wire`] authority judges.
#[derive(Clone, Debug)]
pub struct LinkDryRun {
    /// The `a` end's plan key.
    pub a: ObjectKey,
    /// The `b` end's plan key.
    pub b: ObjectKey,
    /// The common ancestor the connect would be issued at.
    pub ancestor: DprcId,
    /// The link-edge provenance key, rendered as the operator's rule trace.
    pub provenance: ProvenanceKey,
    /// The action this pass would take.
    pub action: LinkDryRunAction,
}

/// What the read-only link planner would do for one edge (cross-dprc-links task 5.5), the
/// [`WirePlan`] outcome lifted to the edge: a fresh connect, nothing (already wired), a pending
/// end a later pass resolves, or the typed held-end refusal.
#[derive(Clone, Debug)]
pub enum LinkDryRunAction {
    /// A connect would issue (fresh); classed [`Class::Disruptive`] like the port connect.
    Connect,
    /// Already wired to the planned peer — nothing to do.
    Converged,
    /// An end is not yet resident (a child dpni uncreated, or its container unobserved).
    Pending,
    /// An end is held by a different peer than planned (DPRC-I5).
    Held(WireHeldRefusal),
}

/// Plans every dpni↔dpni link read-only — the exact connect [`converge_links`] would execute,
/// rendered by `dry-run` without dispatching (cross-dprc-links task 5.5). Reuses the same
/// `resolve_link_edge` resolution and [`plan_wire`] authority, so the predicted transition is
/// the one `ensure` actuates.
///
/// # Errors
/// Propagates a backend read failure.
pub fn plan_links<M: McControl>(plan: &CompiledPlan, mc: &M) -> Result<Vec<LinkDryRun>, Error> {
    let children: BTreeMap<ConstructName, DprcId> = mc
        .observe_containers()?
        .iter()
        .map(|(&id, c)| (c.label.clone(), id))
        .collect();
    // Read-only, so one pool read per container serves every edge's resolution (S27).
    let cache = LinkPoolCache::new(mc);
    let mut out = Vec::new();
    for edge in &plan.edges {
        let Some((a_key, b_key)) = edge.link_edge_dpnis() else {
            continue;
        };
        let action = match resolve_link_edge(&cache, plan, a_key, b_key, &children)? {
            LinkEdgePlan::Skip => LinkDryRunAction::Pending,
            LinkEdgePlan::Wire {
                a: LinkSlot::Have(ea),
                b: LinkSlot::Have(eb),
            } => match plan_wire(
                LinkEndState::Resolved(ea),
                LinkEndState::Resolved(eb),
                mc.dprc_get_connection(ea.dpni)?,
                mc.dprc_get_connection(eb.dpni)?,
            ) {
                WirePlan::Connect(_) => LinkDryRunAction::Connect,
                WirePlan::Nothing => LinkDryRunAction::Converged,
                WirePlan::HeldByOtherPeer(r) => LinkDryRunAction::Held(r),
            },
            // An end to create means a fresh connect follows it.
            LinkEdgePlan::Wire { .. } => LinkDryRunAction::Connect,
        };
        out.push(LinkDryRun {
            a: a_key.clone(),
            b: b_key.clone(),
            ancestor: CONNECT_ANCESTOR,
            provenance: edge.provenance().clone(),
            action,
        });
    }
    Ok(out)
}

/// One dpni↔dpni link's read-only `status --detail` row (cross-dprc-links task 5.5): the link
/// name, each end's observed dpni (absent ⇒ honest unknown), and the connection state judged from
/// `dprc_get_connection`. Display-only; no field gates convergence.
#[derive(Clone, Debug)]
pub struct LinkRow {
    /// The link's construct name.
    pub link: ConstructName,
    /// The `a` end's observed dpni, or `None` when not resident (honest unknown).
    pub a: Option<ObjectRef>,
    /// The `b` end's observed dpni, or `None` when not resident.
    pub b: Option<ObjectRef>,
    /// The connection state read from the ancestor.
    pub connection: LinkConnection,
}

/// A link's connection state as `status --detail` reads it (cross-dprc-links task 5.5), honest
/// about an unavailable read (an end not resident) rather than reporting down or disconnected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkConnection {
    /// End `a` reads connected to end `b`.
    Connected,
    /// End `a` reads connected to a peer other than `b`.
    ConnectedElsewhere(ObjectRef),
    /// End `a` reads disconnected.
    Disconnected,
    /// An end is not resident, so the connection cannot be judged.
    Unknown,
}

fn link_slot_ref(slot: &LinkSlot<'_>) -> Option<ObjectRef> {
    match slot {
        LinkSlot::Have(end) => Some(ObjectRef::new(Family::Dpni, end.dpni.into_inner())),
        LinkSlot::CreateRoot { .. } => None,
    }
}

/// Reads the read-only link rows for `status --detail` (cross-dprc-links task 5.5): each declared
/// dpni↔dpni link's endpoints and connection state, sourced from `dprc_get_connection` at the
/// ancestor. An end not resident renders as the honest unknown, and the command still exits
/// success.
///
/// # Errors
/// Propagates a backend read failure.
pub fn link_rows<M: McControl>(plan: &CompiledPlan, mc: &M) -> Result<Vec<LinkRow>, Error> {
    let children: BTreeMap<ConstructName, DprcId> = mc
        .observe_containers()?
        .iter()
        .map(|(&id, c)| (c.label.clone(), id))
        .collect();
    // Read-only, so one pool read per container serves every edge's resolution (S27).
    let cache = LinkPoolCache::new(mc);
    let mut out = Vec::new();
    for edge in &plan.edges {
        let Some((a_key, b_key)) = edge.link_edge_dpnis() else {
            continue;
        };
        let (a, b) = match resolve_link_edge(&cache, plan, a_key, b_key, &children)? {
            LinkEdgePlan::Wire { a, b } => (link_slot_ref(&a), link_slot_ref(&b)),
            LinkEdgePlan::Skip => (None, None),
        };
        let connection = match (a, b) {
            (Some(a_ref), Some(b_ref)) => {
                match mc.dprc_get_connection(DpniId::new(a_ref.ordinal()))? {
                    Some(peer) if peer == b_ref => LinkConnection::Connected,
                    Some(peer) => LinkConnection::ConnectedElsewhere(peer),
                    None => LinkConnection::Disconnected,
                }
            }
            _ => LinkConnection::Unknown,
        };
        out.push(LinkRow {
            link: edge.provenance().construct.clone(),
            a,
            b,
            connection,
        });
    }
    Ok(out)
}

/// The standing child-keyed [`ChildDeferredVisibility`] obligations for `status --detail`
/// (cross-dprc-links task 5.5): one per bound child whose plan still carries pending residents.
/// Display-only; it gates no convergence verdict (the same read [`converge_population`]'s consent
/// pre-pass judges, here surfaced not actuated).
///
/// # Errors
/// Propagates a backend/kernel read failure.
pub fn link_obligations<M: McControl, K: KernelControl>(
    plan: &CompiledPlan,
    mc: &M,
    kernel: &K,
) -> Result<Vec<ChildDeferredVisibility>, Error> {
    Ok(plan_population(plan, mc, kernel)?
        .iter()
        .filter(|cp| cp.bound && !cp.is_converged())
        .map(|cp| ChildDeferredVisibility::new(cp.label.clone(), cp.child))
        .collect())
}

/// Reads MC state and enriches each DPNI with its kernel netdev name.
///
/// # Errors
/// Propagates backend/kernel read failures.
pub fn observe<M: McControl, K: KernelControl>(
    mc: &M,
    kernel: &K,
) -> Result<ObservedTopology, Error> {
    let mut topo = mc.observe()?;
    for dpni in &mut topo.dpnis {
        dpni.netdev = kernel.netdev_of(dpni.id)?;
    }
    Ok(topo)
}

/// Probes MC liveness by issuing an MC command and retrying until it responds or
/// `timeout` elapses (design D5; ADR-0003). Returns `true` once the MC answers.
///
/// The MC exposes no `firmware_version` sysfs attribute on the target, so readiness
/// can only be detected by a command that round-trips through the firmware — here,
/// `observe`.
///
/// # Errors
/// Never returns the backend error; a failed probe is a not-ready signal that is
/// retried. Returns `Ok(false)` on timeout.
pub fn wait_ready<M: McControl>(
    mc: &M,
    timeout: Duration,
    interval: Duration,
) -> Result<bool, Error> {
    let start = Instant::now();
    loop {
        match mc.observe() {
            Ok(_) => {
                tracing::info!("MC is responsive");
                return Ok(true);
            }
            Err(e) => {
                if start.elapsed() >= timeout {
                    tracing::error!(error = %e, "MC not ready before timeout");
                    return Ok(false);
                }
                tracing::debug!(error = %e, "MC not ready yet; retrying");
                sleep(interval);
            }
        }
    }
}

/// Drives the convergence loop to completion or deadline.
///
/// # Errors
/// Returns an error if a backend operation fails irrecoverably.
pub fn ensure<M: McControl, K: KernelControl>(
    desired: &DesiredTopology,
    mc: &M,
    kernel: &K,
    cfg: ConvergeConfig,
) -> Result<Outcome, Error> {
    let opts = ReconcileOptions { prune: cfg.prune };
    let start = Instant::now();
    // Spans the whole run (not one apply pass) so a same-run rebuild is refused, not churned (pool-objects design D12; ADR-0008 §9).
    let mut created: HashMap<DpmacId, DpniId> = HashMap::new();

    loop {
        let observed = observe(mc, kernel)?;
        let run_created: BTreeSet<DpniId> = created.values().copied().collect();
        let plan = reconcile_with(desired, &observed, opts, &run_created);
        log_plan(&observed, &plan);

        if plan.is_converged() {
            tracing::info!("converged");
            return Ok(Outcome::Converged);
        }

        // The loop-breaker exit, before the headline gate and deadline so a refusal never spins to the deadline (pool-objects design D12; ADR-0008 §9).
        if !plan.refusals.is_empty() {
            tracing::error!(count = plan.refusals.len(), "same-run rebuild refused");
            return Ok(Outcome::RebuildRefused {
                refusals: plan.refusals,
            });
        }

        // Gate on the plan's headline before touching the board (ADR-0015 decision
        // 12): the run may proceed only when the headline is within the allowed
        // class, and disruptive is never implied.
        let headline = plan.headline();
        if headline > cfg.allow {
            tracing::error!(%headline, allowed = %cfg.allow, "plan exceeds allowed disruption class");
            return Ok(Outcome::DisruptionRefused {
                headline,
                allowed: cfg.allow,
            });
        }

        if start.elapsed() >= cfg.deadline {
            let unconverged = unconverged_anchors(&plan);
            tracing::error!(?unconverged, "deadline exceeded before convergence");
            return Ok(Outcome::DeadlineExceeded { unconverged });
        }

        apply(&plan, &observed, mc, kernel, &mut created)?;
        sleep(cfg.poll_interval);
    }
}

/// Applies a plan's transitions once. Wait-only transitions (`Bind`) merely nudge
/// the kernel; the loop re-observes to detect the resulting netdev.
///
/// # Errors
/// Returns an error if any actuation fails.
// `created` is ensure's own run-scoped map, never a caller-chosen hasher.
#[allow(clippy::implicit_hasher)]
pub fn apply<M: McControl, K: KernelControl>(
    plan: &Plan,
    observed: &ObservedTopology,
    mc: &M,
    kernel: &K,
    created: &mut HashMap<DpmacId, DpniId>,
) -> Result<(), Error> {
    for t in &plan.transitions {
        match t {
            Transition::Create { port, label, cfg } => {
                let id = mc.create_dpni(label, cfg)?;
                created.insert(*port, id);
                tracing::info!(%port, %id, %label, "created dpni");
            }
            Transition::Connect { port } => {
                let id = resolve(*port, created, observed)?;
                mc.connect(id, *port)?;
                tracing::info!(%port, %id, "connected dpni to dpmac");
            }
            Transition::SetMac { port, mac } => {
                let id = resolve(*port, created, observed)?;
                mc.set_mac(id, *mac)?;
                tracing::info!(%port, %id, %mac, "set dpni primary mac");
            }
            Transition::Bind { port } => {
                let id = resolve(*port, created, observed)?;
                kernel.bind(id)?;
                tracing::debug!(%port, %id, "nudged bind; awaiting netdev");
            }
            Transition::Disconnect { dpni } => {
                mc.dprc_disconnect(CONNECT_ANCESTOR, *dpni)?;
                tracing::info!(%dpni, "disconnected dpni");
            }
            Transition::Unbind { proof } => {
                // After the disconnect severed the edge, before destroy — a destroy-while-bound is refused (ADR-0008 §8/§9).
                let dpni = proof.dpni();
                kernel.unbind(dpni)?;
                tracing::info!(%dpni, "unbound dpni from fsl_dpaa2_eth");
            }
            Transition::Destroy { dpni } => {
                mc.destroy(*dpni)?;
                tracing::info!(%dpni, "destroyed dpni");
            }
            Transition::SetLabel { dpni, label } => {
                // The matcher's relabel lowering (ADR-0015 decisions 9-10): set-label is
                // decision 9's repair verb, never a destroy/create.
                mc.set_label(*dpni, label)?;
                tracing::info!(%dpni, %label, "relabelled dpni to construct name");
            }
        }
    }
    Ok(())
}

/// Resolves the DPNI addressed by a port-anchored transition.
fn resolve(
    port: DpmacId,
    created: &HashMap<DpmacId, DpniId>,
    observed: &ObservedTopology,
) -> Result<DpniId, Error> {
    created
        .get(&port)
        .copied()
        .or_else(|| observed.dpni_connected_to(port).map(|d| d.id))
        .ok_or_else(|| Error::Backend(format!("no DPNI resolvable for {port}")))
}

/// The set of anchors a non-converged plan still needs to act on.
fn unconverged_anchors(plan: &Plan) -> Vec<DpmacId> {
    let mut anchors = Vec::new();
    for t in &plan.transitions {
        let anchor = match t {
            Transition::Create { port, .. }
            | Transition::Connect { port }
            | Transition::Bind { port }
            | Transition::SetMac { port, .. } => Some(*port),
            _ => None,
        };
        if let Some(a) = anchor
            && !anchors.contains(&a)
        {
            anchors.push(a);
        }
    }
    anchors
}

fn log_plan(observed: &ObservedTopology, plan: &Plan) {
    tracing::debug!(
        dpnis = observed.dpnis.len(),
        dpmacs = observed.dpmacs.len(),
        transitions = plan.transitions.len(),
        "observed state and computed plan"
    );
    for d in &plan.drift {
        tracing::warn!(dpni = %d.dpni, attribute = %d.attribute, detail = %d.detail, "immutable drift refused");
    }
    for a in &plan.assertions {
        tracing::warn!(port = %a.port, field = %a.field, detail = %a.detail, "assert-only mismatch");
    }
}

#[cfg(test)]
mod tests {
    use dpaa2_api::families::dprc::Options;
    use dpaa2_api::intent::compiled::{Container, ProvenanceKey};
    use dpaa2_api::plan::dprc::{ConsumerContainer, ContainerPlan, ContainerVerdict};

    use super::*;

    /// A declared container with no matching convergence (count drift) is a loud
    /// `Error::Backend`, not a silent zip truncation (synthesis row 9).
    #[test]
    fn pair_containers_rejects_mismatched_lengths() {
        let convergence = ConsumerConvergence {
            container: ConsumerContainer {
                tenant: "router".into(),
                label: "router".into(),
                options: Options::DEFAULT,
                placement: Container::Root,
                provenance: ProvenanceKey::new("router", "dprc", ""),
            },
            plan: ContainerPlan::new(),
            verdict: ContainerVerdict::Converged,
        };
        // One declared convergence, zero paired objects: the undispatched-container drift.
        let err = pair_containers(&[], std::slice::from_ref(&convergence))
            .expect_err("mismatched lengths must be a loud error");
        assert!(matches!(err, Error::Backend(_)));
    }

    /// The root-reconcile link pass at the engine seam, driven through the in-memory fake
    /// (cross-dprc-links task 5.2): dpni↔dpni links wire in every container arrangement —
    /// root↔root (create + connect + plug + idempotent re-run), root↔child, and child↔child — the
    /// disruption gate refuses below `Disruptive`, and a held end is a typed rewire refusal.
    mod links {
        use dpaa2_api::contract::fake::FakeBackend;
        use dpaa2_api::core::model::ObjectRef;
        use dpaa2_api::families::dpni::DpniCfg;
        use dpaa2_api::families::dprc::Options;
        use dpaa2_api::intent::refuse::{Compiled, compile};
        use dpaa2_api::intent::{
            Dataplane, Intent, Isolation, Link, Tenant, TenantRef, kernel_tenant,
        };
        use dpaa2_api::testkit::ref_inventory;

        use super::*;

        const LINK: &str = "l0";

        fn link_cfg(allow: Class) -> ConvergeConfig {
            ConvergeConfig {
                deadline: Duration::from_secs(5),
                poll_interval: Duration::ZERO,
                prune: false,
                allow,
            }
        }

        fn isolated(name: TenantName) -> Tenant {
            Tenant {
                name,
                dataplane: Dataplane::UserspacePoll,
                max_cores: 16,
                isolation: Isolation::Isolated,
                renamed: None,
                priority: None,
            }
        }

        fn link(a: TenantRef, b: TenantRef) -> Link {
            Link {
                name: LINK.into(),
                interface_a: a,
                interface_b: b,
                renamed: None,
            }
        }

        fn compiled(tenants: Vec<Tenant>, l: Link) -> Compiled {
            let intent = Intent {
                tenants,
                links: vec![l],
                ..Intent::empty()
            };
            compile(&intent, &ref_inventory(16)).expect("the link intent compiles")
        }

        // root↔root: the kernel end plus a restricted-to-kernel tenant (its dataplane must match the
        // kernel holder), so both link-end dpnis land in dprc.1 sharing the link-name label.
        fn compiled_root_root() -> Compiled {
            let secondary = Tenant {
                name: "sec".into(),
                dataplane: Dataplane::KernelNetlink,
                max_cores: 16,
                isolation: Isolation::Restricted {
                    pool: "kernel".into(),
                },
                renamed: None,
                priority: None,
            };
            compiled(
                vec![kernel_tenant(16), secondary],
                link(TenantRef::Kernel, TenantRef::from_name("sec".into())),
            )
        }

        fn link_label() -> ConstructName {
            ConstructName::from(LINK)
        }

        fn link_rows(mc: &FakeBackend, scope: Option<DprcId>) -> Vec<ObjectRef> {
            mc.observe_pool(scope, Family::Dpni)
                .unwrap()
                .into_iter()
                .filter(|r| r.label.as_str() == LINK)
                .map(|r| r.object)
                .collect()
        }

        fn plugged_ref(mc: &FakeBackend, object: ObjectRef) -> bool {
            mc.observe_pool(None, Family::Dpni)
                .unwrap()
                .iter()
                .any(|r| r.object == object && r.plugged)
        }

        fn connection(mc: &FakeBackend, object: ObjectRef) -> Option<ObjectRef> {
            mc.dprc_get_connection(DpniId::new(object.ordinal()))
                .unwrap()
        }

        // Registers an observed child container and seeds one link-end dpni in it, as the child
        // population would have (Half A), returning the seeded dpni's ref.
        fn seed_child_end(mc: &FakeBackend, tenant: &str) -> (DprcId, ObjectRef) {
            let label = ConstructName::from(tenant);
            let child = mc
                .dprc_create(DprcId::ROOT, Options::DEFAULT, &label)
                .expect("child container");
            let id = mc
                .create_dpni_in(child, &DpniCfg::defaults(), &link_label())
                .expect("seed child link dpni");
            (child, ObjectRef::new(Family::Dpni, id.into_inner()))
        }

        #[test]
        fn root_to_root_creates_connects_plugs_and_is_idempotent() {
            // The headline: an empty board wires a root↔root link — two root dpnis created,
            // connected at the ancestor, both plugged — and a second pass actuates nothing.
            let compiled = compiled_root_root();
            let mc = FakeBackend::new();

            assert_eq!(
                converge_links(&compiled.plan, &mc, link_cfg(Class::Disruptive)).unwrap(),
                LinkOutcome::Converged
            );
            let ends = link_rows(&mc, None);
            assert_eq!(ends.len(), 2, "both root link dpnis created");
            assert_eq!(
                connection(&mc, ends[0]),
                Some(ends[1]),
                "wired to each other"
            );
            assert_eq!(
                connection(&mc, ends[1]),
                Some(ends[0]),
                "symmetric endpoint"
            );
            assert!(
                plugged_ref(&mc, ends[0]) && plugged_ref(&mc, ends[1]),
                "both ends plugged after the connect"
            );

            // Idempotent re-run: no third dpni, no re-connect.
            assert_eq!(
                converge_links(&compiled.plan, &mc, link_cfg(Class::Disruptive)).unwrap(),
                LinkOutcome::Converged
            );
            assert_eq!(link_rows(&mc, None).len(), 2, "no third root dpni");
        }

        #[test]
        fn root_to_child_connects_at_the_ancestor() {
            // A root↔child link: the child end is already resident (population's), the root end is
            // created here and the two connect at the ancestor.
            let compiled = compiled(
                vec![kernel_tenant(16), isolated("neta".into())],
                link(TenantRef::Kernel, TenantRef::from_name("neta".into())),
            );
            let mc = FakeBackend::new();
            let (_child, child_end) = seed_child_end(&mc, "neta");

            assert_eq!(
                converge_links(&compiled.plan, &mc, link_cfg(Class::Disruptive)).unwrap(),
                LinkOutcome::Converged
            );
            let root_ends = link_rows(&mc, None);
            assert_eq!(root_ends.len(), 1, "the root end is created");
            assert_eq!(
                connection(&mc, root_ends[0]),
                Some(child_end),
                "root end wired to the child end at the ancestor"
            );
            assert!(plugged_ref(&mc, root_ends[0]), "the root end is plugged");
        }

        #[test]
        fn child_to_child_connects_both_resident_ends() {
            // A child↔child link: both ends are already resident, so nothing is created — the pass
            // connects the two child dpnis at the ancestor.
            let compiled = compiled(
                vec![isolated("neta".into()), isolated("netb".into())],
                link(
                    TenantRef::from_name("neta".into()),
                    TenantRef::from_name("netb".into()),
                ),
            );
            let mc = FakeBackend::new();
            let (_a, a_end) = seed_child_end(&mc, "neta");
            let (_b, b_end) = seed_child_end(&mc, "netb");

            assert_eq!(
                converge_links(&compiled.plan, &mc, link_cfg(Class::Disruptive)).unwrap(),
                LinkOutcome::Converged
            );
            assert!(link_rows(&mc, None).is_empty(), "no root dpni created");
            assert_eq!(
                connection(&mc, a_end),
                Some(b_end),
                "the two child ends wired"
            );
            assert_eq!(connection(&mc, b_end), Some(a_end), "symmetric endpoint");
        }

        #[test]
        fn below_disruptive_is_refused_and_changes_nothing() {
            // A root↔root link needs two creates, so the headline is Disruptive; a Hitless allow
            // refuses and nothing is created.
            let compiled = compiled_root_root();
            let mc = FakeBackend::new();
            assert_eq!(
                converge_links(&compiled.plan, &mc, link_cfg(Class::Hitless)).unwrap(),
                LinkOutcome::DisruptionRefused {
                    headline: Class::Disruptive,
                    allowed: Class::Hitless,
                }
            );
            assert!(
                link_rows(&mc, None).is_empty(),
                "nothing created under refusal"
            );
        }

        #[test]
        fn an_end_held_by_another_peer_is_a_rewire_refusal() {
            // DPRC-I5: a root end already wired to a foreign peer is a typed rewire refusal, and the
            // pass changes nothing — no silent reconnect.
            let compiled = compiled_root_root();
            let mc = FakeBackend::new();
            let a = mc.create_dpni(&link_label(), &DpniCfg::defaults()).unwrap();
            let _b = mc.create_dpni(&link_label(), &DpniCfg::defaults()).unwrap();
            let foreign = mc
                .create_dpni(&ConstructName::from("foreign"), &DpniCfg::defaults())
                .unwrap();
            let foreign_ref = ObjectRef::new(Family::Dpni, foreign.into_inner());
            mc.dprc_connect(CONNECT_ANCESTOR, a, foreign_ref).unwrap();

            let outcome = converge_links(&compiled.plan, &mc, link_cfg(Class::Disruptive)).unwrap();
            match outcome {
                LinkOutcome::RewireRefused { refusals } => {
                    assert_eq!(refusals.len(), 1);
                    assert_eq!(refusals[0].observed_peer, foreign_ref);
                }
                other => panic!("expected a rewire refusal, got {other:?}"),
            }
            assert_eq!(
                connection(&mc, ObjectRef::new(Family::Dpni, a.into_inner())),
                Some(foreign_ref),
                "the held end is not silently rewired"
            );
        }

        #[test]
        fn a_mixed_edge_foreign_held_end_refuses_before_any_create() {
            // PASS2-F1: one root end exists under the link label wired to a foreign peer, its peer
            // not yet created — the mixed edge refuses before the missing dpni is minted (DPRC-I5).
            let compiled = compiled_root_root();
            let mc = FakeBackend::new();
            let a = mc.create_dpni(&link_label(), &DpniCfg::defaults()).unwrap();
            let foreign = mc
                .create_dpni(&ConstructName::from("foreign"), &DpniCfg::defaults())
                .unwrap();
            let foreign_ref = ObjectRef::new(Family::Dpni, foreign.into_inner());
            mc.dprc_connect(CONNECT_ANCESTOR, a, foreign_ref).unwrap();
            let before = mc.observe_pool(None, Family::Dpni).unwrap().len();

            let outcome = converge_links(&compiled.plan, &mc, link_cfg(Class::Disruptive)).unwrap();
            match outcome {
                LinkOutcome::RewireRefused { refusals } => {
                    assert_eq!(refusals.len(), 1);
                    assert_eq!(refusals[0].observed_peer, foreign_ref);
                    assert_eq!(
                        refusals[0].planned_peer, None,
                        "the planned peer is to-create"
                    );
                }
                other => panic!("expected a rewire refusal, got {other:?}"),
            }
            assert_eq!(
                mc.observe_pool(None, Family::Dpni).unwrap().len(),
                before,
                "no dpni is created under the refusal"
            );
        }
    }

    /// The bound-child post-bind healing at the engine seam (cross-dprc-links task 5.3): a child
    /// bound to vfio-fsl-mc whose plan still needs residents is healed under a Disruptive consent —
    /// its pending creates dispatched, ONE rebind cycle, the post-rebind re-observation the sole
    /// verdict (ADR-0017 decision 2/3) — and is a typed drift refusal under a lesser allow.
    mod healing {
        use dpaa2_api::contract::fake::FakeBackend;
        use dpaa2_api::core::model::{DpmacId, MacMode};
        use dpaa2_api::families::pool_lifecycle::RawDriver;
        use dpaa2_api::intent::refuse::{Compiled, compile};
        use dpaa2_api::intent::{Dataplane, Intent, Isolation, Port, Tenant, TenantRef};
        use dpaa2_api::testkit::ref_inventory;

        use super::*;

        fn compiled_router() -> Compiled {
            let router = Tenant {
                name: "router".into(),
                dataplane: Dataplane::UserspacePoll,
                max_cores: 16,
                isolation: Isolation::Isolated,
                renamed: None,
                priority: None,
            };
            let port = |name: ConstructName, dpmac: u32| Port {
                name,
                dpmac: DpmacId::new(dpmac),
                rate: 10_000,
                tenant: TenantRef::from_name("router".into()),
                mac: None,
                mac_mode: MacMode::Assert,
                renamed: None,
            };
            let intent = Intent {
                tenants: vec![router],
                ports: vec![port("wan0".into(), 7), port("wan1".into(), 9)],
                ..Intent::empty()
            };
            compile(&intent, &ref_inventory(16)).expect("the router intent compiles")
        }

        fn disruptive() -> ConvergeConfig {
            ConvergeConfig {
                allow: Class::Disruptive,
                ..ConvergeConfig::default()
            }
        }

        fn count(mc: &FakeBackend, family: Family) -> usize {
            mc.observe_pool(Some(DprcId::new(2)), family).unwrap().len()
        }

        fn vfio_unbinds(mc: &FakeBackend) -> usize {
            mc.audit()
                .iter()
                .filter(|e| e.starts_with("vfio_unbind:"))
                .count()
        }

        // A reference router whose child container is created and already bound to vfio-fsl-mc, so
        // its empty plan reads bound-and-unconverged (the ADR-0017 post-bind drift the heal targets).
        fn bound_empty_child() -> (Compiled, FakeBackend) {
            let compiled = compiled_router();
            let mc =
                FakeBackend::new().with_bound_dprc(DprcId::new(2), RawDriver::from("vfio-fsl-mc"));
            converge_containers(&compiled.plan, &mc, disruptive()).expect("container");
            (compiled, mc)
        }

        #[test]
        fn consent_heals_a_bound_child_in_one_rebind_cycle() {
            let (compiled, mc) = bound_empty_child();
            assert_eq!(
                converge_population(&compiled.plan, &mc, &mc, disruptive()).unwrap(),
                PopulationOutcome::Converged
            );
            assert_eq!(
                count(&mc, Family::Dpbp),
                2,
                "the trio resident is healed (ADR board case)"
            );
            assert_eq!(
                count(&mc, Family::Dpni),
                2,
                "the dpni rides the same child-keyed path"
            );
            assert_eq!(count(&mc, Family::Dpcon), 10);
            assert_eq!(vfio_unbinds(&mc), 1, "exactly one rebind cycle");
        }

        #[test]
        fn a_lesser_allow_refuses_with_the_typed_child_residue() {
            let (compiled, mc) = bound_empty_child();
            match converge_population(&compiled.plan, &mc, &mc, ConvergeConfig::default()).unwrap()
            {
                PopulationOutcome::DriftRefused { label, residue } => {
                    assert_eq!(label.as_str(), "router");
                    assert!(residue.to_string().contains("kernel-invisible"));
                }
                other => panic!("expected a declined-consent drift refusal, got {other:?}"),
            }
            assert_eq!(
                count(&mc, Family::Dpni),
                0,
                "nothing created under a declined consent"
            );
            assert_eq!(count(&mc, Family::Dpbp), 0);
            assert_eq!(
                vfio_unbinds(&mc),
                0,
                "no rebind cycle under a declined consent"
            );
        }

        #[test]
        fn a_healed_child_re_run_issues_nothing() {
            let (compiled, mc) = bound_empty_child();
            converge_population(&compiled.plan, &mc, &mc, disruptive()).unwrap();
            assert_eq!(
                converge_population(&compiled.plan, &mc, &mc, disruptive()).unwrap(),
                PopulationOutcome::Converged
            );
            assert_eq!(count(&mc, Family::Dpni), 2, "no resident re-created");
            assert_eq!(count(&mc, Family::Dpbp), 2);
            assert_eq!(
                vfio_unbinds(&mc),
                1,
                "no second rebind cycle on the idempotent re-run"
            );
        }
    }

    /// Root-scope pool convergence at the engine seam, driven through the in-memory fake
    /// (pool-objects task 3.4): the phase-1 laws — grow, free-only shrink, prune,
    /// shrink-below-draw refusal, the disruption gate, and idempotence — exercised offline.
    mod pool {
        use dpaa2_api::contract::fake::FakeBackend;
        use dpaa2_api::core::model::{MacMode, ObjectRef};
        use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, PoolDisposition, RawLabel};
        use dpaa2_api::intent::refuse::{Compiled, compile};
        use dpaa2_api::intent::{Intent, Port, TenantRef, kernel_tenant};
        use dpaa2_api::testkit::ref_inventory;

        use super::*;

        // The reserved kernel tenant terminating one 25G port: its dpni and pool companions
        // compile into the root container (Tenant::container ⇒ Root), so the plan carries a
        // real per-family root requirement to converge.
        fn compiled_kernel() -> Compiled {
            let intent = Intent {
                tenants: vec![kernel_tenant(16)],
                ports: vec![Port {
                    name: "lan0".into(),
                    dpmac: DpmacId::new(4),
                    rate: 25_000,
                    tenant: TenantRef::from_name("kernel".into()),
                    mac: None,
                    mac_mode: MacMode::Assert,
                    renamed: None,
                }],
                ..Intent::empty()
            };
            compile(&intent, &ref_inventory(16)).expect("kernel intent compiles")
        }

        fn pool_cfg() -> ConvergeConfig {
            ConvergeConfig {
                deadline: Duration::from_secs(5),
                poll_interval: Duration::ZERO,
                prune: false,
                // Pool creates/destroys are disruptive; the convergence tests allow it.
                allow: Class::Disruptive,
            }
        }

        fn count(mc: &FakeBackend, family: Family) -> i64 {
            i64::try_from(mc.observe_pool(None, family).unwrap().len()).unwrap()
        }

        // Seeds a plugged root dpbp; `drawn` sets the orthogonal draw facet a kernel-face would observe (pool-objects design D10).
        fn seeded(ord: u32, label: RawLabel, drawn: bool) -> ObservedPoolObject {
            ObservedPoolObject {
                object: ObjectRef::new(Family::Dpbp, ord),
                label,
                plugged: true,
                drawn,
            }
        }

        #[test]
        fn converge_pools_grows_root_to_the_derived_counts_and_is_idempotent() {
            let compiled = compiled_kernel();
            let mc = FakeBackend::new().with_inventory(ref_inventory(16));

            assert_eq!(
                converge_pools(&compiled.plan, &mc, pool_cfg(), PoolPass::Grow).unwrap(),
                PoolOutcome::Converged
            );
            // Each trio family reads back its derived count; dpio reads back its seat count.
            for family in POOL_TRIO {
                let req = derived_requirement(&compiled.plan, &Container::Root, family);
                assert_eq!(count(&mc, family.family()), req, "{}", family.name());
            }
            assert_eq!(
                count(&mc, Family::Dpio),
                derived_seats(&compiled.plan, &Container::Root)
            );

            // A second pass over the converged root creates nothing (idempotent).
            let before: Vec<i64> = [Family::Dpmcp, Family::Dpbp, Family::Dpcon, Family::Dpio]
                .map(|f| count(&mc, f))
                .to_vec();
            assert_eq!(
                converge_pools(&compiled.plan, &mc, pool_cfg(), PoolPass::Grow).unwrap(),
                PoolOutcome::Converged
            );
            let after: Vec<i64> = [Family::Dpmcp, Family::Dpbp, Family::Dpcon, Family::Dpio]
                .map(|f| count(&mc, f))
                .to_vec();
            assert_eq!(before, after, "a converged root is not grown a second time");
        }

        #[test]
        fn converge_pools_prunes_a_foreign_free_root_object() {
            let compiled = compiled_kernel();
            // Never-plugged ⇒ the root prune target (ADR-0020 decision 4).
            let foreign = ObservedPoolObject {
                object: ObjectRef::new(Family::Dpbp, 99),
                label: RawLabel::from("vendor"),
                plugged: false,
                drawn: false,
            };
            let mc = FakeBackend::new()
                .with_inventory(ref_inventory(16))
                .with_pool_object(DprcId::ROOT, foreign.clone());

            // Grow-first tops up the deficit; shrink-last reclaims the foreign object (pool-objects design D10).
            assert_eq!(
                converge_pools(&compiled.plan, &mc, pool_cfg(), PoolPass::Grow).unwrap(),
                PoolOutcome::Converged
            );
            assert_eq!(
                converge_pools(&compiled.plan, &mc, pool_cfg(), PoolPass::Shrink).unwrap(),
                PoolOutcome::Converged
            );
            let dpbps = mc.observe_pool(None, Family::Dpbp).unwrap();
            assert!(
                !dpbps.iter().any(|o| o.object == foreign.object),
                "the foreign-free dpbp is reclaimed"
            );
            assert_eq!(
                count(&mc, Family::Dpbp),
                derived_requirement(&compiled.plan, &Container::Root, PoolFamily::Dpbp)
            );
        }

        /// Root grow-only: a managed surplus is never reclaimed; it renders as reboot-required residue (ADR-0020; rootSurplusResidueTest).
        #[test]
        fn converge_pools_reports_a_root_managed_surplus_as_residue() {
            let compiled = compiled_kernel();
            let req = derived_requirement(&compiled.plan, &Container::Root, PoolFamily::Dpbp);
            let mut mc = FakeBackend::new().with_inventory(ref_inventory(16));
            for ord in 0..u32::try_from(req + 2).unwrap() {
                mc = mc.with_pool_object(DprcId::ROOT, seeded(ord, RawLabel::from(KERNEL), false));
            }

            assert_eq!(
                converge_pools(&compiled.plan, &mc, pool_cfg(), PoolPass::Shrink).unwrap(),
                PoolOutcome::Converged
            );
            assert_eq!(
                count(&mc, Family::Dpbp),
                req + 2,
                "root surplus is never reclaimed at runtime"
            );

            let drift = plan_pools(&compiled.plan, &mc).unwrap();
            let dpbp = drift
                .families
                .iter()
                .find(|f| f.family == PoolFamily::Dpbp)
                .expect("dpbp drift");
            assert_eq!(
                dpbp.residue(),
                PoolDisposition::RebootRequired(dpaa2_api::families::pool_lifecycle::PoolResidue {
                    family: PoolFamily::Dpbp,
                    observed: req + 2,
                    required: req,
                })
            );
            let text = crate::render::render_pool_drift(&compiled.plan, &drift);
            assert!(text.contains("reboot-required"), "{text}");
        }

        /// Root drawn-ness is unobservable, so a requirement below draw NEVER refuses at root — the below-draw refusal is child-scoped (ADR-0020 decision 3).
        #[test]
        fn converge_pools_never_refuses_below_draw_at_root() {
            let compiled = compiled_kernel();
            let req = derived_requirement(&compiled.plan, &Container::Root, PoolFamily::Dpbp);
            let mut mc = FakeBackend::new().with_inventory(ref_inventory(16));
            for ord in 0..u32::try_from(req + 1).unwrap() {
                mc = mc.with_pool_object(DprcId::ROOT, seeded(ord, RawLabel::from(KERNEL), true));
            }

            match converge_pools(&compiled.plan, &mc, pool_cfg(), PoolPass::Shrink).unwrap() {
                PoolOutcome::Converged => {}
                other => panic!("root never refuses below draw, got {other:?}"),
            }
            assert_eq!(count(&mc, Family::Dpbp), req + 1);

            let drift = plan_pools(&compiled.plan, &mc).unwrap();
            assert!(drift.shrink_refusal().is_none(), "root raises no refusal");
            let text = crate::render::render_pool_drift(&compiled.plan, &drift);
            assert!(!text.contains("REFUSED"), "{text}");
            assert!(text.contains("reboot-required"), "{text}");
        }

        #[test]
        fn converge_pools_refuses_a_disruptive_pass_on_a_hitless_run() {
            let compiled = compiled_kernel();
            let mc = FakeBackend::new().with_inventory(ref_inventory(16));
            let cfg = ConvergeConfig {
                allow: Class::Hitless,
                ..pool_cfg()
            };
            assert_eq!(
                converge_pools(&compiled.plan, &mc, cfg, PoolPass::Grow).unwrap(),
                PoolOutcome::DisruptionRefused {
                    headline: Class::Disruptive,
                    allowed: Class::Hitless,
                }
            );
            assert!(
                mc.observe_pool(None, Family::Dpbp).unwrap().is_empty(),
                "a refused run actuates nothing"
            );
        }

        #[test]
        fn plan_pools_reports_drift_read_only() {
            let compiled = compiled_kernel();
            let mc = FakeBackend::new().with_inventory(ref_inventory(16));
            let drift = plan_pools(&compiled.plan, &mc).unwrap();

            assert_eq!(drift.families.len(), POOL_TRIO.len());
            // A fresh board needs grows, so the headline is disruptive.
            assert_eq!(drift.headline(), Class::Disruptive);
            assert!(drift.shrink_refusal().is_none());
            // Read-only: the census dispatched nothing.
            assert_eq!(
                mc.observe_pool(None, Family::Dpbp).unwrap(),
                [] as [dpaa2_api::families::pool_lifecycle::ObservedPoolObject; 0]
            );
            // The render names the pass and the families.
            let text = crate::render::render_pool_drift(&compiled.plan, &drift);
            assert!(text.contains("root pool convergence"), "{text}");
            assert!(text.contains("dpio seats"), "{text}");
        }
    }

    /// Child-population convergence at the engine seam (pool-objects design D10): a probe-
    /// discovered below-draw surfaces as the typed [`PopulationOutcome::ShrinkRefused`] on the
    /// operator surface, not an [`Error`].
    mod population {
        use std::collections::BTreeMap;

        use dpaa2_api::contract::fake::FakeBackend;
        use dpaa2_api::core::model::{DpmacId, MacMode, ObjectRef};
        use dpaa2_api::families::dprc::ContainerState;
        use dpaa2_api::families::pool_lifecycle::{
            ObservedPoolObject, RawLabel, derived_requirement,
        };
        use dpaa2_api::intent::refuse::{Compiled, compile};
        use dpaa2_api::intent::{Dataplane, Intent, Isolation, Port, Tenant, TenantRef};
        use dpaa2_api::plan::dprc::ObservedContainer;
        use dpaa2_api::testkit::ref_inventory;

        use super::*;

        // The reference userspace-poll router (two 10G ports, T = 5), whose objects compile
        // into a child container.
        fn compiled_router() -> Compiled {
            let router = Tenant {
                name: "router".into(),
                dataplane: Dataplane::UserspacePoll,
                max_cores: 16,
                isolation: Isolation::Isolated,
                renamed: None,
                priority: None,
            };
            let port = |name: ConstructName, dpmac: u32| Port {
                name,
                dpmac: DpmacId::new(dpmac),
                rate: 10_000,
                tenant: TenantRef::from_name("router".into()),
                mac: None,
                mac_mode: MacMode::Assert,
                renamed: None,
            };
            let intent = Intent {
                tenants: vec![router],
                ports: vec![
                    port(ConstructName::from("wan0"), 7),
                    port(ConstructName::from("wan1"), 9),
                ],
                ..Intent::empty()
            };
            compile(&intent, &ref_inventory(16)).expect("intent compiles")
        }

        fn pop_cfg() -> ConvergeConfig {
            ConvergeConfig {
                deadline: Duration::from_secs(5),
                poll_interval: Duration::ZERO,
                prune: false,
                allow: Class::Disruptive,
            }
        }

        #[test]
        fn child_discovered_draw_is_a_typed_shrink_refused_outcome() {
            let compiled = compiled_router();
            let child = DprcId::new(2);
            let container = Container::Child("router".into());
            let req = derived_requirement(&compiled.plan, &container, PoolFamily::Dpbp);

            // A created, unbound child labelled "router" plus req+1 in-use dpbp: the census
            // reads them free (restool-shaped), so the plan emits a destroy the probe refuses.
            let mut mc = FakeBackend::new().with_container(
                child,
                ObservedContainer {
                    state: ContainerState::Created,
                    options: Options::DEFAULT,
                    label: ConstructName::from("router"),
                    placement: Container::Child("router".into()),
                    residents: BTreeMap::new(),
                },
            );
            for ord in 0..=req {
                mc = mc.with_in_use_pool_object(
                    child,
                    ObservedPoolObject {
                        object: ObjectRef::new(Family::Dpbp, u32::try_from(ord).unwrap()),
                        label: RawLabel::from("router"),
                        plugged: true,
                        drawn: false,
                    },
                );
            }

            match converge_population(&compiled.plan, &mc, &mc, pop_cfg()).unwrap() {
                PopulationOutcome::ShrinkRefused { label, refusal } => {
                    assert_eq!(label.as_str(), "router");
                    assert_eq!(refusal.family, PoolFamily::Dpbp);
                    assert_eq!(refusal.requirement, req);
                    assert_eq!(refusal.drawn, req + 1);
                }
                other => panic!("expected a typed ShrinkRefused, got {other:?}"),
            }
        }
    }
}
