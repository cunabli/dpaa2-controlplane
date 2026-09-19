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
//! English on both surfaces. dpni-typestate task 2.1 grew the *create surface* — the
//! refined ranges, the typed option set, the immutable create block, and the runtime MAC
//! slot. dpni-typestate task 2.2 adds the rest: the programmatic dead-option parity
//! refusals ([`Unrepresentable`] /
//! [`DeadOptionRefusal`], dpni-typestate design D2), the observation projection that
//! excludes write-only `dist_key_size` ([`DpniObservation`], dpni-typestate design D4),
//! and the cfg-vs-MAC drift disposition ([`drift_disposition`], ADR-0001 §4).
//!
//! # The create/runtime split, by construct (dpni-typestate design D1)
//!
//! The baseline's mutability law is absolute — every `dpni_cfg` field is
//! create-time-immutable, there is no `dpni_set_options`, and a resize is destroy +
//! create (`docs/baseline/dpni.md` "Attribute mutability", ADR-0001 §4). The family
//! encodes it structurally: the validated create block [`DpniCfg`] parameterizes a
//! [`Dpni`] and lives inside it by shared reference only (no setter, no `&mut cfg`),
//! while the runtime surface [`RuntimeState`] is mutable state within the type. This
//! family names no transport (ADR-0018, sans-io hexagonal: the backend vocabulary lives
//! in `dpaa2-mc`, not here). Today that runtime surface is exactly one value — the
//! primary MAC — the one runtime mutation in today's create contract; the remaining
//! `dpni_set_*` surface is a named deferral to the mc-portal backend and slots into
//! [`RuntimeState`] without reshaping the family.
//!
//! # Invalid configurations are unrepresentable (dpni-typestate design D2)
//!
//! The twelve live create options (`docs/baseline/dpni.md` "Option inventory: used vs
//! available") are the ten numeric fields — each a refined range type with no
//! constructor for a value outside the board-verified create envelope — the typed option
//! mask [`OptionMask`], and the container placement (`root_container`). The mask
//! is a typed set over the ten named MC 10.39 flags [`DpniOpt`] plus a
//! provenance-carrying raw-mask escape [`RawEscape`] — `0x80000000` (`PFDR_IN_PEB`) is
//! deployed and working but unnamed in any header, so the escape is a first-class
//! constructor carrying *why* the raw value is trusted, not a backdoor accepting
//! arbitrary bits.

use std::collections::BTreeSet;

use crate::core::error::Error;
use crate::core::model::MacAddr;
use crate::intent::Dataplane;

// ---- the ten refined numeric create options (dpni-typestate design D2) ----

