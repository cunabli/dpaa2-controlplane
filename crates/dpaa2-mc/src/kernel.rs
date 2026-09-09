//! `KernelControl` over the fsl-mc sysfs bus.
//!
//! The sysfs mechanics live in [`dpaa2_hal::FslMcSysfs`]; this module keeps the
//! adapter policy. Binding `dpaa2-eth` is normally automatic once a DPNI is
//! plugged, so `bind` is best-effort. Netdev observation reads the DPNI's
//! `net/` directory; a fixed-link DPMAC that the driver does not bind simply
//! has no such entry, which is reported as "no netdev" rather than an error
//! (mc-backend spec).

use std::path::PathBuf;

use dpaa2_api::{DpniId, DprcId, Error, KernelControl, dprc};
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

    /// Overrides the sysfs drivers root (for tests against a fixture tree).
    #[must_use]
    pub fn with_drivers_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.bus = self.bus.with_drivers_root(root);
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

    // ---- child-DPRC VFIO binding (dprc-encapsulation task 3.2) ----
    //
    // Pure sysfs mechanics on the fsl-mc bus: the DPRC's own `driver_override`, the
    // `vfio-fsl-mc` driver's `bind`/`unbind`, and read-back of the `driver` and
    // `iommu_group` links. The driver-name sentinel is `dprc::VFIO_FSL_MC_DRIVER`, read
    // from the core so the adapter never spells its own copy (design D5). Observations
    // report raw sysfs facts; the core judges them (`dprc::VfioBind::classify`). No
    // method touches the MC object or a lifecycle phase — a bus event is not an MC
    // transition (DPRC-I7).

    fn vfio_set_override(&self, dprc: DprcId) -> Result<(), Error> {
        self.bus
            .set_driver_override(&dprc.to_string(), dprc::VFIO_FSL_MC_DRIVER)
            .map_err(Error::Io)
    }

    fn vfio_bind(&self, dprc: DprcId) -> Result<(), Error> {
        self.bus
            .bind_driver(dprc::VFIO_FSL_MC_DRIVER, &dprc.to_string())
            .map_err(Error::Io)
    }

    fn vfio_unbind(&self, dprc: DprcId) -> Result<(), Error> {
        // Unbind, then clear the override — the cleared override is what re-opens
        // fsl_mc_dprc eligibility (the unbind scenario's whole contract).
        let dprc = dprc.to_string();
        self.bus
            .unbind_driver(dprc::VFIO_FSL_MC_DRIVER, &dprc)
            .map_err(Error::Io)?;
        self.bus.set_driver_override(&dprc, "").map_err(Error::Io)
    }

    fn bound_driver(&self, dprc: DprcId) -> Result<Option<String>, Error> {
        self.bus.bound_driver(&dprc.to_string()).map_err(Error::Io)
    }

    fn driver_override(&self, dprc: DprcId) -> Result<Option<String>, Error> {
        self.bus
            .driver_override(&dprc.to_string())
            .map_err(Error::Io)
    }

    fn iommu_group(&self, dprc: DprcId) -> Result<Option<u32>, Error> {
        self.bus.iommu_group(&dprc.to_string()).map_err(Error::Io)
    }
}

#[cfg(test)]
mod tests {
    use dpaa2_api::dprc::{VFIO_FSL_MC_DRIVER, VfioBind};

    use super::*;

