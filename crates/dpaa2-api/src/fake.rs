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

use crate::error::Error;
use crate::model::{
    DpmacId, DpniId, LinkType, MacAddr, ObservedDpmac, ObservedDpni, ObservedTopology,
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
            }),
        }
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

    fn create_dpni(&self) -> Result<DpniId, Error> {
        let mut st = self.state.borrow_mut();
        let id = DpniId::new(st.next_index);
        st.next_index += 1;
        st.dpnis.push(ObservedDpni {
            id,
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
}
