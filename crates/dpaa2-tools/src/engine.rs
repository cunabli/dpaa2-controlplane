//! The imperative shell: observe → reconcile → act → wait → re-observe (design D0; add-dpaa2-provisioning).
//!
//! This is the "imperative shell" wrapped around the pure core. It is generic over
//! the [`McControl`]/[`KernelControl`] trait seams so the whole convergence loop runs
//! against the in-memory fake with no board (design D10; restool-baseline). Actuation resolves the
//! DPNI index for a freshly-created port from the id the MC assigned this pass, and
//! for existing ports from the observed connection edge (design D1; restool-baseline).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::thread::sleep;
use std::time::{Duration, Instant};

use dpaa2_api::contract::{KernelControl, McControl};
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::inventory::{Ceiling, Inventory};
use dpaa2_api::core::model::{DesiredTopology, DpmacId, DpniId, DprcId, ObservedTopology};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpio::derived_seats;
use dpaa2_api::families::dprc::Options;
use dpaa2_api::families::pool_lifecycle::{
    PoolCensus, PoolDeltas, PoolFamily, ShrinkBelowDraw, census_of, derived_requirement,
    drift_disposition,
};
use dpaa2_api::intent::KERNEL;
use dpaa2_api::intent::compiled::{Attributes, CompiledPlan, Container, PlannedObject};
use dpaa2_api::plan::dprc::{
    Attribution, ConsumerConvergence, ContainerPlan, ContainerStep, ContainerVerdict, PruneBucket,
    PruneItem, Verb, attribute_refusal, derive_consumer_containers, plan_consumer_convergence,
    plan_prune, verdict,
};
use dpaa2_api::plan::reconcile::{ReconcileOptions, reconcile_with};
use dpaa2_api::plan::{Class, Plan, Transition};
use dpaa2_mc::{default_dpio_cfg, dispatch_pool_deltas};

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
/// # Errors
/// Propagates a backend read/dispatch error, and reports a survivor (a pruned container
/// still observed) as an [`Error::Backend`].
pub fn prune_containers<M: McControl>(
    plan: &CompiledPlan,
    mc: &M,
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
/// change's scope (companion/dpni are tiles #5/#6) and is an error, not a silent no-op.
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
            "container-only convergence emits no {other:?} (companion/dpni are tiles #5/#6)"
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

/// One root pool family's drift — its observed census, derived requirement, and the
/// count-level disposition [`drift_disposition`] would take (pool-objects task 3.4). The
/// read seam `dry-run`/`status` render, carried as data (the frontend owns its text;
/// restool-baseline design D11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolFamilyDrift {
    /// The pooled family.
    pub family: PoolFamily,
    /// The observed census folded from the root's rows (the conservative restool proxy).
    pub census: PoolCensus,
    /// The plan-derived requirement (ADR-0012 counts, consumed unchanged).
    pub required: i64,
    /// The disposition the family would take, or the below-draw refusal (pool-objects design D3).
    pub disposition: Result<PoolDeltas, ShrinkBelowDraw>,
}

/// The root-scope pool drift across the trio plus the dpio seats — the read the pool
/// convergence phase and the `dry-run`/`status` surfaces share (pool-objects task 3.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolDrift {
    /// Per-trio-family drift, in traversal order (dpmcp→dpbp→dpcon; pool-objects design D8).
    pub families: Vec<PoolFamilyDrift>,
    /// The derived dpio seat count for the root (`derived_seats`).
    pub dpio_required: i64,
    /// The observed dpio seat count in the root.
    pub dpio_observed: i64,
}

impl PoolDrift {
    /// The pass headline (ADR-0015 decision 12): [`Class::Disruptive`] when any trio family
    /// needs a create/destroy/prune or a dpio seat is short, else [`Class::Hitless`] — the
    /// class the run gates on. A pool create/destroy is disruptive, matching the container
    /// steps' class-gating.
    #[must_use]
    pub fn headline(&self) -> Class {
        let trio_work = self
            .families
            .iter()
            .any(|f| f.disposition.is_ok_and(|d| !d.is_empty()));
        if trio_work || self.dpio_required > self.dpio_observed {
            Class::Disruptive
        } else {
            Class::Hitless
        }
    }

