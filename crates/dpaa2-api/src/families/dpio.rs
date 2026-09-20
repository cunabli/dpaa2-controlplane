//! The seat-typed dpio surface — the Rust twin of the Quint model
//! `models/families/dpio.qnt` (`module dpio` + `module dpio_lifecycle`; ADR-0019
//! pattern P3, the seat-typed variant; pool-objects design D1/D4).
//!
//! Filed under `families/` as the per-family vocabulary tile whose `families/dpio.rs`
//! path mirrors that `models/families/dpio.qnt` quint twin one-to-one (ADR-0018).
//!
//! Authored model-first (quint-is-the-spec): the sums, the seat record, and the seat
//! arithmetic here are structurally isomorphic to the Quint sums and predicates, and the
//! ADR-0002 §3 law binds them — same cases, same counts, same judgments; names converge
//! on readable English on both surfaces. The baseline anchor is `docs/baseline/dpio.md`
//! ("Option inventory", "Attribute mutability", "Kernel-side behavior", "Lifecycle
//! ordering and dependencies"; DPIO-I2/I3).
//!
//! # dpio is `pooled: false` — a sibling of the trio, not a member (DPIO-I1)
//!
//! This is NOT an extension of [`pool_lifecycle`](crate::families::pool_lifecycle): dpio
//! is seat arithmetic, never pool custody, so it reuses none of that surface. There is no
//! [`PoolCensus`](crate::families::pool_lifecycle::PoolCensus) here, no fourth
//! [`PoolFamily`](crate::families::pool_lifecycle::PoolFamily) variant (the `compile_fail`
//! witness on that enum), and no trait couples the two — P3's seat-typed member counts
//! dpios by regime and never as one pool (DPIO-I2; pool-objects design D1). The seat is
//! the census unit: a regime-typed count against a regime-typed ceiling, with no
//! per-object identity, no phase/typestate machinery, and no cross-pattern trait
//! (reconciler spec requirement 1, the seat-typed dpio variant clause).
//!
//! # The dpio→dpmcp probe-draw ordering edge stays in the adapter (pool-objects design D4)
//!
//! The dpio driver draws ONE free dpmcp when a consumer probes a plugged dpio — the
//! hidden dependency behind ls-addni's dpmcp-per-dpio rule (`docs/baseline/dpio.md`
//! "Kernel-side behavior"; model `probeDpioAt`, DPIO-I1/DPMCP-I1). An exhausted dpmcp
//! pool does not fail loudly: the probe defers on a silent `-EPROBE_DEFER` loop, an
//! ordering that is *observed, not driven*. Per pool-objects design D4 this edge stays
//! procedural in `dpaa2-mc` (the dpni set-MAC-before-plug precedent); this surface mints
//! no order-hazardous verb and no typestate for it — [`DpioSeat::mark_probed`] records
//! only the observation flag, never a draw. Phase 3 lands the draw itself in the adapter.
//!
//! # The pool-objects design D4 facet marker: `channel_mode` is dead in the kernel (DPIO-I3)
//!
//! The one dpio create-cfg field that smells of P2's cfg-hazard class is `channel_mode`,
//! yet the kernel ignores it: capability reads the reported `num_priorities` alone
//! ([`DpioSeat::notify_capable`]/[`DpioSeat::eight_priority`]), never the mode
//! (`docs/baseline/dpio.md` "Kernel-side behavior"; model `DPIO_I3_modeDead`). A wrong
//! mode changes nothing in the kernel path, so the kernel-regime create-cfg carries no
//! kernel-side hazard class. Whether that earns an ADR-0019 cfg facet is the main loop's
//! and the user's call (pool-objects design D4); this tile reports the evidence, it does
//! not legislate it.

use crate::core::error::Error;
use crate::core::family::Family;
use crate::intent::compiled::{CompiledPlan, Container};

// ---- the two create-cfg sums (model `dpio` types) ----

