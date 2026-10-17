//! The dpsw create surface as compile-time-refined types — the Rust twin of the Quint
//! create-configure state machine (`models/families/dpsw.qnt` module `dpsw_lifecycle`).
//!
//! Filed under `families/` as the per-family vocabulary tile whose `families/dpsw.rs`
//! path mirrors that `models/families/dpsw.qnt` quint twin one-to-one (ADR-0018). The
//! twin lives in module `dpsw_lifecycle`, not `module dpsw`: the thin params face keeps
//! the generic create-surface names (`CreateCfg`, `Refusal`) out of the corpus-wide `.*`
//! import, so this Rust module mirrors `dpsw_lifecycle` (model header, `dpsw.qnt`).
//!
//! Authored model-first (quint-is-the-spec, ADR-0002 §3): the sums, payloads and refined
//! ranges here are structurally isomorphic to the Quint sums — same cases, same payloads,
//! same range semantics; names converge on readable English on both surfaces. This family
//! names no transport (sans-io hexagonal, ADR-0018 — the backend vocabulary lives in
//! `dpaa2-mc`); it carries only the twelve-field create block, its options vocabulary, the
//! kernel-bindability law, the reset facet, and the P1 phase types the reconciler walks.
//!
//! # Immutable by construction (dpsw-typestate design D2; DPSW-I1)
//!
//! Every one of the twelve `dpsw_cfg` fields is create-time-immutable — there is no
//! `dpsw_set_*` for any of them, and a wrongly-created switch is destroyed and recreated,
//! never repaired (`docs/baseline/dpsw.md` "Attribute mutability"). The family encodes it
//! structurally: [`DpswCfg`] wraps the validated create record in a private field with no
//! setter, so a block cannot change after construction — witnessed by the `compile_fail`
//! doctest on [`DpswCfg`]. Because all four bindability fields are immutable, the unbindable
//! shape is not merely detected but unrepresentable (dpsw-typestate design D2) as a
//! [`DpswCfg`].
//!
//! # The kernel-bindability law gates construction (dpsw-typestate design D2; DPSW-I1/I2)
//!
//! [`DpswCfg::new`] is the only writer. The per-field range envelope is enforced upstream
//! by the refined newtypes ([`NumIfs`], [`MaxFdbs`], …), which refuse an out-of-envelope
//! value by name before a record exists; `new` then applies the `dpsw.qnt` `classify`
//! verdict in the model's order — the DPSW-I2 ls-addsw vendor shape first (so a vendor
//! recipe, which is also unbindable, is named by its own refusal), then the general
//! DPSW-I1 unbindable refusal — and only a kernel-bindable record becomes a [`DpswCfg`]
//! (`dpsw.qnt` `classify` / `createAt` accepted branch; V-DPSW-4 banked).

use std::collections::BTreeSet;
use std::fmt;

use crate::core::error::Error;

// ---- the refined range envelope (dpni `ranged_option!` idiom; `dpsw.qnt` `CreateCfg`) ----

