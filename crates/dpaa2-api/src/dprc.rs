//! The child-DPRC container lifecycle as compile-time typestates — the Rust twin
//! of the Quint lifecycle sum (`models/families/dprc.qnt` module `dprc_lifecycle`).
//!
//! Authored model-first (quint-is-the-spec): the sums, payloads and phase set here
//! are structurally isomorphic to the Quint sum, and the ADR-0002 §3 law binds them
//! — same cases, same payloads, same guard semantics; names converge on readable
//! English on both surfaces. The state sum is the parity-tested artifact
//! ([`ContainerState`]); the *transitions* are a refinement of the model's runtime
//! guards (design D3): the two orderings the reconciler delta requires unrepresentable
//! are enforced at the type level, while the guards the model expresses over runtime
//! data stay runtime refusals recording an [`Outcome`].
//!
//! # Compile-time vs. runtime boundary (design D3, reconciler delta)
//!
//! *Unrepresentable at compile time* — the container-phase orderings:
//! - **Populate only while unplugged / plug-then-assign**: [`Container::create_resident`]
//!   and [`Container::assign_in`] exist only on the unplugged faces ([`Created`],
//!   [`Populated`]) via the [`UnpluggedFace`] bound. [`Container<Plugged>`] has no
//!   such method, so assigning into a plugged container — and thus the plug-then-assign
//!   ordering — does not type-check. See the `compile_fail` witness on [`Container::plug`].
//!
//! *Runtime guards recording an [`Outcome`]* — facts the model carries as runtime
//! data, kept runtime so the twin stays isomorphic (lifting them to types would
//! diverge structurally from the model's `Resident { plugged: bool }` / `Options`):
//! - **Plugged-move** (DPRC-I3): a resident's plugged state is a per-resident runtime
//!   bool, not a container typestate — [`Container::move_out`] refuses
//!   [`Refusal::TopologyLockGate`] and leaves membership unchanged.
//! - **Permission matrix** (0x6/0x8/0x4): the `spawn`/`alloc`/`topology_changes` bits
//!   are runtime [`Options`] data.
//! - **Destroy of a container holding a plugged resident** is a disabled precondition
//!   (MC `-EBUSY`), reported [`Teardown::ResidentPlugged`], never one of the three
//!   matrix refusals.
//! - **Child-only id uniqueness**: residents are keyed by [`ResidentId`] in a
//!   per-container [`BTreeMap`], so a duplicate id within *this* container is rejected
//!   ([`ResidentStep::Duplicate`]) — while re-creating in the child an id previously
//!   moved out into the parent is permitted, exactly DPRC-I1's eviction-boundary
//!   character (`dprc.qnt` `createResidentAt`/`assignResidentInAt` guard duplicates
//!   `world.child.residents` only, never `parentResidents`).
//!
//! The [`Locked`] phase keeps its refusable transitions *enabled but refusing*
//! (they return [`Outcome::Refused`], matching the model's enabled-then-`Refused`
//! shape), while [`Plugged`] *omits* `assign`/`create` entirely — two different
//! mechanisms for the two different reasons above.

use std::collections::BTreeMap;

use crate::types::ConstructName;

// ---- the four (plus [`Outcome`]) model sums, each lint-bijected to `dprc.qnt` ----

/// The kernel-side VFIO bind state, carried only on the [`Plugged`] face
/// (`dprc.qnt` `type VfioBind`). A DPRC's plugged state is not restool-mutable
/// (`docs/baseline/dprc.md` "Lifecycle ordering", V-POOL-1 rev 2); binding to
/// `vfio-fsl-mc` is the kernel lever, meaningful only while the container is
/// [`Plugged`], so it lives inside that marker — a two-case enum, not a `bool`
/// (ADR-0002 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfioBind {
    /// Handed to the kernel but not bound to a userspace VFIO driver.
    Unbound,
    /// Bound to `vfio-fsl-mc` — the kernel lever for userspace passthrough.
    BoundVfioFslMc,
}

/// The [`VfioBind`] variant names, in declaration order — the Rust copy of the
/// `dprc.qnt` `type VfioBind` cases as a `&str` list (ADR-0014: an enumeration that
/// restates the model is a linted copy, kept honest by the exhaustive `match` in
/// [`VfioBind::name`]).
pub const VFIO_BIND_VARIANTS: [&str; 2] = ["Unbound", "BoundVfioFslMc"];

impl VfioBind {
    /// This variant's name, the token [`VFIO_BIND_VARIANTS`] lists. The exhaustive
    /// `match` ties that list to the enum (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unbound => "Unbound",
            Self::BoundVfioFslMc => "BoundVfioFslMc",
        }
    }
}

/// A resident's origin, which decides its fate under the eviction law
/// (`dprc.qnt` `type ResidentKind`; ADR-0007 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidentKind {
    /// Created by this container: released (destroyed) with it.
    CreatedIn,
    /// Moved in from elsewhere: evicted one hop up into the parent, unplugged.
    AssignedIn,
}

/// The [`ResidentKind`] variant names, in declaration order — the Rust copy of the
/// `dprc.qnt` `type ResidentKind` cases (ADR-0014, tied by [`ResidentKind::name`]).
pub const RESIDENT_KIND_VARIANTS: [&str; 2] = ["CreatedIn", "AssignedIn"];

impl ResidentKind {
    /// This variant's name, the token [`RESIDENT_KIND_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CreatedIn => "CreatedIn",
            Self::AssignedIn => "AssignedIn",
        }
    }
}

/// The three distinct refusals of the DPRC permission matrix (`dprc.qnt`
/// `type Refusal`; `docs/baseline/dprc.md` unknown-register #3, V-DPRC-2/6).
///
/// A reconciler must read three distinct MC statuses and never collapse them to one
/// denial (design D4): the status carries which option bit refused. This is a
/// *distinct* vocabulary from [`crate::refuse::Refusal`] (the intent-compile refusal
/// set) — the containment matrix and the intent compiler judge different things, so
/// they do not share a type (adapters report, never judge: the classification and its
/// [`Refusal::mc_status`] sentinels live once, here, core-side).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// MC `0x6` Configuration error — `SPAWN_ALLOWED` absent.
    SpawnViolation,
    /// MC `0x8` No resources — `ALLOC_ALLOWED` absent (the child cannot draw pool ids).
    AllocViolation,
    /// MC `0x4` No privilege — `TOPOLOGY_CHANGES` absent, a lock strip, or a plugged-move.
    TopologyLockGate,
}