/// The dpio channel mode (`dpio.qnt` `type ChannelMode`; `docs/baseline/dpio.md`
/// "Option inventory": `--channel-mode` is `DPIO_LOCAL_CHANNEL` | `DPIO_NO_CHANNEL`).
///
/// A recorded create field the kernel later ignores — capability is read from
/// `num_priorities`, not the mode (DPIO-I3, Breaking; see [`DpioSeat::notify_capable`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChannelMode {
    /// `DPIO_LOCAL_CHANNEL` — the restool default (a local dequeue channel).
    LocalChannel,
    /// `DPIO_NO_CHANNEL` — no local channel; `num_priorities` is still sent and still
    /// decides capability (`docs/baseline/dpio.md` "Option inventory" quirk; DPIO-I3).
    NoChannel,
}

/// The [`ChannelMode`] variant names, in declaration order — the Rust copy of the
/// `dpio.qnt` `type ChannelMode` cases (ADR-0014: an enumeration that restates the model
/// is a linted copy, kept honest by the exhaustive `match` in [`ChannelMode::name`]).
pub const CHANNEL_MODE_VARIANTS: [&str; 2] = ["LocalChannel", "NoChannel"];

impl ChannelMode {
    /// The whole channel-mode alphabet — the Rust copy of the `dpio.qnt` `CHANNEL_MODES`
    /// set (both modes, no more).
    pub const CHANNEL_MODES: [Self; 2] = [Self::LocalChannel, Self::NoChannel];

    /// This variant's name, the token [`CHANNEL_MODE_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::LocalChannel => "LocalChannel",
            Self::NoChannel => "NoChannel",
        }
    }
}

/// The consumer regime a dpio seat serves (`dpio.qnt` `type SeatRegime`;
/// `docs/baseline/dpio.md` "Lifecycle ordering and dependencies"): the kernel binds one
/// dpio per online CPU as a shared per-CPU service; DPDK claims two exclusive portals per
/// thread (ADR-0012). dpio counts are typed by regime, never treated as one pool
/// (DPIO-I2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SeatRegime {
    /// The kernel per-CPU service seat — ceiling is the online-CPU count; the extra
    /// dpio is refused `-ERANGE` (`docs/baseline/dpio.md` "Kernel-side behavior").
    KernelSeat,
    /// The DPDK per-thread exclusive portal — ceiling is `2 × threads` (a general and an
    /// ethernet-rx portal per thread; ADR-0012, board-verified).
    DpdkSeat,
}

/// The [`SeatRegime`] variant names, in declaration order — the Rust copy of the
/// `dpio.qnt` `type SeatRegime` cases (ADR-0014, tied by [`SeatRegime::name`]).
pub const SEAT_REGIME_VARIANTS: [&str; 2] = ["KernelSeat", "DpdkSeat"];

impl SeatRegime {
    /// The whole seat-regime alphabet — the Rust copy of the `dpio.qnt` `SEAT_REGIMES`
    /// set (both regimes, no more).
    pub const SEAT_REGIMES: [Self; 2] = [Self::KernelSeat, Self::DpdkSeat];

    /// This variant's name, the token [`SEAT_REGIME_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::KernelSeat => "KernelSeat",
            Self::DpdkSeat => "DpdkSeat",
        }
    }
}

// ---- the priorities newtype: the MC 1..=8 range (model `numPriorities` gate) ----

/// `num_priorities` refined to the MC create range `1..=8` (`dpio.qnt` `createDpioAt`'s
/// `numPriorities >= 1 and numPriorities <= 8` gate; `docs/baseline/dpio.md` "Option
/// inventory": 1–8, default 8).
///
/// Unlike the dpni [`ranged_option!`](crate::families::dpni) fields there is no `0`
/// MC-default sentinel — the dpio create gate admits `1..=8` only, so the newtype has no
/// constructor for `0` or `9`. `8` is the create default and the anchor of
/// [`DpioSeat::eight_priority`] (`dpio.qnt` `dpioEightPriority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Priorities(u8);

impl Priorities {
    /// The inclusive lower bound of the MC create range (`dpio.qnt` `createDpioAt`).
    pub const MIN: u8 = 1;
    /// The inclusive upper bound of the MC create range — the create default
    /// (`docs/baseline/dpio.md` "Option inventory").
    pub const MAX: u8 = 8;

