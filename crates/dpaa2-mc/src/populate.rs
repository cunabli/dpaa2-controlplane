//! Child-container population and the VFIO handoff — the composing edge that serves a
//! userspace dataplane its own populated, VFIO-bound dprc (mc-backend spec requirement 3;
//! pool-objects design D2/D5).
//!
//! This module composes read-back-judged observations exactly as [`pool`](crate::pool)
//! composes the delta→id dispatch and [`probe`](crate::probe) composes the root-bind
//! read-back: [`plan_child_population`] gathers each family's child census against the
//! compiled plan, [`dispatch_child_population`] actuates the deltas through the pure family
//! surface (`dpaa2_api::families`) and returns the post-dispatch census — the read-back IS
//! the observation (the sans-io plan/dispatch split, pool-objects design D11), an exit status never is
//! (mc-backend spec requirement 1). It is idempotent and level-triggered: a converged child
//! yields empty deltas and no create, and a second pass over it does too.
//!
//! [`vfio_handoff`] drives the kernel-side transition dprc-encapsulation modeled as
//! [`Container<Plugged>::bind_vfio`](dpaa2_api::families::dprc::Container): set the
//! `driver_override`, bind `vfio-fsl-mc`, read the bound-driver link back, and let the core
//! judge it ([`VfioBind::classify`]). The read-back is the bus driver link because restool
//! can never observe a plugged child (`dpaa2_api::families::dprc` :308 note); the whole VFIO
//! path is a sysfs `driver_override` + `bind`, outside the MC command whitelist
//! (`docs/baseline/mc-ioctl-policy.md` §3), while the child's dprc verbs go through the same
//! ioctl path the adapter drives (`docs/baseline/mc-ioctl-policy.md` §2a; the bind's
//! propagation to later children is deferred to the next scan, `docs/baseline/dprc.md`
//! "Kernel-defined semantics", ADR-0017).

use std::collections::{BTreeMap, BTreeSet};

use dpaa2_api::contract::{KernelControl, McControl};
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::inventory::Ceiling;
use dpaa2_api::core::model::{DpniId, DprcId, ObjectRef};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpio::derived_seats;
use dpaa2_api::families::dpni::DpniCfg;
use dpaa2_api::families::dprc::VfioBind;
use dpaa2_api::families::pool_lifecycle::{
    PoolCensus, PoolDeltas, PoolFamily, RawDriver, ShrinkBelowDraw, census_of, derived_requirement,
    drift_disposition,
};
use dpaa2_api::intent::compiled::{Attributes, CompiledPlan, Container, ObjectKey};
use dpaa2_api::plan::Class;

use crate::pool::{default_dpio_cfg, dispatch_pool_deltas};

/// The trio traversal a child population converges in: dpmcp→dpbp→dpcon, dependency
/// bottom first (everything draws a dpmcp; pool-objects design D8). dpio is not here — it
/// is a seat, converged separately below (pool-objects design D4).
const TRIO: [PoolFamily; 3] = [PoolFamily::Dpmcp, PoolFamily::Dpbp, PoolFamily::Dpcon];

/// One child dpni the population plans, read-only (pool-objects design D11): its compiled
/// create block and label, the planned peer to connect it to (the plan edge, `None` when
/// the peer is a cross-container dpni whose id this tile cannot resolve), and the observed
/// state — the id when a matching-label dpni already exists, and whether its endpoint
/// already equals the peer (the connect idempotence read). The arity of the containing
/// [`ChildPlan::dpnis`] IS the plan's per-port dpni count, never a constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedChildDpni {
    /// The plan key identity.
    pub key: ObjectKey,
    /// The MC label the create stamps (the port's construct name).
    pub label: ConstructName,
    /// The compiled create block.
    pub cfg: DpniCfg,
    /// The planned connect peer (a root dpmac for a child port-edge), or `None` when the
    /// plan wires a cross-container dpni↔dpni peer (unresolvable to an id this tile).
    pub peer: Option<ObjectRef>,
    /// The observed dpni id when a matching-label row already exists.
    pub observed: Option<DpniId>,
    /// Whether the observed endpoint already equals the peer (connect idempotence).
    pub connected: bool,
}

