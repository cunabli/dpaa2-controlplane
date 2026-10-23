use std::collections::BTreeMap;

use crate::core::error::Error;
use crate::core::family::Family;
use crate::core::inventory::Inventory;
use crate::core::model::{DpmacId, DpniId, DprcId, MacAddr, ObjectRef, ObservedTopology};
use crate::core::types::ConstructName;
use crate::families::dpio::{DpioCfg, Priorities};
use crate::families::dpmac::DpmacObservation;
use crate::families::dpni::{DpniCfg, LinkState};
use crate::families::dprc;
use crate::families::dpseci::{DpseciCfg, OptionMask};
use crate::families::dpsw::{
    BroadcastCfg, ComponentType, CounterReadout, DpswCfg, FdbAgingTime, FloodingCfg, IfLinkState,
    MaxFdbEntries, MaxFdbMcGroups, MaxFdbs, MaxVlans, NumIfs, ObservedOpts, Taildrop,
};
use crate::families::pool_lifecycle::ObservedPoolObject;
use crate::plan::dprc::ObservedContainer;

/// The witnessable detail of one dpseci object, assembled for the `status --detail` row
/// (dpseci-typestate task 4.1) and projected to the observed signature the census judges
/// (dpseci-typestate task 3.3; [`crate::plan::dpseci::observed_sig_of`]). This type is pure
/// read-back; mutation rides the [`McControl::create_dpseci_in`]/[`McControl::destroy_dpseci`]
/// verbs, never a field here.
///
/// The queue counts and per-queue priorities are restool `info`'s honest witnesses — `None`
/// / empty when a line was absent. The options mask and API version ride the privileged
/// MC-portal read in [`portal`](Self::portal), which is honestly
/// [`Unobservable`](DpseciPortalReadout::Unobservable) on an unprivileged run (DPSECI-I3;
/// dpseci-typestate design D5 — `info` discards the options mask, only the portal sees it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpseciDetail {
    /// `number of transmit queues:` from restool `info`, `None` when the line was absent.
    pub num_tx_queues: Option<u8>,
    /// `number of receive queues:` from restool `info`, `None` when the line was absent.
    pub num_rx_queues: Option<u8>,
    /// The per-queue tx priorities from restool `info`, empty when the line was absent.
    pub tx_priorities: Vec<u8>,
    /// The privileged MC-portal readout, or the typed this-run unobservable outcome.
    pub portal: DpseciPortalReadout,
}

/// A dpseci MC-portal readout (`GET_ATTR` options + `GET_API_VERSION`), or the typed
/// this-run unobservable outcome when the portal is unavailable (dpseci-typestate design D5).
/// Unavailability is never an error here: the status row shows honest-unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DpseciPortalReadout {
    /// The portal answered: the decoded options mask and the dpseci API version. The decode is
    /// total, so there is no gap wrapper — the mask may carry attributed raw escapes
    /// (dpseci-hardening design D1).
    Observed {
        /// The decoded options mask — named flags plus any unnamed bit attributed as a raw
        /// escape. The escape is display-face information only; the census stays conservative
        /// (dpseci-hardening design D2; [`crate::plan::dpseci::observed_sig_of`]).
        options: OptionMask,
        /// The dpseci API major version from `GET_API_VERSION`.
        api_major: u16,
        /// The dpseci API minor version from `GET_API_VERSION`.
        api_minor: u16,
    },
    /// The portal was unavailable this run; the reason is carried for display only.
    Unobservable {
        /// Why the read could not run (device missing / permission denied).
        reason: String,
    },
}

/// One switch interface's connection face, read back from the `dpsw info` per-interface
/// block (dpsw-typestate design D6; `docs/baseline/dpsw.md` "Command surface"). The three
/// faces are kept distinct because a missing `connection:` line is **not** a free port: an
/// endpoint of unexpected type with `if_id != 0` prints no `connection:` line at all, which
/// ls-main's `has_endpoint` misreads as free (`docs/baseline/dpsw.md` "Silent-failure notes").
/// That blanked line is [`Unobservable`](Self::Unobservable), never
/// [`Disconnected`](Self::Disconnected) — no plan judges an unobservable port unconnected
/// (DPSW-I7; reconciler spec "A blanked connection line is not freedom").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DpswIfFace {
    /// The port faces a named endpoint (a member dpni or an uplink dpmac).
    Connected(ObjectRef),
    /// The port read back an explicit no-endpoint face — genuinely unconnected.
    Disconnected,
    /// The `connection:` line was absent or blanked; the face cannot be judged this run.
    Unobservable,
}