    /// Builds the refined value, refusing anything outside the MC create range `1..=8`.
    /// `0` (no MC-default sentinel here) and `9..` have no path — the refusal is a
    /// type-boundary error, not a board rejection (`dpio.qnt` `createDpioAt` 1..8 gate).
    ///
    /// # Errors
    /// [`Error::Config`] naming the field and its `1..=8` range when `v` is out of range.
    pub fn new(v: u8) -> Result<Self, Error> {
        if (Self::MIN..=Self::MAX).contains(&v) {
            Ok(Self(v))
        } else {
            Err(Error::Config(format!(
                "num_priorities {v} outside the MC create range {}..={}",
                Self::MIN,
                Self::MAX,
            )))
        }
    }

    /// The raw priority count, in `1..=8`.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

// ---- the seat record: immutable create-cfg + regime + observation flag ----

/// The immutable dpio create-cfg — mode and priorities (`docs/baseline/dpio.md`
/// "Attribute mutability": both are create-time, nothing mutable at runtime through the
/// object API). The two cfg fields of the model's `DpioSeat` record.
///
/// Held by value inside a [`DpioSeat`] with no setter, so a created seat's cfg never
/// mutates — a wrong-cfg repair is destroy + recreate, never mutation
/// (`docs/baseline/dpio.md` "Attribute mutability"). The `channel_mode` half is dead in
/// the kernel (DPIO-I3): capability reads `priorities` alone, so a mode difference changes
/// no kernel-observable behavior (see the module-level pool-objects design D4 facet marker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DpioCfg {
    /// `channel_mode` — recorded, kernel-ignored (DPIO-I3).
    pub mode: ChannelMode,
    /// `num_priorities`, refined to `1..=8`.
    pub priorities: Priorities,
}

/// A dpio seat — the Rust twin of the `dpio.qnt` `DpioSeat` record
/// (`{ mode, numPriorities, regime, probed }`; pool-objects design D4).
///
/// The record splits three ways by lifetime, which is the shape of the facet evidence:
/// the immutable create-[`cfg`](Self::cfg) (`mode` + `priorities`), the structural
/// [`regime`](Self::regime) (the seat type, fixed at create), and the observation-side
/// [`probed`](Self::probed) flag the kernel driver flips when it probes the seat and
/// draws its dpmcp. Only `probed` mutates, and it is an *observation*, not cfg — the
/// count-only seat surface never grows a P2 cfg-drift disposition (see the module-level
/// pool-objects design D4 marker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DpioSeat {
    cfg: DpioCfg,
    regime: SeatRegime,
    probed: bool,
}

impl DpioSeat {
    /// Creates a seat from an immutable cfg and its regime — the `dpio.qnt` `createDpioAt`
    /// record, with `probed: false` (the seat is unprobed until a consumer probes it).
    ///
    /// The seat-ceiling gate is [`admit_seat`] (the `-ERANGE` refusal); this constructor
    /// records a cfg already refined by [`Priorities::new`], so it is infallible.
    #[must_use]
    pub const fn create(cfg: DpioCfg, regime: SeatRegime) -> Self {
        Self {
            cfg,
            regime,
            probed: false,
        }
    }

    /// The immutable create-cfg (by value — `DpioCfg` is `Copy`; there is no `&mut` path,
    /// so the cfg never mutates after create).
    #[must_use]
    pub const fn cfg(self) -> DpioCfg {
        self.cfg
    }

    /// The seat regime (the structural seat type; `dpio.qnt` `DpioSeat.regime`).
    #[must_use]
    pub const fn regime(self) -> SeatRegime {
        self.regime
    }

    /// Whether the kernel driver has probed the seat (`dpio.qnt` `DpioSeat.probed`) — the
    /// observation-side flag, set once the driver drew this seat's dpmcp.
    #[must_use]
    pub const fn probed(self) -> bool {
        self.probed
    }

    /// Records that the driver has probed the seat — the `dpio.qnt` `probeDpioAt` `probed`
    /// flip, and ONLY that flip. The dpmcp draw the model's `markAllocated` performs
    /// alongside it stays adapter-procedural (`dpaa2-mc`, phase 3; see the module-level
    /// probe-draw note): this method touches no pool and draws nothing — it is an
    /// observation, not an order-hazardous verb (pool-objects design D4).
    pub const fn mark_probed(&mut self) {
        self.probed = true;
    }