/// MC status `0x0`: the accepted outcome (`dprc.qnt` `mcStatus`).
pub const MC_STATUS_OK: u8 = 0;
/// MC status `0x4` No privilege — [`Refusal::TopologyLockGate`].
pub const MC_STATUS_NO_PRIVILEGE: u8 = 4;
/// MC status `0x6` Configuration error — [`Refusal::SpawnViolation`].
pub const MC_STATUS_CONFIG_ERROR: u8 = 6;
/// MC status `0x8` No resources — [`Refusal::AllocViolation`].
pub const MC_STATUS_NO_RESOURCES: u8 = 8;

/// The [`Refusal`] variant names, in declaration order — the Rust copy of the
/// `dprc.qnt` `type Refusal` cases (ADR-0014, tied by [`Refusal::name`]).
pub const REFUSAL_VARIANTS: [&str; 3] = ["SpawnViolation", "AllocViolation", "TopologyLockGate"];

impl Refusal {
    /// This variant's name, the token [`REFUSAL_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SpawnViolation => "SpawnViolation",
            Self::AllocViolation => "AllocViolation",
            Self::TopologyLockGate => "TopologyLockGate",
        }
    }

    /// The MC status code this refusal reports — the single core-side sentinel
    /// mapping (design D4; `dprc.qnt` `mcStatus`). A reconciler reads three distinct
    /// values, never one collapsed denial.
    #[must_use]
    pub const fn mc_status(self) -> u8 {
        match self {
            Self::SpawnViolation => MC_STATUS_CONFIG_ERROR,
            Self::AllocViolation => MC_STATUS_NO_RESOURCES,
            Self::TopologyLockGate => MC_STATUS_NO_PRIVILEGE,
        }
    }
}

/// The recorded outcome of a guarded transition (`dprc.qnt` `type Outcome`): the
/// `lastOutcome` the model records — a taken transition that either applied
/// ([`Outcome::Accepted`]) or was refused and left the container unchanged
/// ([`Outcome::Refused`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Outcome {
    /// The transition applied.
    Accepted,
    /// The transition was refused; the container is unchanged.
    Refused(Refusal),
}

/// The [`Outcome`] variant names, in declaration order — the Rust copy of the
/// `dprc.qnt` `type Outcome` cases (ADR-0014, tied by [`Outcome::name`]; Rule 9:
/// a two-entry model copy is bijected too).
pub const OUTCOME_VARIANTS: [&str; 2] = ["Accepted", "Refused"];

impl Outcome {
    /// This variant's name, the token [`OUTCOME_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Accepted => "Accepted",
            Self::Refused(_) => "Refused",
        }
    }

    /// The MC status this outcome reports (`dprc.qnt` `mcStatus`): `0x0` accepted, or
    /// the refusal's `0x4`/`0x6`/`0x8`.
    #[must_use]
    pub const fn mc_status(self) -> u8 {
        match self {
            Self::Accepted => MC_STATUS_OK,
            Self::Refused(r) => r.mc_status(),
        }
    }
}

/// The container lifecycle phase as a runtime sum — the parity-tested twin of
/// `dprc.qnt` `type ContainerState` (design D3): `Declared -> Created -> Populated
/// -> Plugged | Locked -> Emptied -> Destroyed`, VFIO bind state carried only on the
/// [`Plugged`] face. Observed via [`Container::phase`]; the typestate markers
/// ([`Declared`] … [`Destroyed`]) each project to one of these cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerState {
    /// Intent exists; no MC object yet.
    Declared,
    /// `dprc_create` done; empty; unplugged.
    Created,
    /// Holds residents; unplugged; the assign-only face.
    Populated,
    /// Kernel-active; VFIO bind state carried here.
    Plugged(VfioBind),
    /// `set-locked 1`: the sub-hierarchy is locked.
    Locked,
    /// Residents evicted/released; ready to destroy.
    Emptied,
    /// The MC object is gone.
    Destroyed,
}

/// The [`ContainerState`] variant names, in declaration order — the Rust copy of the
/// `dprc.qnt` `type ContainerState` cases (ADR-0014, tied by [`ContainerState::name`]
/// and, at the type level, by the typestate markers' [`Phase::NAME`]).
pub const CONTAINER_STATES: [&str; 7] = [
    "Declared",
    "Created",
    "Populated",
    "Plugged",
    "Locked",
    "Emptied",
    "Destroyed",
];

impl ContainerState {
    /// This variant's name, the token [`CONTAINER_STATES`] lists (ADR-0014). The
    /// [`Plugged`] payload does not change the case name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Declared => "Declared",
            Self::Created => "Created",
            Self::Populated => "Populated",
            Self::Plugged(_) => "Plugged",
            Self::Locked => "Locked",
            Self::Emptied => "Emptied",
            Self::Destroyed => "Destroyed",
        }
    }
}

// ---- the container's create-time-immutable identity and option mask ----

/// Pool-assigned container identity (`dprc.qnt` `type Identity`): `icid` (IOMMU
/// isolation context) and MC portal id, assigned once at create and never rewritten
/// (DPRC-I10; `docs/baseline/dprc.md` "Attribute mutability": create-time-immutable).
/// Immutability is by construction — a [`Container`] sets it in [`Container::create`]
/// and no transition exposes a setter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// IOMMU isolation context id.
    pub icid: u32,
    /// MC command-portal id.
    pub portal_id: u32,
}

/// The pool-assigned identity stand-in (`dprc.qnt` `POOL_IDENTITY`): the point is
/// that no transition rewrites it (DPRC-I10), not the specific numbers.
pub const POOL_IDENTITY: Identity = Identity {
    icid: 100,
    portal_id: 5,
};

/// The per-option-bit permission mask (`dprc.qnt` `type Options`;
/// `docs/baseline/dprc.md` create details). Create-time-immutable like [`Identity`]
/// (DPRC-I10).
//
// The mask is four independent permission bits mirroring the MC `dprc_cfg` option
// word; a bit-per-field struct is the faithful transcription of the model's record,
// so the four-bool shape is deliberate, not a modelling smell.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// `SPAWN_ALLOWED`: may spawn grandchild containers. Absent => [`Refusal::SpawnViolation`].
    pub spawn: bool,
    /// `ALLOC_ALLOWED`: may draw pool ids for created residents. Absent => [`Refusal::AllocViolation`].
    pub alloc: bool,
    /// `OBJ_CREATE_ALLOWED`: modeled for completeness; restool never issues on the
    /// child's own portal, so it never refuses through this project (`dprc.qnt` note).
    pub obj_create: bool,
    /// `TOPOLOGY_CHANGES_ALLOWED`: may `connect`/`disconnect` on this container.
    /// Absent (the child default) => [`Refusal::TopologyLockGate`].
    pub topology_changes: bool,
}

