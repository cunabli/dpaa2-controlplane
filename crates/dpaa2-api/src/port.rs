//! The hexagonal ports: trait seams the pure core depends on (design D0, D6).
//!
//! The reconciler references only these traits, never a concrete `restool` or ioctl
//! type (mc-backend spec: "Core depends only on traits"). Two southbound ports split
//! MC-portal work ([`McControl`]) from kernel-side binding and netdev observation
//! ([`KernelControl`]), because binding is often a state we *wait to observe* rather
//! than an action we execute. One northbound port ([`ConfigSource`]) yields the
//! neutral [`DesiredTopology`].

use crate::error::Error;
use crate::model::{DesiredTopology, DpmacId, DpniId, MacAddr, ObservedTopology};

/// Southbound MC-portal control at MC-command granularity.
///
/// Each method corresponds to a single MC firmware command so a future ioctl
/// implementation maps one-to-one behind the same trait (mc-backend spec).
pub trait McControl {
    /// Reads the current MC state (objects + connection edges) as authoritative.
    ///
    /// # Errors
    /// Returns an error if the backend cannot be queried.
    fn observe(&self) -> Result<ObservedTopology, Error>;

    /// Creates a DPNI object and returns its MC-assigned id.
    ///
    /// # Errors
    /// Returns an error if creation fails.
    fn create_dpni(&self) -> Result<DpniId, Error>;

    /// Connects a single DPNI↔DPMAC edge.
    ///
    /// # Errors
    /// Returns an error if the connection fails.
    fn connect(&self, dpni: DpniId, dpmac: DpmacId) -> Result<(), Error>;

    /// Sets the DPNI primary MAC (used only in actuate mode).
    ///
    /// # Errors
    /// Returns an error if the MAC cannot be set.
    fn set_mac(&self, dpni: DpniId, mac: MacAddr) -> Result<(), Error>;

    /// Disconnects a DPNI from its DPMAC.
    ///
    /// # Errors
    /// Returns an error if the disconnect fails.
    fn disconnect(&self, dpni: DpniId) -> Result<(), Error>;

    /// Destroys a DPNI object.
    ///
    /// # Errors
    /// Returns an error if destruction fails.
    fn destroy(&self, dpni: DpniId) -> Result<(), Error>;
}

/// Southbound kernel-side control: driver binding and netdev observation.
pub trait KernelControl {
    /// Ensures `dpaa2-eth` is bound to `dpni` via the sysfs bind interface where
    /// required. Binding is frequently automatic (plug); implementations may no-op.
    ///
    /// # Errors
    /// Returns an error if an explicit bind is attempted and fails.
    fn bind(&self, dpni: DpniId) -> Result<(), Error>;

    /// Observes the netdev name for `dpni`, or `None` if none exists.
    ///
    /// A fixed-link DPMAC that `dpaa2-eth` does not bind yields `Ok(None)` — the
    /// absence of a netdev is not an error (mc-backend spec).
    ///
    /// # Errors
    /// Returns an error only if the kernel state cannot be read at all.
    fn netdev_of(&self, dpni: DpniId) -> Result<Option<String>, Error>;
}

/// Northbound config source producing the neutral desired topology.
///
/// TOML implements this now; a gNMI/YANG frontend can implement it later and feed
/// the same pure core (design D0).
pub trait ConfigSource {
    /// Loads and validates the desired topology.
    ///
    /// # Errors
    /// Returns an error if the source is unreadable or fails validation.
    fn load(&self) -> Result<DesiredTopology, Error>;
}