    /// Notification capability — the `dpio.qnt` `dpioNotifyCapable` (`num_priorities != 0`;
    /// DPIO-I3, Breaking). Reads `priorities` ALONE, never `channel_mode`, so the mode is
    /// dead: a `NoChannel` seat with priorities reports capable exactly as a `LocalChannel`
    /// one (`docs/baseline/dpio.md` "Kernel-side behavior"; V-READBACK-1). A validly
    /// created seat is always capable, since [`Priorities`] refines out `0`.
    #[must_use]
    pub const fn notify_capable(self) -> bool {
        self.cfg.priorities.get() != 0
    }

    /// Eight-priority mode — the `dpio.qnt` `dpioEightPriority` (`num_priorities == 8`;
    /// DPIO-I3). Mode-independent for the same reason as [`notify_capable`](Self::notify_capable):
    /// the driver reads the reported count, not the channel mode.
    #[must_use]
    pub const fn eight_priority(self) -> bool {
        self.cfg.priorities.get() == Priorities::MAX
    }
}

// ---- seat arithmetic: regime-typed ceiling, census, and the -ERANGE gate (DPIO-I2) ----

/// The seat ceiling for a regime — the `dpio.qnt` `seatCeiling` with the model's fixed
/// `NUM_CPUS`/`NUM_THREADS` lifted to parameters: [`SeatRegime::KernelSeat`] ⇒ `cpus`
/// (one dpio per online CPU), [`SeatRegime::DpdkSeat`] ⇒ `2 * threads` (a general and an
/// ethernet-rx portal per thread; DPIO-I2, ADR-0012). `cpus` is the online-CPU count and
/// `threads` is `T`, both read from the ADR-0012 derivation, never invented here.
#[must_use]
pub const fn seat_ceiling(regime: SeatRegime, cpus: i64, threads: i64) -> i64 {
    match regime {
        SeatRegime::KernelSeat => cpus,
        SeatRegime::DpdkSeat => 2 * threads,
    }
}

/// The seat census for a regime — the `dpio.qnt` `dpioSeatCount` (the count of seats
/// whose regime matches). The seat, not a pool, is the census unit (DPIO-I2).
#[must_use]
pub fn seat_count(seats: &[DpioSeat], regime: SeatRegime) -> i64 {
    i64::try_from(seats.iter().filter(|s| s.regime == regime).count()).unwrap_or(i64::MAX)
}

/// The intent-derived dpio seat count for one container — the count of planned dpio
/// objects the plan places there, consumed unchanged from the compiled plan (ADR-0012
/// regime draws: `2·T` per userspace-poll consumer, one per online CPU for the kernel).
/// This is the seat twin of
/// [`pool_lifecycle::derived_requirement`](crate::families::pool_lifecycle::derived_requirement):
/// dpio is `pooled: false`, so its convergence target is a seat count, not a pool census,
/// but the plan population read is the same shape — a plain filter of the plan, never a
/// re-derivation (the sizing rules already ran in the intent compiler).
#[must_use]
pub fn derived_seats(plan: &CompiledPlan, container: &Container) -> i64 {
    i64::try_from(
        plan.objects
            .iter()
            .filter(|o| o.container() == container && o.key().family == Family::Dpio)
            .count(),
    )
    .unwrap_or(i64::MAX)
}

/// The typed refusal a create past the regime's seat ceiling raises — the Rust twin of the
/// disabled `dpio.qnt` `createDpioAt` guard (`dpioSeatCount < seatCeiling`), the `-ERANGE`
/// shape (`docs/baseline/dpio.md` "Kernel-side behavior": "Number of DPIOs exceeds
/// `NR_CPUS`"; DPIO-I2). A seat past the ceiling is a disabled create, never a recorded
/// over-ceiling state — it surfaces to the operator by regime and counts.
///
/// Namespaced like the [`pool_lifecycle::ShrinkBelowDraw`](crate::families::pool_lifecycle::ShrinkBelowDraw)
/// refusal (a seat refusal is a family concern, not a corpus-wide one). It folds into the
/// crate error idiom via [`From`] ⇒ [`Error::Config`], the [`DeadOptionRefusal`] idiom.
///
/// [`DeadOptionRefusal`]: crate::families::dpni::DeadOptionRefusal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct SeatCeilingExceeded {
    /// The regime whose seat ceiling the create would exceed.
    pub regime: SeatRegime,
    /// The current seat count for the regime (the model's `dpioSeatCount`).
    pub count: i64,
    /// The regime's seat ceiling the count has reached (the model's `seatCeiling`).
    pub ceiling: i64,
}

