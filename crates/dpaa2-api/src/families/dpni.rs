//! The dpni create surface as compile-time-refined types — the Rust twin of the
//! Quint create-option state machine (`models/families/dpni.qnt` module
//! `dpni_lifecycle`).
//!
//! Filed under `families/` as the per-family vocabulary tile whose `families/dpni.rs`
//! path mirrors that `models/families/dpni.qnt` quint twin one-to-one (ADR-0018).
//!
//! Authored model-first (quint-is-the-spec): the sums, payloads and refined ranges
//! here are structurally isomorphic to the Quint sums, and the ADR-0002 §3 law binds
//! them — same cases, same payloads, same range semantics; names converge on readable
//! English on both surfaces. This tile (dpni-typestate task 2.1) grows the *create surface* — the
//! refined ranges, the typed option set, the immutable create block, and the runtime
//! MAC slot; the programmatic dead-option refusals and the observation/drift surface
//! are the following tile.
//!
//! # The create/runtime split, by construct (dpni-typestate design D1)
//!
//! The baseline's mutability law is absolute — every `dpni_cfg` field is
//! create-time-immutable, there is no `dpni_set_options`, and a resize is destroy +
//! create (`docs/baseline/dpni.md` "Attribute mutability", ADR-0001 §4). The family
//! encodes it structurally: the validated create block [`DpniCfg`] parameterizes a
//! [`Dpni`] and lives inside it by shared reference only (no setter, no `&mut cfg`),
//! while the runtime surface [`RuntimeState`] is mutable state within the type. Today
//! that runtime surface is exactly one value — the primary MAC — because it is the
//! only setter the restool transport can drive; the remaining `dpni_set_*` surface is
//! a deferral to the mc-portal backend and slots into [`RuntimeState`] without
//! reshaping the family.
//!
//! # Invalid configurations are unrepresentable (dpni-typestate design D2)
//!
//! The twelve live create options (`docs/baseline/dpni.md` "Option inventory: used vs
//! available") are the ten numeric fields — each a refined range type with no
//! constructor for a value outside the restool-verified envelope — the typed option
//! mask [`OptionMask`], and the `--container` placement (`root_container`). The mask
//! is a typed set over the ten named MC 10.39 flags [`DpniOpt`] plus a
//! provenance-carrying raw-mask escape [`RawEscape`] — `0x80000000` (`PFDR_IN_PEB`) is
//! deployed and working but unnamed in any header, so the escape is a first-class
//! constructor carrying *why* the raw value is trusted, not a backdoor accepting
//! arbitrary bits.

use std::collections::BTreeSet;

use crate::core::error::Error;
use crate::core::model::MacAddr;

// ---- the ten refined numeric create options (dpni-typestate design D2) ----

