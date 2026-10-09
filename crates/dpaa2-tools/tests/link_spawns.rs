//! Spawn-count regression for the link pass (link-hardening task 5.1, S27): each `McControl` read
//! the pass issues (`observe_pool`, `dprc_get_connection`, `observe_containers`) is one restool
//! spawn through the shim, so counting them bounds the per-link spawn cost. A counting decorator
//! wraps the `FakeBackend`, `converge_links` drives a one-link compiled plan, and the total is
//! asserted exact so a regression that re-reads a cached row trips the gate.
//!
//! BEFORE this change the empty-board one-link pass cost 7 spawns (`observe_containers` 1,
//! `observe_pool` 3 — resolve + two plug checks, `dprc_get_connection` 3 — two dispatch reads + one
//! read-back). AFTER the pass cache it costs 3 (`observe_containers` 1, `observe_pool` 1 — the
//! single resolve read, `dprc_get_connection` 1 — the kept post-dispatch read-back): ~7 → ~3/link.

use std::cell::Cell;
use std::collections::BTreeMap;

use dpaa2_api::contract::DpseciDetail;
use dpaa2_api::contract::McControl;
use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::inventory::Inventory;
use dpaa2_api::core::model::{DpmacId, DpniId, DprcId, MacAddr, ObjectRef, ObservedTopology};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpio::{DpioCfg, Priorities};
use dpaa2_api::families::dpmac::DpmacObservation;
use dpaa2_api::families::dpni::{DpniCfg, LinkState};
use dpaa2_api::families::dprc;
use dpaa2_api::families::dpseci::DpseciCfg;
use dpaa2_api::families::pool_lifecycle::ObservedPoolObject;
use dpaa2_api::intent::refuse::{Compiled, compile};
use dpaa2_api::intent::{Dataplane, Intent, Isolation, Link, Tenant, TenantRef, kernel_tenant};
use dpaa2_api::plan::Class;
use dpaa2_api::plan::dprc::ObservedContainer;
use dpaa2_api::testkit::ref_inventory;
use dpaa2_tools::engine::{self, ConvergeConfig};

const LINK: &str = "l0";

// A root↔root link (the `link_faces.rs` idiom): the kernel end plus a restricted-to-kernel tenant,
// so both link-end dpnis land in dprc.1 sharing the link-name label.
fn compiled_root_link() -> Compiled {
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
    let intent = Intent {
        tenants: vec![kernel_tenant(16), secondary],
        links: vec![Link {
            name: LINK.into(),
            interface_a: TenantRef::Kernel,
            interface_b: TenantRef::from_name("sec".into()),
            renamed: None,
        }],
        ..Intent::empty()
    };
    compile(&intent, &ref_inventory(16)).expect("the root link intent compiles")
}

fn disruptive() -> ConvergeConfig {
    ConvergeConfig {
        allow: Class::Disruptive,
        ..ConvergeConfig::default()
    }
}

/// Counts the three `McControl` reads that each map to one restool spawn, delegating every verb to
/// the inner [`FakeBackend`]. The three counted reads are the only link-pass spawns the cache
/// targets; mutating verbs (create/connect/assign) stay uncounted — they are not the bead's budget.
struct SpawnCounter {
    inner: FakeBackend,
    pool: Cell<usize>,
    conn: Cell<usize>,
    containers: Cell<usize>,
}

impl SpawnCounter {
    fn new() -> Self {
        Self {
            inner: FakeBackend::new(),
            pool: Cell::new(0),
            conn: Cell::new(0),
            containers: Cell::new(0),
        }
    }

    fn spawns(&self) -> usize {
        self.pool.get() + self.conn.get() + self.containers.get()
    }

    fn bump(cell: &Cell<usize>) {
        cell.set(cell.get() + 1);
    }
}

impl McControl for SpawnCounter {
    fn observe_pool(
        &self,
        container: Option<DprcId>,
        family: Family,
    ) -> Result<Vec<ObservedPoolObject>, Error> {
        Self::bump(&self.pool);
        self.inner.observe_pool(container, family)
    }

    fn dprc_get_connection(&self, dpni: DpniId) -> Result<Option<ObjectRef>, Error> {
        Self::bump(&self.conn);
        self.inner.dprc_get_connection(dpni)
    }

    fn observe_containers(&self) -> Result<BTreeMap<DprcId, ObservedContainer>, Error> {
        Self::bump(&self.containers);
        self.inner.observe_containers()
    }