/// Mints one refined create-field range type — a `u16` newtype whose only constructor
/// refuses anything outside the restool create envelope `LO..=HI`.
///
/// This is the range envelope the model documents on the `dpsw.qnt` `CreateCfg` record
/// fields (restool `--num-ifs`/`--max-fdbs`/…). One distinct type per field so a value
/// for one field can never be passed for another at [`DpswCfg::new`] (the crate's
/// newtype-per-slot idiom).
macro_rules! ranged {
    ($(#[$meta:meta])* $name:ident, $lo:literal, $hi:literal, $field:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u16);

        impl $name {
            /// Inclusive lower bound of the restool create envelope for this field
            /// (`docs/baseline/dpsw.md` "Option inventory").
            pub const LO: u16 = $lo;

            /// Inclusive upper bound of the restool create envelope for this field
            /// (`docs/baseline/dpsw.md` "Option inventory").
            pub const HI: u16 = $hi;

            /// Builds the refined value, refusing anything outside the restool create
            /// envelope `LO..=HI` (the `dpsw.qnt` `CreateCfg` field range — the range
            /// envelope the model documents per field).
            ///
            /// # Errors
            /// [`Error::Config`] naming the field and its `LO..=HI` range when `v` is out
            /// of envelope.
            pub fn new(v: u16) -> Result<Self, Error> {
                if (Self::LO..=Self::HI).contains(&v) {
                    Ok(Self(v))
                } else {
                    Err(Error::Config(format!(
                        concat!($field, " {} outside the restool create envelope {}..={}"),
                        v,
                        Self::LO,
                        Self::HI,
                    )))
                }
            }

            /// The refined value.
            #[must_use]
            pub const fn get(self) -> u16 {
                self.0
            }
        }
    };
}

ranged! {
    /// `--num-ifs` 1..=65535 — the immutable port count switch-port membership is bounded
    /// by (`dpsw.qnt` `numIfs`; DPSW-I10). `0` has no MC-default meaning here and is refused.
    NumIfs, 1, 65535, "num_ifs"
}
ranged! {
    /// `--max-vlans` 0..=65535 (`0` = MC default) (`dpsw.qnt` `maxVlans`).
    MaxVlans, 0, 65535, "max_vlans"
}
ranged! {
    /// `--max-fdbs` 0..=255 (`dpsw.qnt` `maxFdbs`; the DPSW-I1 bindability law reads
    /// `max_fdbs >= num_ifs`).
    MaxFdbs, 0, 255, "max_fdbs"
}
ranged! {
    /// `--max-fdb-entries` 0..=65535 (`dpsw.qnt` `maxFdbEntries`).
    MaxFdbEntries, 0, 65535, "max_fdb_entries"
}
ranged! {
    /// `--fdb-aging-time` 0..=65535 (`dpsw.qnt` `fdbAgingTime`).
    FdbAgingTime, 0, 65535, "fdb_aging_time"
}
ranged! {
    /// `--max-fdb-mc-groups` 0..=65535 (`dpsw.qnt` `maxFdbMcGroups`).
    MaxFdbMcGroups, 0, 65535, "max_fdb_mc_groups"
}

// ---- the option vocabulary and the enumerated create fields (`dpsw.qnt` `type DpswOpt` /
//      `ComponentType` / `FloodingCfg` / `BroadcastCfg`) ----

/// The seven MC option flags restool exposes for dpsw (`dpsw.qnt` `type DpswOpt`;
/// `docs/baseline/dpsw.md` "Option inventory" `--options` row). Only [`CtrlIfDis`](Self::CtrlIfDis)
/// is load-bearing here: it gates the control interface the kernel bindability predicate
/// reads (DPSW-I1). The other six are carried for parity with the create surface, never
/// derived. A flag outside this vocabulary has no variant, so it is unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DpswOpt {
    /// `DPSW_OPT_FLOODING_DIS`.
    FloodingDis,
    /// `DPSW_OPT_MULTICAST_DIS`.
    MulticastDis,
    /// `DPSW_OPT_CTRL_IF_DIS` — disables the control interface (DPSW-I1).
    CtrlIfDis,
    /// `DPSW_OPT_FLOODING_METERING_DIS`.
    FloodingMeteringDis,
    /// `DPSW_OPT_METERING_EN`.
    MeteringEn,
    /// `DPSW_OPT_BP_PER_IF`.
    BpPerIf,
    /// `DPSW_OPT_LAG_DIS`.
    LagDis,
}

/// The [`DpswOpt`] variant names, in declaration order — the Rust copy of the `dpsw.qnt`
/// `type DpswOpt` cases (ADR-0014: an enumeration that restates the model is a linted
/// copy, kept honest by the exhaustive `match` in [`DpswOpt::name`]).
pub const DPSW_OPT_VARIANTS: [&str; 7] = [
    "FloodingDis",
    "MulticastDis",
    "CtrlIfDis",
    "FloodingMeteringDis",
    "MeteringEn",
    "BpPerIf",
    "LagDis",
];

impl DpswOpt {
    /// The whole MC vocabulary — the Rust copy of the `dpsw.qnt` `MC_VOCABULARY` set (the
    /// seven named flags, no more). A flag outside this array has no variant.
    pub const MC_VOCABULARY: [Self; 7] = [
        Self::FloodingDis,
        Self::MulticastDis,
        Self::CtrlIfDis,
        Self::FloodingMeteringDis,
        Self::MeteringEn,
        Self::BpPerIf,
        Self::LagDis,
    ];

    /// This variant's name, the token [`DPSW_OPT_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::FloodingDis => "FloodingDis",
            Self::MulticastDis => "MulticastDis",
            Self::CtrlIfDis => "CtrlIfDis",
            Self::FloodingMeteringDis => "FloodingMeteringDis",
            Self::MeteringEn => "MeteringEn",
            Self::BpPerIf => "BpPerIf",
            Self::LagDis => "LagDis",
        }
    }
}

/// The dpsw component type (`dpsw.qnt` `type ComponentType`; restool `--component-type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentType {
    /// A customer-VLAN bridge component (`C_VLAN`).
    CVlan,
    /// A service-VLAN bridge component (`S_VLAN`).
    SVlan,
}

/// The [`ComponentType`] variant names, in declaration order — the Rust copy of the
/// `dpsw.qnt` `type ComponentType` cases (ADR-0014, kept honest by [`ComponentType::name`]).
pub const COMPONENT_TYPE_VARIANTS: [&str; 2] = ["CVlan", "SVlan"];

impl ComponentType {
    /// This variant's name, the token [`COMPONENT_TYPE_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CVlan => "CVlan",
            Self::SVlan => "SVlan",
        }
    }
}

/// The flooding domain (`dpsw.qnt` `type FloodingCfg`; restool `--flooding-cfg`). The kernel
/// bindability law requires [`PerFdb`](Self::PerFdb) (DPSW-I1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloodingCfg {
    /// Flooding scoped per VLAN (`PER_VLAN`) — the ls-addsw default (DPSW-I2).
    PerVlan,
    /// Flooding scoped per FDB (`PER_FDB`) — the kernel-bindable shape (DPSW-I1).
    PerFdb,
}

