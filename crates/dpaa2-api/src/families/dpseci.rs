//! The dpseci create surface as compile-time-refined types — the Rust twin of the
//! Quint create-option state machine (`models/families/dpseci.qnt` module
//! `dpseci_lifecycle`).
//!
//! Filed under `families/` as the per-family vocabulary tile whose `families/dpseci.rs`
//! path mirrors that `models/families/dpseci.qnt` quint twin one-to-one (ADR-0018). The
//! twin lives in module `dpseci_lifecycle`, not `module dpseci`: the thin params face
//! `module dpseci` keeps the generic create-surface names (`CreateCfg`, `OptionMask`,
//! `RawEscape`) out of the corpus-wide `.*` import, so this Rust module mirrors
//! `dpseci_lifecycle` (model header, `dpseci.qnt`).
//!
//! Authored model-first (quint-is-the-spec, ADR-0002 §3): the sums, payloads and refined
//! ranges here are structurally isomorphic to the Quint sums — same cases, same payloads,
//! same range semantics; names converge on readable English on both surfaces. This is a
//! P2 configured-object surface (ADR-0019): a create block alone, no transport named here
//! (sans-io hexagonal, ADR-0018 — the backend vocabulary lives in `dpaa2-mc`).
//!
//! # Immutable by construction (dpseci-typestate design D1; DPSECI-I1)
//!
//! The baseline's mutability law is absolute — `options`, the tx/rx queue counts, and the
//! per-queue `priorities[]` are all create-time-immutable, there is no `dpseci_set_*` for
//! any of them (there is no `dpseci_set_tx_queue` at all), and a resize is destroy + create
//! (`docs/baseline/dpseci.md` "Attribute mutability"). The family encodes it structurally:
//! [`DpseciCfg`] has private fields and no setter, so a validated block cannot change after
//! construction — witnessed by the `compile_fail` doctest on [`DpseciCfg`]. There is no
//! runtime surface at all (dpseci-typestate design D1: enable/disable, rx steering and
//! congestion thresholds belong to the consumer, not this surface), so unlike dpni this
//! tile carries no `Created`-state wrapper — the immutable [`DpseciCfg`] *is* the created
//! object's configuration.
//!
//! # Malformed create blocks are constructor refusals (V-DPSECI-1 rev 1)
//!
//! [`DpseciCfg::new`] is the only writer. It classifies the block in restool's own parse
//! order — the queue envelope first, then the priority count match, then the per-entry
//! range — and refuses by name ([`Refusal`]) before any block exists, mirroring restool's
//! parser refusing with exit 234 before it builds an MC command (`docs/baseline/dpseci.md`
//! "Option inventory", V-DPSECI-1 rev 1). No MC-layer validation is modeled: it is
//! unreachable through restool (baseline unknown #1).

use std::collections::BTreeSet;
use std::fmt;

use crate::core::error::Error;

// ---- the option mask: named MC vocabulary plus the provenance-carrying escape ----

/// The closed MC option vocabulary restool accepts for dpseci (`dpseci.qnt` `type
/// DpseciOpt`; `docs/baseline/dpseci.md` "Option inventory": `HAS_CG` 0x20, `HAS_OPR`
/// 0x40, `OPR_SHARED` 0x80). These are the three verified bits — the deployed dprc-script
/// sets all three (the vendor default), the kernel DPL sets `HAS_CG` only. A bit outside
/// this vocabulary has no variant, so it is unrepresentable as a named flag; it rides a
/// [`RawEscape`] instead. Which bits an *intent* derives is a later concern
/// (dpseci-typestate design D4, task 2.2) — this vocabulary only observes the three bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DpseciOpt {
    /// `DPSECI_OPT_HAS_CG` (0x20) — the congestion backstop (DPSECI-I4).
    HasCg,
    /// `DPSECI_OPT_HAS_OPR` (0x40).
    HasOpr,
    /// `DPSECI_OPT_OPR_SHARED` (0x80).
    OprShared,
}

