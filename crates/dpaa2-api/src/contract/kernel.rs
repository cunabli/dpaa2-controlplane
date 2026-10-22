use crate::core::error::Error;
use crate::core::model::{DpniId, DprcId, ObjectRef};
use crate::families::dpmac::CarrierSource;
use crate::families::pool_lifecycle::RawDriver;

/// Southbound kernel-side control: driver binding and netdev observation.
pub trait KernelControl {
    /// Ensures `dpaa2-eth` is bound to `dpni` via the sysfs bind interface where
    /// required. Binding is frequently automatic (plug); implementations may no-op.
    ///
    /// # Errors
    /// Returns an error if an explicit bind is attempted and fails.
    fn bind(&self, dpni: DpniId) -> Result<(), Error>;

    /// Releases `fsl_dpaa2_eth` from `dpni` via the driver's sysfs `unbind` attribute —
    /// the §8-ordered teardown verb (ADR-0008 §8/§9; pool-objects design D12). The
    /// disconnect has already severed the endpoint edge; unbind precedes destroy, since a
    /// destroy issued against a still-bound dpni is refused client-side. Level-triggered:
    /// an unbind of an already-unbound dpni is a no-op, so a teardown re-run converges.
    ///
    /// # Errors
    /// Returns an error if the unbind write fails.
    fn unbind(&self, dpni: DpniId) -> Result<(), Error>;

    /// Observes the netdev name for `dpni`, or `None` if none exists.
    ///
    /// A fixed-link DPMAC that `dpaa2-eth` does not bind yields `Ok(None)` — the
    /// absence of a netdev is not an error (mc-backend spec).
    ///
    /// # Errors
    /// Returns an error only if the kernel state cannot be read at all.
    fn netdev_of(&self, dpni: DpniId) -> Result<Option<String>, Error>;

    /// Observes the link carrier of the bus device a port's arbitration judgment picked
    /// ([`carrier_source`](crate::families::dpmac::carrier_source)): the peer dpni's netdev
    /// for a `KernelOwned` port, the dpmac's own `macN` netdev for `Offered`/`RemoteOwned`
    /// (mc-backend spec "`KernelControl` observes link carrier"; `docs/baseline/dpmac.md`
    /// DPMAC-I6, `docs/baseline/dpni.md` DPNI-I4). One seam over the two-variant
    /// [`CarrierSource`] mirrors hal's single `carrier_of` primitive — the arbitration→device
    /// policy lives once in the core mapping, not split across the seam.
    ///
    /// Raw `Option<bool>` out: `Some(up)` is the netdev's carrier, `None` the absence of any
    /// netdev to read. The caller judges it into
    /// [`CarrierReading`](crate::families::dpmac::CarrierReading) — `None` is the
    /// driverless-port diagnosis, never a down link (adapters report, never judge — design
    /// D5; ADR-0003).
    ///
    /// # Errors
    /// Returns an error only if the carrier state cannot be read at all (a malformed carrier
    /// file, say); an absent netdev is `Ok(None)`, never an error.
    fn carrier(&self, source: CarrierSource) -> Result<Option<bool>, Error>;

    /// Observes the raw driver name bound to `dpni`'s device — the basename of its `driver`
    /// link as a [`RawDriver`], reported verbatim, or `None` when it has no driver. The caller judges the name
    /// (the same report-not-judge stance as [`bound_driver`](Self::bound_driver)): a present
    /// `fsl_dpaa2_eth` link is the per-target probe read-back that decides bind liveness
    /// ([`judge_bind_probe`](crate::core::model::judge_bind_probe)), never the bind write's
    /// exit (`docs/baseline/dpio.md` DPIO-I5; `docs/baseline/dpni.md` DPNI-I4). No default
    /// impl — every backend answers it verbatim.
    ///
    /// # Errors
    /// Returns an error only if the kernel state cannot be read at all.
    fn dpni_driver(&self, dpni: DpniId) -> Result<Option<RawDriver>, Error>;