/// The [`FloodingCfg`] variant names, in declaration order — the Rust copy of the
/// `dpsw.qnt` `type FloodingCfg` cases (ADR-0014, kept honest by [`FloodingCfg::name`]).
pub const FLOODING_CFG_VARIANTS: [&str; 2] = ["PerVlan", "PerFdb"];

impl FloodingCfg {
    /// This variant's name, the token [`FLOODING_CFG_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PerVlan => "PerVlan",
            Self::PerFdb => "PerFdb",
        }
    }
}

/// The broadcast domain (`dpsw.qnt` `type BroadcastCfg`; restool `--broadcast-cfg`). The
/// kernel bindability law requires [`PerFdb`](Self::PerFdb) (DPSW-I1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BroadcastCfg {
    /// Broadcast scoped per object (`PER_OBJECT`) — the ls-addsw default (DPSW-I2).
    PerObject,
    /// Broadcast scoped per FDB (`PER_FDB`) — the kernel-bindable shape (DPSW-I1).
    PerFdb,
}

/// The [`BroadcastCfg`] variant names, in declaration order — the Rust copy of the
/// `dpsw.qnt` `type BroadcastCfg` cases (ADR-0014, kept honest by [`BroadcastCfg::name`]).
pub const BROADCAST_CFG_VARIANTS: [&str; 2] = ["PerObject", "PerFdb"];

impl BroadcastCfg {
    /// This variant's name, the token [`BROADCAST_CFG_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PerObject => "PerObject",
            Self::PerFdb => "PerFdb",
        }
    }
}

// ---- the reset facet, parameterized CONSERVATIVE (DPSW-I4; ADR-0019 reset facet) ----

/// The DPSW-I4 reset-facet scope (`dpsw.qnt` `type ResetScope`). A bind-time `dpsw_reset`
/// dismantles MC-default state before the driver rebuilds its own; the flib has no
/// `set_resetable` analogue, so the scope stays a visible parameter (`docs/baseline/dpsw.md`
/// "Kernel-side behavior").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResetScope {
    /// A total reset: no pre-bind MC-default state survives (the CONSERVATIVE default).
    Total,
    /// A partial reset: MC-default state is left as found (the future refinement).
    Partial,
}

/// The [`ResetScope`] variant names, in declaration order — the Rust copy of the
/// `dpsw.qnt` `type ResetScope` cases (ADR-0014, kept honest by [`ResetScope::name`]).
pub const RESET_SCOPE_VARIANTS: [&str; 2] = ["Total", "Partial"];

impl ResetScope {
    /// This variant's name, the token [`RESET_SCOPE_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Total => "Total",
            Self::Partial => "Partial",
        }
    }
}

/// The DPSW-I4 reset-facet scope, pinned CONSERVATIVE (`dpsw.qnt` `RESET_FACET_SCOPE`): a
/// bind-time reset is [`Total`](ResetScope::Total). Refine to [`Partial`](ResetScope::Partial)
/// when the discovery sitting (dpsw-typestate design D9) measures what reset preserves.
pub const RESET_FACET_SCOPE: ResetScope = ResetScope::Total;

/// MC-default state after a bind-time reset (`dpsw.qnt` `mcDefaultsAfterReset`; DPSW-I4): a
/// total reset dismantles it (`false`); a partial reset leaves it as found (`before`).
#[must_use]
pub const fn mc_defaults_after_reset(before: bool) -> bool {
    match RESET_FACET_SCOPE {
        ResetScope::Total => false,
        ResetScope::Partial => before,
    }
}

// ---- the create-time refusals (`dpsw.qnt` `type Refusal`; DPSW-I1/I2) ----

/// The two create-time refusals a non-bindable dpsw create block earns, refused by name
/// before any switch exists (`dpsw.qnt` `type Refusal`; `docs/baseline/dpsw.md`
/// "Kernel-side behavior", V-DPSW-4 banked). The Rust twin of the model's
/// `Outcome = Accepted | Refused(Refusal)` is `Result<DpswCfg, Refusal>`: the accepted
/// branch is the constructed block, the refused branch is this sum. Folds into the crate
/// error idiom via [`From`] ⇒ [`Error::Config`].
///
/// The per-field range envelope is a distinct boundary refusal carried by the refined
/// newtypes ([`NumIfs`]/[`MaxFdbs`]/…) as [`Error::Config`], not a member of this sum —
/// the model's `type Refusal` names only these two bindability-class verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[must_use]
pub enum Refusal {
    /// DPSW-I2 — the ls-addsw default shape (`PER_VLAN` flooding + `PER_OBJECT` broadcast):
    /// the vendor recipe contradicts the only in-corpus consumer, so it is not an oracle.
    VendorRecipeShape,
    /// DPSW-I1 — any other unbindable shape: control interface off, a flooding or broadcast
    /// domain not `PER_FDB`, or `max_fdbs < num_ifs`.
    UnbindableCfg,
}