/// One switch interface's full read-back from the `dpsw info` per-interface block
/// (dpsw-typestate task 4.2; `docs/baseline/dpsw.md` "Command surface"). The [`face`](Self::face)
/// is display plus the pinned twin of the `has_endpoint` law
/// ([`crate::plan::dpsw::port_unconnected`]), not a census input — convergence is the cfg-signature
/// census only (dpsw-typestate design D6); the link, taildrop, max-frame-length, and counter
/// members are likewise display-only observability, read never judged. Each is an honest optional
/// — `None` when `info` omitted the line — except the counter readout, which is vocabulary-checked
/// and carries its own per-row unread shape (DPSW-I7, absence never a defaulted value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpswIfDetail {
    /// The connection face — display plus the pinned `has_endpoint`-law twin, not a census input.
    pub face: DpswIfFace,
    /// `link state:` typed `up`/`down`/`n/a`, `None` when the line was absent (display-only).
    pub link: Option<IfLinkState>,
    /// The taildrop readout, `None` when the block was absent (display-only).
    pub taildrop: Option<Taildrop>,
    /// `max frame length:`, `None` when the line was absent (display-only).
    pub max_frame_length: Option<u32>,
    /// The vocabulary-checked counter readout, truncation-honest (display-only).
    pub counters: CounterReadout,
}

/// The witnessable detail of one dpsw object, assembled from `dpsw info` (dpsw-typestate
/// task 4.2) and projected to the observable-signature face the dpsw-typestate task 3.3 census
/// judges ([`crate::plan::dpsw::observed_face_of`]). This type is pure read-back; mutation
/// rides the [`McControl::create_dpsw_in`]/[`McControl::destroy_dpsw`] verbs, never a field
/// here.
///
/// Each observable-signature attribute line is honestly optional — `None` when `info` omitted
/// it, so a missing line is an honest unknown the projection reads as unobservable, never a
/// defaulted value (DPSW-I7; dpsw-typestate design D6). The observed options are themselves
/// `Option<ObservedOpts>`: an absent options line decodes to `None`, never the empty set, so a
/// blanked option list is a typed decode escape the projection reads as unobservable (a present
/// options line carries the named MC flags plus any unnamed bit attributed as a display-only raw
/// escape, the dpseci-hardening design D1/D2 precedent).
///
/// The per-interface [`DpswIfDetail`] carries the connection face — display plus the pinned
/// `has_endpoint`-law twin, not a census input — plus the display-only
/// link/taildrop/max-frame-length/counter block (dpsw-typestate task 4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpswDetail {
    /// `num_ifs:` — the immutable port count, `None` when the line was absent.
    pub num_ifs: Option<NumIfs>,
    /// `max_vlans:`, `None` when the line was absent.
    pub max_vlans: Option<MaxVlans>,
    /// `max_fdbs:`, `None` when the line was absent.
    pub max_fdbs: Option<MaxFdbs>,
    /// `max_fdb_entries:`, `None` when the line was absent.
    pub max_fdb_entries: Option<MaxFdbEntries>,
    /// `fdb_aging_time:`, `None` when the line was absent.
    pub fdb_aging_time: Option<FdbAgingTime>,
    /// `max_fdb_mc_groups:`, `None` when the line was absent.
    pub max_fdb_mc_groups: Option<MaxFdbMcGroups>,
    /// `component_type:`, `None` when the line was absent.
    pub component_type: Option<ComponentType>,
    /// `flooding_cfg:`, `None` when the line was absent.
    pub flooding_cfg: Option<FloodingCfg>,
    /// `broadcast_cfg:`, `None` when the line was absent.
    pub broadcast_cfg: Option<BroadcastCfg>,
    /// `frame storage memory size:` — display-only. NOTE: `docs/baseline/dpsw.md` "Command
    /// surface" claims `info` never prints `mem_size`; the V-DPSW-4 capture
    /// (`results/V-DPSW-4-rev1/refusal-dpsw-info.txt`) contradicts that, so it is read here for
    /// display. Whether it joins the census signature is the group-7 sitting, adjudicated by
    /// dpsw-typestate task 8.1 (model untouched); `None` when the line was absent.
    pub mem_size: Option<u16>,
    /// The decoded options — named flags plus any attributed raw escape — or `None` when the
    /// options line was absent (an absent line is a typed decode escape, never the empty set).
    pub options: Option<ObservedOpts>,
    /// The per-interface read-back detail, keyed by interface index.
    pub ifaces: BTreeMap<u32, DpswIfDetail>,
}