/// Mints one refined create-option range type — a `u16` newtype whose only
/// constructor refuses anything outside the restool-verified envelope.
///
/// `0` is the omitted/MC-default sentinel (`docs/baseline/dpni.md` "Option inventory":
/// an omitted flag sends literal 0, so the MC applies its own default) and is valid
/// everywhere, exactly the model's `inRange(v, hi) = v == 0 or (1 <= v <= hi)`. One
/// distinct type per field (the crate's `resource_name!` newtype-per-slot idiom,
/// `core::types`) so the wrong option can never be passed for another.
macro_rules! ranged_option {
    ($(#[$meta:meta])* $name:ident, $hi:literal, $field:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u16);

        impl $name {
            /// The inclusive restool upper bound for this option
            /// (`docs/baseline/dpni.md` "Option inventory" range).
            pub const HI: u16 = $hi;

            /// The omitted option — `0` on the wire, so the MC applies its own default
            /// (`docs/baseline/dpni.md` "Option inventory": an omitted flag sends
            /// literal 0). The value a field carries when intent leaves it unset; valid
            /// everywhere (model `inRange`).
            pub const DEFAULT: Self = Self(0);

            /// Builds the refined value, refusing anything outside the restool
            /// envelope. `0` (the omitted/MC-default sentinel) and `1..=HI` construct;
            /// every other value has no path — the refusal is a type-boundary error,
            /// not a board rejection (spec "Out-of-range option has no constructor";
            /// dpni-typestate design D2).
            ///
            /// # Errors
            /// [`Error::Config`] naming the field and its `1..=HI` range when `v` is
            /// out of envelope.
            pub fn new(v: u16) -> Result<Self, Error> {
                if v == 0 || (1..=Self::HI).contains(&v) {
                    Ok(Self(v))
                } else {
                    Err(Error::Config(format!(
                        concat!($field, " {} outside restool range 1..={} (0 = MC default)"),
                        v,
                        Self::HI,
                    )))
                }
            }

            /// The raw value — `0` for the MC default, else the in-range count.
            #[must_use]
            pub const fn get(self) -> u16 {
                self.0
            }
        }
    };
}

ranged_option! {
    /// `--num-queues` (model `numQueues`, range 1–32; MC default 1).
    NumQueues, 32, "num_queues"
}
ranged_option! {
    /// `--num-tcs` (model `numTcs`, range 1–16; MC default 1).
    NumTcs, 16, "num_tcs"
}
ranged_option! {
    /// `--mac-entries`/`--mac-filter-entries` (model `macFilterEntries`, range 1–80;
    /// MC default 16 — 80 is restool's maximum, not the default).
    MacFilterEntries, 80, "mac_filter_entries"
}
ranged_option! {
    /// `--vlan-entries`/`--vlan-filter-entries` (model `vlanFilterEntries`, range
    /// 1–16; MC default 0 = VLAN filtering disabled).
    VlanFilterEntries, 16, "vlan_filter_entries"
}
ranged_option! {
    /// `--qos-entries` (model `qosEntries`, range 1–64; MC default 0 with one TC —
    /// the `QoS` table exists only for a multi-TC dpni; 64 is restool's maximum).
    QosEntries, 64, "qos_entries"
}
ranged_option! {
    /// `--fs-entries` (model `fsEntries`, range 1–1024; MC default 64).
    FsEntries, 1024, "fs_entries"
}
ranged_option! {
    /// `--num-cgs` (model `numCgs`, range 1–128; MC default one CG per TC). The
    /// deployed heuristic is `num_queues + 8` under `CUSTOM_CG` (unknown-register #3).
    NumCgs, 128, "num_cgs"
}
ranged_option! {
    /// `--dist-key-size` (model `distKeySize`, range 1–56; MC default treated as 24).
    /// Write-only at observation: `dpni_attr` omits it, so the reconciler never reads
    /// it back and never claims drift on it (dpni-typestate design D4; DPNI-I12). It
    /// rides create here; the observation projection that excludes it lands with the
    /// observation/drift tile, not this one.
    DistKeySize, 56, "dist_key_size"
}
ranged_option! {
    /// `--num-channels` (model `numCeetmCh`, range 1–32; MC default single CEETM
    /// channel).
    NumCeetmCh, 32, "num_ceetm_ch"
}
ranged_option! {
    /// `--num-opr` (model `numOpr`, range 1–128; MC default `num_tcs × num_queues`).
    NumOpr, 128, "num_opr"
}

// ---- the option mask: named MC vocabulary plus the provenance-carrying escape ----

/// The MC 10.39 option-flag vocabulary (`dpni.qnt` `type DpniOpt`;
/// `docs/baseline/dpni.md` "Option inventory"). The flib header carries 14 flags
/// ending `STASHING_DIS` 0x2000; the baseline names ten of them — the four unnamed
/// flags are a recorded gap, not invented here, so they have no constructor.
/// `HAS_REPLICATION` (0x4000) is deliberately absent: it lives in restool's map but
/// not the MC 14-flag header (unknown-register #8), so it is not vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DpniOpt {
    /// `DPNI_OPT_SINGLE_SENDER`.
    SingleSender,
    /// `DPNI_OPT_CUSTOM_CG`.
    CustomCg,
    /// `DPNI_OPT_HAS_KEY_MASKING`.
    HasKeyMasking,
    /// `DPNI_OPT_HAS_OPR`.
    HasOpr,
    /// `DPNI_OPT_OPR_PER_TC`.
    OprPerTc,
    /// `DPNI_OPT_TX_FRM_RELEASE`.
    TxFrmRelease,
    /// `DPNI_OPT_HAS_POLICING`.
    HasPolicing,
    /// `DPNI_OPT_SHARED_CONGESTION`.
    SharedCongestion,
    /// `DPNI_OPT_NO_MAC_FILTER`.
    NoMacFilter,
    /// `DPNI_OPT_STASHING_DIS` (0x2000).
    StashingDis,
}