/// The [`Refusal`] variant names, in declaration order — the Rust copy of the `dpsw.qnt`
/// `type Refusal` cases (ADR-0014, kept honest by the exhaustive `match` in [`Refusal::name`]).
pub const REFUSAL_VARIANTS: [&str; 2] = ["VendorRecipeShape", "UnbindableCfg"];

impl Refusal {
    /// This variant's name, the token [`REFUSAL_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::VendorRecipeShape => "VendorRecipeShape",
            Self::UnbindableCfg => "UnbindableCfg",
        }
    }
}

impl fmt::Display for Refusal {
    /// Names the violated bindability law (DPSW-I1/I2).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VendorRecipeShape => write!(
                f,
                "the ls-addsw default shape (PER_VLAN flooding + PER_OBJECT broadcast) is \
                 kernel-unbindable and is not an oracle (DPSW-I2)"
            ),
            Self::UnbindableCfg => write!(
                f,
                "the switch is kernel-unbindable: the control interface must be enabled, \
                 flooding and broadcast must be PER_FDB, and max_fdbs must cover num_ifs \
                 (DPSW-I1)"
            ),
        }
    }
}

impl From<Refusal> for Error {
    /// A create-time bindability refusal is a configuration boundary error — the crate's
    /// error idiom (the sibling of the range-envelope [`Error::Config`]).
    fn from(refusal: Refusal) -> Self {
        Error::Config(refusal.to_string())
    }
}

// ---- the twelve-field create record (`dpsw.qnt` `type CreateCfg`) ----

/// The twelve-field dpsw create record a caller supplies to [`DpswCfg::new`] — the Rust twin
/// of the `dpsw.qnt` `type CreateCfg` the model's `classify`/`createAt` take. The range
/// envelope is already satisfied field-by-field by the refined newtypes; `new` adds the
/// DPSW-I1/I2 classification and freezes the accepted record inside an immutable [`DpswCfg`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DpswCreate {
    /// `--num-ifs` — the immutable port count (`dpsw.qnt` `numIfs`; DPSW-I10).
    pub num_ifs: NumIfs,
    /// `--options` — the seven-flag MC vocabulary (`dpsw.qnt` `options`).
    pub options: BTreeSet<DpswOpt>,
    /// `--max-vlans` 0..=65535 (`dpsw.qnt` `maxVlans`).
    pub max_vlans: MaxVlans,
    /// `--max-fdbs` 0..=255 (`dpsw.qnt` `maxFdbs`; DPSW-I1 reads `>= num_ifs`).
    pub max_fdbs: MaxFdbs,
    /// `--max-fdb-entries` 0..=65535 (`dpsw.qnt` `maxFdbEntries`).
    pub max_fdb_entries: MaxFdbEntries,
    /// `--fdb-aging-time` 0..=65535 (`dpsw.qnt` `fdbAgingTime`).
    pub fdb_aging_time: FdbAgingTime,
    /// `--max-fdb-mc-groups` 0..=65535 (`dpsw.qnt` `maxFdbMcGroups`).
    pub max_fdb_mc_groups: MaxFdbMcGroups,
    /// `--mem-size` — a dead/undocumented field, never observed (`dpsw.qnt` `memSize`; one of
    /// the two unobservable fields the census signature drops).
    pub mem_size: u32,
    /// `--component-type` (`dpsw.qnt` `componentType`).
    pub component_type: ComponentType,
    /// `--flooding-cfg` (`dpsw.qnt` `floodingCfg`; DPSW-I1 requires `PER_FDB`).
    pub flooding_cfg: FloodingCfg,
    /// `--broadcast-cfg` (`dpsw.qnt` `broadcastCfg`; DPSW-I1 requires `PER_FDB`).
    pub broadcast_cfg: BroadcastCfg,
    /// A dead flib field, never settable or printed (`dpsw.qnt` `maxMetersPerIf`; the second
    /// unobservable field the census signature drops).
    pub max_meters_per_if: u32,
}

impl DpswCreate {
    /// Whether the control interface is enabled (`dpsw.qnt` `ctrlIfEnabled`): enabled unless
    /// [`DpswOpt::CtrlIfDis`] is set (DPSW-I1).
    #[must_use]
    fn ctrl_if_enabled(&self) -> bool {
        !self.options.contains(&DpswOpt::CtrlIfDis)
    }

    /// Whether the kernel driver (`fsl_dpaa2_switch`) binds this shape (`dpsw.qnt`
    /// `kernelBindable`; DPSW-I1): the control interface enabled, flooding AND broadcast
    /// `PER_FDB`, and `max_fdbs` covering the per-port FDB count (`max_fdbs >= num_ifs`).
    #[must_use]
    fn kernel_bindable(&self) -> bool {
        self.ctrl_if_enabled()
            && self.flooding_cfg == FloodingCfg::PerFdb
            && self.broadcast_cfg == BroadcastCfg::PerFdb
            && self.max_fdbs.get() >= self.num_ifs.get()
    }