/// The [`DpseciOpt`] variant names, in declaration order — the Rust copy of the
/// `dpseci.qnt` `type DpseciOpt` cases (ADR-0014: an enumeration that restates the model
/// is a linted copy, kept honest by the exhaustive `match` in [`DpseciOpt::name`]).
pub const DPSECI_OPT_VARIANTS: [&str; 3] = ["HasCg", "HasOpr", "OprShared"];

impl DpseciOpt {
    /// The whole MC vocabulary — the Rust copy of the `dpseci.qnt` `MC_VOCABULARY` set
    /// (the three named flags, no more). A flag outside this array has no variant, so it
    /// is unrepresentable by construct.
    pub const MC_VOCABULARY: [Self; 3] = [Self::HasCg, Self::HasOpr, Self::OprShared];

    /// This variant's name, the token [`DPSECI_OPT_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::HasCg => "HasCg",
            Self::HasOpr => "HasOpr",
            Self::OprShared => "OprShared",
        }
    }
}

/// The raw-mask escape — a provenance-carrying constructor, **not** a backdoor
/// (`dpseci.qnt` `type RawEscape = RawBits(int)`; dpni `OptionMask` idiom).
///
/// restool's `--options` parser falls back to `strtoull` on an unrecognized token, so an
/// arbitrary hex bit reaches the MC unnamed (`docs/baseline/dpseci.md` "Option inventory":
/// "unknown tokens fall back to raw hex — arbitrary bits reach MC"). dpseci banks no
/// specific escape bit the way dpni does, so the escape carries its raw value verbatim: an
/// unknown firmware bit is *attributed* in the escapes set, never silently merged into the
/// named flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawEscape(u32);

impl RawEscape {
    /// Builds an escape carrying the raw mask bit (`dpseci.qnt` `RawBits`). The only path
    /// a raw mask bit reaches a mask, so a create can request an unnamed bit and it stays
    /// attributed as an escape, distinct from the named vocabulary.
    #[must_use]
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    /// The raw mask bit this escape carries (`dpseci.qnt` `rawValue`).
    #[must_use]
    pub const fn raw_value(self) -> u32 {
        self.0
    }
}

/// The options mask — a typed set over the MC vocabulary plus the raw escapes
/// (`dpseci.qnt` `type OptionMask`). The shim emits the raw mask it computes itself, so
/// the two halves are the single source.
///
/// Built additively from [`OptionMask::empty`]; there is no path that admits an unnamed
/// flag or an arbitrary raw bit except the attributed [`RawEscape`].
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OptionMask {
    flags: BTreeSet<DpseciOpt>,
    escapes: BTreeSet<RawEscape>,
}

impl OptionMask {
    /// The empty mask — no options set, so `0` on the wire (`dpseci.qnt` `Set()`/`Set()`).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Adds a named MC flag (builder form). Idempotent — the set holds each flag once.
    #[must_use]
    pub fn with_flag(mut self, flag: DpseciOpt) -> Self {
        self.flags.insert(flag);
        self
    }

    /// Adds a provenance-carrying raw escape (builder form). The only path a raw mask bit
    /// reaches a mask.
    #[must_use]
    pub fn with_escape(mut self, escape: RawEscape) -> Self {
        self.escapes.insert(escape);
        self
    }

    /// The named flags this mask carries.
    #[must_use]
    pub fn flags(&self) -> &BTreeSet<DpseciOpt> {
        &self.flags
    }

    /// The raw escapes this mask carries.
    #[must_use]
    pub fn escapes(&self) -> &BTreeSet<RawEscape> {
        &self.escapes
    }

    /// Whether the named flag is set.
    #[must_use]
    pub fn contains(&self, flag: DpseciOpt) -> bool {
        self.flags.contains(&flag)
    }

    /// Whether the raw escape is set.
    #[must_use]
    pub fn contains_escape(&self, escape: RawEscape) -> bool {
        self.escapes.contains(&escape)
    }
}

// ---- the restool-parser refusals (dpseci.qnt `type Refusal`; V-DPSECI-1 rev 1) ----