    fn observe(&self) -> Result<ObservedTopology, Error> {
        self.inner.observe()
    }
    fn read_inventory(&self) -> Result<Inventory, Error> {
        self.inner.read_inventory()
    }
    fn observe_dpmac(&self, dpmac: DpmacId) -> Result<DpmacObservation, Error> {
        self.inner.observe_dpmac(dpmac)
    }
    fn observe_dpseci(&self, container: DprcId, dpseci: ObjectRef) -> Result<DpseciDetail, Error> {
        self.inner.observe_dpseci(container, dpseci)
    }
    fn create_dpni(&self, label: &ConstructName, cfg: &DpniCfg) -> Result<DpniId, Error> {
        self.inner.create_dpni(label, cfg)
    }
    fn create_dpni_in(
        &self,
        container: DprcId,
        cfg: &DpniCfg,
        label: &ConstructName,
    ) -> Result<DpniId, Error> {
        self.inner.create_dpni_in(container, cfg, label)
    }
    fn connect(&self, dpni: DpniId, dpmac: DpmacId) -> Result<(), Error> {
        self.inner.connect(dpni, dpmac)
    }
    fn dprc_connect(&self, ancestor: DprcId, dpni: DpniId, peer: ObjectRef) -> Result<(), Error> {
        self.inner.dprc_connect(ancestor, dpni, peer)
    }
    fn dpni_get_link_state(&self, dpni: DpniId) -> Result<LinkState, Error> {
        self.inner.dpni_get_link_state(dpni)
    }
    fn set_mac(&self, dpni: DpniId, mac: MacAddr) -> Result<(), Error> {
        self.inner.set_mac(dpni, mac)
    }
    fn set_label(&self, dpni: DpniId, label: &ConstructName) -> Result<(), Error> {
        self.inner.set_label(dpni, label)
    }
    fn dprc_disconnect(&self, ancestor: DprcId, dpni: DpniId) -> Result<(), Error> {
        self.inner.dprc_disconnect(ancestor, dpni)
    }
    fn destroy(&self, dpni: DpniId) -> Result<(), Error> {
        self.inner.destroy(dpni)
    }
    fn observe_container(&self, id: DprcId) -> Result<Option<ObservedContainer>, Error> {
        self.inner.observe_container(id)
    }
    fn dprc_create(
        &self,
        parent: DprcId,
        options: dprc::Options,
        label: &ConstructName,
    ) -> Result<DprcId, Error> {
        self.inner.dprc_create(parent, options, label)
    }
    fn dprc_destroy(&self, container: DprcId) -> Result<(), Error> {
        self.inner.dprc_destroy(container)
    }
    fn dprc_assign(
        &self,
        container: DprcId,
        object: ObjectRef,
        child: Option<DprcId>,
        plugged: Option<bool>,
    ) -> Result<(), Error> {
        self.inner.dprc_assign(container, object, child, plugged)
    }
    fn dprc_unassign(&self, parent: DprcId, child: DprcId, object: ObjectRef) -> Result<(), Error> {
        self.inner.dprc_unassign(parent, child, object)
    }
    fn dprc_set_label(&self, container: DprcId, label: &ConstructName) -> Result<(), Error> {
        self.inner.dprc_set_label(container, label)
    }
    fn dprc_set_locked(&self, child: DprcId, locked: bool) -> Result<(), Error> {
        self.inner.dprc_set_locked(child, locked)
    }
    fn dpbp_create(
        &self,
        container: Option<DprcId>,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        self.inner.dpbp_create(container, label)
    }
    fn dpmcp_create(
        &self,
        container: Option<DprcId>,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        self.inner.dpmcp_create(container, label)
    }
    fn dpcon_create(
        &self,
        container: Option<DprcId>,
        priorities: Priorities,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        self.inner.dpcon_create(container, priorities, label)
    }
    fn dpio_create(
        &self,
        container: Option<DprcId>,
        cfg: DpioCfg,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        self.inner.dpio_create(container, cfg, label)
    }
    fn pool_destroy(&self, object: &ObjectRef) -> Result<(), Error> {
        self.inner.pool_destroy(object)
    }
    fn create_dpseci_in(
        &self,
        container: Option<DprcId>,
        cfg: &DpseciCfg,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        self.inner.create_dpseci_in(container, cfg, label)
    }
    fn destroy_dpseci(&self, container: Option<DprcId>, dpseci: &ObjectRef) -> Result<(), Error> {
        self.inner.destroy_dpseci(container, dpseci)
    }
}

#[test]
fn converge_links_spends_three_spawns_per_link() {
    // A one-link empty board: both root ends are created, connected, and plugged this pass. BEFORE
    // the pass cache the pass cost 7 spawns; the cache cuts the redundant resolve/plug reads and the
    // duplicated connection reads, leaving the single resolve read, the kept post-dispatch read-back,
    // and the one per-pass observe_containers (link-hardening task 5.1, S27).
    let compiled = compiled_root_link();
    let mc = SpawnCounter::new();
    let outcome = engine::converge_links(&compiled.plan, &mc, disruptive()).unwrap();
    assert!(
        matches!(outcome, engine::LinkOutcome::Converged),
        "the one-link pass converges: {outcome:?}"
    );
    assert_eq!(
        mc.spawns(),
        3,
        "one-link pass spawns: observe_pool={}, dprc_get_connection={}, observe_containers={} (was 7 before the pass cache)",
        mc.pool.get(),
        mc.conn.get(),
        mc.containers.get(),
    );
}