impl PlannedChildDpni {
    fn needs_create(&self) -> bool {
        self.observed.is_none()
    }

    fn needs_connect(&self) -> bool {
        self.peer.is_some() && (self.observed.is_none() || !self.connected)
    }
}

/// The read-only population plan for one child dprc (pool-objects design D11): pure and
/// renderable, computed from the compiled plan and the child's read-back census. Every
/// count comes from the plan and every observation from a read — no mutation. The
/// [`dispatch_child_population`] half actuates it; the dry-run and status surfaces render it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildPlan {
    /// The child's re-observation handle.
    pub child: DprcId,
    /// The child's MC label (the owning tenant/holder name).
    pub label: ConstructName,
    /// The per-port dpnis the plan places in this child, arity plan-derived.
    pub dpnis: Vec<PlannedChildDpni>,
    /// Per trio family: the derived requirement, the observed census, and the count-level
    /// disposition (or the below-draw refusal).
    pub families: BTreeMap<PoolFamily, (i64, PoolCensus, Result<PoolDeltas, ShrinkBelowDraw>)>,
    /// The dpio seats: `(required, observed)`.
    pub seats: (i64, i64),
    /// Whether the child reads back bound to `vfio-fsl-mc` (ADR-0017 drift gate).
    pub bound: bool,
}

impl ChildPlan {
    /// Whether the child is converged: every planned dpni present and connected, every trio
    /// census meets its requirement, and the dpio seats equal their requirement. The
    /// idempotence witness a second pass reproduces.
    #[must_use]
    pub fn is_converged(&self) -> bool {
        self.dpnis
            .iter()
            .all(|d| !d.needs_create() && !d.needs_connect())
            && self.seats.0 == self.seats.1
            && self
                .families
                .values()
                .all(|(req, census, _)| census.converged(*req))
    }

    /// The pass headline (ADR-0015 decision 12): [`Class::Disruptive`] when any resident
    /// must be created (a dpni, a trio delta, or a dpio seat), else [`Class::Hitless`] — a
    /// connect or a bind is the hitless plug face. Matches the [`ContainerStep`] class
    /// taxonomy (`CreateResident` disruptive, `PlugContainer` hitless).
    ///
    /// [`ContainerStep`]: dpaa2_api::plan::dprc::ContainerStep
    #[must_use]
    pub fn headline(&self) -> Class {
        let creates = self.dpnis.iter().any(PlannedChildDpni::needs_create)
            || self
                .families
                .values()
                .any(|(_, _, d)| d.is_ok_and(|d| !d.is_empty()))
            || self.seats.0 > self.seats.1;
        if creates {
            Class::Disruptive
        } else {
            Class::Hitless
        }
    }

    /// The first below-draw refusal across the trio, if any (pool-objects design D3).
    #[must_use]
    pub fn shrink_refusal(&self) -> Option<ShrinkBelowDraw> {
        self.families.values().find_map(|(_, _, d)| d.err())
    }
}

/// The observed outcome of dispatching a child population — every field a read-back census,
/// never a driven state (mc-backend spec requirement 3: "observable as a census of the
/// child matching the derived counts").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildPopulation {
    /// The child's dpnis as observed after the pass.
    pub dpnis: Vec<ObjectRef>,
    /// Per trio family, the derived requirement paired with the post-dispatch census.
    pub families: BTreeMap<PoolFamily, (i64, PoolCensus)>,
    /// The dpio seats: `(required, observed-after)`.
    pub seats: (i64, i64),
}

impl ChildPopulation {
    /// Whether the child is converged for `dpnis_required` planned dpnis: every trio census
    /// meets its requirement ([`PoolCensus::converged`]), the dpio seats equal their
    /// requirement, and every planned dpni is present. The read-back census is the
    /// observation, so a `true` here is the idempotence witness a second pass reproduces.
    #[must_use]
    pub fn converged(&self, dpnis_required: usize) -> bool {
        self.dpnis.len() >= dpnis_required
            && self.seats.0 == self.seats.1
            && self
                .families
                .values()
                .all(|(requirement, census)| census.converged(*requirement))
    }
}

