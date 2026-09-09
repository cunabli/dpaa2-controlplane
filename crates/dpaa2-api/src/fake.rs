//! An in-memory fake MC/kernel backend (design D10): the hardware-free test seam.
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

use crate::compiled::Container;
use crate::dprc::ContainerState;
use crate::dprc_plan::ObservedContainer;
use crate::error::Error;
use crate::inventory::Inventory;
use crate::model::{
    DpmacId, DpniId, DprcId, LinkType, MacAddr, ObjectRef, ObservedDpmac, ObservedDpni,
    ObservedTopology,
};
use crate::port::{KernelControl, McControl};

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
    /// (design D2). Defaults empty — the board offers nothing until seeded.
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
            }),
        }
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
    /// (design D2; bead gqf.19).
    #[must_use]
    pub fn with_inventory(self, inventory: Inventory) -> Self {
        self.state.borrow_mut().inventory = inventory;
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
        label: &crate::types::ConstructName,
        _num_queues: u32,
    ) -> Result<DpniId, Error> {
        let mut st = self.state.borrow_mut();
        let id = DpniId::new(st.next_index);
        st.next_index += 1;
        // The object is stamped with the construct name at create (ADR-0010 §4 ABA
        // guard), so a re-observe never sees it unlabelled.
        st.dpnis.push(ObservedDpni {
            id,
            label: Some(label.clone()),
            connected_to: None,
            mac: None,
            netdev: None,
            attributes: BTreeMap::new(),
        });
        Ok(id)
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

    fn set_label(&self, dpni: DpniId, label: &crate::types::ConstructName) -> Result<(), Error> {
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

    // `dprc_create` mints a fresh, unplugged child container and records it so a
    // subsequent [`observe_containers`] re-queries it (the create-then-reobserve loop,
    // DPRC-I6). A created DPRC reads back on the [`ContainerState::Created`] (unplugged)
    // face with its create-time mask (the whole mutation — restool cannot plug a DPRC).
    fn dprc_create(
        &self,
        parent: DprcId,
        options: crate::dprc::Options,
        label: &crate::types::ConstructName,
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

    fn dprc_destroy(&self, container: DprcId) -> Result<(), Error> {
        self.state.borrow_mut().containers.remove(&container);
        Ok(())
    }

    fn dprc_assign(
        &self,
        _container: DprcId,
        _object: ObjectRef,
        _child: Option<DprcId>,
        _plugged: Option<bool>,
    ) -> Result<(), Error> {
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
        _label: &crate::types::ConstructName,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn dprc_set_locked(&self, _child: DprcId, _locked: bool) -> Result<(), Error> {
        Ok(())
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

    // The reconcile/convergence tests never bind VFIO (that face is board-only, design
    // D6), so the fake reports an unbound, group-less, override-clear child and accepts
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

    fn bound_driver(&self, _dprc: DprcId) -> Result<Option<String>, Error> {
        Ok(None)
    }

    fn driver_override(&self, _dprc: DprcId) -> Result<Option<String>, Error> {
        Ok(None)
    }

    fn iommu_group(&self, _dprc: DprcId) -> Result<Option<u32>, Error> {
        Ok(None)
    }
}
