//! An in-memory fake MC/kernel backend (design D10; restool-baseline): the hardware-free test seam.
//!
//! Because the southbound is a pair of traits, a test double that implements
//! [`McControl`] and [`KernelControl`] over an in-memory [`ObservedTopology`] lets
//! the full observe → reconcile → act → re-observe loop run with zero hardware. It
//! reproduces the two board-verified behaviours that matter for correctness: a
//! connected DPNI **inherits its DPMAC's MAC**, and a PHY-backed netdev appears
//! **asynchronously** some ticks after connection.
//!
//! Enable with the `testkit` feature to use it from downstream crates.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::contract::{KernelControl, McControl};
use crate::core::error::Error;
use crate::core::family::Family;
use crate::core::inventory::Inventory;
use crate::core::model::{
    DpmacId, DpniId, DprcId, LinkType, MacAddr, ObjectRef, ObservedDpmac, ObservedDpni,
    ObservedTopology,
};
use crate::core::types::ConstructName;
use crate::families::dpio::{DpioCfg, Priorities};
use crate::families::dpni::{DpniCfg, DpniObservation};
use crate::families::dprc::ContainerState;
use crate::families::pool_lifecycle::{ObservedPoolObject, RawDriver, RawLabel};
use crate::intent::compiled::Container;
use crate::plan::dprc::ObservedContainer;

/// The netdev name the fake assigns a DPNI once its PHY-backed link is up.
fn netdev_name(id: DpniId) -> String {
    format!("eth{}", id.into_inner())
}

/// A configured DPMAC the fake exposes (fixed board state).
struct FakeDpmac {
    link_type: LinkType,
    mac: MacAddr,
}

struct FakeState {
    dpmacs: HashMap<DpmacId, FakeDpmac>,
    dpnis: Vec<ObservedDpni>,
    next_index: u32,
    /// Global observation tick, advanced once per [`McControl::observe`].
    tick: u64,
    /// Ticks after connection before a PHY netdev becomes visible.
    bind_latency: u64,
    /// Per-DPNI tick at which its netdev becomes visible.
    ready_at: HashMap<DpniId, u64>,
    /// The hardware offer [`McControl::read_inventory`] returns; injected by tests
    /// (design D2; ADR-0002). Defaults empty — the board offers nothing until seeded.
    inventory: Inventory,
    /// Next child-DPRC id handed out by [`McControl::dprc_create`]; `dprc.1` is the
    /// root, so children start at `dprc.2`.
    next_dprc: u32,
    /// Child containers this backend holds, keyed by handle — the state
    /// [`McControl::dprc_create`] populates and [`McControl::observe_containers`]
    /// re-queries, so a create-then-reobserve loop converges idempotently (DPRC-I6).
    containers: BTreeMap<DprcId, ObservedContainer>,
    /// When set, the next [`McControl::dprc_create`] returns this typed refusal instead
    /// of minting a container — the one-shot seam that drives the refusal (non-zero
    /// exit) path. Consumed on use ([`Error`] is not `Clone`).
    refuse_dprc_create: Option<Error>,
    /// The create blocks handed to [`McControl::create_dpni`], in call order — the fake
    /// records what it was told to build so a test can assert the compiled cfg reached
    /// the backend verbatim (dpni-typestate task 4.1).
    created_cfgs: Vec<(DpniId, DpniCfg)>,
    /// The pool objects [`McControl::dpbp_create`] and friends have minted, observed by
    /// [`McControl::observe_pool`] — each paired with the container it was created in
    /// (`None` ⇒ [`DprcId::ROOT`], the shim default), so a child population is observable
    /// scoped to its own child (pool-objects task 3.3). Each created object is stamped and
    /// plugged (pool-objects task 3.1).
    pool_objects: Vec<(DprcId, ObservedPoolObject)>,
    /// Next pool-object ordinal, shared across families — unique enough for the fake.
    next_pool: u32,
    /// Pool objects a consumer holds that `dprc show` cannot reveal — the ground-truth draw
    /// the unplug probe discovers (pool-objects design D10). `observe_pool` still reports
    /// these `drawn: false` (restool has no draw column), so a reclaim selects them and the
    /// `--plugged=0` probe bounces `-EBUSY`, the in-use refusal that IS the drawn signal.
    in_use: HashSet<ObjectRef>,
    /// Child-dpni connection edges (pool-objects design D11): the peer each dpni was
    /// connected to by [`McControl::connect_in`], read back by
    /// [`McControl::observe_endpoint`]. A child dpni is a pool row (see
    /// [`FakeBackend::create_dpni_in`]), not an [`ObservedDpni`], so its connection lives
    /// here, not on a `connected_to` field.
    endpoints: HashMap<DpniId, ObjectRef>,
    /// The bus driver bound to a child dprc, read by [`KernelControl::bound_driver`]
    /// (pool-objects design D11). Seeded by [`FakeBackend::with_bound_dprc`]: the VFIO
    /// handoff is a board-only face, so `vfio_bind` stays a no-op and a bound child is a
    /// test seeding, the guard input the population pass reads back (ADR-0017).
    bound: HashMap<DprcId, RawDriver>,
}