/// The [`DpniOpt`] variant names, in declaration order — the Rust copy of the
/// `dpni.qnt` `type DpniOpt` cases (ADR-0014: an enumeration that restates the model
/// is a linted copy, kept honest by the exhaustive `match` in [`DpniOpt::name`]).
pub const DPNI_OPT_VARIANTS: [&str; 10] = [
    "SingleSender",
    "CustomCg",
    "HasKeyMasking",
    "HasOpr",
    "OprPerTc",
    "TxFrmRelease",
    "HasPolicing",
    "SharedCongestion",
    "NoMacFilter",
    "StashingDis",
];

impl DpniOpt {
    /// The whole MC 10.39 vocabulary — the Rust copy of the `dpni.qnt` `MC_VOCABULARY`
    /// set (the ten named flags, no more). A flag outside this array has no variant,
    /// so it is unrepresentable by construct.
    pub const MC_VOCABULARY: [Self; 10] = [
        Self::SingleSender,
        Self::CustomCg,
        Self::HasKeyMasking,
        Self::HasOpr,
        Self::OprPerTc,
        Self::TxFrmRelease,
        Self::HasPolicing,
        Self::SharedCongestion,
        Self::NoMacFilter,
        Self::StashingDis,
    ];

    /// This variant's name, the token [`DPNI_OPT_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SingleSender => "SingleSender",
            Self::CustomCg => "CustomCg",
            Self::HasKeyMasking => "HasKeyMasking",
            Self::HasOpr => "HasOpr",
            Self::OprPerTc => "OprPerTc",
            Self::TxFrmRelease => "TxFrmRelease",
            Self::HasPolicing => "HasPolicing",
            Self::SharedCongestion => "SharedCongestion",
            Self::NoMacFilter => "NoMacFilter",
            Self::StashingDis => "StashingDis",
        }
    }
}

/// The raw-mask escape — a first-class, provenance-carrying constructor, **not** a
/// backdoor (dpni-typestate design D2; `dpni.qnt` `type RawEscape`).
///
/// restool's `--options` parser falls back to `strtoull` on an unrecognized token
/// (`docs/baseline/dpni.md` "--options parsing"), so dprc-script passes `0x80000000`
/// (`PFDR_IN_PEB`) — deployed and working, never named in any header in the corpus
/// (unknown-register #10). Rather than accept an arbitrary `u64` silently, the escape
/// is an enum: each variant *is* the provenance (why and where the raw value is
/// trusted), so the typed set stays the single source and no unattributed bit can ride
/// a mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RawEscape {
    /// `0x80000000` `PFDR_IN_PEB` — deployed on the PMD profile [verified in use],
    /// unnamed in any MC 10.39 header (unknown-register #10).
    PfdrInPeb,
}

/// The [`RawEscape`] variant names, in declaration order — the Rust copy of the
/// `dpni.qnt` `type RawEscape` cases (ADR-0014, tied by [`RawEscape::name`]).
pub const RAW_ESCAPE_VARIANTS: [&str; 1] = ["PfdrInPeb"];

impl RawEscape {
    /// This variant's name, the token [`RAW_ESCAPE_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PfdrInPeb => "PfdrInPeb",
        }
    }

    /// The raw mask bit this escape carries (`dpni.qnt` `rawValue`): `0x80000000` for
    /// `PFDR_IN_PEB`.
    #[must_use]
    pub const fn raw_value(self) -> u32 {
        match self {
            Self::PfdrInPeb => 0x8000_0000,
        }
    }
}

/// The options mask — a typed set over the MC vocabulary plus the raw escapes
/// (`dpni.qnt` `type OptionMask`). The shim emits the raw mask it computes itself,
/// never operator tokens (design Risks), so the two halves are the single source.
///
/// Built additively from [`OptionMask::empty`]; there is no path that admits an
/// unnamed flag or an arbitrary raw bit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionMask {
    flags: BTreeSet<DpniOpt>,
    escapes: BTreeSet<RawEscape>,
}

impl OptionMask {
    /// The empty mask — `--options` omitted, so `0` on the wire (`ls-addni`'s default;
    /// `dpni.qnt` `Set()`/`Set()`).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Adds a named MC flag (builder form). Idempotent — the set holds each flag once.
    #[must_use]
    pub fn with_flag(mut self, flag: DpniOpt) -> Self {
        self.flags.insert(flag);
        self
    }

    /// Adds a provenance-carrying raw escape (builder form). The only path a raw mask
    /// bit reaches a create block (dpni-typestate design D2).
    #[must_use]
    pub fn with_escape(mut self, escape: RawEscape) -> Self {
        self.escapes.insert(escape);
        self
    }