/// Mints one refined create-option range type — a `u16` newtype whose only
/// constructor refuses anything outside the board-verified create envelope.
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
            /// The inclusive upper bound of the board-verified create envelope for this
            /// option (`docs/baseline/dpni.md` "Option inventory" range).
            pub const HI: u16 = $hi;

            /// The omitted option — `0` on the wire, so the MC applies its own default
            /// (`docs/baseline/dpni.md` "Option inventory": an omitted flag sends
            /// literal 0). The value a field carries when intent leaves it unset; valid
            /// everywhere (model `inRange`).
            pub const DEFAULT: Self = Self(0);

            /// Builds the refined value, refusing anything outside the board-verified
            /// create envelope. `0` (the omitted/MC-default sentinel) and `1..=HI`
            /// construct; every other value has no path — the refusal is a type-boundary
            /// error, not a board rejection (spec "Out-of-range option has no
            /// constructor"; dpni-typestate design D2).
            ///
            /// # Errors
            /// [`Error::Config`] naming the field and its `1..=HI` range when `v` is
            /// out of envelope.
            pub fn new(v: u16) -> Result<Self, Error> {
                if v == 0 || (1..=Self::HI).contains(&v) {
                    Ok(Self(v))
                } else {
                    Err(Error::Config(format!(
                        concat!($field, " {} outside the verified create envelope 1..={} (0 = MC default)"),
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
    /// `num_queues` (model `numQueues`, range 1–32; MC default 1).
    NumQueues, 32, "num_queues"
}
ranged_option! {
    /// `num_tcs` (model `numTcs`, range 1–16; MC default 1).
    NumTcs, 16, "num_tcs"
}
ranged_option! {
    /// `mac_filter_entries` (model `macFilterEntries`, range 1–80; MC default 16 — 80 is
    /// the envelope maximum, not the default).
    MacFilterEntries, 80, "mac_filter_entries"
}
ranged_option! {
    /// `vlan_filter_entries` (model `vlanFilterEntries`, range 1–16; MC default 0 = VLAN
    /// filtering disabled).
    VlanFilterEntries, 16, "vlan_filter_entries"
}
ranged_option! {
    /// `qos_entries` (model `qosEntries`, range 1–64; MC default 0 with one TC — the
    /// `QoS` table exists only for a multi-TC dpni; 64 is the envelope maximum).
    QosEntries, 64, "qos_entries"
}
ranged_option! {
    /// `fs_entries` (model `fsEntries`, range 1–1024; MC default 64).
    FsEntries, 1024, "fs_entries"
}
ranged_option! {
    /// `num_cgs` (model `numCgs`, range 1–128; MC default one CG per TC). The deployed
    /// heuristic is `num_queues + 8` under `CUSTOM_CG` (unknown-register #3).
    NumCgs, 128, "num_cgs"
}
ranged_option! {
    /// `dist_key_size` (model `distKeySize`, range 1–56; MC default treated as 24).
    /// Write-only at observation: `dpni_attr` omits it, so the reconciler never reads
    /// it back and never claims drift on it (dpni-typestate design D4; DPNI-I12); the
    /// exclusion is realized in [`DpniObservation`].
    DistKeySize, 56, "dist_key_size"
}
ranged_option! {
    /// `num_ceetm_ch` (model `numCeetmCh`, range 1–32; MC default single CEETM channel).
    NumCeetmCh, 32, "num_ceetm_ch"
}
ranged_option! {
    /// `num_opr` (model `numOpr`, range 1–128; MC default `num_tcs × num_queues`).
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
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OptionMask {
    flags: BTreeSet<DpniOpt>,
    escapes: BTreeSet<RawEscape>,
}

impl OptionMask {
    /// The empty mask — no options set, so `0` on the wire (the create default;
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
/// container placement, so an out-of-envelope create block cannot be built. Once a
/// block parameterizes a [`Dpni`] it is immutable — the model's `Created(CreateCfg)`
/// payload — because [`Dpni`] owns it privately and exposes it only by shared
/// reference (`docs/baseline/dpni.md` "Attribute mutability": every `dpni_cfg` field
/// is create-time-immutable, LAW 1; dpni-typestate design D1).
///
/// The derived `Eq` is *create-block identity*, not observation equality: `dist_key_size`
/// is write-only (dpni-typestate design D4) and is excluded from drift comparison by a
/// separate observation projection ([`DpniObservation`], the sibling of dprc's
/// `Resident` vs `ObservedResident` split), never by fighting this derive.
///
/// `Ord`/`Hash` let a create block flow through the plan's [`Attributes`](crate::intent::compiled::Attributes)
/// (a `BTreeSet`-hosted, hashable sum); every field already carries them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DpniCfg {
    /// The options mask.
    pub options: OptionMask,
    /// `num_queues`.
    pub num_queues: NumQueues,
    /// `num_tcs`.
    pub num_tcs: NumTcs,
    /// `mac_filter_entries`.
    pub mac_filter_entries: MacFilterEntries,
    /// `vlan_filter_entries`.
    pub vlan_filter_entries: VlanFilterEntries,
    /// `qos_entries`.
    pub qos_entries: QosEntries,
    /// `fs_entries`.
    pub fs_entries: FsEntries,
    /// `num_cgs`.
    pub num_cgs: NumCgs,
    /// `dist_key_size` (write-only at observation; dpni-typestate design D4).
    pub dist_key_size: DistKeySize,
    /// `num_ceetm_ch`.
    pub num_ceetm_ch: NumCeetmCh,
    /// `num_opr`.
    pub num_opr: NumOpr,
    /// The container placement: the root dprc (`true`) or a child (`false`) — model
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

// ---- the interface construct and the two board-verified profiles (dpni-typestate design D3) ----

/// The interface construct an intent gives a dpni (`dpni.qnt` `type InterfaceConstruct`;
/// `docs/baseline/dpni.md` "Intent mapping"; ADR-0005 — the dpni is the object behind
/// every interface construct): a physical port (dpni↔dpmac), a host-injection pair
/// (dpni↔dpni across containers), a loopback (dpni↔self), or a fabric port (dpsw/dpdmux).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceConstruct {
    /// A physical port: dpni↔dpmac.
    PhysicalPort,
    /// A host-injection pair: dpni↔dpni across containers.
    InjectionPair,
    /// A loopback: dpni↔self.
    Loopback,
    /// A fabric port: a dpsw/dpdmux endpoint.
    FabricPort,
}

/// The [`InterfaceConstruct`] variant names, in declaration order — the Rust copy of the
/// `dpni.qnt` `type InterfaceConstruct` cases (ADR-0014: an enumeration that restates the
/// model is a linted copy, kept honest by the exhaustive `match` in
/// [`InterfaceConstruct::name`]).
pub const INTERFACE_CONSTRUCT_VARIANTS: [&str; 4] =
    ["PhysicalPort", "InjectionPair", "Loopback", "FabricPort"];

impl InterfaceConstruct {
    /// The whole construct set — the Rust copy of the `dpni.qnt` `CONSTRUCTS` set.
    pub const CONSTRUCTS: [Self; 4] = [
        Self::PhysicalPort,
        Self::InjectionPair,
        Self::Loopback,
        Self::FabricPort,
    ];

    /// This variant's name, the token [`INTERFACE_CONSTRUCT_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PhysicalPort => "PhysicalPort",
            Self::InjectionPair => "InjectionPair",
            Self::Loopback => "Loopback",
            Self::FabricPort => "FabricPort",
        }
    }
}

/// The two board-verified consumer option profiles (`dpni.qnt` `type OptionProfile`;
/// `docs/baseline/dpni.md` production-profiles list; ADR-0005): both the option mask and
/// the full create block are consumer-typed, never operator-supplied
/// (dpni-typestate design D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Profile {
    /// The PMD-facing dpni (userspace poll): `SINGLE_SENDER`, `CUSTOM_CG`,
    /// `HAS_KEY_MASKING`, `HAS_OPR`, `OPR_PER_TC` and the raw `0x80000000`, 16q/16tc.
    Pmd,
    /// The kernel-facing dpni (kernel-netlink injection pairs): `HAS_KEY_MASKING` only,
    /// 1q/1tc — `PFDR_IN_PEB` and `SINGLE_SENDER` absent so the kernel side tx's freely.
    Kernel,
}

impl Profile {
    /// The profile's option mask (`dpni.qnt` `PMD_PROFILE`/`KERNEL_PROFILE`,
    /// `profileMask`). Built through the additive [`OptionMask`] builder, so it carries
    /// only named flags and the provenance-carrying escape.
    #[must_use]
    pub fn mask(self) -> OptionMask {
        match self {
            Self::Pmd => OptionMask::empty()
                .with_flag(DpniOpt::SingleSender)
                .with_flag(DpniOpt::CustomCg)
                .with_flag(DpniOpt::HasKeyMasking)
                .with_flag(DpniOpt::HasOpr)
                .with_flag(DpniOpt::OprPerTc)
                .with_escape(RawEscape::PfdrInPeb),
            Self::Kernel => OptionMask::empty().with_flag(DpniOpt::HasKeyMasking),
        }
    }

    /// The full create block the profile derives (`dpni.qnt` `profileCfg`). Fields the
    /// production scripts leave unset ride their MC default (`0`): PMD is 16q/16tc, vlan
    /// 16, qos 64, fs 1, `num_cgs = num_queues + 8 = 24` (the deployed `CUSTOM_CG`
    /// heuristic, unknown-register #3), one CEETM channel, in a child container; kernel
    /// is 1q/1tc with everything else at its default. The literals are in-envelope by
    /// construction, so the fallible range constructors never fail here.
    ///
    /// # Panics
    /// Panics only if a profile literal is misdeclared outside the board-verified create
    /// envelope (`dpni.qnt` `profileCfg`), which the fixed constants forbid; the
    /// range-boundary tests catch a bad edit before this can fire.
    #[must_use]
    pub fn cfg(self) -> DpniCfg {
        let ok = "profile literal is inside the board-verified create envelope";
        match self {
            Self::Pmd => DpniCfg {
                options: self.mask(),
                num_queues: NumQueues::new(16).expect(ok),
                num_tcs: NumTcs::new(16).expect(ok),
                vlan_filter_entries: VlanFilterEntries::new(16).expect(ok),
                qos_entries: QosEntries::new(64).expect(ok),
                fs_entries: FsEntries::new(1).expect(ok),
                num_cgs: NumCgs::new(24).expect(ok),
                num_ceetm_ch: NumCeetmCh::new(1).expect(ok),
                ..DpniCfg::defaults()
            },
            Self::Kernel => DpniCfg {
                options: self.mask(),
                num_queues: NumQueues::new(1).expect(ok),
                num_tcs: NumTcs::new(1).expect(ok),
                ..DpniCfg::defaults()
            },
        }
    }
}

/// The outcome of profile derivation (`dpni.qnt` `type ProfileOutcome`): a priced
/// consumer derives one of the two profiles; `UserspaceEvent` is unpriced (ADR-0012) and
/// refused upstream ([`crate::intent::refuse`]), so it derives none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProfileOutcome {
    /// The consumer's board-verified profile.
    Derived(Profile),
    /// An unpriced consumer — no dpni profile.
    Unpriced,
}

/// Derives the option profile purely from the consumer and its interface construct
/// (`dpni.qnt` `deriveProfile`; dpni-typestate design D3): the operator never writes an
/// option. The board keys the profile on the [`Dataplane`] — a userspace-poll process
/// gets PMD, the kernel gets kernel — so the construct rides the signature for totality
/// but does not split the two current profiles (a third profile is a documented
/// amendment, dpni-typestate design D3). `UserspaceEvent` is unpriced and derives none.
#[must_use]
pub fn derive_profile(dp: Dataplane, _ic: InterfaceConstruct) -> ProfileOutcome {
    match dp {
        Dataplane::KernelNetlink => ProfileOutcome::Derived(Profile::Kernel),
        Dataplane::UserspacePoll => ProfileOutcome::Derived(Profile::Pmd),
        Dataplane::UserspaceEvent => ProfileOutcome::Unpriced,
    }
}

// ---- the runtime surface: state within the immutable type (dpni-typestate design D1) ----

/// The runtime surface — mutable state carried within a [`Dpni`] beside its immutable
/// [`DpniCfg`] (dpni-typestate design D1).
///
/// Today it is exactly the primary MAC, the one runtime mutation in today's create
/// contract (`docs/baseline/dpni.md` "Command surface": the sole post-create mutation
/// the current transport exposes). The remaining 35 `dpni_set_*` setters are a named
/// deferral to the mc-portal backend and add fields *here*, without reshaping the family.
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

    /// Sets the primary MAC — the one runtime mutation in today's create contract
    /// (`docs/baseline/dpni.md` "Attribute mutability"). Touches only the runtime
    /// surface; the create block is untouched.
    pub fn set_primary_mac(&mut self, mac: MacAddr) {
        self.runtime.primary_mac = mac;
    }
}