/// The three restool-parser refusals a malformed dpseci create block earns, refused by
/// name before any MC command is built (`dpseci.qnt` `type Refusal`;
/// `docs/baseline/dpseci.md` "Option inventory", V-DPSECI-1 rev 1 — exit 234 at restool's
/// own parser). The Rust twin of the model's `Outcome = Accepted | Refused(Refusal)` is
/// `Result<DpseciCfg, Refusal>`: the accepted branch is the constructed block, the refused
/// branch is this sum. Folds into the crate error idiom via [`From`] ⇒ [`Error::Config`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[must_use]
pub enum Refusal {
    /// `num-queues` outside `1..=DPSECI_MAX_QUEUE_NUM`.
    QueueCountOutOfRange,
    /// The `priorities` count does not equal `num-queues`.
    PriorityCountMismatch,
    /// A priority is `0` or above `8`.
    PriorityOutOfRange,
}

/// The [`Refusal`] variant names, in declaration order — the Rust copy of the
/// `dpseci.qnt` `type Refusal` cases (ADR-0014: an enumeration that restates the model is
/// a linted copy, kept honest by the exhaustive `match` in [`Refusal::name`]).
pub const REFUSAL_VARIANTS: [&str; 3] = [
    "QueueCountOutOfRange",
    "PriorityCountMismatch",
    "PriorityOutOfRange",
];

impl Refusal {
    /// This variant's name, the token [`REFUSAL_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::QueueCountOutOfRange => "QueueCountOutOfRange",
            Self::PriorityCountMismatch => "PriorityCountMismatch",
            Self::PriorityOutOfRange => "PriorityOutOfRange",
        }
    }
}

impl fmt::Display for Refusal {
    /// Names the violated bound, referencing the [`DpseciCfg`] constants so the bound lives
    /// once (guu.4a).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueCountOutOfRange => write!(
                f,
                "num-queues outside the restool envelope 1..={} (DPSECI_MAX_QUEUE_NUM)",
                DpseciCfg::MAX_QUEUE_NUM,
            ),
            Self::PriorityCountMismatch => {
                write!(f, "priorities count must equal num-queues")
            }
            Self::PriorityOutOfRange => write!(
                f,
                "a priority is outside the restool envelope {}..={}",
                DpseciCfg::MIN_PRIORITY,
                DpseciCfg::MAX_PRIORITY,
            ),
        }
    }
}

impl From<Refusal> for Error {
    /// A restool-parser refusal is a configuration boundary error — the crate's error idiom
    /// (the sibling of dpni's range-constructor [`Error::Config`]).
    fn from(refusal: Refusal) -> Self {
        Error::Config(refusal.to_string())
    }
}

// ---- the immutable create block (dpseci-typestate design D1; DPSECI-I1) ----

/// The validated dpseci create block — the options mask, the queue count and the
/// per-queue priorities as one immutable record (`dpseci.qnt` `type CreateCfg`;
/// `docs/baseline/dpseci.md` "Option inventory" / "Attribute mutability").
///
/// Every field is private and there is no setter, so a block cannot change after
/// construction — the model's `Created(CreateCfg)` payload, immutable for the object's life
/// (DPSECI-I1 by construction: no mutating verb exists on this surface). The only writer is
/// [`new`](Self::new), which refuses a malformed block by name, so a `DpseciCfg` that exists
/// has always passed restool's parse order.
///
/// ```compile_fail
/// use dpaa2_api::families::dpseci::{DpseciCfg, OptionMask};
/// let cfg = DpseciCfg::new(OptionMask::empty(), 1, vec![1]).unwrap();
/// // Every field is private and there is no setter: mutating a create-time value after
/// // construction does not type-check (DPSECI-I1 by construction;
/// // dpseci-typestate design D1; docs/baseline/dpseci.md "Attribute mutability").
/// cfg.num_queues = 2;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DpseciCfg {
    options: OptionMask,
    num_queues: usize,
    priorities: Vec<u8>,
}

