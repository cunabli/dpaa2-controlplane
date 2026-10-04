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

use dpaa2_api::contract::{DpseciDetail, DpseciPortalReadout, KernelControl, McControl};
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::inventory::Ceiling;
use dpaa2_api::core::model::{DpniId, DprcId, ObjectRef};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpio::derived_seats;
use dpaa2_api::families::dprc::VfioBind;
use dpaa2_api::families::dpseci::DpseciCfg;
use dpaa2_api::families::pool_lifecycle::{
    CustodyScope, PoolCensus, PoolFamily, RawDriver, ShrinkBelowDraw, census_of,
    derived_requirement, drift_disposition,
};
use dpaa2_api::intent::compiled::{Attributes, CompiledPlan, Container, ObjectKey};
use dpaa2_api::plan::dpseci::{ObservedSig, census_delta, observed_sig_of, sig_census, sig_of};
use dpaa2_api::plan::populate::{ChildDpseci, ChildPlan, DpseciCensus, PlannedChildDpni};

use crate::pool::{default_dpio_cfg, dispatch_pool_deltas};

/// The trio traversal a child population converges in: dpmcp→dpbp→dpcon, dependency
/// bottom first (everything draws a dpmcp; pool-objects design D8). dpio is not here — it
/// is a seat, converged separately below (pool-objects design D4).
const TRIO: [PoolFamily; 3] = [PoolFamily::Dpmcp, PoolFamily::Dpbp, PoolFamily::Dpcon];

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
    /// The child's dpseci face re-judged from the post-dispatch read-back (the multiset census
    /// of dpseci-typestate design D9): a converged census, or the typed unobservable outcome.
    pub dpseci: ChildDpseci,
    /// The below-draw refusal the trio's managed destroy DISCOVERED through the probe — `Some`
    /// when a family's held rows could not shrink to its requirement (pool-objects design D10),
    /// which stops the pass (first family wins). The caller surfaces it typed, never an error.
    pub refusal: Option<ShrinkBelowDraw>,
}