impl Options {
    /// The child-DPRC default option mask (`dprc.qnt` `DEFAULT_OPTIONS`, DPRC-I4):
    /// the four restool defaults; `TOPOLOGY_CHANGES` belongs to the root, not a child.
    pub const DEFAULT: Self = Self {
        spawn: true,
        alloc: true,
        obj_create: true,
        topology_changes: false,
    };
}

// ---- residents ----

/// A resident object's id — the child-container-local key that keys the per-container
/// [`BTreeMap`] and so gives child-only uniqueness by construction (see the module
/// note on DPRC-I1). Any MC object family may be a resident, so this is a bare index,
/// not a family-tagged handle.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResidentId(u32);

impl ResidentId {
    /// Wraps a raw resident id.
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// The raw id — the last-resort accessor for a map key or comparison.
    #[must_use]
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

impl From<u32> for ResidentId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl core::fmt::Display for ResidentId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "resident.{}", self.0)
    }
}

impl core::fmt::Debug for ResidentId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self}")
    }
}

/// A resident of a container (`dprc.qnt` `type Resident`), keyed elsewhere by
/// [`ResidentId`]. Its [`ResidentKind`] decides its eviction fate; `plugged` is the
/// per-resident runtime bit the plugged-move guard (DPRC-I3) reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resident {
    /// Origin, deciding the eviction fate (ADR-0007 §3).
    pub kind: ResidentKind,
    /// Whether the resident is handed to a driver (`assign --plugged=1`).
    pub plugged: bool,
}

/// The eviction destination — the parent container's residents (`dprc.qnt`
/// `World.parentResidents`). The one hop up an [`ResidentKind::AssignedIn`] resident
/// takes when its container is destroyed or emptied.
#[derive(Debug, Clone, Default)]
pub struct Parent {
    residents: BTreeMap<ResidentId, Resident>,
}

impl Parent {
    /// An empty parent.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The resident with this id, if the parent holds it.
    #[must_use]
    pub fn get(&self, id: ResidentId) -> Option<&Resident> {
        self.residents.get(&id)
    }

    /// Whether the parent holds a resident with this id.
    #[must_use]
    pub fn contains(&self, id: ResidentId) -> bool {
        self.residents.contains_key(&id)
    }

    /// The parent's residents, keyed by id.
    #[must_use]
    pub fn residents(&self) -> &BTreeMap<ResidentId, Resident> {
        &self.residents
    }
}

// ---- typestate markers (one per `ContainerState` case) ----

/// A lifecycle phase marker — the type-level projection of one [`ContainerState`]
/// case. Implementors carry exactly what the Quint case carries (nothing, or
/// [`Plugged`]'s [`VfioBind`]), keeping the type set structurally isomorphic to the
/// model sum (ADR-0002 §3).
pub trait Phase {
    /// The phase name, equal to the matching [`CONTAINER_STATES`] entry — the
    /// type-level half of the ADR-0014 tie parity asserts against the enum.
    const NAME: &'static str;

    /// This phase projected to the observable [`ContainerState`] (reading the VFIO
    /// bind for [`Plugged`]).
    fn as_state(&self) -> ContainerState;
}

/// An active, mutable-label phase (`dprc.qnt` `isActive`): every phase but
/// [`Declared`] and [`Destroyed`]. Grants [`Container::set_label`] and
/// [`Container::bus_remove_event`].
pub trait Active: Phase {}

/// An unplugged, assignable face (`dprc.qnt` `unpluggedFace`): [`Created`] and
/// [`Populated`]. Grants the assign-class transitions — this bound is the compile-time
/// enforcement of "assign only while unplugged".
pub trait UnpluggedFace: Active {}

/// A non-locked, kernel-reachable face — [`Created`], [`Populated`], [`Plugged`]:
/// the faces where `connect`/`empty`/`destroy` engage the containment law rather than
/// being lock-stripped.
pub trait LiveFace: Active {}

/// A non-locked face that manages resident plug state — [`Populated`] and
/// [`Plugged`]: the faces holding residents to plug/unplug/move without a lock strip.
pub trait UnlockedHolder: Active {}

macro_rules! phase_marker {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl Phase for $name {
            const NAME: &'static str = stringify!($name);
            fn as_state(&self) -> ContainerState {
                ContainerState::$name
            }
        }
    };
}

phase_marker! {
    /// Intent exists; no MC object yet. Not [`Active`] — a declared container has no
    /// label to repair and no residents.
    Declared
}
phase_marker! {
    /// `dprc_create` done; empty; unplugged.
    Created
}
phase_marker! {
    /// Holds residents; unplugged; the assign-only face.
    Populated
}
phase_marker! {
    /// `set-locked 1`: the sub-hierarchy is locked; the create/destroy/assign/plug
    /// classes are stripped ([`Refusal::TopologyLockGate`]) while `set-label` survives.
    Locked
}
phase_marker! {
    /// Residents evicted/released; ready to destroy.
    Emptied
}
phase_marker! {
    /// The MC object is gone.
    Destroyed
}

/// Kernel-active; carries the [`VfioBind`] state (the Quint `Plugged(VfioBind)`
/// payload). Constructed only by [`Container::plug`]; the bind is read via
/// [`Container::vfio`] and driven by [`Container::bind_vfio`]/[`Container::unbind_vfio`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plugged {
    bind: VfioBind,
}

impl Phase for Plugged {
    const NAME: &'static str = "Plugged";
    fn as_state(&self) -> ContainerState {
        ContainerState::Plugged(self.bind)
    }
}

impl Active for Created {}
impl Active for Populated {}
impl Active for Plugged {}
impl Active for Locked {}
impl Active for Emptied {}

impl UnpluggedFace for Created {}
impl UnpluggedFace for Populated {}

impl LiveFace for Created {}
impl LiveFace for Populated {}
impl LiveFace for Plugged {}

impl UnlockedHolder for Populated {}
impl UnlockedHolder for Plugged {}

// ---- transition result types (isomorphic to the model's guarded control flow) ----

/// The result of an assign-class transition ([`Container::create_resident`],
/// [`Container::assign_in`]) on an [`UnpluggedFace`]. Three arms mirror the model's
/// control flow: an accepted placement advances to [`Populated`], a permission/lock
/// refusal records the [`Refusal`] and leaves the source phase `S` unchanged, and a
/// duplicate id is the disabled precondition (`dprc.qnt` `not(residents.exists ...)`,
/// child-only — DPRC-I1 boundary), also unchanged.
#[derive(Debug)]
#[must_use]
pub enum ResidentStep<S: Phase> {
    /// Accepted: the resident was placed; the container is now [`Populated`].
    Placed(Container<Populated>),
    /// Refused with the recorded status; the container is unchanged.
    Refused(Container<S>, Refusal),
    /// Disabled: the id already names a resident of *this* container; unchanged.
    Duplicate(Container<S>, ResidentId),
}