    /// A fixture fsl-mc bus: a `devices/dprc.2` node, a `drivers/vfio-fsl-mc` dir, and
    /// an `iommu_groups/11` dir, plus a [`SysfsKernel`] pointed at them. The whole VFIO
    /// face runs against this tree with no board (the sysfs-mock idiom).
    struct Fixture {
        base: PathBuf,
        devices: PathBuf,
        drivers: PathBuf,
        kernel: SysfsKernel,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!("dpaa2-mc-vfio-{tag}"));
            let _ = std::fs::remove_dir_all(&base);
            let devices = base.join("devices");
            let drivers = base.join("drivers");
            std::fs::create_dir_all(devices.join("dprc.2")).unwrap();
            std::fs::create_dir_all(drivers.join(VFIO_FSL_MC_DRIVER)).unwrap();
            std::fs::create_dir_all(base.join("iommu_groups/11")).unwrap();
            let kernel = SysfsKernel::new("dprc.1")
                .with_devices_root(&devices)
                .with_drivers_root(&drivers);
            Self {
                base,
                devices,
                drivers,
                kernel,
            }
        }

        /// Model the kernel's post-bind links: `driver` -> vfio-fsl-mc, `iommu_group` -> 11.
        fn link_bound(&self) {
            let dprc_dir = self.devices.join("dprc.2");
            std::os::unix::fs::symlink(
                self.drivers.join(VFIO_FSL_MC_DRIVER),
                dprc_dir.join("driver"),
            )
            .unwrap();
            std::os::unix::fs::symlink(
                self.base.join("iommu_groups/11"),
                dprc_dir.join("iommu_group"),
            )
            .unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn bind_scenario_sets_override_binds_and_is_observed_bound_with_a_group() {
        // Spec scenario "Bind a scratch child to VFIO": set override + bind ⇒ observed
        // bound to vfio-fsl-mc and its IOMMU group exists.
        let fx = Fixture::new("bind");
        let dprc = DprcId::new(2);

        fx.kernel.vfio_set_override(dprc).expect("override");
        assert_eq!(
            fx.kernel.driver_override(dprc).expect("read override"),
            Some(VFIO_FSL_MC_DRIVER.to_owned())
        );

        fx.kernel.vfio_bind(dprc).expect("bind");
        assert_eq!(
            std::fs::read_to_string(fx.drivers.join(VFIO_FSL_MC_DRIVER).join("bind")).unwrap(),
            "dprc.2"
        );

        fx.link_bound();
        let bound = fx.kernel.bound_driver(dprc).expect("bound driver");
        assert_eq!(bound.as_deref(), Some(VFIO_FSL_MC_DRIVER));
        // The core judges the raw name into the plugged-face bind state.
        assert_eq!(
            VfioBind::classify(bound.as_deref()),
            VfioBind::BoundVfioFslMc
        );
        assert_eq!(fx.kernel.iommu_group(dprc).expect("group"), Some(11));
    }

    #[test]
    fn unbind_scenario_clears_the_override_and_reads_back_unbound() {
        // Spec scenario "Unbind restores the unbound state": unbind + clear override ⇒
        // observed unbound (no driver), override cleared, eligible for fsl_mc_dprc again.
        let fx = Fixture::new("unbind");
        let dprc = DprcId::new(2);
        fx.kernel.vfio_set_override(dprc).expect("override");

        fx.kernel.vfio_unbind(dprc).expect("unbind");
        assert_eq!(
            std::fs::read_to_string(fx.drivers.join(VFIO_FSL_MC_DRIVER).join("unbind")).unwrap(),
            "dprc.2"
        );
        // The override is cleared as part of the unbind contract, so it reads unset.
        assert_eq!(
            fx.kernel.driver_override(dprc).expect("override read"),
            None
        );
        // No `driver` link ⇒ no bound driver ⇒ the core reads it Unbound.
        let bound = fx.kernel.bound_driver(dprc).expect("bound");
        assert_eq!(bound, None);
        assert_eq!(VfioBind::classify(bound.as_deref()), VfioBind::Unbound);
    }

    #[test]
    fn observation_reads_absent_facts_as_none_never_an_error() {
        // A pristine child (no override, no driver, no group) reports every observation
        // as None — an absent fact is not an error (adapters report, never judge).
        let fx = Fixture::new("absent");
        let dprc = DprcId::new(2);
        assert_eq!(fx.kernel.driver_override(dprc).unwrap(), None);
        assert_eq!(fx.kernel.bound_driver(dprc).unwrap(), None);
        assert_eq!(fx.kernel.iommu_group(dprc).unwrap(), None);
    }

    #[test]
    fn override_propagation_is_observed_on_a_subsequently_added_child() {
        // The propagation surface: a child added after the container was bound inherits
        // the override, observed here as a pre-set driver_override on a child that was
        // never explicitly overridden by this adapter (the kernel's bus notifier wrote
        // it). The observation reports the raw inherited value.
        let fx = Fixture::new("propagation");
        let added = fx.devices.join("dprc.5");
        std::fs::create_dir_all(&added).unwrap();
        std::fs::write(added.join("driver_override"), VFIO_FSL_MC_DRIVER).unwrap();

        let child = DprcId::new(5);
        assert_eq!(
            fx.kernel
                .driver_override(child)
                .expect("inherited override"),
            Some(VFIO_FSL_MC_DRIVER.to_owned())
        );
    }
}