// ---- the unrepresentable options and their parity refusals (dpni-typestate design D2) ----

/// The eleven dead create knobs — the create-time MAC and the ten v9-era `max_*`
/// fields — plus the never-settable `num_rx_tcs` (`dpni.qnt` `type Unrepresentable`;
/// `docs/baseline/dpni.md` "Dead options" / "Never settable").
///
/// None has a field in [`DpniCfg`]: they are unrepresentable *by construct*, exactly as
/// the model gives them no `CreateCfg` field. This enum exists only so a programmatic
/// refusal *names* each one — the vocabulary-v2 parity precedent
/// ([`crate::intent::refuse::Refusal`], ADR-0014): every item an operator might reach
/// for is answered by a named refusal, so the dpni-typestate design-D11 rows stay
/// two-sided. Accepting one is worse than an error — legacy tooling takes the knob and
/// leaks a dpni (`docs/baseline/dpni.md` "Silent-failure notes").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unrepresentable {
    /// The create-time MAC address — a dead create knob; the primary MAC is a *runtime*
    /// mutation ([`Dpni::set_primary_mac`]), never a create option.
    MacAddrCreate,
    /// `max_senders` — a v9-era dead create knob.
    MaxSenders,
    /// `max_tcs` — a v9-era dead create knob.
    MaxTcs,
    /// `max_dist_per_tc` — a v9-era dead create knob.
    MaxDistPerTc,
    /// `max_fs_entries_per_tc` — a v9-era dead create knob.
    MaxFsEntriesPerTc,
    /// `max_unicast_filters` — a v9-era dead create knob.
    MaxUnicastFilters,
    /// `max_multicast_filters` — a v9-era dead create knob.
    MaxMulticastFilters,
    /// `max_vlan_filters` — a v9-era dead create knob.
    MaxVlanFilters,
    /// `max_qos_entries` — a v9-era dead create knob.
    MaxQosEntries,
    /// `max_qos_key_size` — a v9-era dead create knob.
    MaxQosKeySize,
    /// `max_dist_key_size` — a v9-era dead create knob.
    MaxDistKeySize,
    /// `num_rx_tcs` — wired into the MC create command but absent from the create
    /// contract (`docs/baseline/dpni.md` "Never settable").
    NumRxTcs,
}