    /// The first below-draw refusal, if any (pool-objects design D3): a requirement under the drawn count
    /// surfaces to the operator by name and count, never a teardown of a live consumer.
    #[must_use]
    pub fn shrink_refusal(&self) -> Option<ShrinkBelowDraw> {
        self.families.iter().find_map(|f| f.disposition.err())
    }
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
/// ceiling is unlistable (ADR-0011; consistent with the `populate_child` gap, bead
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
        let disposition =
            drift_disposition(family, census, required, &ceiling_of(&inventory, family));
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
/// task 3.4; pool-objects design D3), mirroring [`populate_child`](dpaa2_mc::populate_child) at root scope:
/// census → disposition → dispatch → read-back verdict, composing only the phase-2/3 pure
/// functions and the [`dispatch_pool_deltas`] edge (no new policy).
///
/// The pass, in order:
/// - Reads the whole drift ([`plan_pools`]). A family whose requirement fell below its drawn
///   count surfaces as [`PoolOutcome::ShrinkRefused`] before anything is dispatched — a
///   free-only shrink never tears down a live consumer (pool-objects design D3).
/// - Gates the deltas' headline against `cfg.allow` (ADR-0015 decision 12): a pool
///   create/destroy/prune is [`Class::Disruptive`], so a hitless run refuses it, matching the
///   container steps' class-gating.
/// - Dispatches each trio family in traversal order (dpmcp→dpbp→dpcon; pool-objects design D8) through
///   [`dispatch_pool_deltas`] — free-only victim selection, prune riding `PoolDeltas::prune`
///   as the disposition emits it — and judges convergence by the post-dispatch census
///   read-back ([`PoolCensus::converged`]).
/// - Grows the dpio seat deficit plain (`dpio_create`, NOT `create_dpio_seat`): the
///   dpio→dpmcp probe pairing is the kernel dpio driver's own draw, and the dpmcp pool the
///   trio just converged supplies it (pool-objects design D4). Seats are grown, never shrunk,
///   as [`populate_child`](dpaa2_mc::populate_child) does.
///
/// Idempotent and level-triggered: a converged root yields an empty-headline drift and
/// returns [`PoolOutcome::Converged`] without dispatching, and a second pass over it does too.
///
/// # Errors
/// Propagates a backend read/dispatch error, and reports a family that failed to reach its
/// requirement after dispatch as an [`Error::Backend`] (the container-convergence precedent;
/// the operator re-runs, level-triggered — e.g. after a ceiling-capped grow).
pub fn converge_pools<M: McControl>(
    plan: &CompiledPlan,
    mc: &M,
    cfg: ConvergeConfig,
) -> Result<PoolOutcome, Error> {
    let drift = plan_pools(plan, mc)?;

    // A below-draw requirement is the one refusal, surfaced before any dispatch (pool-objects design D3).
    if let Some(refusal) = drift.shrink_refusal() {
        tracing::error!(
            family = refusal.family.name(),
            requirement = refusal.requirement,
            drawn = refusal.drawn,
            "root pool requirement below drawn count"
        );
        return Ok(PoolOutcome::ShrinkRefused { refusal });
    }

    // Gate on the headline before touching the board (ADR-0015 decision 12).
    let headline = drift.headline();
    if headline > cfg.allow {
        tracing::error!(%headline, allowed = %cfg.allow, "root pool convergence exceeds allowed disruption class");
        return Ok(PoolOutcome::DisruptionRefused {
            headline,
            allowed: cfg.allow,
        });
    }
    if headline == Class::Hitless {
        // Nothing to do: a converged root (or one only awaiting kernel-side draw).
        return Ok(PoolOutcome::Converged);
    }

    let declared = root_declared(plan);

    // trio: dispatch each family's deltas and judge by post-dispatch read-back.
    for f in &drift.families {
        // Every disposition is Ok here: shrink_refusal ruled out every Err above.
        let Ok(deltas) = f.disposition else {
            continue;
        };
        if deltas.is_empty() {
            continue;
        }
        let label = root_family_label(plan, f.family.family());
        let dispatch = dispatch_pool_deltas(mc, None, f.family, deltas, &label, &declared)?;
        let after = census_of(&dispatch.after, &declared);
        if !after.converged(f.required) {
            return Err(Error::Backend(format!(
                "root {} pool did not converge after dispatch: managed {} of required {}",
                f.family.name(),
                after.managed(),
                f.required
            )));
        }
    }

    // dpio seats: grow the deficit plain (the dpmcp probe pairing is the kernel driver's own
    // draw; pool-objects design D4). Grown, never shrunk — the populate_child convention.
    let deficit = (drift.dpio_required - drift.dpio_observed).max(0);
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

    Ok(PoolOutcome::Converged)
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

    loop {
        let observed = observe(mc, kernel)?;
        let plan = reconcile_with(desired, &observed, opts);
        log_plan(&observed, &plan);

        if plan.is_converged() {
            tracing::info!("converged");
            return Ok(Outcome::Converged);
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

        apply(&plan, &observed, mc, kernel)?;
        sleep(cfg.poll_interval);
    }
}

/// Applies a plan's transitions once. Wait-only transitions (`Bind`) merely nudge
/// the kernel; the loop re-observes to detect the resulting netdev.
///
/// # Errors
/// Returns an error if any actuation fails.
pub fn apply<M: McControl, K: KernelControl>(
    plan: &Plan,
    observed: &ObservedTopology,
    mc: &M,
    kernel: &K,
) -> Result<(), Error> {
    // DPNIs created during this pass, keyed by their destination DPMAC.
    let mut created: HashMap<DpmacId, DpniId> = HashMap::new();

    for t in &plan.transitions {
        match t {
            Transition::Create { port, label, cfg } => {
                let id = mc.create_dpni(label, cfg)?;
                created.insert(*port, id);
                tracing::info!(%port, %id, %label, "created dpni");
            }
            Transition::Connect { port } => {
                let id = resolve(*port, &created, observed)?;
                mc.connect(id, *port)?;
                tracing::info!(%port, %id, "connected dpni to dpmac");
            }
            Transition::SetMac { port, mac } => {
                let id = resolve(*port, &created, observed)?;
                mc.set_mac(id, *mac)?;
                tracing::info!(%port, %id, %mac, "set dpni primary mac");
            }
            Transition::Bind { port } => {
                let id = resolve(*port, &created, observed)?;
                kernel.bind(id)?;
                tracing::debug!(%port, %id, "nudged bind; awaiting netdev");
            }
            Transition::Disconnect { dpni } => {
                mc.disconnect(*dpni)?;
                tracing::info!(%dpni, "disconnected dpni");
            }
            Transition::Unbind { dpni } => {
                // The driver releases the netdev on disconnect/destroy; nothing to
                // force here. Logged for auditability.
                tracing::info!(%dpni, "unbind (driver releases on teardown)");
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

    /// Root-scope pool convergence at the engine seam, driven through the in-memory fake
    /// (pool-objects task 3.4): the phase-1 laws — grow, free-only shrink, prune,
    /// shrink-below-draw refusal, the disruption gate, and idempotence — exercised offline.
    mod pool {
        use dpaa2_api::contract::fake::FakeBackend;
        use dpaa2_api::core::model::{MacMode, ObjectRef};
        use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};
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
                converge_pools(&compiled.plan, &mc, pool_cfg()).unwrap(),
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
                converge_pools(&compiled.plan, &mc, pool_cfg()).unwrap(),
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
            let foreign = seeded(99, RawLabel::from("vendor"), false); // undeclared, undrawn ⇒ prune target
            let mc = FakeBackend::new()
                .with_inventory(ref_inventory(16))
                .with_pool_object(DprcId::ROOT, foreign.clone());

            assert_eq!(
                converge_pools(&compiled.plan, &mc, pool_cfg()).unwrap(),
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

        #[test]
        fn converge_pools_shrinks_a_free_managed_surplus() {
            let compiled = compiled_kernel();
            let req = derived_requirement(&compiled.plan, &Container::Root, PoolFamily::Dpbp);
            // Seed req+2 free (undrawn) dpbp wearing the kernel name ⇒ a surplus of 2 the shrink reclaims through the unplug probe.
            let mut mc = FakeBackend::new().with_inventory(ref_inventory(16));
            for ord in 0..u32::try_from(req + 2).unwrap() {
                mc = mc.with_pool_object(DprcId::ROOT, seeded(ord, RawLabel::from(KERNEL), false));
            }

            assert_eq!(
                converge_pools(&compiled.plan, &mc, pool_cfg()).unwrap(),
                PoolOutcome::Converged
            );
            assert_eq!(count(&mc, Family::Dpbp), req, "shrunk to the derived count");
        }

        #[test]
        fn converge_pools_refuses_a_requirement_below_draw() {
            let compiled = compiled_kernel();
            let req = derived_requirement(&compiled.plan, &Container::Root, PoolFamily::Dpbp);
            // Seed req+1 drawn (plugged) managed dpbp ⇒ the requirement sits below the draw, a
            // free-only shrink cannot reach it, so it refuses and tears nothing down.
            let mut mc = FakeBackend::new().with_inventory(ref_inventory(16));
            for ord in 0..u32::try_from(req + 1).unwrap() {
                mc = mc.with_pool_object(DprcId::ROOT, seeded(ord, RawLabel::from(KERNEL), true));
            }

            match converge_pools(&compiled.plan, &mc, pool_cfg()).unwrap() {
                PoolOutcome::ShrinkRefused { refusal } => {
                    assert_eq!(refusal.family, PoolFamily::Dpbp);
                    assert_eq!(refusal.requirement, req);
                    assert_eq!(refusal.drawn, req + 1);
                }
                other => panic!("expected a below-draw refusal, got {other:?}"),
            }
            // Nothing was actuated: the drawn rows survive and no other family was grown.
            assert_eq!(count(&mc, Family::Dpbp), req + 1);
            assert_eq!(
                count(&mc, Family::Dpmcp),
                0,
                "no dispatch before the refusal"
            );

            // The dry-run seam renders the refusal (the REFUSED branch).
            let drift = plan_pools(&compiled.plan, &mc).unwrap();
            let text = crate::render::render_pool_drift(&compiled.plan, &drift);
            assert!(text.contains("REFUSED"), "{text}");
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
                converge_pools(&compiled.plan, &mc, cfg).unwrap(),
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
            assert!(mc.observe_pool(None, Family::Dpbp).unwrap().is_empty());
            // The render names the pass and the families.
            let text = crate::render::render_pool_drift(&compiled.plan, &drift);
            assert!(text.contains("root pool convergence"), "{text}");
            assert!(text.contains("dpio seats"), "{text}");
        }
    }
}