/// The common ancestor a child-port connect is issued from (pool-objects design D11): the
/// child dpni and its root dpmac (or a sibling child's dpni) share the root dprc.1 ancestor,
/// so the DPNI-I9 connect is issued there ([`McControl::connect_in`]).
const CONNECT_ANCESTOR: DprcId = DprcId::ROOT;

/// The planned peer of a child dpni, read from the compiled plan's edges (pool-objects D11):
/// a root dpmac for a child port-edge, or `None` for a dpni↔dpni wire whose peer id this tile
/// cannot resolve (the board-witnessed case is child↔dpmac, pool-objects task 3.12).
fn planned_peer(plan: &CompiledPlan, key: &ObjectKey) -> Option<ObjectRef> {
    plan.edges.iter().find_map(|e| {
        e.port_edge_dpni()
            .filter(|(k, _)| *k == key)
            .map(|(_, dpmac)| ObjectRef::new(Family::Dpmac, dpmac.into_inner()))
    })
}

/// Reads a child dprc's population plan (pool-objects design D11): pure of any mutation —
/// the arity of the child's dpnis, each one's planned peer and observed/connected state, the
/// trio census and disposition, the dpio seat counts, and the VFIO bind state. The count→
/// individual boundary stays in the plan (pool-objects design D2): every count comes from
/// the compiled plan and every observation from a read.
///
/// # Errors
/// Propagates the first [`Error`] any observation raises.
pub fn plan_child_population<M: McControl, K: KernelControl>(
    mc: &M,
    kernel: &K,
    child: DprcId,
    plan: &CompiledPlan,
    container: &Container,
    label: &ConstructName,
    declared: &BTreeSet<ConstructName>,
) -> Result<ChildPlan, Error> {
    // dpni arity IS the plan's per-port dpni count for this container (never a constant).
    let dpni_rows = mc.observe_pool(Some(child), Family::Dpni)?;
    let mut dpnis = Vec::new();
    for obj in plan
        .objects
        .iter()
        .filter(|o| o.container() == container && o.key().family == Family::Dpni)
    {
        let Attributes::Dpni { cfg } = obj.attributes() else {
            continue;
        };
        let peer = planned_peer(plan, obj.key());
        let observed = dpni_rows
            .iter()
            .find(|r| r.label.as_str() == obj.label().as_str())
            .map(|r| DpniId::new(r.object.ordinal()));
        let connected = match observed {
            Some(id) => mc.observe_endpoint(id)? == peer,
            None => false,
        };
        dpnis.push(PlannedChildDpni {
            key: obj.key().clone(),
            label: obj.label().clone(),
            cfg: cfg.clone(),
            peer,
            observed,
            connected,
        });
    }

    // trio: census + count-level disposition, read-only (dispatch actuates the deltas).
    let mut families = BTreeMap::new();
    for family in TRIO {
        let rows = mc.observe_pool(Some(child), family.family())?;
        let census = census_of(&rows, declared);
        let requirement = derived_requirement(plan, container, family);
        let disposition = drift_disposition(family, census, requirement, &Ceiling::Unknown);
        families.insert(family, (requirement, census, disposition));
    }

    let required = derived_seats(plan, container);
    let observed_seats =
        i64::try_from(mc.observe_pool(Some(child), Family::Dpio)?.len()).unwrap_or(i64::MAX);
    let bound = matches!(
        VfioBind::classify(kernel.bound_driver(child)?.as_ref().map(RawDriver::as_str)),
        VfioBind::BoundVfioFslMc
    );

    Ok(ChildPlan {
        child,
        label: label.clone(),
        dpnis,
        families,
        seats: (required, observed_seats),
        bound,
    })
}