/// In-memory fake implementing both southbound ports over a shared state.
pub struct FakeBackend {
    state: RefCell<FakeState>,
}

impl FakeBackend {
    /// Creates an empty backend with no DPMACs and immediate netdev appearance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RefCell::new(FakeState {
                dpmacs: HashMap::new(),
                dpnis: Vec::new(),
                next_index: 1,
                tick: 0,
                bind_latency: 0,
                ready_at: HashMap::new(),
                inventory: Inventory::default(),
                next_dprc: 2,
                containers: BTreeMap::new(),
                refuse_dprc_create: None,
                created_cfgs: Vec::new(),
                pool_objects: Vec::new(),
                next_pool: 0,
                in_use: HashSet::new(),
                endpoints: HashMap::new(),
                bound: HashMap::new(),
            }),
        }
    }

    /// Mints one plugged, stamped pool object of `family` in `container` (`None` ⇒
    /// [`DprcId::ROOT`], the shim default) and records it so a later
    /// [`McControl::observe_pool`] of that container reads it back (create → stamp → plug;
    /// pool-objects task 3.1).
    fn push_pool_object(
        &self,
        container: Option<DprcId>,
        family: Family,
        label: &ConstructName,
    ) -> ObjectRef {
        let mut st = self.state.borrow_mut();
        let object = ObjectRef::new(family, st.next_pool);
        st.next_pool += 1;
        st.pool_objects.push((
            container.unwrap_or(DprcId::ROOT),
            ObservedPoolObject {
                object,
                label: RawLabel::from(label.as_str()),
                plugged: true,
                drawn: false,
            },
        ));
        object
    }

    /// Seeds a pool object into `container` as if a prior run or bare restool had left it,
    /// so a test can inject an orphan the create verbs cannot mint — an unplugged foreign
    /// row a child population then prunes (pool-objects task 3.3). The ordinal advances the
    /// next-pool counter so a later create never collides.
    #[must_use]
    pub fn with_pool_object(self, container: DprcId, object: ObservedPoolObject) -> Self {
        {
            let mut st = self.state.borrow_mut();
            if object.object.ordinal() >= st.next_pool {
                st.next_pool = object.object.ordinal() + 1;
            }
            st.pool_objects.push((container, object));
        }
        self
    }

    /// Seeds a pool object a consumer draws that `dprc show` cannot reveal: `observe_pool`
    /// reports it `drawn: false` like restool, but the unplug probe (`dprc assign
    /// --plugged=0`) bounces `-EBUSY` — the in-use refusal that IS the drawn signal
    /// (pool-objects design D10). `object.drawn` is forced `false` so the row reads free to a
    /// census; the draw lives only in the hidden in-use set.
    #[must_use]
    pub fn with_in_use_pool_object(
        self,
        container: DprcId,
        mut object: ObservedPoolObject,
    ) -> Self {
        object.drawn = false;
        let objref = object.object;
        let backend = self.with_pool_object(container, object);
        backend.state.borrow_mut().in_use.insert(objref);
        backend
    }

    /// The create blocks handed to [`McControl::create_dpni`], in call order, so a test
    /// can assert the compiled cfg reached the backend verbatim (dpni-typestate task 4.1).
    #[must_use]
    pub fn created_cfgs(&self) -> Vec<(DpniId, DpniCfg)> {
        self.state.borrow().created_cfgs.clone()
    }

    /// Makes the next [`McControl::dprc_create`] refuse with `error` — a typed shim
    /// refusal (`Error::McStatus`/`Error::RestoolGuard`) — so tests exercise the refusal
    /// path (typed attribution, non-zero exit) without a board. One-shot: consumed on
    /// the first create.
    #[must_use]
    pub fn with_dprc_create_refusal(self, error: Error) -> Self {
        self.state.borrow_mut().refuse_dprc_create = Some(error);
        self
    }

    /// Seeds the hardware offer [`McControl::read_inventory`] returns, so the
    /// compile path can be driven with a chosen board offer and no hardware
    /// (design D2; ADR-0002; bead gqf.19).
    #[must_use]
    pub fn with_inventory(self, inventory: Inventory) -> Self {
        self.state.borrow_mut().inventory = inventory;
        self
    }

    /// Seeds a child container at `id` as if a prior run or bare restool had created it,
    /// so tests can inject an orphan whose fingerprint [`McControl::dprc_create`] cannot
    /// produce — a partial mask, a voided label, or seeded residents. Feeds the
    /// dprc-encapsulation task 4.3 prune fixtures; the id advances the next-child counter
    /// so a later create never collides.
    #[must_use]
    pub fn with_container(self, id: DprcId, container: ObservedContainer) -> Self {
        {
            let mut st = self.state.borrow_mut();
            st.containers.insert(id, container);
            if id.into_inner() >= st.next_dprc {
                st.next_dprc = id.into_inner() + 1;
            }
        }
        self
    }

    /// Seeds a child dprc as VFIO-bound, the guard input the population pass reads back
    /// (pool-objects design D11; ADR-0017): `bound_driver` returns `driver`, so a bound
    /// child with pending residents is the typed drift refusal and a bound converged child
    /// skips the handoff. The board-only bind face is never actuated by the fake, so this
    /// is the only way to reach a bound child in a hardware-free test.
    #[must_use]
    pub fn with_bound_dprc(self, id: DprcId, driver: RawDriver) -> Self {
        self.state.borrow_mut().bound.insert(id, driver);
        self
    }

    /// Sets how many observation ticks pass after connect before a PHY netdev
    /// appears, simulating the driver's asynchronous probe.
    #[must_use]
    pub fn with_bind_latency(self, ticks: u64) -> Self {
        self.state.borrow_mut().bind_latency = ticks;
        self
    }

    /// Registers a DPMAC with the given link type and burned-in MAC.
    #[must_use]
    pub fn with_dpmac(self, id: DpmacId, link_type: LinkType, mac: MacAddr) -> Self {
        self.state
            .borrow_mut()
            .dpmacs
            .insert(id, FakeDpmac { link_type, mac });
        self
    }

    /// Seeds an already-connected (and, for PHY, already-bound) DPNI, as if a prior
    /// run or the DPL had provisioned it. Used to test idempotence and foreign
    /// preservation.
    #[must_use]
    pub fn with_connected_dpni(self, dpni: DpniId, dpmac: DpmacId) -> Self {
        {
            let mut st = self.state.borrow_mut();
            let mac = st.dpmacs.get(&dpmac).map(|m| m.mac);
            let netdev = match st.dpmacs.get(&dpmac).map(|m| m.link_type) {
                Some(LinkType::Phy) => Some(netdev_name(dpni)),
                _ => None,
            };
            st.dpnis.push(ObservedDpni {
                id: dpni,
                label: None,
                connected_to: Some(dpmac),
                mac,
                netdev,
                attributes: BTreeMap::new(),
                cfg_observation: None,
            });
            if dpni.into_inner() >= st.next_index {
                st.next_index = dpni.into_inner() + 1;
            }
        }
        self
    }

    /// Returns the netdev name of the DPNI connected to `dpmac`, if visible now.
    /// Test convenience for asserting the rename target.
    #[must_use]
    pub fn netdev_for_dpmac(&self, dpmac: DpmacId) -> Option<String> {
        let st = self.state.borrow();
        let dpni = st.dpnis.iter().find(|d| d.connected_to == Some(dpmac))?;
        Self::visible_netdev(&st, dpni)
    }

    fn visible_netdev(st: &FakeState, dpni: &ObservedDpni) -> Option<String> {
        let dpmac = dpni.connected_to?;
        let fm = st.dpmacs.get(&dpmac)?;
        if fm.link_type != LinkType::Phy {
            return None;
        }
        let ready = st.ready_at.get(&dpni.id).copied().unwrap_or(0);
        if st.tick >= ready {
            Some(netdev_name(dpni.id))
        } else {
            None
        }
    }
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl McControl for FakeBackend {
    fn observe(&self) -> Result<ObservedTopology, Error> {
        let mut st = self.state.borrow_mut();
        st.tick += 1;

        let dpmacs = st
            .dpmacs
            .iter()
            .map(|(id, m)| ObservedDpmac {
                id: *id,
                link_type: m.link_type,
                mac: Some(m.mac),
            })
            .collect();

        // MC observe reports objects and edges but not netdevs; the shell enriches
        // netdev via KernelControl. We still fill netdev here for backends that use
        // the fake standalone in reconcile tests.
        let dpnis = st
            .dpnis
            .iter()
            .map(|d| ObservedDpni {
                netdev: Self::visible_netdev(&st, d),
                ..d.clone()
            })
            .collect();

        Ok(ObservedTopology { dpnis, dpmacs })
    }

    fn read_inventory(&self) -> Result<Inventory, Error> {
        Ok(self.state.borrow().inventory.clone())
    }

    fn create_dpni(
        &self,
        label: &crate::core::types::ConstructName,
        cfg: &DpniCfg,
    ) -> Result<DpniId, Error> {
        let mut st = self.state.borrow_mut();
        let id = DpniId::new(st.next_index);
        st.next_index += 1;
        // Record the handed-in block so a test can assert it reached the backend (dpni-typestate task 4.1).
        st.created_cfgs.push((id, cfg.clone()));
        // The object is stamped with the construct name at create (ADR-0010 §4 ABA
        // guard), so a re-observe never sees it unlabelled.
        st.dpnis.push(ObservedDpni {
            id,
            label: Some(label.clone()),
            connected_to: None,
            mac: None,
            netdev: None,
            attributes: BTreeMap::new(),
            // Project the create's read-back so fake-vs-reconcile tests exercise cfg drift (7fv.2).
            cfg_observation: Some(DpniObservation::project(cfg)),
        });
        Ok(id)
    }

    // A bare child dpni is modeled as a plugged, stamped pool row in `container`, so
    // `observe_pool(Some(child), Dpni)` reads it back — the dpni half of a child
    // population (pool-objects task 3.3). No dependency chain and no connect, matching
    // the shim's bare create; the id follows the pool ordinal.
    fn create_dpni_in(
        &self,
        container: DprcId,
        _cfg: &DpniCfg,
        label: &ConstructName,
    ) -> Result<DpniId, Error> {
        let obj = self.push_pool_object(Some(container), Family::Dpni, label);
        Ok(DpniId::new(obj.ordinal()))
    }

    fn connect(&self, dpni: DpniId, dpmac: DpmacId) -> Result<(), Error> {
        let mut st = self.state.borrow_mut();
        let tick = st.tick;
        let latency = st.bind_latency;
        let inherited = st.dpmacs.get(&dpmac).map(|m| m.mac);
        let obj = st
            .dpnis
            .iter_mut()
            .find(|d| d.id == dpni)
            .ok_or_else(|| Error::Backend(format!("{dpni} does not exist")))?;
        obj.connected_to = Some(dpmac);
        // A connected DPNI inherits the DPMAC's MAC (board-verified).
        if obj.mac.is_none() {
            obj.mac = inherited;
        }
        st.ready_at.insert(dpni, tick + latency);
        Ok(())
    }

    // A plain insert records the ancestor-connect edge; a same-peer re-connect is a no-op,
    // the idempotence the converge relies on (pool-objects design D11).
    fn connect_in(&self, _ancestor: DprcId, dpni: DpniId, peer: ObjectRef) -> Result<(), Error> {
        self.state.borrow_mut().endpoints.insert(dpni, peer);
        Ok(())
    }

    // A recorded child-dpni edge, else a root dpni's `connected_to` as a dpmac ref, else None.
    fn observe_endpoint(&self, dpni: DpniId) -> Result<Option<ObjectRef>, Error> {
        let st = self.state.borrow();
        if let Some(&peer) = st.endpoints.get(&dpni) {
            return Ok(Some(peer));
        }
        Ok(st
            .dpnis
            .iter()
            .find(|d| d.id == dpni)
            .and_then(|d| d.connected_to)
            .map(|m| ObjectRef::new(Family::Dpmac, m.into_inner())))
    }

    fn set_mac(&self, dpni: DpniId, mac: MacAddr) -> Result<(), Error> {
        let mut st = self.state.borrow_mut();
        let obj = st
            .dpnis
            .iter_mut()
            .find(|d| d.id == dpni)
            .ok_or_else(|| Error::Backend(format!("{dpni} does not exist")))?;
        obj.mac = Some(mac);
        Ok(())
    }

    fn set_label(
        &self,
        dpni: DpniId,
        label: &crate::core::types::ConstructName,
    ) -> Result<(), Error> {
        let mut st = self.state.borrow_mut();
        let obj = st
            .dpnis
            .iter_mut()
            .find(|d| d.id == dpni)
            .ok_or_else(|| Error::Backend(format!("{dpni} does not exist")))?;
        obj.label = Some(label.clone());
        Ok(())
    }

    fn disconnect(&self, dpni: DpniId) -> Result<(), Error> {
        let mut st = self.state.borrow_mut();
        let obj = st
            .dpnis
            .iter_mut()
            .find(|d| d.id == dpni)
            .ok_or_else(|| Error::Backend(format!("{dpni} does not exist")))?;
        obj.connected_to = None;
        obj.netdev = None;
        st.ready_at.remove(&dpni);
        Ok(())
    }

    fn destroy(&self, dpni: DpniId) -> Result<(), Error> {
        let mut st = self.state.borrow_mut();
        st.dpnis.retain(|d| d.id != dpni);
        st.ready_at.remove(&dpni);
        Ok(())
    }

    fn observe_containers(&self) -> Result<BTreeMap<DprcId, ObservedContainer>, Error> {
        Ok(self.state.borrow().containers.clone())
    }

    // Per-candidate re-observation is a map lookup here; `None` is honest absence (dpni-typestate design D5).
    fn observe_container(&self, id: DprcId) -> Result<Option<ObservedContainer>, Error> {
        Ok(self.state.borrow().containers.get(&id).cloned())
    }

    // `dprc_create` mints a fresh, unplugged child container and records it so a
    // subsequent [`observe_containers`] re-queries it (the create-then-reobserve loop,
    // DPRC-I6). A created DPRC reads back on the [`ContainerState::Created`] (unplugged)
    // face with its create-time mask (the whole mutation — restool cannot plug a DPRC).
    fn dprc_create(
        &self,
        parent: DprcId,
        options: crate::families::dprc::Options,
        label: &crate::core::types::ConstructName,
    ) -> Result<DprcId, Error> {
        let mut st = self.state.borrow_mut();
        if let Some(error) = st.refuse_dprc_create.take() {
            return Err(error);
        }
        let id = DprcId::new(st.next_dprc);
        st.next_dprc += 1;
        // The container-only scope creates consumers under the root (`dprc.1`,
        // [`DprcId::ROOT`]); the fake models that single placement.
        debug_assert_eq!(parent, DprcId::ROOT);
        st.containers.insert(
            id,
            ObservedContainer {
                state: ContainerState::Created,
                options,
                label: label.clone(),
                placement: Container::Root,
                residents: BTreeMap::new(),
            },
        );
        Ok(id)
    }

    // A plugged resident bounces destroy `-EBUSY` (MC `0x10`); the eviction law unplugs first (ADR-0007 §3).
    fn dprc_destroy(&self, container: DprcId) -> Result<(), Error> {
        let mut st = self.state.borrow_mut();
        if let Some(observed) = st.containers.get(&container)
            && observed.residents.values().any(|r| r.plugged)
        {
            return Err(Error::McStatus { status: 0x10 });
        }
        st.containers.remove(&container);
        Ok(())
    }

    // `--plugged=0` is the reclaim unplug probe: the MC refuses it `-EBUSY` (`0x10`) when the object is drawn — the in-use refusal IS the drawn signal (pool-objects design D10).
    fn dprc_assign(
        &self,
        _container: DprcId,
        object: ObjectRef,
        child: Option<DprcId>,
        plugged: Option<bool>,
    ) -> Result<(), Error> {
        if child.is_some() {
            return Ok(());
        }
        let Some(plugged) = plugged else {
            return Ok(());
        };
        let mut st = self.state.borrow_mut();
        let in_use = st.in_use.contains(&object);
        if let Some((_, obj)) = st.pool_objects.iter_mut().find(|(_, o)| o.object == object) {
            if !plugged && (obj.drawn || in_use) {
                return Err(Error::McStatus { status: 0x10 });
            }
            obj.plugged = plugged;
        }
        Ok(())
    }

    fn dprc_unassign(
        &self,
        _parent: DprcId,
        _child: DprcId,
        _object: ObjectRef,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn dprc_set_label(
        &self,
        _container: DprcId,
        _label: &crate::core::types::ConstructName,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn dprc_set_locked(&self, _child: DprcId, _locked: bool) -> Result<(), Error> {
        Ok(())
    }

    // The pool verbs record the create's `container` (None ⇒ root); each create stamps and
    // plugs so `observe_pool` reads it back plugged and undrawn (pool-objects task 3.1; draw seeded separately, design D10).
    fn dpbp_create(
        &self,
        container: Option<DprcId>,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        Ok(self.push_pool_object(container, Family::Dpbp, label))
    }

    fn dpmcp_create(
        &self,
        container: Option<DprcId>,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        Ok(self.push_pool_object(container, Family::Dpmcp, label))
    }

    fn dpcon_create(
        &self,
        container: Option<DprcId>,
        _priorities: Priorities,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        Ok(self.push_pool_object(container, Family::Dpcon, label))
    }

    fn dpio_create(
        &self,
        container: Option<DprcId>,
        _cfg: DpioCfg,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        Ok(self.push_pool_object(container, Family::Dpio, label))
    }

    fn pool_destroy(&self, object: &ObjectRef) -> Result<(), Error> {
        self.state
            .borrow_mut()
            .pool_objects
            .retain(|(_, o)| &o.object != object);
        Ok(())
    }

    fn observe_pool(
        &self,
        container: Option<DprcId>,
        family: Family,
    ) -> Result<Vec<ObservedPoolObject>, Error> {
        let want = container.unwrap_or(DprcId::ROOT);
        Ok(self
            .state
            .borrow()
            .pool_objects
            .iter()
            .filter(|(c, o)| *c == want && o.object.family() == family)
            .map(|(_, o)| o.clone())
            .collect())
    }
}

impl KernelControl for FakeBackend {
    fn bind(&self, _dpni: DpniId) -> Result<(), Error> {
        // Binding is automatic on plug for `dpaa2-eth`; nothing to force here.
        Ok(())
    }

    fn netdev_of(&self, dpni: DpniId) -> Result<Option<String>, Error> {
        let st = self.state.borrow();
        let Some(obj) = st.dpnis.iter().find(|d| d.id == dpni) else {
            return Ok(None);
        };
        Ok(Self::visible_netdev(&st, obj))
    }

    // One honest source: the fake reports fsl_dpaa2_eth bound exactly when its netdev is
    // visible, so dpni_driver and netdev_of never disagree (DPNI-I4 read-back).
    fn dpni_driver(&self, dpni: DpniId) -> Result<Option<RawDriver>, Error> {
        let st = self.state.borrow();
        Ok(st
            .dpnis
            .iter()
            .find(|d| d.id == dpni)
            .and_then(|obj| Self::visible_netdev(&st, obj))
            .map(|_| RawDriver::from("fsl_dpaa2_eth")))
    }

    // The reconcile/convergence tests never bind VFIO (that face is board-only, design
    // D6; ADR-0004), so the fake reports an unbound, group-less, override-clear child and accepts
    // the actuations as no-ops.
    fn vfio_set_override(&self, _dprc: DprcId) -> Result<(), Error> {
        Ok(())
    }

    fn vfio_bind(&self, _dprc: DprcId) -> Result<(), Error> {
        Ok(())
    }

    fn vfio_unbind(&self, _dprc: DprcId) -> Result<(), Error> {
        Ok(())
    }

    fn bound_driver(&self, dprc: DprcId) -> Result<Option<RawDriver>, Error> {
        Ok(self.state.borrow().bound.get(&dprc).cloned())
    }

    fn driver_override(&self, _dprc: DprcId) -> Result<Option<String>, Error> {
        Ok(None)
    }

    fn iommu_group(&self, _dprc: DprcId) -> Result<Option<u32>, Error> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both directions plus the idempotent re-connect path (pool-objects design D11).
    #[test]
    fn connect_in_then_endpoint_reads_back_and_is_idempotent() {
        let backend = FakeBackend::new();
        let label = ConstructName::from("tenant-port");
        let dpni = backend
            .create_dpni_in(DprcId::new(2), &DpniCfg::defaults(), &label)
            .expect("create child dpni");
        let peer = ObjectRef::new(Family::Dpmac, 7);

        assert_eq!(backend.observe_endpoint(dpni).expect("read"), None);

        backend
            .connect_in(DprcId::new(1), dpni, peer)
            .expect("connect");
        assert_eq!(backend.observe_endpoint(dpni).expect("read"), Some(peer));

        backend
            .connect_in(DprcId::new(1), dpni, peer)
            .expect("re-connect");
        assert_eq!(backend.observe_endpoint(dpni).expect("read"), Some(peer));
    }

    // A dpni↔dpni peer reads back with its own family (the cross-container case, DPNI-I9).
    #[test]
    fn endpoint_reads_back_a_dpni_peer() {
        let backend = FakeBackend::new();
        let dpni = backend
            .create_dpni_in(
                DprcId::new(2),
                &DpniCfg::defaults(),
                &ConstructName::from("a"),
            )
            .expect("create");
        let peer = ObjectRef::new(Family::Dpni, 9);
        backend
            .connect_in(DprcId::new(1), dpni, peer)
            .expect("connect");
        assert_eq!(backend.observe_endpoint(dpni).expect("read"), Some(peer));
    }
}