    // ---- child-DPRC VFIO binding (dprc-encapsulation, task 3.2) ----
    //
    // The `vfio-fsl-mc` bind path for a child DPRC (`docs/baseline/dprc.md`
    // "Kernel-defined semantics"; design D3/D6; ADR-0002, ADR-0004). The driver has no match table, so
    // binding is `driver_override` write + `bind`; unbind + a cleared override restore
    // `fsl_mc_dprc` eligibility. This face OBSERVES and ACTUATES the kernel bus; it
    // never advances the [`dprc::Container`](crate::families::dprc::Container) lifecycle phase — a
    // Linux bus event maps to no MC transition (DPRC-I7), and the plugged-face
    // [`dprc::VfioBind`] the model carries is derived from these observations by the
    // core, not written by the adapter (adapters report, never judge — design D4/D5; ADR-0003).
    // The observation verbs report raw sysfs facts (bound-driver name, override value,
    // IOMMU-group id); [`dprc::VfioBind::classify`](crate::families::dprc::VfioBind::classify)
    // does the judging.

    /// Writes [`dprc::VFIO_FSL_MC_DRIVER`](crate::families::dprc::VFIO_FSL_MC_DRIVER) to the child
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

    /// Binds a kernel-driven child DPRC to [`dprc::FSL_MC_DPRC_DRIVER`](crate::families::dprc::FSL_MC_DPRC_DRIVER):
    /// writes `driver_override = fsl_mc_dprc` on the child, then writes the container id to that
    /// driver's `bind` attribute. A runtime-created child dprc reads back `unplugged` in its parent
    /// and restool refuses to plug a dprc by design; the fsl-mc bus match skips unplugged objects,
    /// so a bare bind returns `ENODEV` with no probe at all — the match refusing, not a probe
    /// failure. The override is therefore mandatory, not optional: it bypasses the plugged check
    /// (the path `vfio-fsl-mc` already uses on never-plugged child dprcs). Both writes race the
    /// asynchronous post-`restool dprc sync` bus scan, so an unscanned device (`ENOENT`/`ENODEV`) is
    /// deferral — the next ensure pass re-kicks — not failure (V-DPSW-5 rev 7, bead
    /// dpaa2-controlplane-dbf.19, finding 54; ADR-0013 §283-293).
    ///
    /// # Errors
    /// Returns an error if a sysfs write fails for any reason other than the unscanned-device race.
    fn dprc_kernel_bind(&self, dprc: DprcId) -> Result<(), Error>;

    /// Unbinds the child DPRC from `vfio-fsl-mc` and clears its `driver_override`,
    /// restoring eligibility for [`dprc::FSL_MC_DPRC_DRIVER`](crate::families::dprc::FSL_MC_DPRC_DRIVER)
    /// — the unbind scenario's whole contract (the cleared override is what re-opens the
    /// default driver). The MC object and its residents survive untouched (DPRC-I7).
    ///
    /// # Errors
    /// Returns an error if the unbind or the override clear fails.
    fn vfio_unbind(&self, dprc: DprcId) -> Result<(), Error>;

    /// Observes the child DPRC's bound driver name — what the sysfs `driver` link
    /// reports as a [`RawDriver`], or `None` when it has no driver. A raw report: the caller maps it to a
    /// bind state with [`dprc::VfioBind::classify`](crate::families::dprc::VfioBind::classify)
    /// (adapters report, never judge — design D5; ADR-0003).
    ///
    /// # Errors
    /// Returns an error only if the kernel state cannot be read at all.
    fn bound_driver(&self, dprc: DprcId) -> Result<Option<RawDriver>, Error>;

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

    /// Releases whatever driver currently binds a flat root fsl-mc device `object` (a
    /// `dpsw.N` for the switched-domain teardown) — reads the bound driver first and unbinds
    /// only while one holds the device, so a re-run over an already-unbound device is a no-op
    /// (level-triggered idempotence, the sibling of the dpni [`unbind`](Self::unbind) and the
    /// child [`vfio_unbind`](Self::vfio_unbind)). The unbind precedes the typed `destroy_dpsw`,
    /// since a destroy issued against a still-bound root device is refused (ADR-0008 §8;
    /// `docs/baseline/dpsw.md` "Lifecycle ordering"). Unlike [`unbind`](Self::unbind), the
    /// driver name is not fixed (`fsl_dpaa2_switch`): a dpsw the kernel left unbound carries
    /// none, so the read-back decides it (adapters report, never judge).
    ///
    /// # Errors
    /// Returns an error if the bound-driver read or the unbind write fails.
    fn unbind_current(&self, object: &ObjectRef) -> Result<(), Error>;
}
