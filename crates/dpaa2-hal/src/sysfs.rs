//! The fsl-mc sysfs bus: device enumeration paths and the `dpaa2-eth` driver
//! bind attribute.
//!
//! Operations are mechanical reads/writes of the sysfs layout and report plain
//! [`std::io::Error`]s; what an absent attribute or a busy driver *means* is
//! the caller's policy.

use std::io;
use std::path::PathBuf;

const FSL_MC_DEVICES: &str = "/sys/bus/fsl-mc/devices";
const ETH_DRIVER_BIND: &str = "/sys/bus/fsl-mc/drivers/fsl_dpaa2_eth/bind";

/// The fsl-mc sysfs bus rooted at one container (typically `dprc.1`).
pub struct FslMcSysfs {
    container: String,
    devices_root: PathBuf,
    bind_path: PathBuf,
}

impl FslMcSysfs {
    /// Speaks for the given root container at the default sysfs paths.
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

    /// Whether the `dpaa2-eth` driver exposes its bind attribute at all.
    #[must_use]
    pub fn eth_bind_exists(&self) -> bool {
        self.bind_path.exists()
    }

    /// Writes `<container>/<device>` to the `dpaa2-eth` driver bind attribute.
    ///
    /// # Errors
    ///
    /// Propagates the write error verbatim — `ResourceBusy` for an
    /// already-bound device, `NotFound` when the driver is not loaded.
    pub fn bind_eth(&self, device: &str) -> io::Result<()> {
        let id = format!("{}/{device}", self.container);
        std::fs::write(&self.bind_path, id.as_bytes())
    }

    /// First netdev name under `<container>/<device>/net`, if any.
    ///
    /// A missing `net/` directory means no netdev is bound (e.g. a fixed-link
    /// DPMAC) and reads as `Ok(None)` — that much is sysfs layout, not policy.
    ///
    /// # Errors
    ///
    /// Propagates any other I/O error from reading the directory.
    pub fn netdev_of(&self, device: &str) -> io::Result<Option<String>> {
        // /sys/bus/fsl-mc/devices/<container>/<device>/net/
        let dir = self
            .devices_root
            .join(&self.container)
            .join(device)
            .join("net");
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netdev_of_reads_fixture_tree() {
        let root = std::env::temp_dir().join("dpaa2-hal-sysfs-test");
        let net = root.join("dprc.1/dpni.7/net/eth1");
        std::fs::create_dir_all(&net).unwrap();
        let bus = FslMcSysfs::new("dprc.1").with_devices_root(&root);
        assert_eq!(bus.netdev_of("dpni.7").unwrap(), Some("eth1".to_owned()));
        assert_eq!(bus.netdev_of("dpni.8").unwrap(), None);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