/// The [`Unrepresentable`] variant names, in declaration order — the Rust copy of the
/// `dpni.qnt` `type Unrepresentable` cases (ADR-0014: an enumeration that restates the
/// model is a linted copy, kept honest by the exhaustive `match` in
/// [`Unrepresentable::name`]).
pub const UNREPRESENTABLE_VARIANTS: [&str; 12] = [
    "MacAddrCreate",
    "MaxSenders",
    "MaxTcs",
    "MaxDistPerTc",
    "MaxFsEntriesPerTc",
    "MaxUnicastFilters",
    "MaxMulticastFilters",
    "MaxVlanFilters",
    "MaxQosEntries",
    "MaxQosKeySize",
    "MaxDistKeySize",
    "NumRxTcs",
];

impl Unrepresentable {
    /// The eleven dead options — the Rust copy of the `dpni.qnt` `DEAD_OPTIONS` set
    /// (the create-time MAC and the ten `max_*` fields; `num_rx_tcs` is not a dead
    /// option, it is never-settable, so it is not here).
    pub const DEAD_OPTIONS: [Self; 11] = [
        Self::MacAddrCreate,
        Self::MaxSenders,
        Self::MaxTcs,
        Self::MaxDistPerTc,
        Self::MaxFsEntriesPerTc,
        Self::MaxUnicastFilters,
        Self::MaxMulticastFilters,
        Self::MaxVlanFilters,
        Self::MaxQosEntries,
        Self::MaxQosKeySize,
        Self::MaxDistKeySize,
    ];

