//! `KernelControl` over the fsl-mc sysfs bus.
//!
//! Binding `dpaa2-eth` is normally automatic once a DPNI is plugged, so `bind` is
//! best-effort. Netdev observation reads the DPNI's `net/` directory; a fixed-link
//! DPMAC that the driver does not bind simply has no such entry, which is reported as
//! "no netdev" rather than an error (mc-backend spec).

use std::path::PathBuf;

use dpaa2_api::{DpniId, Error, KernelControl};

const FSL_MC_DEVICES: &str = "/sys/bus/fsl-mc/devices";
const ETH_DRIVER_BIND: &str = "/sys/bus/fsl-mc/drivers/fsl_dpaa2_eth/bind";

/// Reads DPAA2 netdev state from sysfs under a given root container.
pub struct SysfsKernel {
    container: String,
    devices_root: PathBuf,
    bind_path: PathBuf,
}

impl SysfsKernel {
    /// Observes the given root container (typically `dprc.1`) at the default sysfs
    /// paths.
    #[must_use]
    pub fn new(container: impl Into<String>) -> Self {
        Self {
            container: container.into(),
            devices_root: PathBuf::from(FSL_MC_DEVICES),
            bind_path: PathBuf::from(ETH_DRIVER_BIND),
        }
    }

    /// Overrides the sysfs devices root (for tests against a fixture tree).
    #[must_use]
    pub fn with_devices_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.devices_root = root.into();
        self
    }

    fn net_dir(&self, dpni: DpniId) -> PathBuf {
        // /sys/bus/fsl-mc/devices/<container>/dpni.N/net/
        self.devices_root
            .join(&self.container)
            .join(dpni.to_string())
            .join("net")
    }
}

impl KernelControl for SysfsKernel {
    fn bind(&self, dpni: DpniId) -> Result<(), Error> {
        // Binding is usually automatic; only attempt an explicit bind if the driver
        // bind attribute exists, and treat "already bound" as success.
        if !self.bind_path.exists() {
            return Ok(());
        }
        let id = format!("{}/{dpni}", self.container);
        match std::fs::write(&self.bind_path, id.as_bytes()) {
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
        let dir = self.net_dir(dpni);
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            // No net/ directory: no netdev bound (e.g. fixed link). Not an error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::Io(e)),
        };
        for entry in entries {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                return Ok(Some(name.to_owned()));
            }
        }
        Ok(None)
    }
}