/// The result of a resident-scoped op ([`Container::plug_resident`],
/// [`Container::unplug_resident`], [`Container::move_out`]). Mirrors `lastOutcome`
/// plus the model's disabled precondition (`Inapplicable`), the phase never changing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum ResidentOp {
    /// Accepted: the op applied.
    Done,
    /// Refused with the recorded status; membership/plug-state unchanged.
    Refused(Refusal),
    /// Disabled precondition: no resident with this id in the state the op requires.
    Inapplicable(ResidentId),
}

/// The result of a teardown ([`Container::empty`], [`Container::destroy`]) on a
/// [`LiveFace`]. Either it advances (eviction applied), or a still-plugged resident
/// makes it a disabled precondition — MC `-EBUSY`, modeled as a non-step, not one of
/// the three matrix refusals (`dprc.qnt` `destroyContainer` guard).
#[derive(Debug)]
#[must_use]
pub enum Teardown<Next: Phase, Same: Phase> {
    /// Advanced to the next phase; eviction applied.
    Done(Container<Next>),
    /// Disabled: a resident is still plugged; the container is unchanged.
    ResidentPlugged(Container<Same>),
}

/// The result of [`Container::unlock`] (`dprc.qnt` `unlock`): the phase restored
/// depends on whether the container still holds residents — [`Created`] if empty,
/// else [`Populated`]. A runtime value determines the resulting typestate, so the
/// return is a sum.
#[derive(Debug)]
#[must_use]
pub enum Unlocked {
    /// No residents: restored to [`Created`].
    Empty(Container<Created>),
    /// Residents present: restored to [`Populated`].
    Occupied(Container<Populated>),
}

// ---- the container ----

/// A child DPRC container in lifecycle phase `S` (`dprc.qnt` `type Container`).
///
/// [`identity`](Container::identity) and [`options`](Container::options) are set once
/// at [`create`](Container::create) and never rewritten (DPRC-I10, immutable by
/// construction: the fields are private and no transition takes them). `label` is the
/// one attribute mutable in every active phase, [`Locked`] included (V-DPRC-3).
/// Residents are keyed by [`ResidentId`] so id uniqueness is scoped to this container.
#[derive(Debug, Clone)]
pub struct Container<S: Phase> {
    identity: Identity,
    options: Options,
    label: ConstructName,
    residents: BTreeMap<ResidentId, Resident>,
    state: S,
}

impl<S: Phase> Container<S> {
    /// Carries the create-time-immutable fields forward into the next phase — the one
    /// place a phase change is realized, so `identity`/`options` are never rewritten.
    fn into_phase<T: Phase>(self, state: T) -> Container<T> {
        Container {
            identity: self.identity,
            options: self.options,
            label: self.label,
            residents: self.residents,
            state,
        }
    }

    /// The observable [`ContainerState`] (with the VFIO bind for [`Plugged`]).
    #[must_use]
    pub fn phase(&self) -> ContainerState {
        self.state.as_state()
    }

    /// The pool-assigned, immutable identity (DPRC-I10).
    #[must_use]
    pub fn identity(&self) -> Identity {
        self.identity
    }

    /// The create-time, immutable option mask (DPRC-I10).
    #[must_use]
    pub fn options(&self) -> Options {
        self.options
    }

    /// The container's label.
    #[must_use]
    pub fn label(&self) -> &ConstructName {
        &self.label
    }

    /// The container's residents, keyed by id.
    #[must_use]
    pub fn residents(&self) -> &BTreeMap<ResidentId, Resident> {
        &self.residents
    }
}

impl Container<Declared> {
    /// A freshly declared container: intent exists, no MC object yet (`dprc.qnt`
    /// `init`). Identity is the unassigned stand-in until [`create`](Container::create).
    #[must_use]
    pub fn declare() -> Self {
        Container {
            identity: Identity {
                icid: 0,
                portal_id: 0,
            },
            options: Options::DEFAULT,
            label: ConstructName::from(""),
            residents: BTreeMap::new(),
            state: Declared,
        }
    }

    /// `dprc create`: `Declared -> Created` (`dprc.qnt` `createContainerWith`).
    /// Identity ([`POOL_IDENTITY`]) and the option mask are pool-assigned here, once,
    /// and never rewritten thereafter (DPRC-I10). Always accepted.
    #[must_use]
    pub fn create(self, options: Options) -> Container<Created> {
        Container {
            identity: POOL_IDENTITY,
            options,
            label: self.label,
            residents: self.residents,
            state: Created,
        }
    }
}

impl<S: UnpluggedFace> Container<S> {
    /// Create a resident IN the child (a [`ResidentKind::CreatedIn`] object;
    /// `dprc.qnt` `createResidentAt`). Duplicate id within this container is the
    /// disabled precondition; `ALLOC_ALLOWED` absent => [`Refusal::AllocViolation`]
    /// (`0x8`); otherwise placed, advancing to [`Populated`].
    pub fn create_resident(self, id: ResidentId) -> ResidentStep<S> {
        if self.residents.contains_key(&id) {
            return ResidentStep::Duplicate(self, id);
        }
        if !self.options.alloc {
            return ResidentStep::Refused(self, Refusal::AllocViolation);
        }
        ResidentStep::Placed(self.place(id, ResidentKind::CreatedIn))
    }

    /// Assign a foreign object IN (a [`ResidentKind::AssignedIn`] resident;
    /// `dprc.qnt` `assignResidentInAt`) — valid only on the unplugged face (this
    /// bound). Duplicate id within this container is the disabled precondition;
    /// otherwise placed, advancing to [`Populated`]. No `ALLOC` gate: assign draws no
    /// pool id.
    pub fn assign_in(self, id: ResidentId) -> ResidentStep<S> {
        if self.residents.contains_key(&id) {
            return ResidentStep::Duplicate(self, id);
        }
        ResidentStep::Placed(self.place(id, ResidentKind::AssignedIn))
    }

    /// Places a resident and advances to [`Populated`] (shared accepted path of
    /// [`create_resident`](Self::create_resident)/[`assign_in`](Self::assign_in)).
    fn place(self, id: ResidentId, kind: ResidentKind) -> Container<Populated> {
        let mut next = self.into_phase(Populated);
        next.residents.insert(
            id,
            Resident {
                kind,
                plugged: false,
            },
        );
        next
    }