    /// Whether this is the ls-addsw default shape (`dpsw.qnt` `isVendorRecipeShape`;
    /// DPSW-I2): `PER_VLAN` flooding + `PER_OBJECT` broadcast. The vendor recipe is not an
    /// oracle, so it is a refused shape, never a created switch.
    #[must_use]
    fn is_vendor_recipe_shape(&self) -> bool {
        self.flooding_cfg == FloodingCfg::PerVlan && self.broadcast_cfg == BroadcastCfg::PerObject
    }
}

/// The validated dpsw create block — a kernel-bindable [`DpswCreate`] frozen immutable
/// (`dpsw.qnt` the `Switch` payload / `classify` accepted branch; `docs/baseline/dpsw.md`
/// "Attribute mutability").
///
/// The record is private and there is no setter, so a block cannot change after
/// construction (DPSW-I1 by construction: no mutating verb exists on this surface). The only
/// writer is [`new`](Self::new), which refuses a non-bindable block by name, so a `DpswCfg`
/// that exists has always passed the DPSW-I1/I2 classification.
///
/// ```compile_fail
/// use dpaa2_api::families::dpsw::{DpswCfg, DpswCreate};
/// let cfg: DpswCfg = make_bindable();
/// // The validated record is a private field with no setter: mutating a create-time value
/// // after construction does not type-check (DPSW-I1 by construction; docs/baseline/dpsw.md
/// // "Attribute mutability"; dpsw-typestate design D2).
/// cfg.create.num_ifs = cfg.create.num_ifs;
/// # fn make_bindable() -> DpswCfg { unreachable!() }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DpswCfg {
    create: DpswCreate,
}

impl DpswCfg {
    /// Builds a create block, refusing a non-bindable one by name (`dpsw.qnt` `classify` +
    /// `createAt`, accepted branch). The range envelope is already satisfied by the refined
    /// newtypes in [`DpswCreate`]; the DPSW-I2 ls-addsw vendor shape is checked first (so a
    /// vendor recipe, which is also unbindable, is named by its own refusal), then the
    /// general DPSW-I1 unbindable refusal, mirroring the model's `classify` order exactly.
    ///
    /// # Errors
    /// [`Refusal::VendorRecipeShape`] when the record is the ls-addsw default shape
    /// (`PER_VLAN` flooding + `PER_OBJECT` broadcast); [`Refusal::UnbindableCfg`] when any
    /// other DPSW-I1 clause fails (control interface off, a domain not `PER_FDB`, or
    /// `max_fdbs < num_ifs`).
    pub fn new(create: DpswCreate) -> Result<Self, Refusal> {
        if create.is_vendor_recipe_shape() {
            return Err(Refusal::VendorRecipeShape);
        }
        if !create.kernel_bindable() {
            return Err(Refusal::UnbindableCfg);
        }
        Ok(Self { create })
    }

    /// The immutable port count (`dpsw.qnt` `numIfs`; DPSW-I10).
    #[must_use]
    pub const fn num_ifs(&self) -> u16 {
        self.create.num_ifs.get()
    }

    /// The MC option flags (by shared reference — there is no `&mut` path).
    #[must_use]
    pub const fn options(&self) -> &BTreeSet<DpswOpt> {
        &self.create.options
    }

    /// `--max-vlans` (`dpsw.qnt` `maxVlans`).
    #[must_use]
    pub const fn max_vlans(&self) -> u16 {
        self.create.max_vlans.get()
    }

    /// `--max-fdbs` (`dpsw.qnt` `maxFdbs`).
    #[must_use]
    pub const fn max_fdbs(&self) -> u16 {
        self.create.max_fdbs.get()
    }

    /// `--max-fdb-entries` (`dpsw.qnt` `maxFdbEntries`).
    #[must_use]
    pub const fn max_fdb_entries(&self) -> u16 {
        self.create.max_fdb_entries.get()
    }

    /// `--fdb-aging-time` (`dpsw.qnt` `fdbAgingTime`).
    #[must_use]
    pub const fn fdb_aging_time(&self) -> u16 {
        self.create.fdb_aging_time.get()
    }

    /// `--max-fdb-mc-groups` (`dpsw.qnt` `maxFdbMcGroups`).
    #[must_use]
    pub const fn max_fdb_mc_groups(&self) -> u16 {
        self.create.max_fdb_mc_groups.get()
    }

    /// `--component-type` (`dpsw.qnt` `componentType`).
    #[must_use]
    pub const fn component_type(&self) -> ComponentType {
        self.create.component_type
    }

    /// `--flooding-cfg` (`dpsw.qnt` `floodingCfg`).
    #[must_use]
    pub const fn flooding_cfg(&self) -> FloodingCfg {
        self.create.flooding_cfg
    }