impl DpseciCfg {
    /// The queue-pair ceiling of one dpseci device (`dpseci.qnt` `DPSECI_MAX_QUEUE_NUM`;
    /// `docs/baseline/dpseci.md` "Option inventory": `--num-queues` 1–16). Verified in the
    /// NXP trees (`DPSECI_MAX_QUEUE_NUM == 16`).
    pub const MAX_QUEUE_NUM: usize = 16;

    /// The lowest priority restool's parser accepts (`docs/baseline/dpseci.md` "Option
    /// inventory": each 1–8; priority 0 is refused).
    pub const MIN_PRIORITY: u8 = 1;

    /// The highest priority restool's parser accepts (`docs/baseline/dpseci.md` "Option
    /// inventory": each 1–8; a priority above 8 is refused).
    pub const MAX_PRIORITY: u8 = 8;

    /// Builds a create block, refusing a malformed one by name (`dpseci.qnt` `classify` +
    /// `createAt`, accepted branch). Checks run in restool's own parse order — the queue
    /// envelope first, then the priority count match, then the per-entry range — so the
    /// refusal a caller sees is the one restool's parser would raise first (V-DPSECI-1
    /// rev 1; `docs/baseline/dpseci.md` "Option inventory").
    ///
    /// # Errors
    /// [`Refusal::QueueCountOutOfRange`] when `num_queues` is outside `1..=MAX_QUEUE_NUM`;
    /// [`Refusal::PriorityCountMismatch`] when `priorities.len()` does not equal
    /// `num_queues`; [`Refusal::PriorityOutOfRange`] when a priority is outside
    /// `MIN_PRIORITY..=MAX_PRIORITY`.
    pub fn new(
        options: OptionMask,
        num_queues: usize,
        priorities: Vec<u8>,
    ) -> Result<Self, Refusal> {
        if !(1..=Self::MAX_QUEUE_NUM).contains(&num_queues) {
            return Err(Refusal::QueueCountOutOfRange);
        }
        if priorities.len() != num_queues {
            return Err(Refusal::PriorityCountMismatch);
        }
        if !priorities
            .iter()
            .all(|&p| (Self::MIN_PRIORITY..=Self::MAX_PRIORITY).contains(&p))
        {
            return Err(Refusal::PriorityOutOfRange);
        }
        Ok(Self {
            options,
            num_queues,
            priorities,
        })
    }

    /// The options mask (by shared reference — there is no `&mut` path).
    #[must_use]
    pub fn options(&self) -> &OptionMask {
        &self.options
    }

    /// The queue count — one `--num-queues` drives both tx and rx
    /// (`docs/baseline/dpseci.md` "Command surface").
    #[must_use]
    pub fn num_queues(&self) -> usize {
        self.num_queues
    }

    /// The per-queue tx priorities, length equal to [`num_queues`](Self::num_queues).
    #[must_use]
    pub fn priorities(&self) -> &[u8] {
        &self.priorities
    }

    /// Whether this block carries the congestion backstop — present iff `HAS_CG` was set at
    /// create (`dpseci.qnt` `congestionBackstop`; DPSECI-I4). A pure function of the create
    /// options and nothing else: the block is immutable, so no later operation can mint or
    /// drop it (`docs/baseline/dpseci.md` "Kernel-side behavior": congestion is configured
    /// only when the object has `HAS_CG`).
    #[must_use]
    pub fn congestion_backstop(&self) -> bool {
        self.options.contains(DpseciOpt::HasCg)
    }
}

#[cfg(test)]
mod tests {
    //! Parity of the create-surface sums with `models/families/dpseci.qnt`
    //! `dpseci_lifecycle`, the classify-order refusals, the raw-escape provenance, the
    //! congestion birth capability, and the two board-verified profiles. The compile-time
    //! immutability of the create block is witnessed by the `compile_fail` doctest on
    //! [`DpseciCfg`].

    use super::*;

    // ---- parity: each model sum's Rust copy is complete (ADR-0014) ----