    /// Spawn a grandchild container (`dprc.qnt` `spawnGrandchild`). `SPAWN_ALLOWED`
    /// absent => [`Refusal::SpawnViolation`] (`0x6`); otherwise accepted. The
    /// grandchild itself is elided (design D5), so an accept leaves the container
    /// unchanged.
    pub fn spawn_grandchild(&self) -> Outcome {
        if self.options.spawn {
            Outcome::Accepted
        } else {
            Outcome::Refused(Refusal::SpawnViolation)
        }
    }

    /// `set-locked 1`: lock the child and its hierarchy (`dprc.qnt` `lockHierarchy`):
    /// `unplugged face -> Locked`. Always accepted.
    #[must_use]
    pub fn lock(self) -> Container<Locked> {
        self.into_phase(Locked)
    }
}

impl<S: LiveFace> Container<S> {
    /// `connect`/`disconnect` issued ON the child (`dprc.qnt` `connectEndpoints`).
    /// `TOPOLOGY_CHANGES_ALLOWED` absent (the child default) => [`Refusal::TopologyLockGate`]
    /// (`0x4`, V-DPRC-2-TOPO-1); present => accepted. No state change.
    pub fn connect_endpoints(&self) -> Outcome {
        if self.options.topology_changes {
            Outcome::Accepted
        } else {
            Outcome::Refused(Refusal::TopologyLockGate)
        }
    }

    /// Clean empty-first teardown -> [`Emptied`] (`dprc.qnt` `emptyContainer`; the NXP
    /// `destroy_dynamic_dpl` order, ADR-0007 context). A still-plugged resident is the
    /// disabled precondition ([`Teardown::ResidentPlugged`]); otherwise eviction
    /// applies — [`ResidentKind::CreatedIn`] residents released, [`ResidentKind::AssignedIn`]
    /// evicted unplugged into `parent`.
    pub fn empty(self, parent: &mut Parent) -> Teardown<Emptied, S> {
        if self.residents.values().any(|r| r.plugged) {
            return Teardown::ResidentPlugged(self);
        }
        Teardown::Done(self.evict_into(Emptied, parent))
    }

    /// `dprc destroy` (`dprc.qnt` `destroyContainer`; ADR-0007 §3). A non-empty
    /// destroy is a VALID transition: a still-plugged resident is the disabled
    /// precondition (MC `-EBUSY`, [`Teardown::ResidentPlugged`]); otherwise eviction
    /// applies and the container reaches [`Destroyed`], its post-state predicted.
    pub fn destroy(self, parent: &mut Parent) -> Teardown<Destroyed, S> {
        if self.residents.values().any(|r| r.plugged) {
            return Teardown::ResidentPlugged(self);
        }
        Teardown::Done(self.evict_into(Destroyed, parent))
    }

    /// Applies the eviction law (ADR-0007 §3) into `parent` and advances to `target`
    /// with an empty resident set: [`ResidentKind::CreatedIn`] residents are dropped
    /// (released with the container), [`ResidentKind::AssignedIn`] residents are moved
    /// one hop up into `parent`, **unplugged** — the plug bit is cleared so the parent
    /// never tracks a resident as bound that VFIO does not hold (`dprc.qnt`
    /// `evictInto`, DPRC-I1: a created resident never crosses the boundary).
    fn evict_into<T: Phase>(self, target: T, parent: &mut Parent) -> Container<T> {
        for (id, resident) in &self.residents {
            if resident.kind == ResidentKind::AssignedIn {
                parent.residents.insert(
                    *id,
                    Resident {
                        plugged: false,
                        ..*resident
                    },
                );
            }
        }
        let mut next = self.into_phase(target);
        next.residents.clear();
        next
    }
}

impl<S: UnlockedHolder> Container<S> {
    /// Plug a resident (`assign --plugged=1`; `dprc.qnt` `plugResidentAt`) — the
    /// kernel bind/unbind lever on a resident. Applies only to a resident currently
    /// unplugged; otherwise [`ResidentOp::Inapplicable`].
    pub fn plug_resident(&mut self, id: ResidentId) -> ResidentOp {
        self.set_resident_plugged(id, true)
    }

    /// Unplug a resident (`assign --plugged=0`; `dprc.qnt` `unplugResidentAt`).
    /// Applies only to a resident currently plugged; otherwise [`ResidentOp::Inapplicable`].
    pub fn unplug_resident(&mut self, id: ResidentId) -> ResidentOp {
        self.set_resident_plugged(id, false)
    }

    /// Shared body of [`plug_resident`](Self::plug_resident)/[`unplug_resident`](Self::unplug_resident):
    /// flips the plug bit only when it actually changes.
    fn set_resident_plugged(&mut self, id: ResidentId, plugged: bool) -> ResidentOp {
        match self.residents.get_mut(&id) {
            Some(r) if r.plugged != plugged => {
                r.plugged = plugged;
                ResidentOp::Done
            }
            _ => ResidentOp::Inapplicable(id),
        }
    }

    /// Move a resident OUT, one hop up into `parent` (`dprc.qnt` `moveResidentOutAt`).
    /// The plugged-move precondition (DPRC-I3): a plugged resident cannot move =>
    /// [`Refusal::TopologyLockGate`] (`0x4`), membership unchanged. No such resident =>
    /// [`ResidentOp::Inapplicable`]. Otherwise the resident leaves, arriving in the
    /// parent unplugged. This guard is runtime, not compile-time: the resident's plug
    /// bit is per-resident data, and typestating it would diverge from the model's
    /// `Resident { plugged: bool }`.
    pub fn move_out(&mut self, id: ResidentId, parent: &mut Parent) -> ResidentOp {
        let Some(resident) = self.residents.get(&id).copied() else {
            return ResidentOp::Inapplicable(id);
        };
        if resident.plugged {
            return ResidentOp::Refused(Refusal::TopologyLockGate);
        }
        self.residents.remove(&id);
        parent.residents.insert(
            id,
            Resident {
                plugged: false,
                ..resident
            },
        );
        ResidentOp::Done
    }
}

impl Container<Populated> {
    /// Hand the container to the kernel/VFIO: `Populated -> Plugged(Unbound)`
    /// (`dprc.qnt` `plugContainer`). Available only from [`Populated`] — a container
    /// must hold residents before it is plugged, so plugging an empty [`Created`]
    /// container does not type-check.
    ///
    /// The reconciler delta requires plug-then-assign unrepresentable: once plugged,
    /// no assign-class method exists, so the following does not compile.
    ///
    /// ```compile_fail
    /// use dpaa2_api::dprc::{Container, Options, ResidentId, ResidentStep};
    /// let created = Container::declare().create(Options::DEFAULT);
    /// let populated = match created.create_resident(ResidentId::new(1)) {
    ///     ResidentStep::Placed(c) => c,
    ///     _ => unreachable!(),
    /// };
    /// let plugged = populated.plug();
    /// // No `assign_in` on `Container<Plugged>`: plug-then-assign is unrepresentable.
    /// let _ = plugged.assign_in(ResidentId::new(2));
    /// ```
    #[must_use]
    pub fn plug(self) -> Container<Plugged> {
        self.into_phase(Plugged {
            bind: VfioBind::Unbound,
        })
    }
}