impl ChildPopulation {
    /// Whether the child is converged for `dpnis_required` planned dpnis: every trio census
    /// meets its requirement ([`PoolCensus::converged`]), the dpio seats MEET their
    /// requirement — a surplus converges grow-only and is reported as the typed residue
    /// (`dpio.qnt` `seatDisposition`; pool-objects design D4/D10) — and every planned dpni is
    /// present, and no below-draw refusal was discovered. The read-back census is the
    /// observation, so a `true` here is the idempotence witness a second pass reproduces.
    #[must_use]
    pub fn converged(&self, dpnis_required: usize) -> bool {
        self.refusal.is_none()
            && self.dpnis.len() >= dpnis_required
            && self.seats.1 >= self.seats.0
            && self
                .families
                .values()
                .all(|(requirement, census)| census.converged(*requirement))
            && self.dpseci.converged()
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

/// The compiled dpseci create blocks for one container (dpseci-typestate design D9): the
/// planned half of the census, in declaration order. Position is not a hardware identity, so
/// the order is immaterial — the census is a multiset (ADR-0015 decision 5).
fn planned_dpseci_cfgs(plan: &CompiledPlan, container: &Container) -> Vec<DpseciCfg> {
    plan.objects
        .iter()
        .filter(|o| o.container() == container && o.key().family == Family::Dpseci)
        .filter_map(|o| match o.attributes() {
            Attributes::Dpseci { cfg } => Some(cfg.clone()),
            _ => None,
        })
        .collect()
}

/// The reason a dpseci's signature could not be judged, for display on the unobservable face
/// (dpseci-typestate design D9): the portal's own reason when it was unavailable, else the
/// honest gap where an open portal named no vocabulary options or restool `info` gave no queue
/// count.
fn unobservable_reason(detail: &DpseciDetail) -> String {
    match &detail.portal {
        DpseciPortalReadout::Unobservable { reason } => reason.clone(),
        DpseciPortalReadout::Observed { .. } => {
            "dpseci signature not nameable (an unnamed option bit is attributed, or the queue count is absent)".to_owned()
        }
    }
}

/// Judges one child's dpseci population as a multiset census of the observable signature
/// (dpseci-typestate design D9), filtered to the tenant's custody: only rows whose label
/// equals the tenant's `ConstructName` are counted or ever destroyed — foreign/unlabelled rows
/// are never touched (the pool `label_membership` precedent). If any custody-matched row's
/// signature cannot be read, the whole face is [`ChildDpseci::Unobservable`] — absence of
/// evidence is never drift, so the census judges nothing (ADR-0018; V-LIFE-DPSECI-1).
///
/// Model twin: `dpseci.qnt` `judgeCensus`, run `censusUnobservableMemberJudgesNothingTest`.
fn judge_dpseci<M: McControl>(
    mc: &M,
    child: DprcId,
    label: &ConstructName,
    planned: &[DpseciCfg],
) -> Result<ChildDpseci, Error> {
    let rows = mc.observe_pool(Some(child), Family::Dpseci)?;
    let mut observed = Vec::new();
    for row in rows.iter().filter(|r| r.label.as_str() == label.as_str()) {
        let detail = mc.observe_dpseci(child, row.object)?;
        match observed_sig_of(&detail) {
            ObservedSig::Observed(sig) => observed.push((sig, row.object)),
            ObservedSig::Unobservable => {
                return Ok(ChildDpseci::Unobservable {
                    reason: unobservable_reason(&detail),
                });
            }
        }
    }
    let planned_census = sig_census(planned.iter().map(sig_of));
    let observed_census = sig_census(observed.iter().map(|(sig, _)| sig.clone()));
    let delta = census_delta(&planned_census, &observed_census);
    Ok(ChildDpseci::Census(DpseciCensus {
        delta,
        planned: planned.to_vec(),
        observed,
    }))
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
        // Child scope: the unplug-probe reclaim and below-draw refusal fire unchanged (ADR-0020 decision 3).
        let disposition = drift_disposition(
            family,
            census,
            requirement,
            &Ceiling::Unknown,
            CustodyScope::ChildScope,
        );
        families.insert(family, (requirement, census, disposition));
    }

    let required = derived_seats(plan, container);
    let observed_seats =
        i64::try_from(mc.observe_pool(Some(child), Family::Dpio)?.len()).unwrap_or(i64::MAX);
    let bound = matches!(
        VfioBind::classify(kernel.bound_driver(child)?.as_ref().map(RawDriver::as_str)),
        VfioBind::BoundVfioFslMc
    );

    let dpseci = judge_dpseci(mc, child, label, &planned_dpseci_cfgs(plan, container))?;

    Ok(ChildPlan {
        child,
        label: label.clone(),
        dpnis,
        families,
        seats: (required, observed_seats),
        dpseci,
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
///   ([`dispatch_pool_deltas`]). A PRE-DISPATCH below-draw disposition (the census already read
///   the draw) folds to [`Error::Config`] through `?`; a DISCOVERED below-draw (the probe
///   refusal on a row the census read free) returns typed on [`ChildPopulation::refusal`] and
///   stops the pass at that family — never an error (pool-objects design D10).
/// - **dpio seats**: creates the seat deficit as plain `dpio_create`s, NOT
///   [`create_dpio_seat`](crate::pool::create_dpio_seat): the dpmcp-probe pairing is the
///   kernel dpio driver's draw, and a VFIO child's dpio is userspace-consumed
///   (`docs/baseline/dpio.md`; ADR-0012).
///
/// # Errors
/// Returns the first [`Error`] any create, connect, destroy, or dispatch raises; a PRE-DISPATCH
/// requirement below a family's drawn count surfaces as the typed [`ShrinkBelowDraw`] folded to
/// [`Error::Config`], while a probe-DISCOVERED one is carried on [`ChildPopulation::refusal`],
/// not returned as an error.
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

    // dpseci: the census delta drives N creates and M destroys (dpseci-typestate design D9); a
    // create renders a desired cfg of the surplus signature, a destroy drains a tenant-labelled
    // observed row. The unobservable face judged nothing, so it actuates nothing.
    if let ChildDpseci::Census(census) = &cplan.dpseci {
        for (sig, count) in &census.delta.creates {
            let count = usize::try_from(*count).unwrap_or(0);
            for cfg in census
                .planned
                .iter()
                .filter(|cfg| &sig_of(cfg) == sig)
                .take(count)
            {
                mc.create_dpseci_in(Some(child), cfg, label)?;
            }
        }
        for (sig, count) in &census.delta.destroys {
            let count = usize::try_from(*count).unwrap_or(0);
            for (_, object) in census.observed.iter().filter(|(s, _)| s == sig).take(count) {
                mc.destroy_dpseci(Some(child), object)?;
            }
        }
    }

    let mut families = BTreeMap::new();
    let mut refusal = None;
    for family in TRIO {
        let (requirement, _census, disposition) = cplan.families[&family];
        let deltas = disposition?;
        let dispatch = dispatch_pool_deltas(
            mc,
            Some(child),
            family,
            deltas,
            requirement,
            label,
            declared,
            CustodyScope::ChildScope,
        )?;
        families.insert(family, (requirement, census_of(&dispatch.after, declared)));
        // A discovered below-draw stops the pass at this family, returned typed, not an error (pool-objects design D10).
        if dispatch.refusal.is_some() {
            refusal = dispatch.refusal;
            break;
        }
    }

    let (required, _observed) = cplan.seats;
    // A discovered refusal stops the pass before the seat grow (pool-objects design D10).
    if refusal.is_none() {
        let observed =
            i64::try_from(mc.observe_pool(Some(child), Family::Dpio)?.len()).unwrap_or(i64::MAX);
        // The `(required - observed).max(0)` deficit is the count-level `admit_seat` gate (ADR-0012); a typed pre-gate needs regime-typed observed seats the census cannot read.
        for _ in 0..(required - observed).max(0) {
            mc.dpio_create(Some(child), default_dpio_cfg(), label)?;
        }
    }
    let observed_after =
        i64::try_from(mc.observe_pool(Some(child), Family::Dpio)?.len()).unwrap_or(i64::MAX);

    let dpnis = mc
        .observe_pool(Some(child), Family::Dpni)?
        .iter()
        .map(|r| r.object)
        .collect();

    // Re-judge the dpseci face from the post-dispatch read-back (dpseci-typestate design D9); the
    // unobservable face dispatched nothing, so it carries forward unchanged.
    let dpseci = match &cplan.dpseci {
        ChildDpseci::Unobservable { reason } => ChildDpseci::Unobservable {
            reason: reason.clone(),
        },
        ChildDpseci::Census(census) => judge_dpseci(mc, child, label, &census.planned)?,
    };

    Ok(ChildPopulation {
        dpnis,
        families,
        seats: (required, observed_after),
        dpseci,
        refusal,
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
    use dpaa2_api::families::dpio::{SeatDisposition, SeatRegime};
    use dpaa2_api::families::dpseci::{DpseciOpt, OptionMask};
    use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};
    use dpaa2_api::intent::refuse::compile;
    use dpaa2_api::intent::{Crypto, Dataplane, Intent, Isolation, Port, Tenant, TenantRef};
    use dpaa2_api::plan::Class;
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
    fn child_seat_surplus_reports_typed_residue() {
        use dpaa2_api::families::dpio::SeatResidue;

        let compiled = compiled_router();
        let child = DprcId::new(2);
        let label = ConstructName::from("router");
        let mc = FakeBackend::new();

        let (_cplan, first) = populate(&mc, child, &compiled);
        assert_eq!(first.seats, (10, 10), "the derived seat count 2·T");

        for _ in 0..3 {
            mc.dpio_create(Some(child), default_dpio_cfg(), &label)
                .expect("seed extra dpio");
        }
        assert_eq!(
            count(&mc, child, Family::Dpio),
            13,
            "seeded surplus present"
        );

        let container = Container::Child("router".into());
        let cplan = plan_child_population(
            &mc,
            &mc,
            child,
            &compiled.plan,
            &container,
            &label,
            &declared(),
        )
        .expect("plan");
        assert_eq!(
            cplan.dpio_disposition(),
            SeatDisposition::RebootRequired(SeatResidue {
                regime: SeatRegime::DpdkSeat,
                observed: 13,
                required: 10,
            }),
            "the surplus is the typed DpdkSeat residue"
        );
        assert!(
            cplan.is_converged(),
            "a seat surplus stays converged (grow-only)"
        );
        assert_eq!(cplan.headline(), Class::Hitless, "surplus-only is hitless");

        let pop = dispatch_child_population(&mc, &cplan, &declared()).expect("dispatch");
        assert_eq!(
            count(&mc, child, Family::Dpio),
            13,
            "no seat created, no destroy"
        );
        assert_eq!(
            pop.seats,
            (10, 13),
            "grow-only outcome: required 10, observed 13"
        );
        assert!(
            pop.converged(cplan.dpnis.len()),
            "the typed grow-only outcome converges: {pop:?}"
        );
    }

    #[test]
    fn child_below_draw_pre_dispatch_refuses_typed() {
        // pool-objects design D3: a kernel-face `drawn: true` row puts the requirement below the draw pre-dispatch — shrink_refusal names it typed.
        let compiled = compiled_router();
        let child = DprcId::new(2);
        let container = Container::Child("router".into());
        let label = ConstructName::from("router");
        let req = derived_requirement(&compiled.plan, &container, PoolFamily::Dpbp);

        let mut mc = FakeBackend::new();
        for ord in 0..=req {
            mc = mc.with_pool_object(
                child,
                ObservedPoolObject {
                    object: ObjectRef::new(Family::Dpbp, u32::try_from(ord).unwrap()),
                    label: RawLabel::from("router"),
                    plugged: true,
                    drawn: true,
                },
            );
        }

        let cplan = plan_child_population(
            &mc,
            &mc,
            child,
            &compiled.plan,
            &container,
            &label,
            &declared(),
        )
        .expect("plan");
        assert_eq!(
            cplan.shrink_refusal(),
            Some(ShrinkBelowDraw {
                family: PoolFamily::Dpbp,
                requirement: req,
                drawn: req + 1,
            }),
            "the census-read draw is the pre-dispatch refusal"
        );
    }

    #[test]
    fn child_discovered_draw_refuses_typed() {
        // pool-objects design D10: in-use rows read free, so the census emits a destroy; the probe finds every candidate held and the dispatch returns the typed refusal, held rows surviving.
        let compiled = compiled_router();
        let child = DprcId::new(2);
        let container = Container::Child("router".into());
        let label = ConstructName::from("router");
        let req = derived_requirement(&compiled.plan, &container, PoolFamily::Dpbp);

        let held: Vec<ObjectRef> = (0..=req)
            .map(|ord| ObjectRef::new(Family::Dpbp, u32::try_from(ord).unwrap()))
            .collect();
        let mut mc = FakeBackend::new();
        for &object in &held {
            mc = mc.with_in_use_pool_object(
                child,
                ObservedPoolObject {
                    object,
                    label: RawLabel::from("router"),
                    plugged: true,
                    drawn: false,
                },
            );
        }

        let cplan = plan_child_population(
            &mc,
            &mc,
            child,
            &compiled.plan,
            &container,
            &label,
            &declared(),
        )
        .expect("plan");
        let pop =
            dispatch_child_population(&mc, &cplan, &declared()).expect("dispatch, not an error");
        assert_eq!(
            pop.refusal,
            Some(ShrinkBelowDraw {
                family: PoolFamily::Dpbp,
                requirement: req,
                drawn: req + 1,
            }),
            "every candidate held: the discovered below-draw refusal with proven counts"
        );
        assert!(
            !pop.converged(cplan.dpnis.len()),
            "a refused pass is not converged"
        );
        let dpbps = mc.observe_pool(Some(child), Family::Dpbp).unwrap();
        for &object in &held {
            assert!(
                dpbps.iter().any(|r| r.object == object),
                "the held victim survives"
            );
        }
    }

    #[test]
    fn discovered_draw_skips_to_free_victim() {
        // freeOnlyShrinkTest twin (pool-objects design D10): one held row plus free surplus enough for the quota — the shrink converges through the free victims, the held one survives.
        let compiled = compiled_router();
        let child = DprcId::new(2);
        let container = Container::Child("router".into());
        let label = ConstructName::from("router");
        let req = derived_requirement(&compiled.plan, &container, PoolFamily::Dpbp);

        let held = ObjectRef::new(Family::Dpbp, 50);
        let mut mc = FakeBackend::new().with_in_use_pool_object(
            child,
            ObservedPoolObject {
                object: held,
                label: RawLabel::from("router"),
                plugged: true,
                drawn: false,
            },
        );
        let free: Vec<ObjectRef> = (0..=(req + 1))
            .map(|ord| ObjectRef::new(Family::Dpbp, 60 + u32::try_from(ord).unwrap()))
            .collect();
        for &object in &free {
            mc = mc.with_pool_object(
                child,
                ObservedPoolObject {
                    object,
                    label: RawLabel::from("router"),
                    plugged: true,
                    drawn: false,
                },
            );
        }

        let cplan = plan_child_population(
            &mc,
            &mc,
            child,
            &compiled.plan,
            &container,
            &label,
            &declared(),
        )
        .expect("plan");
        let pop = dispatch_child_population(&mc, &cplan, &declared()).expect("dispatch");
        assert!(
            pop.refusal.is_none(),
            "the quota is reachable through the free victims: {pop:?}"
        );
        assert!(
            pop.converged(cplan.dpnis.len()),
            "the free-only shrink converges"
        );
        let dpbps = mc.observe_pool(Some(child), Family::Dpbp).unwrap();
        assert!(
            dpbps.iter().any(|r| r.object == held),
            "the held victim survives"
        );
        assert_eq!(
            i64::try_from(dpbps.len()).unwrap(),
            req,
            "the child shrank to exactly the requirement"
        );
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

    // ---- the dpseci observable-signature census (dpseci-typestate task 3.3; design D9) ----

    // A `[[crypto]]` tenant "sec", one dpseci per `flows` block, isolated (dpseci-typestate design D3/D4).
    fn compiled_crypto(flows: &[i64]) -> dpaa2_api::intent::refuse::Compiled {
        let sec = Tenant {
            name: "sec".into(),
            dataplane: Dataplane::UserspacePoll,
            max_cores: 16,
            isolation: Isolation::Isolated,
            renamed: None,
            priority: None,
        };
        let crypto = flows
            .iter()
            .map(|&flows| Crypto {
                tenant: "sec".into(),
                flows,
            })
            .collect();
        let intent = Intent {
            tenants: vec![sec],
            crypto,
            ..Intent::empty()
        };
        compile(&intent, &ref_inventory(16)).expect("the crypto intent compiles")
    }

    fn sec_declared() -> BTreeSet<ConstructName> {
        BTreeSet::from([ConstructName::from("sec")])
    }

    fn sec_populate(
        mc: &FakeBackend,
        child: DprcId,
        compiled: &dpaa2_api::intent::refuse::Compiled,
    ) -> (ChildPlan, ChildPopulation) {
        let container = Container::Child("sec".into());
        let label = ConstructName::from("sec");
        let cplan = plan_child_population(
            mc,
            mc,
            child,
            &compiled.plan,
            &container,
            &label,
            &sec_declared(),
        )
        .expect("plan");
        let pop = dispatch_child_population(mc, &cplan, &sec_declared()).expect("dispatch");
        (cplan, pop)
    }

    fn dpseci_rows(mc: &FakeBackend, child: DprcId) -> Vec<ObservedPoolObject> {
        mc.observe_pool(Some(child), Family::Dpseci).unwrap()
    }

    // The read-back detail a dpseci of `num_queues`/`options` shows, for seeding a board state.
    fn observed_detail(num_queues: u8, options: OptionMask) -> DpseciDetail {
        DpseciDetail {
            num_tx_queues: Some(num_queues),
            num_rx_queues: Some(num_queues),
            tx_priorities: vec![2; usize::from(num_queues)],
            portal: DpseciPortalReadout::Observed {
                options,
                api_major: 5,
                api_minor: 4,
            },
        }
    }

    fn has_cg() -> OptionMask {
        OptionMask::empty().with_flag(DpseciOpt::HasCg)
    }

    #[test]
    fn dpseci_absent_creates_one_per_signature_and_is_idempotent() {
        // dpseci-typestate design D9 / censusAbsentCreatesTest + censusTwoSignaturesTest: one create per signature, then idempotent.
        let compiled = compiled_crypto(&[4, 8]);
        let child = DprcId::new(2);
        let mc = FakeBackend::new();

        let (cplan, pop) = sec_populate(&mc, child, &compiled);
        assert!(
            cplan.dpseci.is_disruptive(),
            "two absent signatures to create"
        );
        assert_eq!(cplan.headline(), Class::Disruptive);
        assert_eq!(dpseci_rows(&mc, child).len(), 2, "one dpseci per signature");
        assert!(pop.dpseci.converged());
        assert!(pop.converged(cplan.dpnis.len()), "{pop:?}");

        let (cplan2, _pop2) = sec_populate(&mc, child, &compiled);
        assert!(
            cplan2.dpseci.converged(),
            "the re-plan reads the census converged"
        );
        assert!(!cplan2.dpseci.is_disruptive());
        assert_eq!(dpseci_rows(&mc, child).len(), 2, "no third dpseci");
    }

    #[test]
    fn dpseci_signature_mismatch_destroys_then_creates() {
        // dpseci-typestate design D5/D9 / censusSignatureMismatchTest: a signature mismatch is one destroy AND one create.
        let compiled = compiled_crypto(&[8]);
        let child = DprcId::new(2);
        let seeded = ObjectRef::new(Family::Dpseci, 50);
        let mc = FakeBackend::new().with_dpseci_object(
            child,
            seeded,
            &ConstructName::from("sec"),
            observed_detail(8, OptionMask::empty()),
        );

        let (cplan, pop) = sec_populate(&mc, child, &compiled);
        match &cplan.dpseci {
            ChildDpseci::Census(c) => {
                assert_eq!(c.delta.creates.values().sum::<i64>(), 1, "one create");
                assert_eq!(c.delta.destroys.values().sum::<i64>(), 1, "one destroy");
            }
            other @ ChildDpseci::Unobservable { .. } => panic!("expected a census, got {other:?}"),
        }
        assert_eq!(cplan.headline(), Class::Disruptive);

        let rows = dpseci_rows(&mc, child);
        assert!(
            !rows.iter().any(|r| r.object == seeded),
            "the mismatched signature is destroyed"
        );
        assert_eq!(rows.len(), 1, "exactly the replacement dpseci");
        let detail = mc.observe_dpseci(child, rows[0].object).unwrap();
        assert!(
            matches!(&detail.portal, DpseciPortalReadout::Observed { options: m, .. } if m.contains(DpseciOpt::HasCg)),
            "the replacement carries the desired HAS_CG signature"
        );
        assert!(pop.dpseci.converged());
    }

    #[test]
    fn dpseci_matching_board_is_zero_actions() {
        // dpseci-typestate design D9 / censusMatchEmptyDeltaTest: an already-observed board is zero actions.
        let compiled = compiled_crypto(&[4, 8]);
        let child = DprcId::new(2);
        let four = ObjectRef::new(Family::Dpseci, 40);
        let eight = ObjectRef::new(Family::Dpseci, 80);
        let mc = FakeBackend::new()
            .with_dpseci_object(
                child,
                four,
                &ConstructName::from("sec"),
                observed_detail(4, has_cg()),
            )
            .with_dpseci_object(
                child,
                eight,
                &ConstructName::from("sec"),
                observed_detail(8, has_cg()),
            );

        let (cplan, _pop) = sec_populate(&mc, child, &compiled);
        assert!(
            matches!(&cplan.dpseci, ChildDpseci::Census(c) if c.delta.is_empty()),
            "a matching multiset is zero-delta: {:?}",
            cplan.dpseci
        );
        let rows = dpseci_rows(&mc, child);
        assert_eq!(rows.len(), 2, "no dpseci created");
        assert!(
            rows.iter().any(|r| r.object == four) && rows.iter().any(|r| r.object == eight),
            "both seeded dpsecis survive — no destroy+recreate churn"
        );
    }

    #[test]
    fn dpseci_unobservable_portal_judges_nothing() {
        // dpseci-typestate design D9 / V-LIFE-DPSECI-1: an unobservable portal makes the census judge nothing.
        let compiled = compiled_crypto(&[8]);
        let child = DprcId::new(2);
        let seeded = ObjectRef::new(Family::Dpseci, 50);
        let blind = DpseciDetail {
            num_tx_queues: Some(8),
            num_rx_queues: Some(8),
            tx_priorities: vec![2; 8],
            portal: DpseciPortalReadout::Unobservable {
                reason: "no /dev/dprc portal this run".to_owned(),
            },
        };
        let mc = FakeBackend::new().with_dpseci_object(
            child,
            seeded,
            &ConstructName::from("sec"),
            blind,
        );

        let (cplan, pop) = sec_populate(&mc, child, &compiled);
        assert!(
            matches!(cplan.dpseci, ChildDpseci::Unobservable { .. }),
            "an unobservable signature judges nothing: {:?}",
            cplan.dpseci
        );
        assert!(cplan.dpseci.converged(), "no pending dpseci work");
        assert!(
            !cplan.dpseci.is_disruptive(),
            "unobservable contributes no drift"
        );

        let rows = dpseci_rows(&mc, child);
        assert_eq!(rows.len(), 1, "no create and no destroy under unobservable");
        assert!(
            rows.iter().any(|r| r.object == seeded),
            "the unobservable dpseci is never destroyed (a spurious rebuild is permanent loss)"
        );
        assert!(matches!(pop.dpseci, ChildDpseci::Unobservable { .. }));
    }

    #[test]
    fn dpseci_reordering_crypto_blocks_is_position_independent() {
        // dpseci-typestate design D9 / censusPositionIndependentTest: a crypto-block reorder is zero-delta (multiset).
        let child = DprcId::new(2);
        let mc = FakeBackend::new();
        sec_populate(&mc, child, &compiled_crypto(&[4, 8]));
        assert_eq!(dpseci_rows(&mc, child).len(), 2);

        let reversed = compiled_crypto(&[8, 4]);
        let container = Container::Child("sec".into());
        let label = ConstructName::from("sec");
        let cplan = plan_child_population(
            &mc,
            &mc,
            child,
            &reversed.plan,
            &container,
            &label,
            &sec_declared(),
        )
        .expect("plan");
        assert!(
            matches!(&cplan.dpseci, ChildDpseci::Census(c) if c.delta.is_empty()),
            "a reordered intent over a converged board is zero-delta: {:?}",
            cplan.dpseci
        );
        assert!(cplan.dpseci.converged());
    }

    #[test]
    fn dpseci_foreign_row_is_never_counted_or_destroyed() {
        // dpseci-typestate custody (pool label_membership precedent): a foreign-labelled row is never counted or destroyed.
        let compiled = compiled_crypto(&[8]);
        let child = DprcId::new(2);
        let foreign = ObjectRef::new(Family::Dpseci, 77);
        let mc = FakeBackend::new().with_dpseci_object(
            child,
            foreign,
            &ConstructName::from("vendor"),
            observed_detail(8, has_cg()),
        );

        let (cplan, _pop) = sec_populate(&mc, child, &compiled);
        match &cplan.dpseci {
            ChildDpseci::Census(c) => {
                assert!(c.observed.is_empty(), "custody excludes the vendor row");
                assert_eq!(
                    c.delta.creates.values().sum::<i64>(),
                    1,
                    "sec's dpseci is still absent — the foreign match does not count"
                );
                assert!(
                    c.delta.destroys.is_empty(),
                    "a foreign row is never destroyed"
                );
            }
            other @ ChildDpseci::Unobservable { .. } => panic!("expected a census, got {other:?}"),
        }

        let rows = dpseci_rows(&mc, child);
        assert!(
            rows.iter().any(|r| r.object == foreign),
            "the foreign dpseci survives"
        );
        assert_eq!(rows.len(), 2, "the foreign row plus one created sec dpseci");
    }
}