/// Actuates a child population plan and reports the read-back census (pool-objects D11;
/// mc-backend spec requirement 3), idempotent and level-triggered — the adapter drives, the
/// core judges.
///
/// The pass, in order:
/// - **dpnis**: creates each planned dpni the plan found absent
///   ([`McControl::create_dpni_in`]), then connects it to its planned peer from the common
///   ancestor ([`McControl::connect_in`], the DPNI-I9 form without a root plug), skipping a
///   dpni already connected to that peer.
/// - **trio** (dpmcp→dpbp→dpcon): dispatches each family's planned deltas
///   ([`dispatch_pool_deltas`]) — a below-draw disposition surfaces as the typed
///   [`ShrinkBelowDraw`] folded to [`Error::Config`].
/// - **dpio seats**: creates the seat deficit as plain `dpio_create`s, NOT
///   [`create_dpio_seat`](crate::pool::create_dpio_seat): the dpmcp-probe pairing is the
///   kernel dpio driver's draw, and a VFIO child's dpio is userspace-consumed
///   (`docs/baseline/dpio.md`; ADR-0012).
///
/// # Errors
/// Returns the first [`Error`] any create, connect, destroy, or dispatch raises; a
/// requirement below a family's drawn count surfaces as the typed [`ShrinkBelowDraw`]
/// folded to [`Error::Config`].
pub fn dispatch_child_population<M: McControl>(
    mc: &M,
    cplan: &ChildPlan,
    declared: &BTreeSet<ConstructName>,
) -> Result<ChildPopulation, Error> {
    let child = cplan.child;
    let label = &cplan.label;

    for d in &cplan.dpnis {
        let id = match d.observed {
            Some(id) => id,
            None => mc.create_dpni_in(child, &d.cfg, &d.label)?,
        };
        if let Some(peer) = d.peer
            && d.needs_connect()
        {
            mc.connect_in(CONNECT_ANCESTOR, id, peer)?;
        }
    }

    let mut families = BTreeMap::new();
    for family in TRIO {
        let (requirement, _census, disposition) = cplan.families[&family];
        let deltas = disposition?;
        let dispatch = dispatch_pool_deltas(mc, Some(child), family, deltas, label, declared)?;
        families.insert(family, (requirement, census_of(&dispatch.after, declared)));
    }

    let (required, _observed) = cplan.seats;
    let observed =
        i64::try_from(mc.observe_pool(Some(child), Family::Dpio)?.len()).unwrap_or(i64::MAX);
    for _ in 0..(required - observed).max(0) {
        mc.dpio_create(Some(child), default_dpio_cfg(), label)?;
    }
    let observed_after =
        i64::try_from(mc.observe_pool(Some(child), Family::Dpio)?.len()).unwrap_or(i64::MAX);

    let dpnis = mc
        .observe_pool(Some(child), Family::Dpni)?
        .iter()
        .map(|r| r.object)
        .collect();

    Ok(ChildPopulation {
        dpnis,
        families,
        seats: (required, observed_after),
    })
}

/// Drives the child's `vfio-fsl-mc` handoff and reports the read-back bind state — the
/// kernel-side transition dprc-encapsulation modeled as
/// [`Container<Plugged>::bind_vfio`](dpaa2_api::families::dprc::Container) (mc-backend spec
/// requirement 3). Sets the `driver_override`, binds, reads the bound-driver link back, and
/// lets the core judge it ([`VfioBind::classify`]) — the adapter reports, the core judges.
///
/// The verdict comes from the bus `driver` link, not the bind write's exit, because restool
/// can never observe a plugged child (`dpaa2_api::families::dprc` :308 note): the child's
/// plugged/bound state lives only on the kernel bus. The whole path is a sysfs
/// `driver_override` + `bind`, outside the MC command whitelist
/// (`docs/baseline/dprc.md` "Kernel-defined semantics": `vfio-fsl-mc` has no match table;
/// `docs/baseline/mc-ioctl-policy.md` §3).
///
/// # Errors
/// Returns the first [`Error`] the override write, the bind, or the bound-driver read
/// raises.
pub fn vfio_handoff<K: KernelControl>(kernel: &K, child: DprcId) -> Result<VfioBind, Error> {
    kernel.vfio_set_override(child)?;
    kernel.vfio_bind(child)?;
    let bound = kernel.bound_driver(child)?;
    Ok(VfioBind::classify(bound.as_ref().map(RawDriver::as_str)))
}

