use crate::core::error::Error;
use crate::core::model::{DpniId, DprcId};

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

    // ---- child-DPRC VFIO binding (dprc-encapsulation, task 3.2) ----
    //
    // The `vfio-fsl-mc` bind path for a child DPRC (`docs/baseline/dprc.md`
    // "Kernel-defined semantics"; design D3/D6). The driver has no match table, so
    // binding is `driver_override` write + `bind`; unbind + a cleared override restore
    // `fsl_mc_dprc` eligibility. This face OBSERVES and ACTUATES the kernel bus; it
    // never advances the [`dprc::Container`](crate::dprc::Container) lifecycle phase — a
    // Linux bus event maps to no MC transition (DPRC-I7), and the plugged-face
    // [`dprc::VfioBind`] the model carries is derived from these observations by the
    // core, not written by the adapter (adapters report, never judge — design D4/D5).
    // The observation verbs report raw sysfs facts (bound-driver name, override value,
    // IOMMU-group id); [`dprc::VfioBind::classify`](crate::dprc::VfioBind::classify)
    // does the judging.

    /// Writes [`dprc::VFIO_FSL_MC_DRIVER`](crate::dprc::VFIO_FSL_MC_DRIVER) to the child
    /// DPRC's `driver_override` — the first half of the bind (no match table, so the
    /// override is the only path). Idempotent: rewriting the same value is a no-op at
    /// the kernel.
    ///
    /// # Errors
    /// Returns an error if the sysfs write fails.
    fn vfio_set_override(&self, dprc: DprcId) -> Result<(), Error>;

    /// Binds the child DPRC to `vfio-fsl-mc` (writes the container id to the driver's
    /// `bind` attribute) — the second half, after [`vfio_set_override`](Self::vfio_set_override).
    /// Once bound, the kernel propagates the override to every subsequently-added child
    /// of the container (observed via [`driver_override`](Self::driver_override)).
    ///
    /// # Errors
    /// Returns an error if the bind write fails.
    fn vfio_bind(&self, dprc: DprcId) -> Result<(), Error>;

    /// Unbinds the child DPRC from `vfio-fsl-mc` and clears its `driver_override`,
    /// restoring eligibility for [`dprc::FSL_MC_DPRC_DRIVER`](crate::dprc::FSL_MC_DPRC_DRIVER)
    /// — the unbind scenario's whole contract (the cleared override is what re-opens the
    /// default driver). The MC object and its residents survive untouched (DPRC-I7).
    ///
    /// # Errors
    /// Returns an error if the unbind or the override clear fails.
    fn vfio_unbind(&self, dprc: DprcId) -> Result<(), Error>;

    /// Observes the child DPRC's bound driver name — what the sysfs `driver` link
    /// reports, or `None` when it has no driver. A raw report: the caller maps it to a
    /// bind state with [`dprc::VfioBind::classify`](crate::dprc::VfioBind::classify)
    /// (adapters report, never judge — design D5).
    ///
    /// # Errors
    /// Returns an error only if the kernel state cannot be read at all.
    fn bound_driver(&self, dprc: DprcId) -> Result<Option<String>, Error>;

    /// Observes the child DPRC's `driver_override` value, or `None` when it is unset —
    /// the propagation surface: a subsequently-added child of a bound container reads
    /// back the inherited override even before its own bind (`docs/baseline/dprc.md`
    /// "Kernel-defined semantics"; the board suite 5.2 settles whether container-only
    /// population can exercise this). Visibility of that inherited override is deferred
    /// to the next container scan, never trusted at create time (ADR-0017).
    ///
    /// # Errors
    /// Returns an error only if the kernel state cannot be read at all.
    fn driver_override(&self, dprc: DprcId) -> Result<Option<String>, Error>;

    /// Observes the child DPRC's IOMMU-group id (the VFIO grouping unit), or `None`
    /// when the device has no group. Presence of a group is the bind scenario's second
    /// observable; the value is reported raw, not judged.
    ///
    /// # Errors
    /// Returns an error only if the kernel state cannot be read at all.
    fn iommu_group(&self, dprc: DprcId) -> Result<Option<u32>, Error>;
}