    /// The whole unrepresentable set — the Rust copy of the `dpni.qnt` `UNREPRESENTABLE`
    /// set ([`DEAD_OPTIONS`](Self::DEAD_OPTIONS) plus [`NumRxTcs`](Self::NumRxTcs)).
    pub const UNREPRESENTABLE: [Self; 12] = [
        Self::MacAddrCreate,
        Self::MaxSenders,
        Self::MaxTcs,
        Self::MaxDistPerTc,
        Self::MaxFsEntriesPerTc,
        Self::MaxUnicastFilters,
        Self::MaxMulticastFilters,
        Self::MaxVlanFilters,
        Self::MaxQosEntries,
        Self::MaxQosKeySize,
        Self::MaxDistKeySize,
        Self::NumRxTcs,
    ];

    /// This variant's name, the token [`UNREPRESENTABLE_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MacAddrCreate => "MacAddrCreate",
            Self::MaxSenders => "MaxSenders",
            Self::MaxTcs => "MaxTcs",
            Self::MaxDistPerTc => "MaxDistPerTc",
            Self::MaxFsEntriesPerTc => "MaxFsEntriesPerTc",
            Self::MaxUnicastFilters => "MaxUnicastFilters",
            Self::MaxMulticastFilters => "MaxMulticastFilters",
            Self::MaxVlanFilters => "MaxVlanFilters",
            Self::MaxQosEntries => "MaxQosEntries",
            Self::MaxQosKeySize => "MaxQosKeySize",
            Self::MaxDistKeySize => "MaxDistKeySize",
            Self::NumRxTcs => "NumRxTcs",
        }
    }

    /// Why the item has no constructor — a dead v9-era create knob or the never-settable
    /// field (`docs/baseline/dpni.md` "Dead options" / "Never settable" /
    /// "Silent-failure notes"). Backend-neutral: the CLI-token map moves to `dpaa2-mc`
    /// with the transport (ADR-0018), never onto this domain surface.
    #[must_use]
    pub const fn why(self) -> &'static str {
        match self {
            Self::NumRxTcs => {
                "wired into the MC create command but absent from the create contract \
                 (baseline \"Never settable\")"
            }
            _ => {
                "a v9-era create knob absent from the MC v10 create contract; legacy \
                 tooling accepts it and leaks a dpni (baseline \"Dead options\" / \
                 \"Silent-failure notes\")"
            }
        }
    }

    /// The named programmatic refusal for this item (`dpni.qnt`
    /// `Refusal::DeadOptionRefusal(Unrepresentable)`): the answer when a caller asks
    /// whether the option is expressible is a refusal that *names* it, never a silent
    /// absence (spec "A dead option is refused by name"; dpni-typestate design D2).
    pub const fn refuse(self) -> DeadOptionRefusal {
        DeadOptionRefusal(self)
    }
}

/// A dead-option / `num_rx_tcs` request, refused by name — the Rust twin of the
/// `dpni.qnt` `Refusal::DeadOptionRefusal(Unrepresentable)` arm.
///
/// The model's `type Refusal` has two other arms this tile does not own, so they are
/// mirrored where they land, not here: `RangeViolation` is the range constructors'
/// [`Error::Config`] (dpni-typestate task 2.1, [`NumQueues::new`] et al.), and
/// `UnpricedDataplane` belongs to profile derivation (dpni-typestate task 3.1). This
/// wrapper carries the [`Unrepresentable`] so rendering derives the human string from
/// the variant, and it folds into the crate's error idiom via [`From`] ⇒ [`Error::Config`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct DeadOptionRefusal(pub Unrepresentable);

impl DeadOptionRefusal {
    /// The refused item.
    #[must_use]
    pub const fn option(self) -> Unrepresentable {
        self.0
    }
}

impl core::fmt::Display for DeadOptionRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} is unrepresentable: {}", self.0.name(), self.0.why())
    }
}

impl From<DeadOptionRefusal> for Error {
    /// A dead-option refusal is a configuration boundary error — the crate's error idiom
    /// (the sibling of the range constructors' [`Error::Config`]).
    fn from(refusal: DeadOptionRefusal) -> Self {
        Error::Config(refusal.to_string())
    }
}