impl core::fmt::Display for SeatCeilingExceeded {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} seats are at the ceiling {} (count {}): the extra dpio is refused -ERANGE",
            self.regime.name(),
            self.ceiling,
            self.count,
        )
    }
}

impl From<SeatCeilingExceeded> for Error {
    /// A seat-ceiling refusal is a configuration boundary error — the crate error idiom
    /// (the sibling of the dpni [`DeadOptionRefusal`]'s [`Error::Config`]).
    ///
    /// [`DeadOptionRefusal`]: crate::families::dpni::DeadOptionRefusal
    fn from(refusal: SeatCeilingExceeded) -> Self {
        Error::Config(refusal.to_string())
    }
}

/// The seat-bound create gate — the `dpio.qnt` `createDpioAt` guard
/// `dpioSeatCount(dpioSeats, regime) < seatCeiling(regime)` (DPIO-I2). A create is
/// admitted while the regime's seats are below its ceiling; at the ceiling it is refused
/// [`SeatCeilingExceeded`] (the `-ERANGE` shape), naming the regime and counts.
///
/// Pure and sans-io: it counts the seats the caller supplies against a ceiling derived
/// from `cpus`/`threads` (ADR-0012), observing and driving nothing.
///
/// # Errors
/// Returns [`SeatCeilingExceeded`] when the regime's seat count has reached its ceiling —
/// the extra dpio is refused rather than recorded over-ceiling.
pub fn admit_seat(
    seats: &[DpioSeat],
    regime: SeatRegime,
    cpus: i64,
    threads: i64,
) -> Result<(), SeatCeilingExceeded> {
    let count = seat_count(seats, regime);
    let ceiling = seat_ceiling(regime, cpus, threads);
    if count < ceiling {
        Ok(())
    } else {
        Err(SeatCeilingExceeded {
            regime,
            count,
            ceiling,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Parity of the seat-typed surface with `models/families/dpio.qnt` `dpio_lifecycle`:
    //! the regime-typed ceilings (`seatCeiling`), the seat census (`dpioSeatCount`), the
    //! seat-bound `-ERANGE` gate (`seatBoundRefusedTest`), the `1..=8` priorities gate
    //! (`createDpioAt`), the mode-dead capability judgment (`DPIO_I3_modeDead` /
    //! `noChannelReportsPrioritiesTest`), and the linted-enum trios (ADR-0014).
    //!
    //! `probeDrawTest`/`probeExhaustionDisabledTest` have NO api-side surface: the
    //! dpio→dpmcp probe draw (and its `-EPROBE_DEFER` exhaustion) is adapter-procedural
    //! (`dpaa2-mc`, phase 3; pool-objects design D4), so this tile carries no test for
    //! them — only [`DpioSeat::mark_probed`] records the observation flag, tested below.

    use super::*;

    fn kernel_seat_at(priorities: u8) -> DpioSeat {
        DpioSeat::create(
            DpioCfg {
                mode: ChannelMode::LocalChannel,
                priorities: Priorities::new(priorities).expect("in range"),
            },
            SeatRegime::KernelSeat,
        )
    }

    // ---- linted-enum trios (ADR-0014) ----

    #[test]
    fn channel_mode_variants_match_the_enum() {
        for m in ChannelMode::CHANNEL_MODES {
            assert!(CHANNEL_MODE_VARIANTS.contains(&m.name()), "{}", m.name());
        }
        assert_eq!(
            ChannelMode::CHANNEL_MODES.len(),
            CHANNEL_MODE_VARIANTS.len()
        );
    }

    #[test]
    fn seat_regime_variants_match_the_enum() {
        for r in SeatRegime::SEAT_REGIMES {
            assert!(SEAT_REGIME_VARIANTS.contains(&r.name()), "{}", r.name());
        }
        assert_eq!(SeatRegime::SEAT_REGIMES.len(), SEAT_REGIME_VARIANTS.len());
    }

    // ---- priorities range: 0 and 9 refused, 1 and 8 admitted (createDpioAt 1..8) ----

    #[test]
    fn priorities_gate_the_mc_range() {
        assert!(
            Priorities::new(0).is_err(),
            "0 has no MC-default sentinel here"
        );
        let refused = Priorities::new(9).expect_err("9 is out of range");
        let msg = refused.to_string();
        assert!(msg.contains("num_priorities"), "{msg}");
        assert!(msg.contains("1..=8"), "{msg}");
        assert_eq!(Priorities::new(1).expect("1 admits").get(), 1);
        assert_eq!(Priorities::new(8).expect("8 admits").get(), 8);
    }

    // ---- seatCeiling: kernel = cpus, dpdk = 2*threads (DPIO-I2, ADR-0012) ----

    #[test]
    fn seat_ceiling_is_regime_typed() {
        // Kernel container: one dpio per online CPU (board's dprc.1 holds 16 for 16 cores).
        assert_eq!(seat_ceiling(SeatRegime::KernelSeat, 16, 5), 16);
        // DPDK child: 2 per consumer thread (board runs 10 for T=5).
        assert_eq!(seat_ceiling(SeatRegime::DpdkSeat, 16, 5), 10);
        // The model's fixed NUM_CPUS=2 / NUM_THREADS=1 lift to parameters unchanged.
        assert_eq!(seat_ceiling(SeatRegime::KernelSeat, 2, 1), 2);
        assert_eq!(seat_ceiling(SeatRegime::DpdkSeat, 2, 1), 2);
    }

    // ---- dpioSeatCount: the census counts per regime ----

    #[test]
    fn seat_count_is_per_regime() {
        let kernel = kernel_seat_at(8);
        let dpdk = DpioSeat::create(
            DpioCfg {
                mode: ChannelMode::NoChannel,
                priorities: Priorities::new(8).expect("in range"),
            },
            SeatRegime::DpdkSeat,
        );
        let seats = [kernel, kernel, dpdk];
        assert_eq!(seat_count(&seats, SeatRegime::KernelSeat), 2);
        assert_eq!(seat_count(&seats, SeatRegime::DpdkSeat), 1);
        assert_eq!(seat_count(&[], SeatRegime::KernelSeat), 0);
    }

    // ---- derived_seats: the plan population of dpio in a container (ADR-0012 2·T) ----

    #[test]
    fn derived_seats_counts_dpio_by_container() {
        use crate::core::model::{DpmacId, MacMode};
        use crate::intent::refuse::compile;
        use crate::intent::{Dataplane, Intent, Isolation, Port, Tenant, TenantRef};
        use crate::testkit::ref_inventory;

        // A userspace-poll consumer terminating one 10G port (T = 1 + 2 = 3) draws 2·T
        // dpio into its own child, read straight off the plan (ADR-0012, not re-derived).
        let intent = Intent {
            tenants: vec![Tenant {
                name: "vpp".into(),
                dataplane: Dataplane::UserspacePoll,
                max_cores: 16,
                isolation: Isolation::Isolated,
                renamed: None,
            }],
            ports: vec![Port {
                name: "wan0".into(),
                dpmac: DpmacId::new(7),
                rate: 10_000,
                tenant: TenantRef::from_name("vpp".into()),
                mac: None,
                mac_mode: MacMode::Assert,
                renamed: None,
            }],
            ..Intent::empty()
        };
        let compiled = compile(&intent, &ref_inventory(16)).expect("intent compiles");
        let child = Container::Child("vpp".into());
        let expected = i64::try_from(
            compiled
                .plan
                .objects
                .iter()
                .filter(|o| o.container() == &child && o.key().family == Family::Dpio)
                .count(),
        )
        .unwrap();
        assert_eq!(derived_seats(&compiled.plan, &child), expected);
        assert_eq!(derived_seats(&compiled.plan, &child), 6); // 2·T, T = 3
    }

    // ---- seatBoundRefusedTest: kernel seats top out at the ceiling, next is -ERANGE ----

    #[test]
    fn seat_bound_refused_at_the_regime_ceiling() {
        // NUM_CPUS = 2: the first two kernel creates admit (0 < 2, 1 < 2).
        let none: [DpioSeat; 0] = [];
        assert!(admit_seat(&none, SeatRegime::KernelSeat, 2, 1).is_ok());
        let one = [kernel_seat_at(8)];
        assert!(admit_seat(&one, SeatRegime::KernelSeat, 2, 1).is_ok());
        // At the ceiling the third is refused, naming the regime and counts.
        let full = [kernel_seat_at(8), kernel_seat_at(8)];
        let refusal = admit_seat(&full, SeatRegime::KernelSeat, 2, 1).unwrap_err();
        assert_eq!(
            refusal,
            SeatCeilingExceeded {
                regime: SeatRegime::KernelSeat,
                count: 2,
                ceiling: 2,
            }
        );
        // Display renders regime and counts; it folds to Error::Config.
        let rendered = refusal.to_string();
        assert!(rendered.contains("KernelSeat"), "{rendered}");
        assert!(rendered.contains('2'), "{rendered}");
        let e: Error = refusal.into();
        assert!(matches!(e, Error::Config(_)));
    }

    #[test]
    fn seat_admits_below_the_ceiling_for_both_regimes() {
        // A DPDK regime at 2*threads: 3 seats below the ceiling 4 (threads=2) admit.
        let three = [
            DpioSeat::create(
                DpioCfg {
                    mode: ChannelMode::LocalChannel,
                    priorities: Priorities::new(8).expect("in range"),
                },
                SeatRegime::DpdkSeat,
            ),
            DpioSeat::create(
                DpioCfg {
                    mode: ChannelMode::LocalChannel,
                    priorities: Priorities::new(8).expect("in range"),
                },
                SeatRegime::DpdkSeat,
            ),
            DpioSeat::create(
                DpioCfg {
                    mode: ChannelMode::LocalChannel,
                    priorities: Priorities::new(8).expect("in range"),
                },
                SeatRegime::DpdkSeat,
            ),
        ];
        assert!(admit_seat(&three, SeatRegime::DpdkSeat, 2, 2).is_ok());
    }

    // ---- noChannelReportsPrioritiesTest / DPIO_I3_modeDead (DPIO-I3, Breaking) ----

    #[test]
    fn no_channel_still_reports_priorities() {
        // A NO_CHANNEL dpio at 8 priorities is still capable and 8-priority — the reported
        // count decides, the mode does not fold it away.
        let seat = DpioSeat::create(
            DpioCfg {
                mode: ChannelMode::NoChannel,
                priorities: Priorities::new(8).expect("in range"),
            },
            SeatRegime::KernelSeat,
        );
        assert!(seat.notify_capable());
        assert!(seat.eight_priority());
    }

    #[test]
    fn mode_is_dead_capability_reads_priorities() {
        // DPIO_I3_modeDead: flipping channel_mode never changes capability or 8-priority
        // status — capability is mode-independent for every priority count.
        for p in Priorities::MIN..=Priorities::MAX {
            let priorities = Priorities::new(p).expect("in range");
            let local = DpioSeat::create(
                DpioCfg {
                    mode: ChannelMode::LocalChannel,
                    priorities,
                },
                SeatRegime::KernelSeat,
            );
            let no_channel = DpioSeat::create(
                DpioCfg {
                    mode: ChannelMode::NoChannel,
                    priorities,
                },
                SeatRegime::KernelSeat,
            );
            assert_eq!(local.notify_capable(), no_channel.notify_capable());
            assert_eq!(local.eight_priority(), no_channel.eight_priority());
        }
    }

    // ---- notify_capable judgment (dpioNotifyCapable: num_priorities != 0) ----

    #[test]
    fn notify_capable_reads_the_priority_count() {
        // A validly created seat is always capable, since Priorities refines out 0; the
        // predicate reads the count (dpioNotifyCapable), not the mode.
        assert!(kernel_seat_at(1).notify_capable());
        assert!(kernel_seat_at(8).notify_capable());
        assert!(!kernel_seat_at(1).eight_priority());
        assert!(kernel_seat_at(8).eight_priority());
    }

    // ---- mark_probed: the observation flag flips, the cfg is untouched ----

    #[test]
    fn mark_probed_records_only_the_observation() {
        let mut seat = kernel_seat_at(8);
        assert!(!seat.probed());
        let cfg_before = seat.cfg();
        seat.mark_probed();
        assert!(seat.probed());
        assert_eq!(seat.cfg(), cfg_before); // cfg untouched — probed is observation, not cfg
    }
}
