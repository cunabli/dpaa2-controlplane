//! The hexagonal ports: trait seams the pure core depends on (design D0, D6).
//!
//! The reconciler references only these traits, never a concrete `restool` or ioctl
//! type (mc-backend spec: "Core depends only on traits"). Two southbound ports split
//! MC-portal work ([`McControl`]) from kernel-side binding and netdev observation
//! ([`KernelControl`]), because binding is often a state we *wait to observe* rather
//! than an action we execute. One northbound port ([`ConfigSource`]) yields the
//! neutral [`Intent`].

use std::collections::BTreeMap;

use crate::dprc;
use crate::dprc_plan::ObservedContainer;
use crate::error::Error;
use crate::intent::Intent;
use crate::inventory::Inventory;
use crate::model::{DpmacId, DpniId, DprcId, MacAddr, ObjectRef, ObservedTopology};
use crate::types::ConstructName;

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

    /// Reads the board's hardware offer — [`compile`](crate::compile)'s second
    /// input, read never written (task 3.5, design D2; bead gqf.19).
    ///
    /// The dpmac attributes are immutable and come from `dpmac info` (DPMAC-I3);
    /// each dpmac's [`Availability`](crate::Availability) is the ADR-0003 §3
    /// safety matrix, DPL-owned objects are [`Foreign`](crate::Availability::Foreign)
    /// (ADR-0001 §4), and each derived family carries one three-valued
    /// [`Ceiling`](crate::Ceiling) (ADR-0011). It reads the board and invents no
    /// number: an unmeasured family is [`Ceiling::Unknown`](crate::Ceiling::Unknown).
    ///
    /// # Errors
    /// Returns an error if the backend cannot be queried.
    fn read_inventory(&self) -> Result<Inventory, Error>;

    /// Creates a DPNI object, stamped with the owning construct's name as its MC
    /// label, and returns its MC-assigned id. Stamping at create closes the read-back
    /// window in which a fresh object would otherwise show unlabelled and be misread
    /// as foreign (ADR-0010 §4 ABA guard; ADR-0015 decision 9).
    ///
    /// `num_queues` is the compiled transmit-queue sizing the plan carries; 0 means
    /// unsized and the backend applies its host-derived default (synthesis L2/B3).
    ///
    /// # Errors
    /// Returns an error if creation fails.
    fn create_dpni(&self, label: &ConstructName, num_queues: u32) -> Result<DpniId, Error>;

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

    /// Writes the construct name as the object's MC label (`dprc set-label`), decision
    /// 9's repair verb (ADR-0015): the seam that re-associates an unanchored object to
    /// its intent and stamps an anchored one for debuggability. Board-verified to land
    /// even on a locked container (ADR-0010 §4 / V-DPRC-3).
    ///
    /// # Errors
    /// Returns an error if the label cannot be written.
    fn set_label(&self, dpni: DpniId, label: &ConstructName) -> Result<(), Error>;

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

    // ---- child-DPRC container verbs (dprc-encapsulation, task 3.1) ----
    //
    // One method per restool `dprc` subcommand, at MC-command granularity, so a
    // future ioctl backend maps each one-to-one (mc-backend spec). Each surfaces a
    // refusal as a typed [`Error`]: an MC firmware status as [`Error::McStatus`] (raw,
    // core-judged via `dprc::Refusal`/`dprc_plan::attribute_mc`), a restool client-side
    // guard as [`Error::RestoolGuard`] (design D4; `docs/baseline/dprc.md` DPRC-I3).
    // The [`dprc`] containment vocabulary stays module-namespaced — imported from its
    // module path, never flat re-exported — because its `Options` is a distinct type
    // from the intent-compile surface (design D4).

    /// Re-observes every child container the root holds, keyed by its re-observation
    /// handle [`DprcId`], as freshly-queried [`ObservedContainer`]s (design D2/D6;
    /// reconciler delta "Mutation visibility is established only by re-observation",
    /// DPRC-I6). This is the read half of container convergence: a step's success verdict
    /// comes from re-querying the affected container here, never from a bus rescan
    /// (`sync`) or an assumed dispatch outcome. The adapter reports raw observations; the
    /// verdict is judged core-side (`dprc_plan::verdict`).
    ///
    /// # Errors
    /// Returns an error if the backend cannot be queried.
    fn observe_containers(&self) -> Result<BTreeMap<DprcId, ObservedContainer>, Error>;

    /// `dprc create <parent> [--options] [--label]`: mints a child container under
    /// `parent` and returns its MC-assigned [`DprcId`] for re-observation. The new
    /// child reads back **unplugged** — a created DPRC is never driver-bound, and
    /// restool cannot plug a DPRC (`docs/baseline/dprc.md` "Lifecycle ordering",
    /// V-POOL-1 rev 2) — so this issues no plug.
    ///
    /// # Errors
    /// Returns [`Error::McStatus`] if the MC refuses (e.g. `0x6` when the parent lacks
    /// `SPAWN_ALLOWED`), [`Error::RestoolGuard`] on a client-side refusal, or
    /// [`Error::Parse`] if the created id cannot be read back.
    fn dprc_create(
        &self,
        parent: DprcId,
        options: dprc::Options,
        label: &ConstructName,
    ) -> Result<DprcId, Error>;

    /// `dprc destroy <container>`: destroys the child container. A container holding a
    /// plugged resident is refused `-EBUSY` at the MC ([`Error::McStatus`]); a restool
    /// client guard may fire first ([`Error::RestoolGuard`]) — the two stay distinct so
    /// the caller can tell an MC precondition from a client refusal.
    ///
    /// # Errors
    /// Returns [`Error::McStatus`], [`Error::RestoolGuard`], or [`Error::Backend`].
    fn dprc_destroy(&self, container: DprcId) -> Result<(), Error>;

    /// `dprc assign <container> --object=<o> [--child=<c>] [--plugged=0|1]`: moves
    /// `object` into `child` (when `child` is `Some`) and/or sets its plugged state
    /// (when `plugged` is `Some`) — the single restool subcommand that does both. A
    /// plugged object cannot be moved: restool refuses that client-side before any MC
    /// command ([`Error::RestoolGuard`]; DPRC-I3).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`] (e.g. `0x4` No privilege on a sibling move) or
    /// [`Error::RestoolGuard`] (the plugged-move guard).
    fn dprc_assign(
        &self,
        container: DprcId,
        object: ObjectRef,
        child: Option<DprcId>,
        plugged: Option<bool>,
    ) -> Result<(), Error>;

    /// `dprc unassign <parent> --child=<c> --object=<o>`: moves `object` one hop up
    /// from `child` back to `parent` (the eviction direction, ADR-0007 §3).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`] or [`Error::RestoolGuard`].
    fn dprc_unassign(&self, parent: DprcId, child: DprcId, object: ObjectRef) -> Result<(), Error>;

    /// `dprc set-label <container> --label=<s>`: rewrites the container's MC label —
    /// the label-drift repair verb (design open question: label drift on a locked
    /// container is repairable; the verb lands even under lock, V-DPRC-3). Distinct
    /// from [`McControl::set_label`], which labels a DPNI.
    ///
    /// # Errors
    /// Returns [`Error::McStatus`] or [`Error::RestoolGuard`].
    fn dprc_set_label(&self, container: DprcId, label: &ConstructName) -> Result<(), Error>;

    /// `dprc set-locked <child> --locked=0|1`: locks or unlocks the child container and
    /// its entire sub-hierarchy (`docs/baseline/dprc.md` "Command surface"; the lock
    /// strips create/destroy/assign/unassign/lock from the hierarchy, DPRC-I11).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`] or [`Error::RestoolGuard`].
    fn dprc_set_locked(&self, child: DprcId, locked: bool) -> Result<(), Error>;
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

    /// Writes [`dprc::VFIO_FSL_MC_DRIVER`] to the child
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
    /// restoring eligibility for [`dprc::FSL_MC_DPRC_DRIVER`]
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
    /// population can exercise this).
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

/// Northbound config source producing the neutral [`Intent`].
///
/// TOML implements this now; a gNMI/YANG frontend can implement it later and feed
/// the same pure core (design D0). The frontend parses and validates *intent* only;
/// it does not compile — turning an [`Intent`] plus an [`Inventory`] into the
/// object plan is [`compile`](crate::compile)'s, not the frontend's (design D10).
pub trait ConfigSource {
    /// Loads and validates the declared intent.
    ///
    /// # Errors
    /// Returns an error if the source is unreadable or fails validation.
    fn load(&self) -> Result<Intent, Error>;
}