    /// The named flags this mask carries.
    #[must_use]
    pub fn flags(&self) -> &BTreeSet<DpniOpt> {
        &self.flags
    }

    /// The raw escapes this mask carries.
    #[must_use]
    pub fn escapes(&self) -> &BTreeSet<RawEscape> {
        &self.escapes
    }

    /// Whether the named flag is set.
    #[must_use]
    pub fn contains(&self, flag: DpniOpt) -> bool {
        self.flags.contains(&flag)
    }

    /// Whether the raw escape is set.
    #[must_use]
    pub fn contains_escape(&self, escape: RawEscape) -> bool {
        self.escapes.contains(&escape)
    }
}

// ---- the immutable create block (dpni-typestate design D1) ----

/// The validated create block — the twelve live create options as one record
/// (`dpni.qnt` `type CreateCfg`; `docs/baseline/dpni.md` "Option inventory").
///
/// Every field is a refined type ([range types](NumQueues) / [`OptionMask`]) or the
/// `--container` placement, so an out-of-envelope create block cannot be built. Once a
/// block parameterizes a [`Dpni`] it is immutable — the model's `Created(CreateCfg)`
/// payload — because [`Dpni`] owns it privately and exposes it only by shared
/// reference (`docs/baseline/dpni.md` "Attribute mutability": every `dpni_cfg` field
/// is create-time-immutable, LAW 1; dpni-typestate design D1).
///
/// The derived `Eq` is *create-block identity*, not observation equality: `dist_key_size`
/// is write-only (dpni-typestate design D4) and is excluded from drift comparison by a
/// separate observation projection (the observation/drift tile, the sibling of dprc's
/// `Resident` vs `ObservedResident` split), never by fighting this derive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpniCfg {
    /// The options mask (`--options`).
    pub options: OptionMask,
    /// `--num-queues`.
    pub num_queues: NumQueues,
    /// `--num-tcs`.
    pub num_tcs: NumTcs,
    /// `--mac-entries`/`--mac-filter-entries`.
    pub mac_filter_entries: MacFilterEntries,
    /// `--vlan-entries`/`--vlan-filter-entries`.
    pub vlan_filter_entries: VlanFilterEntries,
    /// `--qos-entries`.
    pub qos_entries: QosEntries,
    /// `--fs-entries`.
    pub fs_entries: FsEntries,
    /// `--num-cgs`.
    pub num_cgs: NumCgs,
    /// `--dist-key-size` (write-only at observation; dpni-typestate design D4).
    pub dist_key_size: DistKeySize,
    /// `--num-channels`.
    pub num_ceetm_ch: NumCeetmCh,
    /// `--num-opr`.
    pub num_opr: NumOpr,
    /// `--container`: the root dprc (`true`) or a child (`false`) — model
    /// `rootContainer`.
    pub root_container: bool,
}

impl DpniCfg {
    /// A create block with every option at its MC default (all range fields `0`, an
    /// empty mask, a child container) — the bare `dpni create` the baseline's
    /// create-default row (DPNI-I7) reads back as 1 queue / 1 TC / defaults. The base
    /// a caller overrides field by field, always in-envelope by construction.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            options: OptionMask::empty(),
            num_queues: NumQueues::DEFAULT,
            num_tcs: NumTcs::DEFAULT,
            mac_filter_entries: MacFilterEntries::DEFAULT,
            vlan_filter_entries: VlanFilterEntries::DEFAULT,
            qos_entries: QosEntries::DEFAULT,
            fs_entries: FsEntries::DEFAULT,
            num_cgs: NumCgs::DEFAULT,
            dist_key_size: DistKeySize::DEFAULT,
            num_ceetm_ch: NumCeetmCh::DEFAULT,
            num_opr: NumOpr::DEFAULT,
            root_container: false,
        }
    }
}

// ---- the runtime surface: state within the immutable type (dpni-typestate design D1) ----

/// The runtime surface — mutable state carried within a [`Dpni`] beside its immutable
/// [`DpniCfg`] (dpni-typestate design D1).
///
/// Today it is exactly the primary MAC, the only setter the restool transport can
/// drive (`docs/baseline/dpni.md` "Command surface": `update --mac-addr` is the sole
/// post-create mutation). The remaining 35 `dpni_set_*` setters are a named deferral
/// to the mc-portal backend and add fields *here*, without reshaping the family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeState {
    primary_mac: MacAddr,
}

