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
const FSL_MC_DRIVERS: &str = "/sys/bus/fsl-mc/drivers";

/// The fsl-mc sysfs bus rooted at one container (typically `dprc.1`).
pub struct FslMcSysfs {
    container: String,
    devices_root: PathBuf,
    bind_path: PathBuf,
    drivers_root: PathBuf,
}

impl FslMcSysfs {
    /// Speaks for the given root container at the default sysfs paths.
    #[must_use]
    pub fn new(container: impl Into<String>) -> Self {
        Self {
            container: container.into(),
            devices_root: PathBuf::from(FSL_MC_DEVICES),
            bind_path: PathBuf::from(ETH_DRIVER_BIND),
            drivers_root: PathBuf::from(FSL_MC_DRIVERS),
        }
    }

    /// Overrides the sysfs devices root (for tests against a fixture tree).
    #[must_use]
    pub fn with_devices_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.devices_root = root.into();
        self
    }

    /// Overrides the sysfs drivers root — `<root>/<driver>/{bind,unbind}` (for tests
    /// against a fixture tree).
    #[must_use]
    pub fn with_drivers_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.drivers_root = root.into();
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

    // ---- child-DPRC VFIO binding (fsl-mc has no match table; driver_override only) ----
    //
    // A DPRC is its own bus device: it sits directly under the devices root as
    // `<devices_root>/<dprc>` (e.g. `dprc.2`), not nested under a container like the
    // netdev path above. The driver name (`vfio-fsl-mc`) is passed in — the sentinel is
    // the caller's, since policy lives above this layer.

    /// Writes `value` to `<devices_root>/<dprc>/driver_override` (empty `value` clears
    /// it, restoring default-driver eligibility).
    ///
    /// # Errors
    /// Propagates the write error verbatim.
    pub fn set_driver_override(&self, dprc: &str, value: &str) -> io::Result<()> {
        let path = self.devices_root.join(dprc).join("driver_override");
        std::fs::write(path, value.as_bytes())
    }

    /// The current `driver_override` value of `<dprc>`, trimmed; an empty or absent
    /// file reads as `Ok(None)` (unset). This is the override-propagation observable.
    ///
    /// # Errors
    /// Propagates any I/O error other than a missing file.
    pub fn driver_override(&self, dprc: &str) -> io::Result<Option<String>> {
        let path = self.devices_root.join(dprc).join("driver_override");
        match std::fs::read_to_string(&path) {
            Ok(s) => {
                let s = s.trim();
                Ok((!s.is_empty()).then(|| s.to_owned()))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Writes `dprc` to `<drivers_root>/<driver>/bind`.
    ///
    /// # Errors
    /// Propagates the write error verbatim.
    pub fn bind_driver(&self, driver: &str, dprc: &str) -> io::Result<()> {
        let path = self.drivers_root.join(driver).join("bind");
        std::fs::write(path, dprc.as_bytes())
    }

    /// Writes `dprc` to `<drivers_root>/<driver>/unbind`.
    ///
    /// # Errors
    /// Propagates the write error verbatim.
    pub fn unbind_driver(&self, driver: &str, dprc: &str) -> io::Result<()> {
        let path = self.drivers_root.join(driver).join("unbind");
        std::fs::write(path, dprc.as_bytes())
    }

    /// The name of the driver bound to `<dprc>` — the basename of the `driver` symlink
    /// — or `Ok(None)` when the device has no driver.
    ///
    /// # Errors
    /// Propagates any I/O error other than a missing link.
    pub fn bound_driver(&self, dprc: &str) -> io::Result<Option<String>> {
        Self::link_basename(&self.devices_root.join(dprc).join("driver"))
    }

    /// The IOMMU-group id of `<dprc>` — the basename of the `iommu_group` symlink,
    /// parsed — or `Ok(None)` when the device has no group (or a non-numeric one).
    ///
    /// # Errors
    /// Propagates any I/O error other than a missing link.
    pub fn iommu_group(&self, dprc: &str) -> io::Result<Option<u32>> {
        Ok(
            Self::link_basename(&self.devices_root.join(dprc).join("iommu_group"))?
                .and_then(|s| s.parse().ok()),
        )
    }

    /// The final path component of a sysfs symlink target; `Ok(None)` when the link is
    /// absent — sysfs layout, not policy.
    fn link_basename(path: &std::path::Path) -> io::Result<Option<String>> {
        match std::fs::read_link(path) {
            Ok(target) => Ok(target
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_owned)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
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

    #[test]
    fn vfio_override_bind_and_observation_round_trip() {
        // A fixture bus with a child DPRC device and the vfio-fsl-mc driver dir. The
        // override write, the driver-link and iommu-group reads all hit the fixture, so
        // the whole VFIO face is exercised with no board.
        let base = std::env::temp_dir().join("dpaa2-hal-vfio-test");
        let _ = std::fs::remove_dir_all(&base);
        let devices = base.join("devices");
        let drivers = base.join("drivers");
        let dprc_dir = devices.join("dprc.2");
        std::fs::create_dir_all(&dprc_dir).unwrap();
        std::fs::create_dir_all(drivers.join("vfio-fsl-mc")).unwrap();
        let group_dir = base.join("iommu_groups/11");
        std::fs::create_dir_all(&group_dir).unwrap();

        let bus = FslMcSysfs::new("dprc.1")
            .with_devices_root(&devices)
            .with_drivers_root(&drivers);

        // Unset override reads None; setting it reads back the value; clearing → None.
        assert_eq!(bus.driver_override("dprc.2").unwrap(), None);
        bus.set_driver_override("dprc.2", "vfio-fsl-mc").unwrap();
        assert_eq!(
            bus.driver_override("dprc.2").unwrap(),
            Some("vfio-fsl-mc".to_owned())
        );

        // No driver link yet, no group yet.
        assert_eq!(bus.bound_driver("dprc.2").unwrap(), None);
        assert_eq!(bus.iommu_group("dprc.2").unwrap(), None);

        // The bind write lands in the driver's bind attribute.
        bus.bind_driver("vfio-fsl-mc", "dprc.2").unwrap();
        assert_eq!(
            std::fs::read_to_string(drivers.join("vfio-fsl-mc/bind")).unwrap(),
            "dprc.2"
        );
        // Model the kernel's post-bind links: driver -> vfio-fsl-mc, iommu_group -> 11.
        std::os::unix::fs::symlink(drivers.join("vfio-fsl-mc"), dprc_dir.join("driver")).unwrap();
        std::os::unix::fs::symlink(&group_dir, dprc_dir.join("iommu_group")).unwrap();
        assert_eq!(
            bus.bound_driver("dprc.2").unwrap(),
            Some("vfio-fsl-mc".to_owned())
        );
        assert_eq!(bus.iommu_group("dprc.2").unwrap(), Some(11));

        bus.set_driver_override("dprc.2", "").unwrap();
        assert_eq!(bus.driver_override("dprc.2").unwrap(), None);

        std::fs::remove_dir_all(&base).unwrap();
    }
}