    /// `--broadcast-cfg` (`dpsw.qnt` `broadcastCfg`).
    #[must_use]
    pub const fn broadcast_cfg(&self) -> BroadcastCfg {
        self.create.broadcast_cfg
    }
}

// ---- the P1 phase machine types (dpsw-typestate design D2; `dpsw.qnt` `SwitchPhase` /
//      `SwitchData`) ----

/// The switch phase (`dpsw.qnt` `type SwitchPhase`): created (MC-default state intact, not
/// yet bound), deferred (the probe ran but the container pool lacked the dpbp — still
/// unbound, no draw, no reset: the silent `-EPROBE_DEFER` face, DPSW-I9), or bound (kernel
/// bound, reset facet applied, DPSW-I4). There is no failure phase for starvation: a starved
/// probe DEFERS SILENTLY (DPSW-I9). The world/action machine that walks these phases is the
/// MBT harness's job (dpsw-typestate task 6.1); this tile ports only the types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SwitchPhase {
    /// Created: MC-default state intact, not yet bound.
    Created,
    /// Deferred: the probe starved on the pool, so the switch is unbound with no draw and no
    /// reset — the silent defer face, never a failure (DPSW-I9).
    Deferred,
    /// Bound: the kernel bound the switch and the reset facet was applied (DPSW-I4).
    Bound,
}

/// The [`SwitchPhase`] variant names, in declaration order — the Rust copy of the
/// `dpsw.qnt` `type SwitchPhase` cases (ADR-0014, kept honest by [`SwitchPhase::name`]).
pub const SWITCH_PHASE_VARIANTS: [&str; 3] = ["Created", "Deferred", "Bound"];

impl SwitchPhase {
    /// This variant's name, the token [`SWITCH_PHASE_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Deferred => "Deferred",
            Self::Bound => "Bound",
        }
    }
}

/// A created switch (`dpsw.qnt` `type SwitchData`): its immutable [`DpswCfg`], its phase, and
/// whether MC-default state (VLAN 1, FDB 0) still stands — dismantled by the bind-time reset
/// (DPSW-I4). A [`Deferred`](SwitchPhase::Deferred) switch is unbound, took no draw, and left
/// its MC-default state untouched — no reset ran (DPSW-I9, defer-not-fail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchData {
    /// The immutable create block (`dpsw.qnt` `cfg`).
    pub cfg: DpswCfg,
    /// The switch phase (`dpsw.qnt` `phase`).
    pub phase: SwitchPhase,
    /// Whether MC-default state still stands (`dpsw.qnt` `mcDefaults`; DPSW-I4).
    pub mc_defaults: bool,
}

#[cfg(test)]
mod tests {
    //! Parity of the create-surface sums with `models/families/dpsw.qnt` `dpsw_lifecycle`,
    //! the DPSW-I1/I2 classify-order refusals, the range envelope, and the DPSW-I4 reset
    //! facet. The compile-time immutability of the create block is witnessed by the
    //! `compile_fail` doctest on [`DpswCfg`].

    use super::*;

    // ---- parity: each model sum's Rust copy is complete (ADR-0014) ----