impl Container<Plugged> {
    /// The VFIO bind state carried on this face.
    #[must_use]
    pub fn vfio(&self) -> VfioBind {
        self.state.bind
    }

    /// Bind `vfio-fsl-mc` (`dprc.qnt` `bindVfio`): `Plugged(Unbound) -> Plugged(Bound)`.
    /// Meaningful only on this face; a no-op if already bound. Always accepted.
    pub fn bind_vfio(&mut self) -> Outcome {
        self.state.bind = VfioBind::BoundVfioFslMc;
        Outcome::Accepted
    }

    /// Unbind `vfio-fsl-mc` (`dprc.qnt` `unbindVfio`): `Plugged(Bound) -> Plugged(Unbound)`.
    /// A no-op if already unbound. Always accepted.
    pub fn unbind_vfio(&mut self) -> Outcome {
        self.state.bind = VfioBind::Unbound;
        Outcome::Accepted
    }
}

impl Container<Emptied> {
    /// `dprc destroy` of an already-emptied container: `Emptied -> Destroyed`
    /// (`dprc.qnt` `destroyContainer` from `Emptied`). No residents remain, so it is
    /// unconditional.
    #[must_use]
    pub fn destroy(self) -> Container<Destroyed> {
        self.into_phase(Destroyed)
    }
}

impl Container<Locked> {
    /// Create a resident under lock: the create class is stripped =>
    /// [`Refusal::TopologyLockGate`] (`0x4`, DPRC-I11 remainder). Enabled-but-refusing
    /// (the phase is unchanged), mirroring the model's `Refused` shape; the lock strip
    /// dominates, so no allocation or duplicate check is reached.
    pub fn create_resident(&self, _id: ResidentId) -> Outcome {
        Outcome::Refused(Refusal::TopologyLockGate)
    }

    /// Assign a resident under lock: the assign class is stripped => `0x4`.
    pub fn assign_in(&self, _id: ResidentId) -> Outcome {
        Outcome::Refused(Refusal::TopologyLockGate)
    }

    /// Spawn under lock: the create class is stripped => `0x4`.
    pub fn spawn_grandchild(&self) -> Outcome {
        Outcome::Refused(Refusal::TopologyLockGate)
    }

    /// `connect` under lock => `0x4` (`dprc.qnt` `connectEndpoints`).
    pub fn connect_endpoints(&self) -> Outcome {
        Outcome::Refused(Refusal::TopologyLockGate)
    }

    /// `assign --plugged` under lock is refused No privilege (`0x4`, V-DPRC-3); the
    /// resident reads back unplugged (unchanged).
    pub fn plug_resident(&self, _id: ResidentId) -> Outcome {
        Outcome::Refused(Refusal::TopologyLockGate)
    }

    /// `assign --plugged=0` under lock => `0x4`.
    pub fn unplug_resident(&self, _id: ResidentId) -> Outcome {
        Outcome::Refused(Refusal::TopologyLockGate)
    }

    /// Move a resident out under lock: the assign class is stripped => `0x4`,
    /// membership unchanged.
    pub fn move_out(&self, _id: ResidentId) -> Outcome {
        Outcome::Refused(Refusal::TopologyLockGate)
    }

    /// `dprc destroy` under lock: the destroy class is stripped => `0x4` (`dprc.qnt`
    /// `destroyContainer`, lock branch). Unlock first to tear down.
    pub fn destroy(&self) -> Outcome {
        Outcome::Refused(Refusal::TopologyLockGate)
    }

    /// `set-locked 0` from the root (`dprc.qnt` `unlock`): restores the unlocked face
    /// — [`Created`] if empty, else [`Populated`]. Always accepted.
    pub fn unlock(self) -> Unlocked {
        if self.residents.is_empty() {
            Unlocked::Empty(self.into_phase(Created))
        } else {
            Unlocked::Occupied(self.into_phase(Populated))
        }
    }
}

impl<S: Active> Container<S> {
    /// `set-label` (`dprc.qnt` `setLabelAt`): accepted in every active phase, [`Locked`]
    /// included — label drift on a locked container is repairable (V-DPRC-3, design
    /// Open Question resolved to its default). The one attribute mutable under lock.
    pub fn set_label(&mut self, label: ConstructName) -> Outcome {
        self.label = label;
        Outcome::Accepted
    }

    /// A Linux bus event on a kernel-active container — a `device_del`, a driver
    /// unbind, or a stray rescan notification (`dprc.qnt` `busRemoveEvent`). DPRC-I7
    /// (Breaking, `docs/baseline/dprc.md` "Kernel-side behavior"): object removal is
    /// Linux-side only — the MC object and its residents survive untouched — so the
    /// event leaves the container unchanged but for the recorded [`Outcome::Accepted`].
    /// No transition maps a bus event to [`Destroyed`]; this method advances nothing,
    /// which is exactly the DPRC-I7 guarantee.
    pub fn bus_remove_event(&self) -> Outcome {
        Outcome::Accepted
    }
}

#[cfg(test)]
mod tests {
    //! Parity of the state sum with `models/families/dprc.qnt` `dprc_lifecycle`, and
    //! the directed lifecycle runs mirrored as typestate walks. The transitions the
    //! reconciler delta requires unrepresentable are witnessed by the `compile_fail`
    //! doctest on [`Container::plug`] rather than a runtime test.

    use super::*;

    // ---- parity: each model sum's Rust copy is complete, both directions ----

