//! `KernelControl` over the fsl-mc sysfs bus.
//!
//! The sysfs mechanics live in [`dpaa2_hal::FslMcSysfs`]; this module keeps the
//! adapter policy. Binding `dpaa2-eth` is normally automatic once a DPNI is
//! plugged, so `bind` is best-effort. Netdev observation reads the DPNI's
//! `net/` directory; a fixed-link DPMAC that the driver does not bind simply
//! has no such entry, which is reported as "no netdev" rather than an error
//! (mc-backend spec).

use std::path::PathBuf;

use dpaa2_api::{DpniId, Error, KernelControl};
use dpaa2_hal::FslMcSysfs;

/// Reads DPAA2 netdev state from sysfs under a given root container.
pub struct SysfsKernel {
    bus: FslMcSysfs,
}

impl SysfsKernel {
    /// Observes the given root container (typically `dprc.1`) at the default sysfs
    /// paths.
    #[must_use]
    pub fn new(container: impl Into<String>) -> Self {
        Self {
            bus: FslMcSysfs::new(container),
        }
    }

    /// Overrides the sysfs devices root (for tests against a fixture tree).
    #[must_use]
    pub fn with_devices_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.bus = self.bus.with_devices_root(root);
        self
    }
}

impl KernelControl for SysfsKernel {
    fn bind(&self, dpni: DpniId) -> Result<(), Error> {
        // Binding is usually automatic; only attempt an explicit bind if the driver
        // bind attribute exists, and treat "already bound" as success.
        if !self.bus.eth_bind_exists() {
            return Ok(());
        }
        match self.bus.bind_eth(&dpni.to_string()) {
            Ok(()) => Ok(()),
            // Already bound (EBUSY) or not-applicable — not fatal for convergence.
            Err(e) if e.kind() == std::io::ErrorKind::ResourceBusy => Ok(()),
            Err(e) => {
                tracing::debug!(%dpni, error = %e, "explicit bind failed (continuing)");
                Ok(())
            }
        }
    }

    fn netdev_of(&self, dpni: DpniId) -> Result<Option<String>, Error> {
        self.bus.netdev_of(&dpni.to_string()).map_err(Error::Io)
    }
}
