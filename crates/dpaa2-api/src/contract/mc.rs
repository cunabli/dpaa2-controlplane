use std::collections::BTreeMap;

use crate::core::error::Error;
use crate::core::inventory::Inventory;
use crate::core::model::{DpmacId, DpniId, DprcId, MacAddr, ObjectRef, ObservedTopology};
use crate::core::types::ConstructName;
use crate::families::dprc;
use crate::plan::dprc::ObservedContainer;

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

    /// Reads the board's hardware offer — [`compile`](crate::intent::refuse::compile)'s second
    /// input, read never written (task 3.5, design D2; ADR-0002; bead gqf.19).
    ///
    /// The dpmac attributes are immutable and come from `dpmac info` (DPMAC-I3);
    /// each dpmac's [`Availability`](crate::core::inventory::Availability) is the ADR-0003 §3
    /// safety matrix, DPL-owned objects are [`Foreign`](crate::core::inventory::Availability::Foreign)
    /// (ADR-0001 §4), and each derived family carries one three-valued
    /// [`Ceiling`](crate::core::inventory::Ceiling) (ADR-0011). It reads the board and invents no
    /// number: an unmeasured family is [`Ceiling::Unknown`](crate::core::inventory::Ceiling::Unknown).
    ///
    /// # Errors
    /// Returns an error if the backend cannot be queried.
    fn read_inventory(&self) -> Result<Inventory, Error>;

    /// Creates a DPNI object, stamped with the owning construct's name as its MC
    /// label, and returns its MC-assigned id. Stamping at create closes the read-back
    /// window in which a fresh object would otherwise show unlabelled and be misread
    /// as foreign (ADR-0010 §4 ABA guard; ADR-0015 decision 9).
    ///
    /// `cfg` is the compiled, in-envelope create block the plan carries
    /// (dpni-typestate task 4.1; design D3): the backend renders it verbatim, never
    /// re-deriving an option or a size. Sizing rides inside as
    /// [`DpniCfg::num_queues`](crate::families::dpni::DpniCfg::num_queues); 0 means
    /// unsized and the backend applies its host-derived default (synthesis L2/B3),
    /// preserving the prior contract.
    ///
    /// # Errors
    /// Returns an error if creation fails.
    fn create_dpni(
        &self,
        label: &ConstructName,
        cfg: &crate::families::dpni::DpniCfg,
    ) -> Result<DpniId, Error>;

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
    // guard as [`Error::RestoolGuard`] (design D4; ADR-0003; `docs/baseline/dprc.md` DPRC-I3).
    // The [`dprc`] containment vocabulary stays module-namespaced — imported from its
    // module path, never flat re-exported — because its `Options` is a distinct type
    // from the intent-compile surface (design D4; ADR-0003).

    /// Re-observes every child container the root holds, keyed by its re-observation
    /// handle [`DprcId`], as freshly-queried [`ObservedContainer`]s (design D2/D6 (ADR-0002, ADR-0004);
    /// reconciler delta "Mutation visibility is established only by re-observation",
    /// DPRC-I6). This is the read half of container convergence: a step's success verdict
    /// comes from re-querying the affected container here, never from a bus rescan
    /// (`sync`) or an assumed dispatch outcome. The adapter reports raw observations; the
    /// verdict is judged core-side (`dprc_plan::verdict`).
    ///
    /// # Errors
    /// Returns an error if the backend cannot be queried.
    fn observe_containers(&self) -> Result<BTreeMap<DprcId, ObservedContainer>, Error>;

    /// Re-observes exactly one child container by its re-observation handle, returning the
    /// same [`ObservedContainer`] shape as [`McControl::observe_containers`], or `Ok(None)` when `id`
    /// is not a child of the root — honest absence, the signal a prune verdict reads to
    /// assert a destroyed id is gone without a bus rescan.
    ///
    /// Per-candidate re-observation replaces the full root rescan (1+2N spawns ×4 per
    /// ensure) with a read scoped to that container only (dpni-typestate design D5; DPRC-I6
    /// re-observation law). The OI-3 dpmcp-budget measurement (bead am0.2, board evidence
    /// V-DPRC-13-rev1) found NO leak — the dpmcp census held flat at 203 across all five
    /// censuses — so this seam is recorded as a spawn-count/latency fix, not a leak fix.
    ///
    /// # Errors
    /// Returns an error if the backend cannot be queried.
    fn observe_container(&self, id: DprcId) -> Result<Option<ObservedContainer>, Error>;

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