    #[test]
    fn container_states_match_the_enum_and_the_model() {
        // dprc.qnt `type ContainerState`: the exact seven cases, in order.
        let sample = [
            ContainerState::Declared,
            ContainerState::Created,
            ContainerState::Populated,
            ContainerState::Plugged(VfioBind::Unbound),
            ContainerState::Locked,
            ContainerState::Emptied,
            ContainerState::Destroyed,
        ];
        // No missing case: every enum variant's name is in the list.
        for s in sample {
            assert!(CONTAINER_STATES.contains(&s.name()), "{}", s.name());
        }
        // No extra case, duplicate-free, exactly the model's seven.
        assert_eq!(CONTAINER_STATES.len(), sample.len());
        let mut seen = CONTAINER_STATES.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), CONTAINER_STATES.len(), "duplicate state name");
    }

    #[test]
    fn typestate_markers_match_the_container_states() {
        // The type-level half of the ADR-0014 tie: each marker's NAME is a state.
        for name in [
            Declared::NAME,
            Created::NAME,
            Populated::NAME,
            Plugged::NAME,
            Locked::NAME,
            Emptied::NAME,
            Destroyed::NAME,
        ] {
            assert!(CONTAINER_STATES.contains(&name), "{name}");
        }
        // And every state has a marker (same count, so the two sets coincide).
        assert_eq!(CONTAINER_STATES.len(), 7);
    }

    #[test]
    fn vfio_bind_variants_match_the_enum() {
        for v in [VfioBind::Unbound, VfioBind::BoundVfioFslMc] {
            assert!(VFIO_BIND_VARIANTS.contains(&v.name()));
        }
        assert_eq!(VFIO_BIND_VARIANTS, ["Unbound", "BoundVfioFslMc"]);
    }

    #[test]
    fn resident_kind_variants_match_the_enum() {
        for k in [ResidentKind::CreatedIn, ResidentKind::AssignedIn] {
            assert!(RESIDENT_KIND_VARIANTS.contains(&k.name()));
        }
        assert_eq!(RESIDENT_KIND_VARIANTS, ["CreatedIn", "AssignedIn"]);
    }

    #[test]
    fn refusal_variants_match_the_enum() {
        for r in [
            Refusal::SpawnViolation,
            Refusal::AllocViolation,
            Refusal::TopologyLockGate,
        ] {
            assert!(REFUSAL_VARIANTS.contains(&r.name()));
        }
        assert_eq!(
            REFUSAL_VARIANTS,
            ["SpawnViolation", "AllocViolation", "TopologyLockGate"]
        );
    }

    #[test]
    fn outcome_variants_match_the_enum() {
        for o in [
            Outcome::Accepted,
            Outcome::Refused(Refusal::TopologyLockGate),
        ] {
            assert!(OUTCOME_VARIANTS.contains(&o.name()));
        }
        assert_eq!(OUTCOME_VARIANTS, ["Accepted", "Refused"]);
    }

    // ---- helpers ----

    fn created() -> Container<Created> {
        Container::declare().create(Options::DEFAULT)
    }

    fn placed(c: Container<Created>, id: u32, kind: ResidentKind) -> Container<Populated> {
        let step = match kind {
            ResidentKind::CreatedIn => c.create_resident(ResidentId::new(id)),
            ResidentKind::AssignedIn => c.assign_in(ResidentId::new(id)),
        };
        match step {
            ResidentStep::Placed(p) => p,
            _ => panic!("expected the resident to be placed"),
        }
    }

    // ---- directed runs (dprc.qnt) mirrored as typestate walks ----

    #[test]
    fn lifecycle_test() {
        // dprc.qnt `lifecycleTest`: create -> resident -> plug -> bind -> unbind.
        let c = placed(created(), 1, ResidentKind::CreatedIn);
        let mut c = c.plug();
        assert_eq!(c.phase(), ContainerState::Plugged(VfioBind::Unbound));
        assert_eq!(c.bind_vfio(), Outcome::Accepted);
        assert_eq!(c.vfio(), VfioBind::BoundVfioFslMc);
        assert_eq!(c.unbind_vfio(), Outcome::Accepted);
        assert_eq!(c.phase(), ContainerState::Plugged(VfioBind::Unbound));
    }

    #[test]
    fn refusal_statuses_distinct_test() {
        // dprc.qnt `refusalStatusesDistinctTest`: the three statuses are distinct.
        let mut statuses = [
            Refusal::SpawnViolation.mc_status(),
            Refusal::AllocViolation.mc_status(),
            Refusal::TopologyLockGate.mc_status(),
        ];
        statuses.sort_unstable();
        assert_eq!(statuses, [4, 6, 8]);
    }

    #[test]
    fn spawn_refused_0x6_test() {
        // dprc.qnt `spawnRefused0x6Test`: SPAWN absent => 0x6.
        let c = Container::declare().create(Options {
            spawn: false,
            ..Options::DEFAULT
        });
        let out = c.spawn_grandchild();
        assert_eq!(out, Outcome::Refused(Refusal::SpawnViolation));
        assert_eq!(out.mc_status(), 6);
    }

    #[test]
    fn alloc_refused_0x8_test() {
        // dprc.qnt `allocRefused0x8Test`: ALLOC absent => 0x8, nothing created.
        let c = Container::declare().create(Options {
            alloc: false,
            ..Options::DEFAULT
        });
        match c.create_resident(ResidentId::new(1)) {
            ResidentStep::Refused(c, r) => {
                assert_eq!(r, Refusal::AllocViolation);
                assert_eq!(r.mc_status(), 8);
                assert!(c.residents().is_empty());
            }
            _ => panic!("expected an AllocViolation refusal"),
        }
    }

    #[test]
    fn connect_topology_gate_test() {
        // dprc.qnt `connectNoTopologyRefused0x4Test` / `connectWithTopologyTest`.
        let denied = created().connect_endpoints();
        assert_eq!(denied, Outcome::Refused(Refusal::TopologyLockGate));
        assert_eq!(denied.mc_status(), 4);

        let allowed = Container::declare()
            .create(Options {
                topology_changes: true,
                ..Options::DEFAULT
            })
            .connect_endpoints();
        assert_eq!(allowed, Outcome::Accepted);
    }

    #[test]
    fn plugged_move_refused_0x4_test() {
        // dprc.qnt `pluggedMoveRefused0x4Test`: moving a plugged resident => 0x4, stays.
        let mut c = placed(created(), 1, ResidentKind::CreatedIn);
        assert_eq!(c.plug_resident(ResidentId::new(1)), ResidentOp::Done);
        let mut parent = Parent::new();
        let out = c.move_out(ResidentId::new(1), &mut parent);
        assert_eq!(out, ResidentOp::Refused(Refusal::TopologyLockGate));
        assert!(c.residents().contains_key(&ResidentId::new(1)));
        assert!(!parent.contains(ResidentId::new(1)));
    }

    #[test]
    fn eviction_both_kinds_test() {
        // dprc.qnt `evictionBothKindsTest`: non-empty destroy, both resident kinds.
        let c = placed(created(), 1, ResidentKind::CreatedIn);
        let ResidentStep::Placed(c) = c.assign_in(ResidentId::new(2)) else {
            panic!("expected resident 2 assigned in");
        };
        let mut parent = Parent::new();
        let Teardown::Done(c) = c.destroy(&mut parent) else {
            panic!("expected the destroy to complete");
        };
        assert_eq!(c.phase(), ContainerState::Destroyed);
        assert!(c.residents().is_empty());
        // CreatedIn (1) released with the container; AssignedIn (2) re-parented unplugged.
        assert!(!parent.contains(ResidentId::new(1)));
        let re_parented = parent
            .get(ResidentId::new(2))
            .expect("assigned-in returns home");
        assert!(!re_parented.plugged);
    }

    #[test]
    fn dprc_i1_created_resident_never_crosses_the_boundary() {
        // dprc.qnt `DPRC_I1Test`: the CreatedIn resident stays in-container (released),
        // only the AssignedIn resident evicts one hop up.
        let c = placed(created(), 1, ResidentKind::CreatedIn);
        let ResidentStep::Placed(c) = c.assign_in(ResidentId::new(2)) else {
            panic!("expected resident 2 assigned in");
        };
        let mut parent = Parent::new();
        let Teardown::Done(_) = c.destroy(&mut parent) else {
            panic!();
        };
        assert!(!parent.contains(ResidentId::new(1)));
        assert!(parent.contains(ResidentId::new(2)));
    }

    #[test]
    fn child_only_uniqueness_permits_recreate_after_move_out() {
        // The task 1.2 handoff: uniqueness is scoped to THIS container. An id moved out
        // into the parent may be re-created in the child (DPRC-I1 eviction boundary).
        let mut c = placed(created(), 1, ResidentKind::AssignedIn);
        let mut parent = Parent::new();
        assert_eq!(
            c.move_out(ResidentId::new(1), &mut parent),
            ResidentOp::Done
        );
        assert!(parent.contains(ResidentId::new(1)));
        // Re-creating id 1 in the child is permitted (not a global-uniqueness error).
        match c.create_resident(ResidentId::new(1)) {
            ResidentStep::Placed(c) => assert!(c.residents().contains_key(&ResidentId::new(1))),
            _ => panic!("re-creating a moved-out id in the child must be permitted"),
        }
    }

    #[test]
    fn duplicate_id_within_child_is_rejected() {
        let c = placed(created(), 1, ResidentKind::CreatedIn);
        match c.create_resident(ResidentId::new(1)) {
            ResidentStep::Duplicate(_, id) => assert_eq!(id, ResidentId::new(1)),
            _ => panic!("a duplicate id in the same container must be rejected"),
        }
    }

    #[test]
    fn teardown_reachable_test() {
        // dprc.qnt `teardownReachableTest`: empty-first then destroy reaches Destroyed.
        let c = placed(created(), 1, ResidentKind::CreatedIn);
        let ResidentStep::Placed(c) = c.assign_in(ResidentId::new(2)) else {
            panic!("expected resident 2 assigned in");
        };
        let mut parent = Parent::new();
        let Teardown::Done(c) = c.empty(&mut parent) else {
            panic!("expected empty to complete");
        };
        assert_eq!(c.phase(), ContainerState::Emptied);
        let c = c.destroy();
        assert_eq!(c.phase(), ContainerState::Destroyed);
    }

    #[test]
    fn destroy_with_plugged_resident_is_ebusy_not_a_matrix_refusal() {
        // F-ebusy: a still-plugged resident disables destroy (MC -EBUSY), reported as a
        // non-step, never one of the three matrix refusals.
        let mut c = placed(created(), 1, ResidentKind::CreatedIn);
        assert_eq!(c.plug_resident(ResidentId::new(1)), ResidentOp::Done);
        let mut parent = Parent::new();
        match c.destroy(&mut parent) {
            Teardown::ResidentPlugged(c) => {
                assert!(c.residents().contains_key(&ResidentId::new(1)));
            }
            Teardown::Done(_) => panic!("destroy must not proceed with a plugged resident"),
        }
    }

    #[test]
    fn label_under_lock_test() {
        // dprc.qnt `labelUnderLockTest`: set-label accepted on a locked child.
        let c = placed(created(), 1, ResidentKind::CreatedIn);
        let mut c = c.lock();
        let out = c.set_label(ConstructName::from("repaired"));
        assert_eq!(out, Outcome::Accepted);
        assert_eq!(c.phase(), ContainerState::Locked);
        assert_eq!(c.label().as_str(), "repaired");
    }

    #[test]
    fn lock_strips_create_test() {
        // dprc.qnt `lockStripsCreateTest`: create under lock => 0x4.
        let c = placed(created(), 1, ResidentKind::CreatedIn).lock();
        let out = c.create_resident(ResidentId::new(2));
        assert_eq!(out, Outcome::Refused(Refusal::TopologyLockGate));
        assert_eq!(out.mc_status(), 4);
    }

    #[test]
    fn lock_refuses_plug_test() {
        // dprc.qnt `lockRefusesPlugTest`: assign --plugged under lock => 0x4, unplugged.
        let c = placed(created(), 1, ResidentKind::CreatedIn).lock();
        let out = c.plug_resident(ResidentId::new(1));
        assert_eq!(out, Outcome::Refused(Refusal::TopologyLockGate));
        assert_eq!(out.mc_status(), 4);
        assert!(!c.residents()[&ResidentId::new(1)].plugged);
    }

    #[test]
    fn unlock_restores_test() {
        // dprc.qnt `unlockRestoresTest`: unlock restores the create class.
        let c = placed(created(), 1, ResidentKind::CreatedIn).lock();
        let Unlocked::Occupied(c) = c.unlock() else {
            panic!("a populated container unlocks to Populated");
        };
        match c.create_resident(ResidentId::new(2)) {
            ResidentStep::Placed(c) => assert!(c.residents().contains_key(&ResidentId::new(2))),
            _ => panic!("create must be accepted after unlock"),
        }
    }

    #[test]
    fn dprc_i7_bus_event_is_not_an_mc_destroy() {
        // dprc.qnt `DPRC_I7Test`: a bus event leaves a plugged, bound container intact.
        let c = placed(created(), 1, ResidentKind::CreatedIn);
        let mut c = c.plug();
        assert_eq!(c.bind_vfio(), Outcome::Accepted);
        let out = c.bus_remove_event();
        assert_eq!(out, Outcome::Accepted);
        assert_eq!(c.phase(), ContainerState::Plugged(VfioBind::BoundVfioFslMc));
        assert!(c.residents().contains_key(&ResidentId::new(1)));
    }

    #[test]
    fn identity_and_options_are_immutable_across_transitions() {
        // DPRC-I10: identity is pool-assigned once; options never drift off the menu.
        let c = created();
        assert_eq!(c.identity(), POOL_IDENTITY);
        let c = placed(c, 1, ResidentKind::CreatedIn);
        let c = c.lock();
        assert_eq!(c.identity(), POOL_IDENTITY);
        assert_eq!(c.options(), Options::DEFAULT);
    }
}