/// A created DPNI — the model's `Created(CreateCfg)` state (`dpni.qnt` `type
/// DpniState`), the immutable create block [`DpniCfg`] carried beside its mutable
/// [`RuntimeState`] (dpni-typestate design D1).
///
/// The create block is immutable once here: [`cfg`](Self::cfg) hands it out only by
/// shared reference and there is no cfg setter, so no `dpni_cfg` field is mutable after
/// creation. The one runtime mutation is [`set_primary_mac`](Self::set_primary_mac).
///
/// ```compile_fail
/// use dpaa2_api::families::dpni::{Dpni, DpniCfg};
/// let dpni = Dpni::create(DpniCfg::defaults());
/// // No cfg setter, and `cfg()` yields `&DpniCfg`: mutating the create block after
/// // creation does not type-check (dpni-typestate design D1).
/// dpni.cfg().root_container = true;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dpni {
    cfg: DpniCfg,
    runtime: RuntimeState,
}

impl Dpni {
    /// Creates a DPNI from a validated create block (`dpni.qnt` `createRawAt`, accepted
    /// branch). The block is in-envelope by construction, so this is infallible; the
    /// runtime primary MAC starts at [`MacAddr::ZERO`] until the DPMAC inherits or a
    /// setter writes it (`docs/baseline/dpni.md` "MAC precedence", ADR-0001 C2).
    #[must_use]
    pub fn create(cfg: DpniCfg) -> Self {
        Self {
            cfg,
            runtime: RuntimeState {
                primary_mac: MacAddr::ZERO,
            },
        }
    }

    /// The immutable create block (by shared reference — there is no `&mut` path).
    #[must_use]
    pub fn cfg(&self) -> &DpniCfg {
        &self.cfg
    }

    /// The runtime primary MAC.
    #[must_use]
    pub fn primary_mac(&self) -> MacAddr {
        self.runtime.primary_mac
    }

    /// `dpni update --mac-addr` — the one runtime mutation restool can drive
    /// (`docs/baseline/dpni.md` "Attribute mutability"). Touches only the runtime
    /// surface; the create block is untouched.
    pub fn set_primary_mac(&mut self, mac: MacAddr) {
        self.runtime.primary_mac = mac;
    }
}

#[cfg(test)]
mod tests {
    //! Parity of the create-surface sums with `models/families/dpni.qnt`
    //! `dpni_lifecycle`, the range accept/refuse boundaries, the raw-escape provenance,
    //! and the create/runtime immutability split. The compile-time immutability of the
    //! create block is witnessed by the `compile_fail` doctest on [`Dpni`].

    use super::*;

    // ---- parity: each model sum's Rust copy is complete ----