/// Southbound MC-portal control at MC-command granularity.
///
/// Each method corresponds to a single MC firmware command so a future ioctl
/// implementation maps one-to-one behind the same trait (mc-backend spec):
/// [`create_dpni`](Self::create_dpni) is a single create verb, with companions riding the
/// pool passes the reconciler converges separately (backend split recorded in ADR-0018).
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

    /// Reads one dpmac's witnessable observation surface through a single `restool dpmac
    /// info dpmac.N` spawn — the DPC-born attributes, the burned-in MAC, and the
    /// vocabulary-checked counters (dpmac-typestate spec "The restool shim reads the dpmac
    /// observation surface"; dpmac-typestate design D4). One spawn per dpmac, by the
    /// whitelist fence: this verb never drives `set_protocol`/`set_params`/MDIO or a dpmac
    /// link command, which
    /// are outside the `/dev/dprc.N` whitelist (dpmac-typestate design Non-Goals;
    /// `docs/baseline/dpmac.md` "Command surface").
    ///
    /// A deviating counter row count is a typed
    /// [`CounterReadout::VersionSignal`](crate::families::dpmac::CounterReadout::VersionSignal),
    /// never a parse error, and restool's silent counter skips are never read as zeros
    /// (DPMAC-I7; `docs/baseline/dpmac.md` "Counter skew"). A dead shim spawn — restool
    /// aborts via `assert(false)` on out-of-enum values (`docs/baseline/dpmac.md`
    /// "Silent-failure notes") — surfaces as a typed observation failure, never as
    /// inherited partial state.
    ///
    /// # Errors
    /// Returns [`Error::Backend`] on a dead/killed spawn, [`Error::McStatus`] or
    /// [`Error::RestoolGuard`] on a refusal, or [`Error::Parse`] when a required attribute
    /// line is absent or carries a token outside the typed vocabulary.
    fn observe_dpmac(&self, dpmac: DpmacId) -> Result<DpmacObservation, Error>;

    /// Reads one dpseci object's witnessable detail for the `status --detail` row
    /// (dpseci-typestate task 4.1): restool `info`'s queue counts and per-queue priorities,
    /// plus the privileged MC-portal `GET_ATTR` options and `GET_API_VERSION` when a portal
    /// node is reachable. The portal half is honestly
    /// [`DpseciPortalReadout::Unobservable`] on an unprivileged run, never an error
    /// (dpseci-typestate design D5). `dpseci` names the object for both the `info` spawn and
    /// the portal OPEN; `container` names the container the census is scoped to, while which
    /// portal node the read rides is backend policy (the restool backend rides the root node).
    ///
    /// This read is the observed half of the census seam (dpseci-typestate task 3.3): its
    /// signature feeds [`create_dpseci_in`](Self::create_dpseci_in)/[`destroy_dpseci`](Self::destroy_dpseci)
    /// through the pure census judgment, mirroring [`observe_dpmac`](Self::observe_dpmac)'s
    /// report-only shape.
    ///
    /// # Errors
    /// Returns [`Error::McStatus`]/[`Error::RestoolGuard`] on a refusal of the `info` spawn or
    /// an MC refusal over an open portal, or [`Error::Backend`] on a dead spawn or unexpected
    /// transport failure; portal *unavailability* is the typed unobservable outcome, not an error.
    fn observe_dpseci(&self, container: DprcId, dpseci: ObjectRef) -> Result<DpseciDetail, Error>;

    /// Creates a DPNI object, stamped with the owning construct's name as its MC
    /// label, and returns its MC-assigned id. Stamping at create closes the read-back
    /// window in which a fresh object would otherwise show unlabelled and be misread
    /// as foreign (ADR-0010 §4 ABA guard; ADR-0015 decision 9).
    ///
    /// `cfg` is the compiled, in-envelope create block the plan carries
    /// (dpni-typestate task 4.1; design D3): the backend renders it verbatim, never
    /// re-deriving an option or a size. Sizing rides inside as
    /// [`DpniCfg::num_queues`]; 0 means
    /// unsized and the backend applies its host-derived default (synthesis L2/B3),
    /// preserving the prior contract.
    ///
    /// # Errors
    /// Returns an error if creation fails.
    fn create_dpni(&self, label: &ConstructName, cfg: &DpniCfg) -> Result<DpniId, Error>;

    /// Creates a **bare** DPNI in a child container `container`, stamped `label` and
    /// plugged in that child — the child-population create (pool-objects design D2), the
    /// deliberate divergence from [`create_dpni`](Self::create_dpni)'s root path. Unlike
    /// that root create, this issues **no** private dependency chain and **no** connect:
    /// a child's companions come from the pool disposition the reconciler converges
    /// separately (`dpaa2_mc::populate::dispatch_child_population`), not from a consumer's own
    /// transactional chain, and a VFIO-consumed child is never kernel-connected here.
    ///
    /// `cfg` is the same compiled, in-envelope create block [`create_dpni`](Self::create_dpni)
    /// renders verbatim; sizing rides inside as
    /// [`DpniCfg::num_queues`] (0 ⇒ the
    /// backend's host-derived default).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`], [`Error::RestoolGuard`], [`Error::Backend`], or
    /// [`Error::Parse`] if the created id cannot be read back.
    fn create_dpni_in(
        &self,
        container: DprcId,
        cfg: &DpniCfg,
        label: &ConstructName,
    ) -> Result<DpniId, Error>;

    /// Connects a single DPNI↔DPMAC edge.
    ///
    /// # Errors
    /// Returns an error if the connection fails.
    fn connect(&self, dpni: DpniId, dpmac: DpmacId) -> Result<(), Error>;

    /// Carries `DPRC_CONNECT` (`docs/baseline/mc-ioctl-policy.md` row 13; mc-backend spec):
    /// connects `dpni` to `peer` issued from their common ancestor `ancestor` — the
    /// DPNI-I9 connect form (`docs/baseline/dpni.md` DPNI-I9): `dprc connect <ancestor>
    /// --endpoint1=<dpni> --endpoint2=<peer>`, with **no** root plug step. This is the
    /// child-port divergence from the root-shaped [`connect`](Self::connect), whose
    /// `dprc assign --plugged` targets the shim's own container — the wrong container for
    /// a child dpni (pool-objects design D11). `peer` is an [`ObjectRef`] so both the
    /// child-dpni↔root-dpmac and the cross-container dpni↔dpni cases render.
    ///
    /// The reconciler reads [`dprc_get_connection`](Self::dprc_get_connection) first, so a
    /// re-run over an already-connected edge issues nothing (idempotence, DPNI-I9's
    /// both-endpoints-disconnected precondition).
    ///
    /// **Board-witness marker:** dpni(child)↔dpmac(root) is DPNI-I9-*allowed* but board-
    /// verified only for dpni↔dpni (kdpni pairs in production). The child↔dpmac case is
    /// witnessed on the board or it fails loud (pool-objects task 4.3); there is no
    /// runtime gating here.
    ///
    /// # Errors
    /// Returns an error if the connection fails.
    fn dprc_connect(&self, ancestor: DprcId, dpni: DpniId, peer: ObjectRef) -> Result<(), Error>;

    /// Carries `DPRC_GET_CONNECTION` (`docs/baseline/mc-ioctl-policy.md` row 17; mc-backend
    /// spec): reads the object `dpni` is currently connected to from its `dpni info`
    /// `endpoint:` line, or `Ok(None)` when disconnected (`No object associated`) — the
    /// idempotence read the child-port converge issues before
    /// [`dprc_connect`](Self::dprc_connect) to ask "already connected to X?"
    /// (pool-objects design D11). The restool-text parse stays behind the trait so the #10
    /// portal implementation drops in under the same typed return (cross-dprc-links design D6).
    /// The
    /// peer is an [`ObjectRef`] so a dpmac and a cross-container dpni peer both read back
    /// (`docs/baseline/dpni.md` DPNI-I9).
    ///
    /// # Errors
    /// Returns an error if the backend cannot be queried.
    fn dprc_get_connection(&self, dpni: DpniId) -> Result<Option<ObjectRef>, Error>;

    /// Carries `DPNI_GET_LINK_STATE` (`docs/baseline/mc-ioctl-policy.md` row 30; mc-backend
    /// spec): reads `dpni`'s typed up/down [`LinkState`] from the `dpni info` `link status:
    /// <n> - <word>` line. This is the dpni's *link* state, distinct from the DPRC
    /// *connection* the [`dprc_get_connection`](Self::dprc_get_connection) `endpoint:` line
    /// carries — the endpoint line's `link is up` suffix is connection state, never read as
    /// link state (`docs/baseline/dpmac.md` "the observation surface is weaker than the
    /// state"). The restool-text parse stays behind the trait so the #10 portal
    /// implementation drops in under the same typed return (cross-dprc-links design D6); this
    /// change adds no `/dev/dprc.N` portal read-slice (ADR-0021).
    ///
    /// # Errors
    /// Returns [`Error::Parse`] when the `link status:` line is absent or carries a token
    /// outside the `up`/`down` vocabulary, or a backend error on a failed query.
    fn dpni_get_link_state(&self, dpni: DpniId) -> Result<LinkState, Error>;

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

    /// Carries `DPRC_DISCONNECT` (`docs/baseline/mc-ioctl-policy.md` row 14; mc-backend
    /// spec): disconnects `dpni` from its peer, issued at their common ancestor `ancestor`
    /// (the root constant `CONNECT_ANCESTOR` today) so the cross-container form is
    /// ancestor-explicit, never scoped to either child (cross-dprc-links design D6).
    ///
    /// # Errors
    /// Returns an error if the disconnect fails.
    fn dprc_disconnect(&self, ancestor: DprcId, dpni: DpniId) -> Result<(), Error>;

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

    // ---- pool-family create/destroy verbs (pool-objects task 3.1) ----
    // The delta→id dispatch edge (pool-objects design D2) resolves a count to N creates; each
    // stamps its consumer's label and plugs (ADR-0015; ADR-0010 §4 ABA guard). `container` is
    // `None` for the shim root. Per-verb create options are from the baselines (rustdoc below).

    /// Creates one dpbp (buffer pool) in `container`, stamped `label` and plugged, and
    /// returns its [`ObjectRef`]. dpbp has zero create options (`docs/baseline/dpbp.md`
    /// "Option inventory": the `dpbp_cfg.options` placeholder is discarded by the flib).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`] (e.g. `0x8` No resources at the pool floor, DPBP-I7),
    /// [`Error::RestoolGuard`], [`Error::Backend`], or [`Error::Parse`] if the created id
    /// cannot be read back.
    fn dpbp_create(
        &self,
        container: Option<DprcId>,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error>;

    /// Creates one dpmcp (MC command portal) in `container`, stamped `label` and plugged,
    /// and returns its [`ObjectRef`]. dpmcp takes no create option the reconciler sets
    /// (`docs/baseline/dpmcp.md` "Option inventory": the one option token and the
    /// pool-assigned portal id are both left at their restool defaults).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`], [`Error::RestoolGuard`], [`Error::Backend`], or
    /// [`Error::Parse`].
    fn dpmcp_create(
        &self,
        container: Option<DprcId>,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error>;

    /// Creates one dpcon (concentrator) with `priorities` channel priority levels in
    /// `container`, stamped `label` and plugged, and returns its [`ObjectRef`].
    ///
    /// `--num-priorities` is dpcon's sole create option (`docs/baseline/dpcon.md` "Option
    /// inventory": 1–8, **default 2** — unlike dpio's default of 8). The `1..=8` range and
    /// meaning are identical to dpio's, so the same [`Priorities`] newtype carries it (the
    /// shared MC create-range refinement, `families::dpio`); nothing in the corpus drives
    /// priority > 0 today (DPCON-I3), so the level count is opaque capacity.
    ///
    /// # Errors
    /// Returns [`Error::McStatus`], [`Error::RestoolGuard`], [`Error::Backend`], or
    /// [`Error::Parse`].
    fn dpcon_create(
        &self,
        container: Option<DprcId>,
        priorities: Priorities,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error>;

    /// Creates one dpio (`QBMan` software portal) with the create-cfg `cfg` in `container`,
    /// stamped `label` and plugged, and returns its [`ObjectRef`]. dpio takes
    /// `--channel-mode` and `--num-priorities` (`docs/baseline/dpio.md` "Option
    /// inventory"); the mode is dead in the kernel (DPIO-I3) but is still rendered.
    ///
    /// This is the raw create only. The dpio→dpmcp probe-draw ordering (a dpmcp must
    /// exist before a consumer probes the dpio, DPIO-I1/DPMCP-I1) is procedural in the
    /// adapter, not a verb obligation (mc-backend spec; the set-MAC-before-plug
    /// precedent) — the `dpaa2_mc::pool::create_dpio_seat` helper sequences the pair.
    ///
    /// # Errors
    /// Returns [`Error::McStatus`] (e.g. `-ERANGE` past the online-CPU seat ceiling,
    /// DPIO-I2), [`Error::RestoolGuard`], [`Error::Backend`], or [`Error::Parse`].
    fn dpio_create(
        &self,
        container: Option<DprcId>,
        cfg: DpioCfg,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error>;

    /// Destroys one pool object, addressed by its [`ObjectRef`] (which carries the
    /// family). Renders `<family> destroy <object>` then a bus `sync`, mirroring the dpni
    /// [`destroy`](Self::destroy) precedent. The one destroy verb serves all four families
    /// — a pool object is anonymous, so its family (on the ref) is the only per-family
    /// datum a destroy needs (pool-objects design D2).
    ///
    /// A driver-bound object is refused by the MC (`docs/baseline/dpbp.md`: `destroy`
    /// refuses driver-bound objects) as a typed [`Error::McStatus`] — the enforcement
    /// backstop behind the free-only shrink discipline (pool-objects design D3). The
    /// caller selects only free victims; this verb never checks custody itself.
    ///
    /// # Errors
    /// Returns [`Error::McStatus`], [`Error::RestoolGuard`], or [`Error::Backend`].
    fn pool_destroy(&self, object: &ObjectRef) -> Result<(), Error>;

    // ---- dpseci census create/destroy verbs (dpseci-typestate task 3.3; design D9) ----
    // Distinct verbs: destroy re-observes presence to confirm it, unlike the pool destroy.

    /// Creates one dpseci in `container` (`None` ⇒ the shim root), stamped `label` and
    /// plugged, rendering the typed [`DpseciCfg`] verbatim — the compiled block the plan
    /// carries (`Attributes::Dpseci`), never re-derived (dpseci-typestate design D3). The
    /// create fills one planned-signature surplus the census resolved (the multiset judgment
    /// of dpseci-typestate design D9); priorities ride from the desired cfg, never observed.
    ///
    /// # Errors
    /// Returns [`Error::McStatus`]/[`Error::RestoolGuard`] on a refusal, [`Error::Backend`] on
    /// a dead spawn, or [`Error::Parse`] when the created id cannot be read back.
    fn create_dpseci_in(
        &self,
        container: Option<DprcId>,
        cfg: &DpseciCfg,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error>;

    /// Destroys one dpseci in `container`, confirming destruction by re-observing presence,
    /// not by the exit code: in a child container restool overwrites the destroy error with
    /// the `dprc_close` result (`docs/baseline/dpseci.md` "Silent-failure notes"). The
    /// destroy drains one observed-signature surplus the census resolved (the multiset
    /// judgment of dpseci-typestate design D9); the caller selects only tenant-labelled
    /// victims bearing that signature.
    ///
    /// # Errors
    /// Returns [`Error::McStatus`]/[`Error::RestoolGuard`] on a refusal, or [`Error::Backend`]
    /// when the object is still present after a success-reported destroy.
    fn destroy_dpseci(&self, container: Option<DprcId>, dpseci: &ObjectRef) -> Result<(), Error>;

    // ---- dpsw census verbs (dpsw-typestate task 3.3; design D6) ----
    // Distinct verbs: destroy re-observes presence to confirm it, mirroring destroy_dpseci.

    /// Reads one dpsw object's witnessable detail from a single `dpsw info` spawn — the
    /// observed half of the census seam (dpsw-typestate task 3.3): its signature feeds
    /// [`create_dpsw_in`](Self::create_dpsw_in)/[`destroy_dpsw`](Self::destroy_dpsw) through the
    /// pure census judgment ([`crate::plan::dpsw::observed_face_of`]), mirroring
    /// [`observe_dpmac`](Self::observe_dpmac)'s report-only shape. `dpsw` names the object for
    /// the `info` spawn; `container` names the container the census is scoped to. Portal/info
    /// unavailability is the typed honest-unknown face in the [`DpswDetail`], never an error,
    /// exactly as [`observe_dpseci`](Self::observe_dpseci) treats it (dpsw-typestate design D6).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`]/[`Error::RestoolGuard`] on a refusal of the `info` spawn, or
    /// [`Error::Backend`] on a dead spawn; a missing line is the typed unobservable face, not an
    /// error.
    fn observe_dpsw(&self, container: DprcId, dpsw: ObjectRef) -> Result<DpswDetail, Error>;

    /// Creates one dpsw in `container` (`None` ⇒ the shim root), stamped `label` and plugged,
    /// rendering the typed [`DpswCfg`] verbatim — the compiled block the plan carries, never
    /// re-derived; the dpsw-typestate task 4.1 render must emit exactly what
    /// [`crate::plan::dpsw::cfg_from_attributes`] built (dpsw-typestate design D6). The create
    /// fills one planned-signature surplus the census resolved (the multiset judgment of
    /// dpsw-typestate design D6).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`]/[`Error::RestoolGuard`] on a refusal, [`Error::Backend`] on a
    /// dead spawn, or [`Error::Parse`] when the created id cannot be read back.
    fn create_dpsw_in(
        &self,
        container: Option<DprcId>,
        cfg: &DpswCfg,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error>;

    /// Destroys one dpsw in `container`, confirming destruction by re-observing presence, never
    /// the exit code: `dpsw destroy` in a child container overwrites the destroy error with
    /// `dprc_close`'s result and exits 0 while the object survives (`docs/baseline/dpsw.md`
    /// "Silent-failure notes"; DPSW-I7). The destroy drains one observed-signature surplus the
    /// census resolved (the multiset judgment of dpsw-typestate design D6).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`]/[`Error::RestoolGuard`] on a refusal, or [`Error::Backend`]
    /// when the object is still present after a success-reported destroy.
    fn destroy_dpsw(&self, container: Option<DprcId>, dpsw: &ObjectRef) -> Result<(), Error>;

    /// Connects one switch interface `dpsw.N.ifx` to `peer`, issued from their common
    /// ancestor `ancestor`: `dprc connect <ancestor> --endpoint1=dpsw.N.M --endpoint2=<peer>`
    /// (the ADR-0022 sub-indexed endpoint discipline, `*.N.M`). This is the dpsw sibling of the
    /// dpni-anchored [`dprc_connect`](Self::dprc_connect): a switch endpoint is an interface of
    /// the dpsw object, which that dpni-keyed verb cannot name. `peer` is an [`ObjectRef`] so
    /// both dpsw edge rows render — the member `dpni ↔ dpsw.N.M` and the uplink `dpsw.N.M ↔
    /// dpmac` (ADR-0022). The port index is bounded by the created `num_ifs` in planning
    /// (DPSW-I10), so this verb renders whatever edge it is handed; it plugs nothing — the dpsw
    /// plug is `dprc_assign(..., Some(true))` issued last (`docs/baseline/dpsw.md` "Lifecycle
    /// ordering"; bead dpaa2-controlplane-dbf.22).
    ///
    /// # Errors
    /// Returns [`Error::McStatus`]/[`Error::RestoolGuard`] on a refusal, or [`Error::Backend`]
    /// on a dead spawn.
    fn connect_switch_port(
        &self,
        ancestor: DprcId,
        dpsw: ObjectRef,
        ifx: u32,
        peer: ObjectRef,
    ) -> Result<(), Error>;

    /// Observes one pool family's objects in `container` through a single `dprc show`,
    /// filtered to `family`, each row reported verbatim as an [`ObservedPoolObject`] —
    /// read-back is the only observation, exit status never is (pool-objects task 3.1;
    /// mc-backend spec requirement 1). The adapter reports raw labels; the core judges
    /// custody ([`census_of`](crate::families::pool_lifecycle::census_of); PASS5-F1).
    ///
    /// # Errors
    /// Returns an error if the backend cannot be queried.
    fn observe_pool(
        &self,
        container: Option<DprcId>,
        family: Family,
    ) -> Result<Vec<ObservedPoolObject>, Error>;
}
