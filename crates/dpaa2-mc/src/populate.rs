//! Child-container population and the VFIO handoff — the composing edge that serves a
//! userspace dataplane its own populated, VFIO-bound dprc (mc-backend spec requirement 3;
//! pool-objects design D2/D5).
//!
//! This module composes read-back-judged observations exactly as [`pool`](crate::pool)
//! composes the delta→id dispatch and [`probe`](crate::probe) composes the root-bind
//! read-back: [`populate_child`] gathers each family's child census, hands the counts to
//! the pure family surface (`dpaa2_api::families`), dispatches the deltas, and returns the
//! post-dispatch census — the read-back IS the observation, an exit status never is
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
use dpaa2_api::core::model::{DprcId, ObjectRef};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpio::derived_seats;
use dpaa2_api::families::dprc::VfioBind;
use dpaa2_api::families::pool_lifecycle::{
    PoolCensus, PoolFamily, RawDriver, census_of, derived_requirement, drift_disposition,
};
use dpaa2_api::intent::compiled::{Attributes, CompiledPlan, Container};

use crate::pool::{default_dpio_cfg, dispatch_pool_deltas};

/// The trio traversal a child population converges in: dpmcp→dpbp→dpcon, dependency
/// bottom first (everything draws a dpmcp; pool-objects design D8). dpio is not here — it
/// is a seat, converged separately below (pool-objects design D4).
const TRIO: [PoolFamily; 3] = [PoolFamily::Dpmcp, PoolFamily::Dpbp, PoolFamily::Dpcon];

/// The observed outcome of populating a child container — every field a read-back census,
/// never a driven state (mc-backend spec requirement 3: "observable as a census of the
/// child matching the derived counts").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildPopulation {
    /// The child's dpni as observed after the pass, or `None` when the plan carries none.
    pub dpni: Option<ObjectRef>,
    /// Per trio family, the derived requirement paired with the post-dispatch census.
    pub families: BTreeMap<PoolFamily, (i64, PoolCensus)>,
    /// The dpio seats: `(required, observed-after)`.
    pub seats: (i64, i64),
}

impl ChildPopulation {
    /// Whether the child is converged — the core predicates only: every trio census meets
    /// its requirement ([`PoolCensus::converged`]), the dpio seats equal their requirement,
    /// and the plan's dpni is present. The read-back census is the observation, so a `true`
    /// here is the idempotence witness a second [`populate_child`] pass reproduces.
    #[must_use]
    pub fn converged(&self) -> bool {
        self.dpni.is_some()
            && self.seats.0 == self.seats.1
            && self
                .families
                .values()
                .all(|(requirement, census)| census.converged(*requirement))
    }
}