// ---- the observation projection: dist_key_size excluded by construct (dpni-typestate design D4) ----

/// The dpni observation surface — the Rust twin of the `dpni.qnt` `type Observation`.
///
/// `dpni_attr` omits `dist_key_size`, so it can never be read back and is write-only
/// (`docs/baseline/dpni.md` "Attribute mutability", DPNI-I12; dpni-typestate design D4).
/// This projection carries every observable create-option field but *not* `dist_key_size`
/// — the exclusion is by construct (there is no field to compare), never a runtime skip,
/// so drift can never be claimed on it. It is the sibling of dprc's
/// [`ObservedContainer`](crate::plan::dprc::ObservedContainer) split: the create block
/// [`DpniCfg`] keeps its `Eq` for create-block identity, while drift is judged on this
/// separate type, so the `dist_key_size` exclusion never fights the [`DpniCfg`] derive.
///
/// Placement (`root_container`) is likewise absent: it is the container placement, not a
/// resize-triggering cfg field, and the model's `observe` omits it too — a dpni's
/// container is the assign/move machinery's concern (companion tile #6), not this
/// cfg-drift surface (dpni-typestate design D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpniObservation {
    /// The observed options mask.
    pub options: OptionMask,
    /// The observed `num_queues`.
    pub num_queues: NumQueues,
    /// The observed `num_tcs`.
    pub num_tcs: NumTcs,
    /// The observed `mac_filter_entries`.
    pub mac_filter_entries: MacFilterEntries,
    /// The observed `vlan_filter_entries`.
    pub vlan_filter_entries: VlanFilterEntries,
    /// The observed `qos_entries`.
    pub qos_entries: QosEntries,
    /// The observed `fs_entries`.
    pub fs_entries: FsEntries,
    /// The observed `num_cgs`.
    pub num_cgs: NumCgs,
    /// The observed `num_ceetm_ch`.
    pub num_ceetm_ch: NumCeetmCh,
    /// The observed `num_opr`.
    pub num_opr: NumOpr,
}

impl DpniObservation {
    /// Projects a create block to its observable surface — the Rust twin of the
    /// `dpni.qnt` `observe`: it drops write-only `dist_key_size` and the container
    /// placement (dpni-typestate design D4), so two blocks differing only in those fields
    /// project equal and cannot drift.
    #[must_use]
    pub fn project(cfg: &DpniCfg) -> Self {
        Self {
            options: cfg.options.clone(),
            num_queues: cfg.num_queues,
            num_tcs: cfg.num_tcs,
            mac_filter_entries: cfg.mac_filter_entries,
            vlan_filter_entries: cfg.vlan_filter_entries,
            qos_entries: cfg.qos_entries,
            fs_entries: cfg.fs_entries,
            num_cgs: cfg.num_cgs,
            num_ceetm_ch: cfg.num_ceetm_ch,
            num_opr: cfg.num_opr,
        }
    }
}

// ---- drift disposition: cfg drift is destroy+create, MAC-only is the mutation ----

/// What planning does about an observed dpni that differs from the desired one — the
/// pure decision the reconciler acts on (spec "Cfg drift plans destroy-and-create" /
/// "Primary MAC mutation plans without touching cfg").
///
/// A sans-io decision function ([`drift_disposition`]) produces it; wiring it to the
/// executor/shim is a later tile (dpni-typestate task 4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum DpniDisposition {
    /// Desired and observed agree — nothing to do.
    Converged,
    /// Only the primary MAC differs — the one runtime mutation
    /// ([`Dpni::set_primary_mac`]), no destroy + create.
    PrimaryMacMutation,
    /// A create-immutable cfg field differs — repair is refused; the object is destroyed
    /// and recreated (ADR-0001 §4; `docs/baseline/dpni.md` "Attribute mutability", LAW 1).
    DestroyThenCreate,
}