    #[test]
    fn dpseci_opt_variants_match_the_enum_and_vocabulary() {
        // dpseci.qnt `type DpseciOpt` / `MC_VOCABULARY`: exactly the three named flags.
        for opt in DpseciOpt::MC_VOCABULARY {
            assert!(DPSECI_OPT_VARIANTS.contains(&opt.name()), "{}", opt.name());
        }
        assert_eq!(DpseciOpt::MC_VOCABULARY.len(), DPSECI_OPT_VARIANTS.len());
        let mut seen = DPSECI_OPT_VARIANTS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), DPSECI_OPT_VARIANTS.len(), "duplicate flag name");
    }

    #[test]
    fn refusal_variants_match_the_enum() {
        // dpseci.qnt `type Refusal`: exactly the three restool-parser refusals.
        for r in [
            Refusal::QueueCountOutOfRange,
            Refusal::PriorityCountMismatch,
            Refusal::PriorityOutOfRange,
        ] {
            assert!(REFUSAL_VARIANTS.contains(&r.name()), "{}", r.name());
        }
        assert_eq!(REFUSAL_VARIANTS.len(), 3);
    }

    // ---- option mask construction and raw-escape provenance ----

    #[test]
    fn option_mask_builds_additively() {
        let empty = OptionMask::empty();
        assert!(empty.flags().is_empty());
        assert!(empty.escapes().is_empty());

        let mask = OptionMask::empty()
            .with_flag(DpseciOpt::HasCg)
            .with_flag(DpseciOpt::HasCg) // idempotent
            .with_flag(DpseciOpt::HasOpr);
        assert!(mask.contains(DpseciOpt::HasCg));
        assert!(mask.contains(DpseciOpt::HasOpr));
        assert!(!mask.contains(DpseciOpt::OprShared));
        assert_eq!(mask.flags().len(), 2);
    }

    #[test]
    fn raw_escape_rides_the_mask_attributed_not_merged() {
        // dpseci.qnt `RAW_ESCAPE_CFG`: an unknown bit 0x100 rides the escapes set, never
        // merged into the named flags.
        let mask = OptionMask::empty()
            .with_flag(DpseciOpt::HasCg)
            .with_escape(RawEscape::new(0x100));
        assert!(mask.contains_escape(RawEscape::new(0x100)));
        assert_eq!(mask.escapes().len(), 1);
        assert_eq!(mask.escapes().iter().next().unwrap().raw_value(), 0x100);
        assert_eq!(mask.flags().len(), 1);
        assert!(mask.contains(DpseciOpt::HasCg));
    }

    // ---- constructor refusals in classify order (dpseci.qnt `classify`; V-DPSECI-1 rev 1) ----

    #[test]
    fn queue_count_out_of_range_is_refused_first() {
        // dpseci.qnt `queueCountOutOfRangeRefusedTest`: a queue count above the ceiling
        // (17 > 16) is refused, nothing created.
        let err = DpseciCfg::new(OptionMask::empty(), 17, vec![2; 17]).unwrap_err();
        assert_eq!(err, Refusal::QueueCountOutOfRange);
        assert_eq!(
            DpseciCfg::new(OptionMask::empty(), 0, vec![]).unwrap_err(),
            Refusal::QueueCountOutOfRange
        );
    }

    #[test]
    fn priority_count_mismatch_is_refused_second() {
        // dpseci.qnt `priorityCountMismatchRefusedTest`: 2 queues with priorities [1] is a
        // count mismatch, nothing created.
        let err = DpseciCfg::new(OptionMask::empty(), 2, vec![1]).unwrap_err();
        assert_eq!(err, Refusal::PriorityCountMismatch);
    }

    #[test]
    fn priority_out_of_range_is_refused_last() {
        // dpseci.qnt `priorityZeroRefusedTest` / `priorityAboveEightRefusedTest`: a
        // priority of 0 or above 8 is refused, nothing created.
        assert_eq!(
            DpseciCfg::new(OptionMask::empty(), 2, vec![1, 0]).unwrap_err(),
            Refusal::PriorityOutOfRange
        );
        assert_eq!(
            DpseciCfg::new(OptionMask::empty(), 2, vec![1, 9]).unwrap_err(),
            Refusal::PriorityOutOfRange
        );
    }

    #[test]
    fn classify_order_queue_envelope_beats_a_coincident_priority_fault() {
        // The queue envelope is checked before the count match and the per-entry range, so
        // a block that violates all three earns QueueCountOutOfRange (dpseci.qnt `classify`
        // order; restool's own parse order, V-DPSECI-1 rev 1).
        let err = DpseciCfg::new(OptionMask::empty(), 17, vec![0]).unwrap_err();
        assert_eq!(err, Refusal::QueueCountOutOfRange);
    }

    #[test]
    fn classify_order_count_mismatch_beats_a_coincident_range_fault() {
        // The count match is checked before the per-entry range: a block whose count is
        // wrong AND carries an out-of-range priority earns PriorityCountMismatch.
        let err = DpseciCfg::new(OptionMask::empty(), 2, vec![0]).unwrap_err();
        assert_eq!(err, Refusal::PriorityCountMismatch);
    }

    #[test]
    fn refusal_display_names_the_violated_bound() {
        assert!(
            Refusal::QueueCountOutOfRange.to_string().contains("1..=16"),
            "{}",
            Refusal::QueueCountOutOfRange
        );
        assert!(Refusal::PriorityCountMismatch.to_string().contains("count"));
        assert!(Refusal::PriorityOutOfRange.to_string().contains("1..=8"));

        match Error::from(Refusal::QueueCountOutOfRange) {
            Error::Config(msg) => assert!(msg.contains("num-queues"), "{msg}"),
            other => panic!("expected Error::Config, got {other:?}"),
        }
    }

    // ---- the two board-verified profiles (dpseci.qnt KERNEL_CFG / PRODUCTION_CFG) ----

    #[test]
    fn kernel_profile_is_accepted_and_carries_the_backstop() {
        // dpseci.qnt `KERNEL_CFG` / `congestionBirthWithCgTest`: 16 queues, all priorities
        // 1, HAS_CG only — accepted, read back verbatim, carries the backstop.
        let cfg = DpseciCfg::new(
            OptionMask::empty().with_flag(DpseciOpt::HasCg),
            16,
            vec![1; 16],
        )
        .expect("the kernel profile is inside the restool envelope");
        assert_eq!(cfg.num_queues(), 16);
        assert_eq!(cfg.priorities(), &[1u8; 16]);
        assert!(cfg.options().contains(DpseciOpt::HasCg));
        assert!(cfg.congestion_backstop());
    }

    #[test]
    fn production_profile_is_accepted_and_reads_back_verbatim() {
        // dpseci.qnt `PRODUCTION_CFG` / `createAcceptedReadbackTest`: 8 queues, all
        // priorities 2, all three vendor-default option bits.
        let options = OptionMask::empty()
            .with_flag(DpseciOpt::HasCg)
            .with_flag(DpseciOpt::HasOpr)
            .with_flag(DpseciOpt::OprShared);
        let cfg = DpseciCfg::new(options, 8, vec![2; 8])
            .expect("the production profile is inside the restool envelope");
        assert_eq!(cfg.num_queues(), 8);
        assert_eq!(cfg.priorities(), &[2u8; 8]);
        assert_eq!(cfg.options().flags().len(), 3);
        assert!(cfg.congestion_backstop());
    }

    // ---- DPSECI-I4: congestion backstop present iff HAS_CG at construction ----

    #[test]
    fn congestion_backstop_tracks_has_cg_both_ways() {
        // dpseci.qnt `congestionBirthWithoutCgTest` / `NO_CG_CFG`: the production shape
        // without HAS_CG carries no backstop; with it, it does.
        let without_cg = DpseciCfg::new(
            OptionMask::empty()
                .with_flag(DpseciOpt::HasOpr)
                .with_flag(DpseciOpt::OprShared),
            8,
            vec![2; 8],
        )
        .unwrap();
        assert!(!without_cg.congestion_backstop());

        let with_cg = DpseciCfg::new(
            OptionMask::empty().with_flag(DpseciOpt::HasCg),
            8,
            vec![2; 8],
        )
        .unwrap();
        assert!(with_cg.congestion_backstop());
    }
}