    #[test]
    fn dpsw_opt_variants_match_the_enum_and_vocabulary() {
        for opt in DpswOpt::MC_VOCABULARY {
            assert!(DPSW_OPT_VARIANTS.contains(&opt.name()), "{}", opt.name());
        }
        assert_eq!(DpswOpt::MC_VOCABULARY.len(), DPSW_OPT_VARIANTS.len());
        let mut seen = DPSW_OPT_VARIANTS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), DPSW_OPT_VARIANTS.len(), "duplicate flag name");
    }

    #[test]
    fn component_type_variants_match_the_enum() {
        for c in [ComponentType::CVlan, ComponentType::SVlan] {
            assert!(COMPONENT_TYPE_VARIANTS.contains(&c.name()), "{}", c.name());
        }
        assert_eq!(COMPONENT_TYPE_VARIANTS.len(), 2);
    }

    #[test]
    fn flooding_cfg_variants_match_the_enum() {
        for fcfg in [FloodingCfg::PerVlan, FloodingCfg::PerFdb] {
            assert!(
                FLOODING_CFG_VARIANTS.contains(&fcfg.name()),
                "{}",
                fcfg.name()
            );
        }
        assert_eq!(FLOODING_CFG_VARIANTS.len(), 2);
    }

    #[test]
    fn broadcast_cfg_variants_match_the_enum() {
        for b in [BroadcastCfg::PerObject, BroadcastCfg::PerFdb] {
            assert!(BROADCAST_CFG_VARIANTS.contains(&b.name()), "{}", b.name());
        }
        assert_eq!(BROADCAST_CFG_VARIANTS.len(), 2);
    }

    #[test]
    fn reset_scope_variants_match_the_enum() {
        for r in [ResetScope::Total, ResetScope::Partial] {
            assert!(RESET_SCOPE_VARIANTS.contains(&r.name()), "{}", r.name());
        }
        assert_eq!(RESET_SCOPE_VARIANTS.len(), 2);
    }

    #[test]
    fn refusal_variants_match_the_enum() {
        for r in [Refusal::VendorRecipeShape, Refusal::UnbindableCfg] {
            assert!(REFUSAL_VARIANTS.contains(&r.name()), "{}", r.name());
        }
        assert_eq!(REFUSAL_VARIANTS.len(), 2);
    }

    #[test]
    fn switch_phase_variants_match_the_enum() {
        for p in [
            SwitchPhase::Created,
            SwitchPhase::Deferred,
            SwitchPhase::Bound,
        ] {
            assert!(SWITCH_PHASE_VARIANTS.contains(&p.name()), "{}", p.name());
        }
        assert_eq!(SWITCH_PHASE_VARIANTS.len(), 3);
    }

    // ---- a kernel-bindable builder, varied per refusal (`dpsw.qnt` sample cfgs) ----

    /// The one kernel-bindable shape (`dpsw.qnt` `BINDABLE_CFG`), with the varying
    /// bindability fields as parameters so each DPSW-I1/I2 facet can be isolated.
    fn create(
        num_ifs: u16,
        options: BTreeSet<DpswOpt>,
        max_fdbs: u16,
        flooding_cfg: FloodingCfg,
        broadcast_cfg: BroadcastCfg,
    ) -> DpswCreate {
        DpswCreate {
            num_ifs: NumIfs::new(num_ifs).unwrap(),
            options,
            max_vlans: MaxVlans::new(16).unwrap(),
            max_fdbs: MaxFdbs::new(max_fdbs).unwrap(),
            max_fdb_entries: MaxFdbEntries::new(1024).unwrap(),
            fdb_aging_time: FdbAgingTime::new(300).unwrap(),
            max_fdb_mc_groups: MaxFdbMcGroups::new(32).unwrap(),
            mem_size: 0,
            component_type: ComponentType::CVlan,
            flooding_cfg,
            broadcast_cfg,
            max_meters_per_if: 0,
        }
    }

    fn bindable() -> DpswCreate {
        create(
            2,
            BTreeSet::new(),
            2,
            FloodingCfg::PerFdb,
            BroadcastCfg::PerFdb,
        )
    }

    // ---- the create verdict (`dpsw.qnt` `classify` / `createBindableAcceptedTest`) ----

    #[test]
    fn bindable_block_is_accepted_and_carries_its_fields() {
        // dpsw.qnt `createBindableAcceptedTest`: the 2-port bindable shape is accepted and
        // reads back verbatim.
        let cfg = DpswCfg::new(bindable()).expect("the bindable shape passes DPSW-I1");
        assert_eq!(cfg.num_ifs(), 2);
        assert_eq!(cfg.max_fdbs(), 2);
        assert_eq!(cfg.max_vlans(), 16);
        assert_eq!(cfg.max_fdb_entries(), 1024);
        assert_eq!(cfg.fdb_aging_time(), 300);
        assert_eq!(cfg.max_fdb_mc_groups(), 32);
        assert_eq!(cfg.component_type(), ComponentType::CVlan);
        assert_eq!(cfg.flooding_cfg(), FloodingCfg::PerFdb);
        assert_eq!(cfg.broadcast_cfg(), BroadcastCfg::PerFdb);
        assert!(cfg.options().is_empty());
    }

    #[test]
    fn ctrl_if_disabled_is_refused_unbindable() {
        // dpsw.qnt `ctrlIfDisabledRefusedTest` / `NO_CTRLIF_CFG`: CTRL_IF_DIS set ⇒ DPSW-I1.
        let mut opts = BTreeSet::new();
        opts.insert(DpswOpt::CtrlIfDis);
        let err = DpswCfg::new(create(
            2,
            opts,
            2,
            FloodingCfg::PerFdb,
            BroadcastCfg::PerFdb,
        ))
        .unwrap_err();
        assert_eq!(err, Refusal::UnbindableCfg);
    }

    #[test]
    fn flooding_per_vlan_is_refused_unbindable() {
        // dpsw.qnt `kernelBindable`: flooding not PER_FDB ⇒ DPSW-I1 (broadcast PER_FDB keeps
        // it off the vendor-recipe shape).
        let err = DpswCfg::new(create(
            2,
            BTreeSet::new(),
            2,
            FloodingCfg::PerVlan,
            BroadcastCfg::PerFdb,
        ))
        .unwrap_err();
        assert_eq!(err, Refusal::UnbindableCfg);
    }

    #[test]
    fn broadcast_per_object_is_refused_unbindable() {
        // dpsw.qnt `kernelBindable`: broadcast not PER_FDB ⇒ DPSW-I1 (flooding PER_FDB keeps
        // it off the vendor-recipe shape).
        let err = DpswCfg::new(create(
            2,
            BTreeSet::new(),
            2,
            FloodingCfg::PerFdb,
            BroadcastCfg::PerObject,
        ))
        .unwrap_err();
        assert_eq!(err, Refusal::UnbindableCfg);
    }

    #[test]
    fn few_fdbs_is_refused_unbindable() {
        // dpsw.qnt `fewFdbsRefusedTest` / `FEW_FDBS_CFG`: max_fdbs below num_ifs ⇒ DPSW-I1.
        let err = DpswCfg::new(create(
            4,
            BTreeSet::new(),
            2,
            FloodingCfg::PerFdb,
            BroadcastCfg::PerFdb,
        ))
        .unwrap_err();
        assert_eq!(err, Refusal::UnbindableCfg);
    }

    #[test]
    fn vendor_recipe_shape_is_refused_by_its_own_name() {
        // dpsw.qnt `vendorRecipeRefusedTest` / `VENDOR_RECIPE_CFG`: PER_VLAN + PER_OBJECT is
        // DPSW-I2 — and because classify checks the vendor shape before the general
        // unbindable clause, the also-unbindable recipe is named VendorRecipeShape, not
        // UnbindableCfg.
        let err = DpswCfg::new(create(
            2,
            BTreeSet::new(),
            2,
            FloodingCfg::PerVlan,
            BroadcastCfg::PerObject,
        ))
        .unwrap_err();
        assert_eq!(err, Refusal::VendorRecipeShape);
    }

    #[test]
    fn refusal_display_and_error_name_the_law() {
        assert!(Refusal::VendorRecipeShape.to_string().contains("PER_VLAN"));
        assert!(Refusal::UnbindableCfg.to_string().contains("max_fdbs"));
        match Error::from(Refusal::UnbindableCfg) {
            Error::Config(msg) => assert!(msg.contains("kernel-unbindable"), "{msg}"),
            other => panic!("expected Error::Config, got {other:?}"),
        }
    }

    // ---- the range envelope (the refined newtypes; `dpsw.qnt` `CreateCfg` field ranges) ----

    #[test]
    fn num_ifs_zero_is_out_of_envelope() {
        // dpsw.qnt `numIfs` range 1..=65535: 0 has no MC-default meaning and is refused.
        let err = NumIfs::new(0).unwrap_err();
        match err {
            Error::Config(msg) => assert!(msg.contains("num_ifs") && msg.contains("1..=65535")),
            other => panic!("expected Error::Config, got {other:?}"),
        }
        assert!(NumIfs::new(1).is_ok());
        assert!(NumIfs::new(65535).is_ok());
    }

    #[test]
    fn max_fdbs_above_255_is_out_of_envelope() {
        // dpsw.qnt `maxFdbs` range 0..=255: 256 is refused, 0 and 255 construct.
        let err = MaxFdbs::new(256).unwrap_err();
        match err {
            Error::Config(msg) => assert!(msg.contains("max_fdbs") && msg.contains("0..=255")),
            other => panic!("expected Error::Config, got {other:?}"),
        }
        assert!(MaxFdbs::new(0).is_ok());
        assert!(MaxFdbs::new(255).is_ok());
    }

    // ---- DPSW-I1 ctrl-if facet and DPSW-I4 reset facet (`dpsw.qnt` pure twins) ----

    #[test]
    fn ctrl_if_enabled_tracks_the_flag() {
        // dpsw.qnt `ctrlIfEnabled`: enabled unless CTRL_IF_DIS is set.
        assert!(bindable().ctrl_if_enabled());
        let mut opts = BTreeSet::new();
        opts.insert(DpswOpt::CtrlIfDis);
        let off = create(2, opts, 2, FloodingCfg::PerFdb, BroadcastCfg::PerFdb);
        assert!(!off.ctrl_if_enabled());
    }

    #[test]
    fn reset_facet_is_total_by_default() {
        // dpsw.qnt `resetFacetTotalTest` / `mcDefaultsAfterReset`: under the CONSERVATIVE
        // total scope no pre-bind MC-default state survives the bind (DPSW-I4).
        assert_eq!(RESET_FACET_SCOPE, ResetScope::Total);
        assert!(!mc_defaults_after_reset(true));
        assert!(!mc_defaults_after_reset(false));
    }

    #[test]
    fn switch_data_carries_the_bound_phase_and_reset_defaults() {
        // dpsw.qnt `SwitchData`: a bound switch applies the reset facet to its MC-default
        // state; a deferred switch leaves it untouched (DPSW-I9 defer-not-fail).
        let cfg = DpswCfg::new(bindable()).unwrap();
        let bound = SwitchData {
            cfg: cfg.clone(),
            phase: SwitchPhase::Bound,
            mc_defaults: mc_defaults_after_reset(true),
        };
        assert_eq!(bound.phase, SwitchPhase::Bound);
        assert!(!bound.mc_defaults);

        let deferred = SwitchData {
            cfg,
            phase: SwitchPhase::Deferred,
            mc_defaults: true,
        };
        assert_eq!(deferred.phase, SwitchPhase::Deferred);
        assert!(deferred.mc_defaults);
    }
}
