//! The fsl-mc sysfs bus: device enumeration paths and the `dpaa2-eth` driver
//! bind attribute.
//!
//! Operations are mechanical reads/writes of the sysfs layout and report plain
//! [`std::io::Error`]s; what an absent attribute or a busy driver *means* is
//! the caller's policy.

use std::io;
use std::path::PathBuf;

const FSL_MC_DEVICES: &str = "/sys/bus/fsl-mc/devices";
const FSL_MC_DRIVERS: &str = "/sys/bus/fsl-mc/drivers";

/// The `dpaa2-eth` driver directory name under the drivers root.
pub const ETH_DRIVER: &str = "fsl_dpaa2_eth";

/// The fsl-mc sysfs bus rooted at one container (typically `dprc.1`).
pub struct FslMcSysfs {
    container: String,
    devices_root: PathBuf,
    drivers_root: PathBuf,
}

impl FslMcSysfs {
    /// Speaks for the given root container at the default sysfs paths.
    #[must_use]
    pub fn new(container: impl Into<String>) -> Self {
        Self {
            container: container.into(),
            devices_root: PathBuf::from(FSL_MC_DEVICES),
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
        self.drivers_root.join(ETH_DRIVER).join("bind").exists()
    }

    /// Writes the bare bus device name (`device`, e.g. `dpni.1`) to the `dpaa2-eth` driver
    /// bind attribute. fsl-mc names bus devices `<type>.<id>` flat, with no container
    /// prefix (`fsl-mc-bus.c` `dev_set_name`), and the driver bind attribute resolves the
    /// written string by bus device name (`bus.c` `bus_find_device_by_name`) — a
    /// `<container>/<device>` string matches nothing and returns `ENODEV`.
    ///
    /// # Errors
    ///
    /// Propagates the write error verbatim — `ResourceBusy` for an
    /// already-bound device, `NotFound` when the driver is not loaded.
    pub fn bind_eth(&self, device: &str) -> io::Result<()> {
        let path = self.drivers_root.join(ETH_DRIVER).join("bind");
        std::fs::write(path, device.as_bytes())
    }

    /// Writes the bare bus device name (`device`) to the `dpaa2-eth` driver unbind
    /// attribute — the reverse of [`bind_eth`](Self::bind_eth), releasing the netdev
    /// driver. The attribute resolves the string by bus device name, which fsl-mc names
    /// `<type>.<id>` flat (`fsl-mc-bus.c` `dev_set_name`; `bus.c`
    /// `bus_find_device_by_name`) — no container prefix.
    ///
    /// # Errors
    ///
    /// Propagates the write error verbatim — `NoDevice` when the device is not bound
    /// to this driver.
    pub fn unbind_eth(&self, device: &str) -> io::Result<()> {
        let path = self.drivers_root.join(ETH_DRIVER).join("unbind");
        std::fs::write(path, device.as_bytes())
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

    /// The carrier state of the netdev under `device`, if one is present.
    ///
    /// Resolves the netdev with [`netdev_of`](Self::netdev_of), then reads
    /// `<container>/<device>/net/<ifname>/carrier`, trimmed: `"1"` → `Some(true)`,
    /// `"0"` → `Some(false)`. No netdev → `Ok(None)` — the same sysfs-layout-not-policy
    /// reading as [`netdev_of`](Self::netdev_of)'s missing-`net/` case; upstream reads
    /// that as the `NoObservable` driverless-port diagnosis (dpmac-typestate design D6),
    /// while hal stays
    /// untyped and policy-free.
    ///
    /// Per-arbitration resolution is the CALLER's policy. A `KernelOwned` port's caller
    /// passes the peer dpni bus device (`dpni.N`, whose netdev appears only once the dpni
    /// binds — `docs/baseline/dpni.md` DPNI-I4). An `Offered`/`RemoteOwned` port's caller
    /// passes the dpmac bus device (`dpmac.N`, whose netdev is `macN` under
    /// `CONFIG_FSL_DPAA2_MAC_NETDEVS` — `docs/baseline/dpmac.md` DPMAC-I6). One primitive
    /// covers every port; hal never sees an arbitration type.
    ///
    /// This is the kernel/PHY-local carrier view. The MC-propagated view
    /// (`dpni_get_link_state`) is a distinct, deliberately unread signal whose deferral
    /// rides the #10 restool-absence ledger anchor (dpmac-typestate design D6 trade-off).
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::InvalidData`] when the carrier file holds anything but
    /// `0`/`1`. The kernel returns `EINVAL` on a carrier read of an admin-down interface —
    /// propagated verbatim; what it means is the caller's policy.
    pub fn carrier_of(&self, device: &str) -> io::Result<Option<bool>> {
        let Some(ifname) = self.netdev_of(device)? else {
            return Ok(None);
        };
        let path = self
            .devices_root
            .join(&self.container)
            .join(device)
            .join("net")
            .join(ifname)
            .join("carrier");
        match std::fs::read_to_string(&path)?.trim() {
            "1" => Ok(Some(true)),
            "0" => Ok(Some(false)),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("carrier: expected 0 or 1, got {other:?}"),
            )),
        }
    }

    /// Whether `dpmac`'s standalone driver exposes its `macN` netdev — the
    /// `CONFIG_FSL_DPAA2_MAC_NETDEVS` reference-pair property (ADR-0008 class;
    /// `docs/baseline/dpmac.md` DPMAC-I6). Suites assert it; everywhere else its absence
    /// just degrades [`carrier_of`](Self::carrier_of) to `Ok(None)`.
    ///
    /// # Errors
    ///
    /// Propagates any I/O error other than a missing `net/` directory.
    pub fn mac_netdev_present(&self, dpmac: &str) -> io::Result<bool> {
        Ok(self.netdev_of(dpmac)?.is_some())
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

    /// The name of the driver bound to a NESTED bus device — the basename of the `driver`
    /// symlink at `<devices_root>/<container>/<device>/driver`, or `Ok(None)` when it has
    /// no driver. A dpni sits nested under its container (the netdev path's layout), unlike
    /// the flat `<devices_root>/<dprc>` a [`bound_driver`](Self::bound_driver) reads. A raw
    /// report — a present `fsl_dpaa2_eth` link is the per-target probe read-back the caller
    /// judges bind liveness by, never the bind write itself (`docs/baseline/dpni.md`
    /// DPNI-I4 the probe precondition; `docs/baseline/dpio.md` DPIO-I5 the write-is-not-the-
    /// judgment). Missing link ⇒ `Ok(None)`, sysfs layout, not policy.
    ///
    /// # Errors
    /// Propagates any I/O error other than a missing link.
    pub fn device_driver(&self, device: &str) -> io::Result<Option<String>> {
        Self::link_basename(
            &self
                .devices_root
                .join(&self.container)
                .join(device)
                .join("driver"),
        )
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
    fn carrier_of_reads_fixture_tree() {
        let root = std::env::temp_dir().join("dpaa2-hal-carrier-test");
        let _ = std::fs::remove_dir_all(&root);

        // KernelOwned port: peer dpni's netdev carrier (docs/baseline/dpni.md DPNI-I4).
        let dpni_net = root.join("dprc.1/dpni.7/net/eth1");
        std::fs::create_dir_all(&dpni_net).unwrap();
        std::fs::write(dpni_net.join("carrier"), "1\n").unwrap();

        // Offered/RemoteOwned port: standalone driver's macN carrier (dpmac.md DPMAC-I6).
        let mac_net = root.join("dprc.1/dpmac.4/net/mac4");
        std::fs::create_dir_all(&mac_net).unwrap();
        std::fs::write(mac_net.join("carrier"), "0\n").unwrap();

        // Driverless port: a device dir with no net/ — upstream's NoObservable diagnosis.
        std::fs::create_dir_all(root.join("dprc.1/dpmac.9")).unwrap();

        let bus = FslMcSysfs::new("dprc.1").with_devices_root(&root);
        assert_eq!(bus.carrier_of("dpni.7").unwrap(), Some(true));
        assert_eq!(bus.carrier_of("dpmac.4").unwrap(), Some(false));
        assert!(bus.mac_netdev_present("dpmac.4").unwrap());
        assert_eq!(bus.carrier_of("dpmac.9").unwrap(), None);
        assert!(!bus.mac_netdev_present("dpmac.9").unwrap());

        // Malformed carrier content is a data error, not a 0/1.
        std::fs::write(mac_net.join("carrier"), "gremlin\n").unwrap();
        assert_eq!(
            bus.carrier_of("dpmac.4").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn device_driver_reads_fixture_tree() {
        let root = std::env::temp_dir().join("dpaa2-hal-driver-test");
        let _ = std::fs::remove_dir_all(&root);
        let dev = root.join("dprc.1/dpni.7");
        std::fs::create_dir_all(&dev).unwrap();
        let driver = root.join("drivers/fsl_dpaa2_eth");
        std::fs::create_dir_all(&driver).unwrap();
        std::os::unix::fs::symlink(&driver, dev.join("driver")).unwrap();
        let bus = FslMcSysfs::new("dprc.1").with_devices_root(&root);
        assert_eq!(
            bus.device_driver("dpni.7").unwrap(),
            Some("fsl_dpaa2_eth".to_owned())
        );
        assert_eq!(bus.device_driver("dpni.8").unwrap(), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn bind_eth_writes_the_derived_driver_bind_attribute() {
        // The bind face derives its path from `drivers_root` like unbind, so a fixture drivers
        // tree exercises it board-free (ADR-0020 pass3 F9).
        let base = std::env::temp_dir().join("dpaa2-hal-bind-eth-test");
        let _ = std::fs::remove_dir_all(&base);
        let drivers = base.join("drivers");
        std::fs::create_dir_all(drivers.join(ETH_DRIVER)).unwrap();
        let bus = FslMcSysfs::new("dprc.1").with_drivers_root(&drivers);

        bus.bind_eth("dpni.7").unwrap();
        assert_eq!(
            std::fs::read_to_string(drivers.join(ETH_DRIVER).join("bind")).unwrap(),
            "dpni.7"
        );
        // The bind attribute now resolves under the fixture root, not the hardcoded sysfs path.
        assert!(bus.eth_bind_exists());

        std::fs::remove_dir_all(&base).unwrap();
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