/// Populates a child dprc with its dpni and derived pool companions, idempotent and
/// level-triggered, judged from read-back (mc-backend spec requirement 3).
/// The count→individual boundary stays in the plan (pool-objects design D2): every count
/// comes from the compiled plan and every verdict from the post-dispatch census — the
/// adapter drives, the core judges.
///
/// The pass, in order:
/// - **dpni**: observes the child's dpni rows (`observe_pool` with [`Family::Dpni`] — the
///   row read is family-generic); if the plan holds a dpni for this container and none is
///   observed, creates exactly one with the compiled cfg ([`McControl::create_dpni_in`]),
///   never a second.
/// - **trio** (dpmcp→dpbp→dpcon): observes each family's census, takes the count-level
///   [`drift_disposition`] against [`derived_requirement`], and dispatches the deltas
///   ([`dispatch_pool_deltas`]) — a foreign-free object is pruned, a requirement below the
///   drawn count surfaces as the typed
///   [`ShrinkBelowDraw`](dpaa2_api::families::pool_lifecycle::ShrinkBelowDraw) error.
/// - **dpio seats**: creates the seat deficit (`derived_seats` minus observed rows) as
///   plain `dpio_create`s, NOT [`create_dpio_seat`](crate::pool::create_dpio_seat): the
///   dpmcp-probe pairing is the *kernel* dpio driver's draw, and a VFIO child's dpio is
///   userspace-consumed (`docs/baseline/dpio.md` "Kernel-side behavior"; ADR-0012 poll
///   draws). The compiled dpio companion is [`Attributes::Unsized`], so the create-cfg is
///   the ls-addni dpio default here, not plan-drawn — a later tile.
///
/// The trio disposition judges against [`Ceiling::Unknown`]: this face threads no
/// inventory, so it takes the admit-and-warn posture (a create is never ceiling-blocked
/// here; the MC's own driver-bound-destroy refusal remains the free-only-shrink backstop,
/// pool-objects design D3).
///
/// # Errors
/// Returns the first [`Error`] any observation, create, destroy, or dispatch raises;
/// a requirement below a family's drawn count surfaces as the typed
/// [`ShrinkBelowDraw`](dpaa2_api::families::pool_lifecycle::ShrinkBelowDraw) folded to
/// [`Error::Config`].
pub fn populate_child<M: McControl>(
    mc: &M,
    child: DprcId,
    plan: &CompiledPlan,
    container: &Container,
    label: &ConstructName,
    declared: &BTreeSet<ConstructName>,
) -> Result<ChildPopulation, Error> {
    // dpni: create at most one, only if the plan wants one and none is observed.
    let dpni_cfg = plan.objects.iter().find_map(|o| {
        if o.container() == container && o.key().family == Family::Dpni {
            match o.attributes() {
                Attributes::Dpni { cfg } => Some(cfg.clone()),
                _ => None,
            }
        } else {
            None
        }
    });
    let mut dpni_rows = mc.observe_pool(Some(child), Family::Dpni)?;
    if let Some(cfg) = &dpni_cfg
        && dpni_rows.is_empty()
    {
        mc.create_dpni_in(child, cfg, label)?;
        dpni_rows = mc.observe_pool(Some(child), Family::Dpni)?;
    }
    let dpni = dpni_rows.first().map(|r| r.object);

    // trio: converge each pooled family's count and read the census straight back.
    let mut families = BTreeMap::new();
    for family in TRIO {
        let rows = mc.observe_pool(Some(child), family.family())?;
        let census = census_of(&rows, declared);
        let requirement = derived_requirement(plan, container, family);
        let deltas = drift_disposition(family, census, requirement, &Ceiling::Unknown)?;
        let dispatch = dispatch_pool_deltas(mc, Some(child), family, deltas, label, declared)?;
        families.insert(family, (requirement, census_of(&dispatch.after, declared)));
    }

    // dpio seats: create the deficit as plain seats (kernel-draw pairing is not ours here).
    let required = derived_seats(plan, container);
    let observed =
        i64::try_from(mc.observe_pool(Some(child), Family::Dpio)?.len()).unwrap_or(i64::MAX);
    for _ in 0..(required - observed).max(0) {
        mc.dpio_create(Some(child), default_dpio_cfg(), label)?;
    }
    let observed_after =
        i64::try_from(mc.observe_pool(Some(child), Family::Dpio)?.len()).unwrap_or(i64::MAX);

    Ok(ChildPopulation {
        dpni,
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
    //! Child population and the VFIO handoff driven through [`FakeBackend`]: a compiled
    //! userspace-poll tenant populates to its derived census (dpbp 2, dpmcp 1, dpcon 3,
    //! dpio 6, one dpni) and reads back converged; a second pass creates nothing (the
    //! idempotence acceptance); a pre-seeded foreign-free object is pruned; and the handoff
    //! classifies the fixture's bound driver.

    use dpaa2_api::contract::fake::FakeBackend;
    use dpaa2_api::core::model::{DpmacId, MacMode};
    use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};
    use dpaa2_api::intent::refuse::compile;
    use dpaa2_api::intent::{Dataplane, Intent, Isolation, Port, Tenant, TenantRef};
    use dpaa2_api::testkit::ref_inventory;

    use super::*;

    // The pool_lifecycle.rs fixture idiom: a userspace-poll tenant terminating one 10G port
    // (T = 1 + 2 = 3), compiled to a plan carrying its companions in its own child.
    fn compiled_vpp() -> dpaa2_api::intent::refuse::Compiled {
        let intent = Intent {
            tenants: vec![Tenant {
                name: "vpp".into(),
                dataplane: Dataplane::UserspacePoll,
                max_cores: 16,
                isolation: Isolation::Isolated,
                renamed: None,
            }],
            ports: vec![Port {
                name: "wan0".into(),
                dpmac: DpmacId::new(7),
                rate: 10_000,
                tenant: TenantRef::from_name("vpp".into()),
                mac: None,
                mac_mode: MacMode::Assert,
                renamed: None,
            }],
            ..Intent::empty()
        };
        compile(&intent, &ref_inventory(16)).expect("intent compiles")
    }

    fn declared() -> BTreeSet<ConstructName> {
        BTreeSet::from([ConstructName::from("vpp")])
    }

    fn count(mc: &FakeBackend, child: DprcId, family: Family) -> usize {
        mc.observe_pool(Some(child), family).unwrap().len()
    }

    #[test]
    fn populate_converges_to_the_derived_census_and_is_idempotent() {
        // pool-objects task 3.3 / mc-backend req 3: the child reads back the regime-derived
        // counts and converged; a second pass creates nothing.
        let compiled = compiled_vpp();
        let child = DprcId::new(2);
        let container = Container::Child("vpp".into());
        let label = ConstructName::from("vpp");
        let mc = FakeBackend::new();

        let first = populate_child(&mc, child, &compiled.plan, &container, &label, &declared())
            .expect("populate");
        assert!(first.converged(), "{first:?}");
        // The derived census: dpbp 2, dpmcp 1, dpcon 3 (dpnis·T), dpio 6 (2·T), one dpni.
        assert_eq!(count(&mc, child, Family::Dpbp), 2);
        assert_eq!(count(&mc, child, Family::Dpmcp), 1);
        assert_eq!(count(&mc, child, Family::Dpcon), 3);
        assert_eq!(count(&mc, child, Family::Dpio), 6);
        assert_eq!(count(&mc, child, Family::Dpni), 1);
        assert!(first.dpni.is_some());
        assert_eq!(first.seats, (6, 6));

        let before: Vec<usize> = [
            Family::Dpbp,
            Family::Dpmcp,
            Family::Dpcon,
            Family::Dpio,
            Family::Dpni,
        ]
        .iter()
        .map(|&f| count(&mc, child, f))
        .collect();
        let second = populate_child(&mc, child, &compiled.plan, &container, &label, &declared())
            .expect("re-populate");
        assert!(second.converged());
        let after: Vec<usize> = [
            Family::Dpbp,
            Family::Dpmcp,
            Family::Dpcon,
            Family::Dpio,
            Family::Dpni,
        ]
        .iter()
        .map(|&f| count(&mc, child, f))
        .collect();
        assert_eq!(
            before, after,
            "a converged child is not grown a second time"
        );
    }

    #[test]
    fn a_foreign_free_object_in_the_child_is_pruned() {
        // pool-objects design D3: an undeclared, unplugged dpbp the population never made is
        // reclaimed by the prune disposition, and the child still converges.
        let compiled = compiled_vpp();
        let child = DprcId::new(2);
        let container = Container::Child("vpp".into());
        let label = ConstructName::from("vpp");
        let foreign = ObservedPoolObject {
            object: ObjectRef::new(Family::Dpbp, 99),
            label: RawLabel::from("vendor"),
            plugged: false,
        };
        let mc = FakeBackend::new().with_pool_object(child, foreign.clone());

        let pop = populate_child(&mc, child, &compiled.plan, &container, &label, &declared())
            .expect("populate");
        assert!(pop.converged(), "{pop:?}");
        let dpbps = mc.observe_pool(Some(child), Family::Dpbp).unwrap();
        assert!(!dpbps.iter().any(|o| o.object == foreign.object), "pruned");
        assert_eq!(dpbps.len(), 2);
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
