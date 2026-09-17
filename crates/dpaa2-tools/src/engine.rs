//! The imperative shell: observe → reconcile → act → wait → re-observe (design D0).
//!
//! This is the "imperative shell" wrapped around the pure core. It is generic over
//! the [`McControl`]/[`KernelControl`] trait seams so the whole convergence loop runs
//! against the in-memory fake with no board (design D10). Actuation resolves the
//! DPNI index for a freshly-created port from the id the MC assigned this pass, and
//! for existing ports from the observed connection edge (design D1).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::thread::sleep;
use std::time::{Duration, Instant};

use dpaa2_api::contract::{KernelControl, McControl};
use dpaa2_api::core::error::Error;
use dpaa2_api::core::model::{DesiredTopology, DpmacId, DpniId, DprcId, ObservedTopology};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dprc::Options;
use dpaa2_api::plan::dprc::{
    Attribution, ConsumerConvergence, ContainerPlan, ContainerStep, ContainerVerdict, PruneBucket,
    PruneItem, Verb, attribute_refusal, derive_consumer_containers, plan_consumer_convergence,
    plan_prune,
};
use dpaa2_api::plan::reconcile::{ReconcileOptions, reconcile_with};
use dpaa2_api::plan::{Class, Plan, Transition};
use dpaa2_api::{CompiledPlan, Container};

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
/// analog of [`Outcome`] (design D2; reconciler delta). Kept distinct because a
/// container refusal is a typed [`Attribution`] (design D4), not a DPMAC-anchored port
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
    /// (design D4) and the refusal shapes (0x6/0x8/0x4, or a restool client guard) stay
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

/// Reconciles every declared consumer's child container toward the compiled intent
/// (design D2; reconciler delta "Consumer convergence is container-only").
///
/// The container half of the product pipeline: it re-observes the board's containers
/// (DPRC-I6 — a fresh MC query, never `sync`), plans container-only via
/// [`plan_consumer_convergence`] (the child DPRC alone, so no companion/dpni step is
/// representable — bead cd3.8), gates the headline against `cfg.allow`, dispatches each
/// step to the task-3.1 verbs, and judges convergence by a second re-observation. A
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

    // Dispatch: each container-only step maps one-to-one to a task-3.1 verb. A typed
    // shim refusal is attributed (design D4) and surfaced discriminated.
    for c in &convergences {
        for step in &c.plan.steps {
            if let Err(e) = dispatch_container_step(step, mc) {
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

    // Verdict by re-observation (DPRC-I6): re-query and judge, never assume the dispatch.
    let observed = mc.observe_containers()?;
    for c in &plan_consumer_convergence(plan, &observed) {
        if let ContainerVerdict::Diverged(reasons) = &c.verdict {
            return Err(Error::Backend(format!(
                "container `{}` did not converge after dispatch: {reasons:?}",
                c.container.label
            )));
        }
    }
    Ok(ContainerOutcome::Converged)
}

/// Plans (without dispatching) container-only convergence for every declared consumer,
/// re-observing the board — the read seam `dry-run` renders (design D2/D6). Each
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

    // Verdict by re-observation only (DPRC-I6): a dispatched survivor is an error, a refused one is expected.
    let after = mc.observe_containers()?;
    for id in &dispatched {
        if after.contains_key(id) {
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

/// Dispatches one container-only step to its task-3.1 verb. The container-only
/// convergence path emits only [`ContainerStep::CreateContainer`] (existence, options,
/// label, placement); any other step is out of this change's scope (companion/dpni are
/// tiles #5/#6) and is an error, not a silent no-op.
fn dispatch_container_step<M: McControl>(step: &ContainerStep, mc: &M) -> Result<(), Error> {
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
            Ok(())
        }
        other => Err(Error::Backend(format!(
            "container-only convergence emits no {other:?} (companion/dpni are tiles #5/#6)"
        ))),
    }
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
/// `timeout` elapses (design D5). Returns `true` once the MC answers.
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
            Transition::Create {
                port,
                label,
                num_queues,
            } => {
                let id = mc.create_dpni(label, *num_queues)?;
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