    #[test]
    fn dpni_opt_variants_match_the_enum_and_vocabulary() {
        // dpni.qnt `type DpniOpt` / `MC_VOCABULARY`: exactly the ten named flags.
        for opt in DpniOpt::MC_VOCABULARY {
            assert!(DPNI_OPT_VARIANTS.contains(&opt.name()), "{}", opt.name());
        }
        assert_eq!(DpniOpt::MC_VOCABULARY.len(), DPNI_OPT_VARIANTS.len());
        let mut seen = DPNI_OPT_VARIANTS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), DPNI_OPT_VARIANTS.len(), "duplicate flag name");
    }

    #[test]
    fn raw_escape_variants_and_value() {
        // dpni.qnt `type RawEscape` / `rawValue`: one escape, value 0x80000000.
        assert!(RAW_ESCAPE_VARIANTS.contains(&RawEscape::PfdrInPeb.name()));
        assert_eq!(RAW_ESCAPE_VARIANTS, ["PfdrInPeb"]);
        assert_eq!(RawEscape::PfdrInPeb.raw_value(), 0x8000_0000);
        assert_eq!(RawEscape::PfdrInPeb.raw_value(), 2_147_483_648);
    }

    // ---- refined ranges: accept and refuse (dpni.qnt `inEnvelope`) ----

    #[test]
    fn every_ranged_option_gates_its_envelope() {
        // The model's `inRange`: 0 (omitted) and 1..=HI accept, HI+1 has no constructor.
        // One accept/refuse triplet per field, over all ten baseline ranges.
        macro_rules! check {
            ($ty:ident) => {{
                assert!($ty::new(0).is_ok(), "0 (MC default) must construct");
                assert!($ty::new(1).is_ok(), "1 must construct");
                assert!($ty::new($ty::HI).is_ok(), "HI must construct");
                assert_eq!($ty::new(0).unwrap().get(), 0);
                assert_eq!($ty::new($ty::HI).unwrap().get(), $ty::HI);
                assert!($ty::new($ty::HI + 1).is_err(), "HI+1 must be refused");
            }};
        }
        check!(NumQueues);
        check!(NumTcs);
        check!(MacFilterEntries);
        check!(VlanFilterEntries);
        check!(QosEntries);
        check!(FsEntries);
        check!(NumCgs);
        check!(DistKeySize);
        check!(NumCeetmCh);
        check!(NumOpr);
    }

    #[test]
    fn num_queues_boundary_mirrors_the_model_run() {
        // dpni.qnt `createOutOfEnvelopeRefusedTest`: num_queues 33 (> 32) is refused,
        // and the error names the field and its range.
        assert_eq!(NumQueues::HI, 32);
        assert!(NumQueues::new(32).is_ok());
        let refused = NumQueues::new(33).expect_err("33 is out of envelope");
        let msg = refused.to_string();
        assert!(msg.contains("num_queues"), "{msg}");
        assert!(msg.contains("1..=32"), "{msg}");
    }

    // ---- option mask construction and raw-escape provenance ----

    #[test]
    fn option_mask_builds_additively() {
        let empty = OptionMask::empty();
        assert!(empty.flags().is_empty());
        assert!(empty.escapes().is_empty());

        let mask = OptionMask::empty()
            .with_flag(DpniOpt::HasKeyMasking)
            .with_flag(DpniOpt::HasKeyMasking) // idempotent
            .with_flag(DpniOpt::SingleSender);
        assert!(mask.contains(DpniOpt::HasKeyMasking));
        assert!(mask.contains(DpniOpt::SingleSender));
        assert!(!mask.contains(DpniOpt::CustomCg));
        assert_eq!(mask.flags().len(), 2);
    }

    #[test]
    fn raw_escape_rides_the_mask_with_provenance() {
        // dpni.qnt `createInEnvelopeAcceptedTest`: the raw escape carries its
        // provenance — the only path a raw mask bit reaches a mask is the named enum.
        let mask = OptionMask::empty().with_escape(RawEscape::PfdrInPeb);
        assert!(mask.contains_escape(RawEscape::PfdrInPeb));
        assert_eq!(mask.escapes().len(), 1);
        assert_eq!(
            mask.escapes().iter().next().unwrap().raw_value(),
            0x8000_0000
        );
    }

    // ---- the create/runtime split (dpni-typestate design D1) ----

    #[test]
    fn dpni_carries_cfg_immutably_and_mutates_only_the_mac() {
        // A PMD-shaped in-envelope block (dpni.qnt `PMD_PROFILE`, 16q/16tc + the raw
        // escape) is accepted; create carries it verbatim, primary MAC starts zero.
        let cfg = DpniCfg {
            options: OptionMask::empty()
                .with_flag(DpniOpt::SingleSender)
                .with_flag(DpniOpt::CustomCg)
                .with_flag(DpniOpt::HasKeyMasking)
                .with_flag(DpniOpt::HasOpr)
                .with_flag(DpniOpt::OprPerTc)
                .with_escape(RawEscape::PfdrInPeb),
            num_queues: NumQueues::new(16).unwrap(),
            num_tcs: NumTcs::new(16).unwrap(),
            ..DpniCfg::defaults()
        };
        let mut dpni = Dpni::create(cfg.clone());
        assert_eq!(dpni.cfg(), &cfg);
        assert_eq!(dpni.primary_mac(), MacAddr::ZERO);

        // The one runtime mutation touches only the MAC; the create block is unchanged.
        let mac = MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);
        dpni.set_primary_mac(mac);
        assert_eq!(dpni.primary_mac(), mac);
        assert_eq!(dpni.cfg(), &cfg);
    }

    #[test]
    fn defaults_is_the_bare_create_block() {
        // The bare `dpni create`: every range field omitted (0 = MC default), empty
        // mask, child container (baseline DPNI-I7 create-default row).
        let cfg = DpniCfg::defaults();
        assert_eq!(cfg.num_queues.get(), 0);
        assert_eq!(cfg.num_tcs.get(), 0);
        assert!(cfg.options.flags().is_empty());
        assert!(cfg.options.escapes().is_empty());
        assert!(!cfg.root_container);
    }
}