/// Decides what a dpni whose observation differs from its desired create block needs
/// (dpni-typestate design D1; ADR-0001 §4). Pure and sans-io: it re-observes nothing and
/// drives nothing — the caller supplies the desired block, the desired primary MAC, the
/// freshly observed projection, and the observed primary MAC.
///
/// Ordering — cfg wins over MAC (a combined cfg+MAC drift is [`DpniDisposition::DestroyThenCreate`]):
/// a create-immutable field mismatch is refuse-and-report, destroy + create, never repair
/// (ADR-0001 §4; `docs/baseline/dpni.md` "Attribute mutability"). A destroy + create
/// rebuilds the object from scratch and re-applies the runtime primary MAC afterward, so a
/// coincident MAC difference is *subsumed* by the rebuild — checking cfg first is exactly
/// why cfg drift dominates. Comparison runs on [`DpniObservation`], so write-only
/// `dist_key_size` can never contribute a difference (dpni-typestate design D4).
pub fn drift_disposition(
    desired: &DpniCfg,
    desired_mac: MacAddr,
    observed: &DpniObservation,
    observed_mac: MacAddr,
) -> DpniDisposition {
    if DpniObservation::project(desired) != *observed {
        DpniDisposition::DestroyThenCreate
    } else if desired_mac != observed_mac {
        DpniDisposition::PrimaryMacMutation
    } else {
        DpniDisposition::Converged
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

    // ---- the two board-verified profiles (dpni.qnt profiles / deriveProfile) ----

    #[test]
    fn profile_masks_match_the_model() {
        // dpni.qnt `PMD_PROFILE`/`KERNEL_PROFILE`: PMD carries five named flags plus the
        // raw escape; kernel carries HAS_KEY_MASKING only, no escape.
        let pmd = Profile::Pmd.mask();
        for f in [
            DpniOpt::SingleSender,
            DpniOpt::CustomCg,
            DpniOpt::HasKeyMasking,
            DpniOpt::HasOpr,
            DpniOpt::OprPerTc,
        ] {
            assert!(pmd.contains(f), "{}", f.name());
        }
        assert_eq!(pmd.flags().len(), 5);
        assert!(pmd.contains_escape(RawEscape::PfdrInPeb));

        let kernel = Profile::Kernel.mask();
        assert!(kernel.contains(DpniOpt::HasKeyMasking));
        assert_eq!(kernel.flags().len(), 1);
        assert!(kernel.escapes().is_empty());
    }

    #[test]
    fn profile_cfgs_carry_the_production_shape() {
        // dpni.qnt `profileCfg`: PMD is 16q/16tc, vlan 16, qos 64, fs 1, num_cgs 24, one
        // CEETM channel, child container; kernel is 1q/1tc, everything else default.
        let pmd = Profile::Pmd.cfg();
        assert_eq!(pmd.num_queues.get(), 16);
        assert_eq!(pmd.num_tcs.get(), 16);
        assert_eq!(pmd.vlan_filter_entries.get(), 16);
        assert_eq!(pmd.qos_entries.get(), 64);
        assert_eq!(pmd.fs_entries.get(), 1);
        assert_eq!(pmd.num_cgs.get(), 24);
        assert_eq!(pmd.num_ceetm_ch.get(), 1);
        assert!(!pmd.root_container);
        assert_eq!(pmd.options, Profile::Pmd.mask());

        let kernel = Profile::Kernel.cfg();
        assert_eq!(kernel.num_queues.get(), 1);
        assert_eq!(kernel.num_tcs.get(), 1);
        assert_eq!(kernel.num_cgs.get(), 0);
        assert_eq!(kernel.options, Profile::Kernel.mask());
    }

    #[test]
    fn derive_profile_keys_on_the_consumer() {
        // dpni.qnt `deriveProfile`: kernel-netlink ⇒ kernel, userspace-poll ⇒ PMD,
        // userspace-event ⇒ unpriced. The construct rides for totality only.
        for ic in InterfaceConstruct::CONSTRUCTS {
            assert_eq!(
                derive_profile(Dataplane::KernelNetlink, ic),
                ProfileOutcome::Derived(Profile::Kernel)
            );
            assert_eq!(
                derive_profile(Dataplane::UserspacePoll, ic),
                ProfileOutcome::Derived(Profile::Pmd)
            );
            assert_eq!(
                derive_profile(Dataplane::UserspaceEvent, ic),
                ProfileOutcome::Unpriced
            );
        }
    }

    #[test]
    fn interface_construct_variants_match_the_enum() {
        // dpni.qnt `type InterfaceConstruct` / `CONSTRUCTS` (ADR-0014).
        for ic in InterfaceConstruct::CONSTRUCTS {
            assert!(
                INTERFACE_CONSTRUCT_VARIANTS.contains(&ic.name()),
                "{}",
                ic.name()
            );
        }
        assert_eq!(
            INTERFACE_CONSTRUCT_VARIANTS.len(),
            InterfaceConstruct::CONSTRUCTS.len()
        );
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

    // ---- the unrepresentable options and their parity refusals (dpni-typestate design D2) ----

    #[test]
    fn unrepresentable_variants_match_the_enum_and_the_sets() {
        // dpni.qnt `type Unrepresentable` / `DEAD_OPTIONS` / `UNREPRESENTABLE` (ADR-0014).
        for u in Unrepresentable::UNREPRESENTABLE {
            assert!(UNREPRESENTABLE_VARIANTS.contains(&u.name()), "{}", u.name());
        }
        assert_eq!(
            UNREPRESENTABLE_VARIANTS.len(),
            Unrepresentable::UNREPRESENTABLE.len()
        );
        let mut seen = UNREPRESENTABLE_VARIANTS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), UNREPRESENTABLE_VARIANTS.len(), "duplicate name");

        assert_eq!(Unrepresentable::DEAD_OPTIONS.len(), 11);
        assert!(!Unrepresentable::DEAD_OPTIONS.contains(&Unrepresentable::NumRxTcs));
        assert!(Unrepresentable::UNREPRESENTABLE.contains(&Unrepresentable::NumRxTcs));
    }

    #[test]
    fn every_unrepresentable_item_fires_a_named_refusal() {
        // spec "A dead option is refused by name": each item is named, never a silent
        // absence (dpni-typestate design D2; the vocabulary-v2 parity precedent).
        for u in Unrepresentable::UNREPRESENTABLE {
            let refusal = u.refuse();
            assert_eq!(refusal.option(), u, "{}", u.name());

            let rendered = refusal.to_string();
            assert!(rendered.contains(u.name()), "{rendered}");

            match Error::from(refusal) {
                Error::Config(msg) => assert!(msg.contains(u.name()), "{msg}"),
                other => panic!("expected Error::Config, got {other:?}"),
            }
        }

        assert!(
            Unrepresentable::NumRxTcs
                .why()
                .contains("wired into the MC create command")
        );
        assert!(
            Unrepresentable::MaxSenders
                .why()
                .contains("v9-era create knob")
        );
    }

    // ---- the observation projection and drift disposition (dpni-typestate design D4) ----

    #[test]
    fn dist_key_size_never_drifts_while_every_projected_field_does() {
        // dpni.qnt `WriteOnlyDistKeySize` / spec "dist_key_size never reports drift":
        // excluded from the projection by construct (dpni-typestate design D4).
        let base = DpniCfg::defaults();
        let observed = DpniObservation::project(&base);
        let mac = MacAddr::ZERO;

        let mut only_dks = base.clone();
        only_dks.dist_key_size = DistKeySize::new(24).unwrap();
        assert_eq!(
            drift_disposition(&only_dks, mac, &observed, mac),
            DpniDisposition::Converged
        );

        macro_rules! drifts {
            ($mutate:expr) => {{
                let mut d = base.clone();
                let mutate: fn(&mut DpniCfg) = $mutate;
                mutate(&mut d);
                assert_eq!(
                    drift_disposition(&d, mac, &observed, mac),
                    DpniDisposition::DestroyThenCreate
                );
            }};
        }
        drifts!(|c| c.options = OptionMask::empty().with_flag(DpniOpt::HasKeyMasking));
        drifts!(|c| c.num_queues = NumQueues::new(1).unwrap());
        drifts!(|c| c.num_tcs = NumTcs::new(1).unwrap());
        drifts!(|c| c.mac_filter_entries = MacFilterEntries::new(1).unwrap());
        drifts!(|c| c.vlan_filter_entries = VlanFilterEntries::new(1).unwrap());
        drifts!(|c| c.qos_entries = QosEntries::new(1).unwrap());
        drifts!(|c| c.fs_entries = FsEntries::new(1).unwrap());
        drifts!(|c| c.num_cgs = NumCgs::new(1).unwrap());
        drifts!(|c| c.num_ceetm_ch = NumCeetmCh::new(1).unwrap());
        drifts!(|c| c.num_opr = NumOpr::new(1).unwrap());
    }

    #[test]
    fn mac_only_drift_plans_the_mutation() {
        // spec "Primary MAC mutation plans without touching cfg".
        let cfg = DpniCfg::defaults();
        let observed = DpniObservation::project(&cfg);
        let desired_mac = MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);
        assert_eq!(
            drift_disposition(&cfg, desired_mac, &observed, MacAddr::ZERO),
            DpniDisposition::PrimaryMacMutation
        );

        assert_eq!(
            drift_disposition(&cfg, MacAddr::ZERO, &observed, MacAddr::ZERO),
            DpniDisposition::Converged
        );
    }

    #[test]
    fn cfg_drift_wins_over_a_coincident_mac_drift() {
        // spec "Cfg drift plans destroy-and-create": cfg drift dominates a coincident
        // MAC drift because the rebuild re-applies the MAC (ADR-0001 §4).
        let base = DpniCfg::defaults();
        let observed = DpniObservation::project(&base);

        let mut drifted = base.clone();
        drifted.num_tcs = NumTcs::new(1).unwrap();

        assert_eq!(
            drift_disposition(&drifted, MacAddr::ZERO, &observed, MacAddr::ZERO),
            DpniDisposition::DestroyThenCreate
        );
        assert_eq!(
            drift_disposition(
                &drifted,
                MacAddr::new([0x02, 0, 0, 0, 0, 0x07]),
                &observed,
                MacAddr::ZERO,
            ),
            DpniDisposition::DestroyThenCreate
        );
    }
}