#[cfg(test)]
mod tests {
    //! Child population and the VFIO handoff driven through [`FakeBackend`]: the reference
    //! router (two 10G ports, T = 5) populates its child to its derived census (dpbp 2,
    //! dpmcp 1, dpcon 10, dpio 10) with TWO dpnis — the arity read from the plan, never a
    //! constant — each connected to its planned dpmac peer; a second pass creates nothing
    //! and re-connects nothing (idempotence); a pre-seeded foreign-free object is pruned; and
    //! the handoff classifies the fixture's bound driver.

    use dpaa2_api::contract::fake::FakeBackend;
    use dpaa2_api::core::model::{DpmacId, MacMode};
    use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};
    use dpaa2_api::intent::refuse::compile;
    use dpaa2_api::intent::{Dataplane, Intent, Isolation, Port, Tenant, TenantRef};
    use dpaa2_api::testkit::ref_inventory;

    use super::*;

    // The reference intent (compile_tests `reference_intent`): a userspace-poll router
    // terminating two 10G ports (T = 1 + 2 + 2 = 5), so its child carries TWO dpnis plus its
    // poll-mode companions — the arity mismatch pool-objects design D11 fixes.
    fn compiled_router() -> dpaa2_api::intent::refuse::Compiled {
        let router = Tenant {
            name: "router".into(),
            dataplane: Dataplane::UserspacePoll,
            max_cores: 16,
            isolation: Isolation::Isolated,
            renamed: None,
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

    fn declared() -> BTreeSet<ConstructName> {
        BTreeSet::from([ConstructName::from("router")])
    }

    fn count(mc: &FakeBackend, child: DprcId, family: Family) -> usize {
        mc.observe_pool(Some(child), family).unwrap().len()
    }

    // Plans then dispatches one child population against the fake (both southbound faces).
    fn populate(
        mc: &FakeBackend,
        child: DprcId,
        compiled: &dpaa2_api::intent::refuse::Compiled,
    ) -> (ChildPlan, ChildPopulation) {
        let container = Container::Child("router".into());
        let label = ConstructName::from("router");
        let cplan = plan_child_population(
            mc,
            mc,
            child,
            &compiled.plan,
            &container,
            &label,
            &declared(),
        )
        .expect("plan");
        let pop = dispatch_child_population(mc, &cplan, &declared()).expect("dispatch");
        (cplan, pop)
    }

    #[test]
    fn plan_arity_is_the_reference_two_dpnis_never_a_constant() {
        // pool-objects design D11: the plan carries two child dpnis, each with its dpmac peer.
        let compiled = compiled_router();
        let mc = FakeBackend::new();
        let container = Container::Child("router".into());
        let label = ConstructName::from("router");
        let cplan = plan_child_population(
            &mc,
            &mc,
            DprcId::new(2),
            &compiled.plan,
            &container,
            &label,
            &declared(),
        )
        .expect("plan");
        assert_eq!(
            cplan.dpnis.len(),
            2,
            "arity from the derivation, not a constant"
        );
        let peers: BTreeSet<u32> = cplan
            .dpnis
            .iter()
            .filter_map(|d| d.peer.map(ObjectRef::ordinal))
            .collect();
        assert_eq!(
            peers,
            BTreeSet::from([7, 9]),
            "each dpni's planned dpmac peer"
        );
    }

    #[test]
    fn populate_converges_to_the_derived_census_and_is_idempotent() {
        // pool-objects design D11 / mc-backend req 3: the child reads back the regime-derived
        // counts with two connected dpnis; a second pass creates and connects nothing.
        let compiled = compiled_router();
        let child = DprcId::new(2);
        let mc = FakeBackend::new();

        let (_cplan, first) = populate(&mc, child, &compiled);
        assert!(first.converged(2), "{first:?}");
        // The derived census: dpbp 2, dpmcp 1, dpcon 10 (dpnis·T), dpio 10 (2·T), two dpnis.
        assert_eq!(count(&mc, child, Family::Dpbp), 2);
        assert_eq!(count(&mc, child, Family::Dpmcp), 1);
        assert_eq!(count(&mc, child, Family::Dpcon), 10);
        assert_eq!(count(&mc, child, Family::Dpio), 10);
        assert_eq!(count(&mc, child, Family::Dpni), 2);
        assert_eq!(first.dpnis.len(), 2);
        assert_eq!(first.seats, (10, 10));

        // Both dpnis connected to their planned dpmac peers (DPNI-I9 endpoint read-back).
        let endpoints: BTreeSet<u32> = first
            .dpnis
            .iter()
            .filter_map(|d| {
                mc.observe_endpoint(DpniId::new(d.ordinal()))
                    .unwrap()
                    .map(ObjectRef::ordinal)
            })
            .collect();
        assert_eq!(
            endpoints,
            BTreeSet::from([7, 9]),
            "connected to dpmac.7 and dpmac.9"
        );

        // Second pass: re-plan reads converged, and dispatch grows/connects nothing.
        let (cplan2, _second) = populate(&mc, child, &compiled);
        assert!(cplan2.is_converged(), "the re-plan is converged");
        assert!(
            cplan2
                .dpnis
                .iter()
                .all(|d| d.observed.is_some() && d.connected),
            "every dpni present and already connected: no re-connect"
        );
        assert_eq!(count(&mc, child, Family::Dpni), 2, "no third dpni");
        assert_eq!(count(&mc, child, Family::Dpio), 10, "no extra seats");
    }

    #[test]
    fn a_foreign_free_object_in_the_child_is_pruned() {
        // pool-objects design D3: an undeclared, unplugged dpbp the population never made is
        // reclaimed by the prune disposition, and the child still converges.
        let compiled = compiled_router();
        let child = DprcId::new(2);
        let foreign = ObservedPoolObject {
            object: ObjectRef::new(Family::Dpbp, 99),
            label: RawLabel::from("vendor"),
            plugged: false,
            drawn: false,
        };
        let mc = FakeBackend::new().with_pool_object(child, foreign.clone());

        let (_cplan, pop) = populate(&mc, child, &compiled);
        assert!(pop.converged(2), "{pop:?}");
        let dpbps = mc.observe_pool(Some(child), Family::Dpbp).unwrap();
        assert!(!dpbps.iter().any(|o| o.object == foreign.object), "pruned");
        assert_eq!(dpbps.len(), 2);
    }

    #[test]
    fn plan_reads_the_vfio_bind_state() {
        // pool-objects design D11 / ADR-0017: the plan reads back whether the child is bound,
        // the drift-refusal gate input. A seeded vfio-fsl-mc child reads bound.
        let compiled = compiled_router();
        let container = Container::Child("router".into());
        let label = ConstructName::from("router");
        let mc = FakeBackend::new().with_bound_dprc(DprcId::new(2), RawDriver::from("vfio-fsl-mc"));
        let cplan = plan_child_population(
            &mc,
            &mc,
            DprcId::new(2),
            &compiled.plan,
            &container,
            &label,
            &declared(),
        )
        .expect("plan");
        assert!(cplan.bound, "a vfio-fsl-mc child reads back bound");
    }

    #[test]
    fn vfio_handoff_classifies_the_bound_driver() {
        // mc-backend req 3: the handoff drives override+bind and reports the read-back bind
        // state. The FakeBackend reports an unbound child (that face is board-only), so the
        // classification is Unbound — the kernel.rs sysfs fixture exercises the bound path.
        let mc = FakeBackend::new();
        let bind = vfio_handoff(&mc, DprcId::new(2)).expect("handoff");
        assert_eq!(bind, VfioBind::Unbound);
    }
}
