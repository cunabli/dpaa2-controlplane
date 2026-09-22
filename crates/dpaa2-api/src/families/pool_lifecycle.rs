//! The P3 counted-companion surface for the allocator trio — the Rust twin of the
//! Quint substrate `models/families/pool_lifecycle.qnt` module `pool_lifecycle`
//! (ADR-0019 pattern P3; pool-objects design D1/D2).
//!
//! One generic shape over the family tag ([`PoolFamily`]) plus a runtime
//! [`Ceiling`], instantiated three times for dpbp,
//! dpmcp and dpcon — the model's `const FAMILY` + `const CEILING`, never a params
//! record (the trio's `FamilyParams` are byte-identical, so a params parameter would
//! be dead flexibility; pool-objects design D1). Authored model-first
//! (quint-is-the-spec): the census/sizing types and the convergence predicates are
//! structurally isomorphic to the Quint substrate, and the ADR-0002 §3 law binds them
//! — same counts, same judgments, names converging on readable English. The baseline
//! anchors are `docs/baseline/dpbp.md`, `docs/baseline/dpmcp.md` and
//! `docs/baseline/dpcon.md` (the census-floor refusal DPBP-I7, the never-reset dirty
//! return DPBP-I3/DPMCP-I3/DPCON-I5).
//!
//! # The count→individual boundary sits in the plan (pool-objects design D2)
//!
//! Everything here reasons in counts per (container, family): observed [`PoolCensus`]
//! versus intent-derived requirement ([`derived_requirement`]). No per-object identity
//! type is minted — companions carry no intent-side identity (they wear their
//! consumer's name, ADR-0015) — and no phase/typestate machinery or cross-pattern
//! trait appears (P1/P2 are different patterns by design, ADR-0019). Turning a count
//! delta into concrete create/destroy verbs against concrete ids is the adapter's job
//! (pool-objects design D2); this module speaks only counts and pure judgments.
//!
//! # The one documented shape divergence (ADR-0002 §3)
//!
//! The model expresses convergence as enabled/disabled machine actions
//! (`growEnabled`/`shrinkEnabled`) plus a boolean refusal flag written by
//! `shrinkBelowDrawAt`. This module carries no machine: its named predicates
//! [`PoolCensus::grow_enabled`], [`PoolCensus::shrink_enabled`],
//! [`PoolCensus::converged`] and [`PoolCensus::shrinks_below_draw`] map 1:1 onto
//! `growEnabled`, `shrinkEnabled`, `isConverged` and the `shrinkBelowDrawAt` guard —
//! pure booleans over the census and requirement, not state transitions
//! ([`PoolCensus::converged`] is the FULL `isConverged`: the managed count meets the
//! requirement AND no prune candidate remains, realized at the count level as
//! `foreign_free == 0`). The count-level disposition [`drift_disposition`] emits a
//! [`PoolDeltas`] of create/destroy/prune counts — each field a model action's firing
//! count — or the typed [`ShrinkBelowDraw`] refusal; turning a delta into concrete
//! create/destroy verbs against concrete ids is the adapter's job, and any
//! plan/`Transition` wiring is phase 3's (pool-objects design D2).

use std::collections::BTreeSet;

use crate::core::error::Error;
use crate::core::family::Family;
use crate::core::inventory::{Availability, Ceiling, judge_label};
use crate::core::model::ObjectRef;
use crate::core::types::{ConstructName, raw_capture};
use crate::intent::compiled::{CompiledPlan, Container};

/// The three pooled families that instantiate the one P3 allocator shape (`pooled:
/// true`; ADR-0019 P3 members minus dpio) — the type-level home of `pooled: true`.
///
/// dpio is deliberately absent: it is `pooled: false` (DPIO-I1), a seat-typed variant,
/// not pool custody, so it has no place in this trio. Its seat-typed surface is a
/// separate deliverable (pool-objects task 2.3), and the absence is witnessed by a
/// `compile_fail`:
///
/// ```compile_fail
/// use dpaa2_api::families::pool_lifecycle::PoolFamily;
/// // dpio is `pooled: false` (DPIO-I1): seat-typed, never pool custody, so it has no
/// // variant in this pooled trio — the seat-typed variant is pool-objects task 2.3.
/// let _ = PoolFamily::Dpio;
/// ```
///
/// The family→pattern table lint that keeps this trio honest against `FamilyParams` is
/// carried by bead dpaa2-controlplane-qtk (ADR-0014's named-bead form; the trio here is
/// a linted copy of the P3 `pooled: true` rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PoolFamily {
    /// Buffer pool (`dpbp.N`) — `FSL_MC_POOL_DPBP`.
    Dpbp,
    /// MC command portal (`dpmcp.N`) — `FSL_MC_POOL_DPMCP`.
    Dpmcp,
    /// Concentrator (`dpcon.N`) — `FSL_MC_POOL_DPCON`.
    Dpcon,
}

/// The [`PoolFamily`] variant names, in declaration order — the Rust copy of the three
/// `pooled: true` P3 rows (ADR-0014: an enumeration that restates the model is a linted
/// copy, kept honest by the exhaustive `match` in [`PoolFamily::name`]).
pub const POOL_FAMILY_VARIANTS: [&str; 3] = ["Dpbp", "Dpmcp", "Dpcon"];

impl PoolFamily {
    /// The whole pooled trio — the Rust copy of the P3 `pooled: true` member set (dpbp,
    /// dpmcp, dpcon; ADR-0019). A family outside this array has no variant, so the
    /// unpooled dpio is unrepresentable here by construct.
    pub const POOL_FAMILIES: [Self; 3] = [Self::Dpbp, Self::Dpmcp, Self::Dpcon];

    /// This variant's name, the token [`POOL_FAMILY_VARIANTS`] lists (ADR-0014).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dpbp => "Dpbp",
            Self::Dpmcp => "Dpmcp",
            Self::Dpcon => "Dpcon",
        }
    }

    /// The corpus-wide [`Family`] this pooled tag denotes — the bridge to the plan key
    /// and inventory, whose domains range over the full sixteen (`core::family`).
    #[must_use]
    pub const fn family(self) -> Family {
        match self {
            Self::Dpbp => Family::Dpbp,
            Self::Dpmcp => Family::Dpmcp,
            Self::Dpcon => Family::Dpcon,
        }
    }
}

/// The observed census of one (container, pool-family) pair — plain count data the
/// adapter feeds in phase 3, mirroring the model's count vocabulary (`pool_lifecycle`
/// `poolPopulation`/`freePool`/draw/`born`; ADR-0011).
///
/// `population` is the model's `poolPopulation` (every plugged member of the family in
/// the container, the census the ceiling gate judges); `free` and `drawn` split it by
/// custody (an undrawn member sits in the free pool, a drawn one is claimed by a
/// consumer); `born` is the DPL-born, free, structurally prune-exempt count (the model
/// anchor `born`, seeded free — a boot-baseline object is foreign, roadmap #14);
/// `foreign_free` is the free count whose label names neither a declared consumer nor the
/// DPL — undeclared capacity the prune reclaims; `born_drawn` is the DPL-born count that
/// reads *drawn* under the custody proxy — the board's plugged boot pool (V-POOL-6,
/// 2026-09-22), netted out of the draw guard so the DPL-born pool never masquerades as a
/// live consumer blocking a managed grow (pool-objects design D3: DPL-born stays untouched).
///
/// The label judgment is the count-level realization of the model's `managed` ghost set.
/// The model tracks each companion the reconciler grew in a `Set[ObjId]`; the count level
/// cannot, so declaredness is judged by label: a reconciler-created companion wears its
/// consumer's name (ADR-0015), [`judge_label`](crate::core::inventory)'s `"dpl"` sentinel
/// marks the DPL-born, and anything else is foreign — the same label-fingerprint
/// declaredness the [`plan::dprc`](crate::plan::dprc) `PruneBucket` decides prune candidacy
/// with. So the free pool splits three ways — `born`, `foreign_free`, and the reconciler's
/// own free managed — and `managed = population - born - foreign_free - born_drawn` counts
/// only the reconciler's companions (the model's `managedCount`), drawn ones included,
/// with BOTH DPL-born subsets (free `born` and plugged `born_drawn`) netted out. A *drawn*
/// foreign individual is count-indistinguishable from a drawn managed one and folds into
/// `drawn`/`managed`: conservative, biasing toward the [`ShrinkBelowDraw`] refusal, never
/// toward a teardown (pool-objects design D2). A *drawn* DPL-born is NOT folded in — it is
/// judged (empty label ⇒ DPL sentinel) and netted into `born_drawn`, so the model's
/// `drawnManaged` guard base is `drawn - born_drawn` (V-POOL-6; pool-objects design D3).
///
/// The invariant arithmetic (`free + drawn == population`, `0 <= born + foreign_free <= free`,
/// `0 <= born_drawn <= drawn`) is a debug assertion in [`PoolCensus::new`]: the adapter is the
/// trust boundary counting its own observation, so a violated invariant is a miscount to catch
/// in test/debug, not untrusted input to reject at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PoolCensus {
    population: i64,
    free: i64,
    drawn: i64,
    born: i64,
    foreign_free: i64,
    born_drawn: i64,
}

impl PoolCensus {
    /// Builds a census from an observation. `population` is every plugged member of the
    /// family in the container, split into `free` (undrawn, in the pool) and `drawn`
    /// (claimed by a consumer); `born` is the free DPL-born, prune-exempt subset; `born_drawn`
    /// is the DPL-born subset the custody proxy reads drawn (the board's plugged boot pool).
    ///
    /// The custody split (`free + drawn == population`), the free subsets
    /// (`0 <= born + foreign_free <= free`) and the drawn DPL-born subset
    /// (`0 <= born_drawn <= drawn`) are debug-asserted — a miscount is an adapter bug, not a
    /// runtime condition (see the type-level note).
    #[must_use]
    pub fn new(
        population: i64,
        free: i64,
        drawn: i64,
        born: i64,
        foreign_free: i64,
        born_drawn: i64,
    ) -> Self {
        debug_assert!(
            population >= 0
                && free >= 0
                && drawn >= 0
                && born >= 0
                && foreign_free >= 0
                && born_drawn >= 0,
            "census counts are non-negative"
        );
        debug_assert!(
            free + drawn == population,
            "custody split: free + drawn == population"
        );
        debug_assert!(
            born + foreign_free <= free,
            "the born and foreign-free subsets are free and disjoint (born + foreign_free <= free); a drawn one folds into drawn"
        );
        debug_assert!(
            born_drawn <= drawn,
            "the drawn DPL-born subset is drawn (born_drawn <= drawn)"
        );
        Self {
            population,
            free,
            drawn,
            born,
            foreign_free,
            born_drawn,
        }
    }

    /// The total plugged population (the model's `poolPopulation`) — the census the
    /// ceiling gate judges (ADR-0011).
    #[must_use]
    pub const fn population(self) -> i64 {
        self.population
    }

    /// The free-pool count (undrawn members).
    #[must_use]
    pub const fn free(self) -> i64 {
        self.free
    }

    /// The total drawn count (every plugged member under the custody proxy) — the accounting
    /// half of `free + drawn == population`. The model's `drawnManaged` guard base nets the
    /// plugged DPL-born out of this: see [`PoolCensus::drawn_managed`].
    #[must_use]
    pub const fn drawn(self) -> i64 {
        self.drawn
    }

    /// The drawn count the plugged DPL-born boot pool occupies (the board's V-POOL-6 pool) —
    /// netted out of the draw guard so the DPL-born never reads as a live consumer.
    #[must_use]
    pub const fn born_drawn(self) -> i64 {
        self.born_drawn
    }

    /// The drawn count that counts toward the draw guard — the model's `drawnManaged`
    /// (`drawn - born_drawn`). A drawn foreign folds in (conservative, biasing to the refusal);
    /// a drawn DPL-born does not (it is the board's boot pool, not a live consumer; V-POOL-6).
    #[must_use]
    pub const fn drawn_managed(self) -> i64 {
        self.drawn - self.born_drawn
    }

    /// The free DPL-born, prune-exempt count (the model anchor `born`; roadmap #14).
    #[must_use]
    pub const fn born(self) -> i64 {
        self.born
    }

    /// The free, undeclared, non-DPL-born count — the count-level realization of the
    /// model's prune target set (`prunable`; pool-objects design D3). It is the prune
    /// emission of [`drift_disposition`] and, when non-zero, the reason a census is not
    /// converged even at the managed requirement.
    #[must_use]
    pub const fn foreign_free(self) -> i64 {
        self.foreign_free
    }

    /// The reconciler-owned count — the model's `managedCount`: the population minus both
    /// DPL-born subsets (free `born` and plugged `born_drawn`) and the undeclared foreign-free,
    /// so neither the boot pool nor an out-of-band create ever counts toward the requirement.
    #[must_use]
    pub const fn managed(self) -> i64 {
        self.population - self.born - self.foreign_free - self.born_drawn
    }

    /// The free reconciler-owned count — the free pool minus the free DPL-born and the
    /// undeclared foreign-free, so neither the prune-exempt boot object nor a prune
    /// candidate is ever a shrink victim (the model's "a free *managed* individual exists"
    /// in `shrinkEnabled`; pool-objects design D3).
    #[must_use]
    pub const fn managed_free(self) -> i64 {
        self.free - self.born - self.foreign_free
    }

    /// Whether the population is below the ceiling, so a create is admitted — the
    /// model's `censusAdmitsCreate` (`poolPopulation < CEILING`; DPBP-I7, ADR-0011).
    /// The refusal is a disabled create, never a recorded over-ceiling state.
    ///
    /// [`Ceiling::Unknown`] has no model counterpart (the model's `CEILING` is always an
    /// int). This follows the intent-layer posture for an unknown ceiling: admit and
    /// warn, never refuse ([`Warning::UnknownCeiling`](crate::intent::refuse) in
    /// `intent::refuse`), so an unmeasured pool does not block convergence.
    #[must_use]
    pub fn admits_create(self, ceiling: &Ceiling) -> bool {
        match ceiling {
            Ceiling::Counted(n) | Ceiling::Observed { n, .. } => self.population < *n,
            Ceiling::Unknown => true,
        }
    }

    /// Grow is enabled at a deficit that the ceiling admits — the model's `growEnabled`
    /// (`managedCount < derivedReq and censusAdmitsCreate`; `pool_lifecycle` :106).
    #[must_use]
    pub fn grow_enabled(self, requirement: i64, ceiling: &Ceiling) -> bool {
        self.managed() < requirement && self.admits_create(ceiling)
    }

    /// Shrink is enabled at a surplus with a free reconciler-owned individual to
    /// destroy — the model's `shrinkEnabled` (`managedCount > derivedReq and` a free
    /// managed exists; `pool_lifecycle` :108). Free-only: the drawn population and the
    /// prune-exempt born are never candidates (pool-objects design D3).
    #[must_use]
    pub fn shrink_enabled(self, requirement: i64) -> bool {
        self.managed() > requirement && self.managed_free() > 0
    }

    /// Converged in full — the model's `isConverged` (`managedCount == derivedReq and
    /// not(anyPrunable)`; `pool_lifecycle` :116): the reconciler-owned count meets the
    /// requirement AND no prune candidate remains. The prune half is realized at the count
    /// level as `foreign_free == 0`, since a foreign-free member is exactly a `prunable`
    /// one (undeclared, non-DPL-born, free; pool-objects design D3). A converged census is
    /// the idempotence witness: [`drift_disposition`] emits an empty [`PoolDeltas`] over it.
    #[must_use]
    pub fn converged(self, requirement: i64) -> bool {
        self.managed() == requirement && self.foreign_free == 0
    }

    /// Whether the requirement has fallen below the managed drawn count — the refusal
    /// precondition a free-only shrink cannot meet (the model's `shrinkBelowDrawAt`
    /// guard `derivedReq < drawnManaged`; pool-objects design D3). The DPL-born boot pool is
    /// netted out ([`drawn_managed`](Self::drawn_managed)), so a plugged DPL-born pool never
    /// forces this refusal (V-POOL-6). A requirement below draw surfaces to the operator, never
    /// a teardown of a live consumer; emitting the typed refusal is the disposition's
    /// (pool-objects task 2.2).
    #[must_use]
    pub fn shrinks_below_draw(self, requirement: i64) -> bool {
        requirement < self.drawn_managed()
    }
}

/// The intent-derived requirement for one (container, pool-family) pair — the count of
/// planned companions of `family` in `container`, consumed unchanged from the compiled
/// plan (ADR-0012 counts; the `derive_consumer_containers` filter idiom,
/// `plan::dprc`). The count→requirement is a plain population of the plan, never a
/// re-derivation; the sizing rules already ran in the intent compiler.
#[must_use]
pub fn derived_requirement(plan: &CompiledPlan, container: &Container, family: PoolFamily) -> i64 {
    let fam = family.family();
    i64::try_from(
        plan.objects
            .iter()
            .filter(|o| o.container() == container && o.key().family == fam)
            .count(),
    )
    .unwrap_or(i64::MAX)
}

/// The typed refusal a requirement below the drawn count raises — the Rust twin of the
/// model's observable `refusal` flag written by `shrinkBelowDrawAt` (`pool_lifecycle`
/// :228; pool-objects design D3). A free-only shrink cannot reach a live consumer, so a
/// requirement under the drawn count surfaces to the operator by name and count, never a
/// forced teardown; [`drift_disposition`] returns it in place of any deltas.
///
/// Namespaced like the `plan::dprc` prune refusals (no `lib.rs` re-export — a P3 refusal
/// is a family concern, not a corpus-wide one). It folds into the crate error idiom via
/// [`From`] ⇒ [`Error::Config`], the [`DeadOptionRefusal`] idiom.
///
/// [`DeadOptionRefusal`]: crate::families::dpni::DeadOptionRefusal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct ShrinkBelowDraw {
    /// The family whose requirement fell below its draw.
    pub family: PoolFamily,
    /// The derived requirement (the model's `derivedReq`).
    pub requirement: i64,
    /// The managed drawn count the requirement fell below (the model's `drawnManaged`; a drawn
    /// foreign folds in, biasing conservative, but the DPL-born boot pool is netted out — see
    /// [`drift_disposition`] and [`PoolCensus::drawn_managed`]).
    pub drawn: i64,
}

impl core::fmt::Display for ShrinkBelowDraw {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} requirement {} is below the drawn count {}: a free-only shrink cannot reach a live consumer",
            self.family.name(),
            self.requirement,
            self.drawn
        )
    }
}

impl From<ShrinkBelowDraw> for Error {
    /// A shrink-below-draw refusal is a configuration boundary error — the crate error
    /// idiom (the sibling of the dpni [`DeadOptionRefusal`]'s [`Error::Config`]).
    ///
    /// [`DeadOptionRefusal`]: crate::families::dpni::DeadOptionRefusal
    fn from(refusal: ShrinkBelowDraw) -> Self {
        Error::Config(refusal.to_string())
    }
}

/// The count-level convergence emission for one (container, pool-family) pair — the Rust
/// twin of the model's three disposition actions, each field the count of one action's
/// firings needed to reach the derived requirement (pool-objects task 2.2; pool-objects design D2/D3).
///
/// Each field maps 1:1 onto a model action: `create` ↔ `growCreateAt` (`pool_lifecycle`
/// :197), `destroy` ↔ `shrinkDestroyAt` (:208, free managed only), `prune` ↔ `pruneAt`
/// (:219, undeclared ∧ non-DPL-born ∧ free); an empty deltas ([`is_empty`](Self::is_empty))
/// ↔ `isConverged` (:116), the idempotence witness. All three counts are non-negative.
///
/// Grow and prune can co-occur — a deficit standing alongside foreign free objects — so the
/// emission is a struct of counts, not a single-verdict enum: one pass both grows toward the
/// requirement and reclaims the undeclared.
///
/// # The count→individual boundary is the dispatch edge (pool-objects design D2)
///
/// These deltas speak only counts; the adapter resolves a delta to concrete ids. A `create`
/// mints that many companions wearing the consumer's name (ADR-0015). A `destroy` or `prune`
/// takes its victims from the *free* set only, selected arbitrarily — anonymity is the
/// pattern's truth, so which free individual goes is not a policy surface (pool-objects design D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct PoolDeltas {
    /// Creates for a deficit — the model's `growCreateAt` firing count, ceiling-capped.
    pub create: i64,
    /// Destroys of free managed individuals for a surplus — the model's `shrinkDestroyAt`
    /// firing count (a drawn individual is never a victim).
    pub destroy: i64,
    /// Prunes of undeclared, non-DPL-born, free objects — the model's `pruneAt` firing
    /// count (the census `foreign_free`).
    pub prune: i64,
}

impl PoolDeltas {
    /// Whether the pass emits nothing — the model's `isConverged`, and the idempotence
    /// witness: a converged census yields an empty deltas, and a second pass over it does
    /// too.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.create == 0 && self.destroy == 0 && self.prune == 0
    }
}

/// Decides the count-level convergence for one (container, pool-family) pair — the Rust
/// twin of the model's disposition actions `growCreateAt`/`shrinkDestroyAt`/`pruneAt`/
/// `shrinkBelowDrawAt` (`pool_lifecycle`; pool-objects design D3). Pure and total: it
/// observes nothing and drives nothing — the caller supplies the census, the derived
/// requirement, and the family ceiling.
///
/// A requirement below the drawn count is the one refusal: a free-only shrink cannot reach
/// a live consumer, so it returns [`ShrinkBelowDraw`] and emits no deltas (spec "emits no
/// destroy"; the model guard `derivedReq < drawnManaged`). The census `drawn` may include a
/// drawn foreign individual — count-indistinguishable from a drawn managed one — which
/// biases toward the refusal, never toward a teardown; that is the conservative direction
/// (pool-objects design D2).
///
/// Otherwise it emits [`PoolDeltas`]:
/// - `create` = the deficit (`requirement - managed`, floored at 0) capped by the ceiling
///   headroom (`ceiling - population` for [`Ceiling::Counted`]/[`Ceiling::Observed`],
///   unbounded for [`Ceiling::Unknown`] — the admit-and-warn posture). A ceiling-capped grow
///   converges the remainder on a later pass (level-triggered).
/// - `destroy` = the surplus (`managed - requirement`, floored at 0) capped by the free
///   managed count — free individuals only, the drawn ones are never candidates and a
///   surplus beyond the free count waits for returns.
/// - `prune` = the census `foreign_free` (undeclared ∧ non-DPL-born ∧ free); the born are
///   structurally exempt because they are not counted foreign.
///
/// Grow and prune co-occur when a deficit stands alongside foreign objects — that is why the
/// emission is a deltas struct, not a single verdict.
///
/// # Errors
/// Returns [`ShrinkBelowDraw`] when `requirement` falls below the census `drawn` count — a
/// free-only shrink cannot reach a live consumer, so the shortfall surfaces to the operator
/// rather than tearing one down. No deltas accompany the refusal.
pub fn drift_disposition(
    family: PoolFamily,
    census: PoolCensus,
    requirement: i64,
    ceiling: &Ceiling,
) -> Result<PoolDeltas, ShrinkBelowDraw> {
    if census.shrinks_below_draw(requirement) {
        return Err(ShrinkBelowDraw {
            family,
            requirement,
            drawn: census.drawn_managed(),
        });
    }
    let deficit = (requirement - census.managed()).max(0);
    let create = match ceiling {
        Ceiling::Counted(n) | Ceiling::Observed { n, .. } => {
            deficit.min((n - census.population()).max(0))
        }
        Ceiling::Unknown => deficit,
    };
    let destroy = (census.managed() - requirement)
        .max(0)
        .min(census.managed_free());
    Ok(PoolDeltas {
        create,
        destroy,
        prune: census.foreign_free(),
    })
}

// ---- the observation surface: raw rows in, a pure census out (pool-objects design D2/D3) ----

raw_capture! {
    /// The MC label column of a pool row, captured verbatim — any content is valid, the empty
    /// string included (the empty column is the DPL sentinel
    /// [`judge_label`](crate::core::inventory) reads as `"dpl"`).
    ///
    /// Deliberately NOT a [`ConstructName`]: a raw label is an *observation to be judged*, not
    /// a declared name to be reified — its invariant is verbatim capture, and it is judged
    /// through [`judge_label`](crate::core::inventory) (via
    /// [`ObservedPoolObject::membership`]), never validated or namespaced. Minted through the
    /// shared [`raw_capture!`](crate::core::types) capture contract it shares with
    /// [`RawDriver`].
    RawLabel
}

impl RawLabel {
    /// Whether the label column was empty — the DPL sentinel's shape. Family-specific, not
    /// part of the shared capture contract, so it stays a hand impl.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

raw_capture! {
    /// A sysfs driver-link basename bound to a device, captured verbatim — the kernel-face
    /// capture, colocated with [`RawLabel`] as the two `raw_capture!` mints; its judges live
    /// in `core::model`.
    ///
    /// Deliberately NOT a [`ConstructName`]: a bound driver is
    /// an *observation to be judged*, not a declared name to be reified — its invariant is
    /// verbatim capture. Presence is consulted by
    /// [`judge_bind_probe`](crate::core::model::judge_bind_probe) (a bound driver plus a netdev
    /// is [`BindProbe::Live`](crate::core::model::BindProbe::Live)); content is compared by
    /// [`VfioBind::classify`](crate::families::dprc::VfioBind::classify) against
    /// [`VFIO_FSL_MC_DRIVER`](crate::families::dprc::VFIO_FSL_MC_DRIVER). It validates nothing at
    /// construction — absence is `None` at the contract, so there is no empty-sentinel role — and
    /// is never namespaced. Minted through the shared [`raw_capture!`](crate::core::types)
    /// capture contract it shares with [`RawLabel`].
    RawDriver
}

/// One pool object as the adapter observed it — the raw `dprc show` row a pool family
/// contributes, reported verbatim (pool-objects task 3.1; the adapter reports, the core
/// judges — PASS5-F1/ADR-0010 §4).
///
/// `label` is the MC label column exactly as read (a [`RawLabel`]): the empty string is
/// the empty column, which [`judge_label`](crate::core::inventory) reads as the DPL
/// sentinel `"dpl"` — never pre-judged into ours/foreign here. `plugged` is the trailing
/// `plugged`/`unplugged` state token.
///
/// # The custody proxy is conservative and restool-bound (DPBP-I2/I3/I4)
///
/// Kernel free/drawn custody is NOT restool-observable: an object enters its container's
/// kernel pool only once the `fsl_mc_allocator` binds it, and the allocator's free/drawn
/// split has no `dprc show` column (`docs/baseline/dpbp.md` "Kernel-side behavior";
/// DPBP-I2's kernel-pool half is root-only until the raw command path, #10). So
/// [`census_of`] uses the one custody signal restool DOES surface — the plugged state —
/// as a conservative proxy: **plugged ⇒ counted drawn** (it may merely be kernel-held,
/// not consumer-drawn), **unplugged ⇒ free**. `born` and `foreign_free` are then the
/// unplugged rows the DPL / a foreign owner labels; a *plugged* DPL-born row is judged the
/// same way (empty label ⇒ DPL sentinel) into `born_drawn` and netted back out of the draw
/// guard, so the board's plugged boot pool never reads as a live consumer (V-POOL-6;
/// pool-objects design D3: DPL-born stays untouched).
///
/// The consequence, stated plainly (pool-objects design D3): the conservative fold now
/// applies to *foreign*-drawn only — a plugged foreign *surplus* the reconciler could in
/// principle reclaim surfaces as a [`ShrinkBelowDraw`] refusal rather than a teardown,
/// because the proxy cannot tell it from a live draw. That is the safe direction — never
/// tear down something that might be kernel-held — and MC's own driver-bound destroy
/// refusal (a typed `McStatus`; `docs/baseline/dpbp.md`: `destroy` refuses driver-bound
/// objects) is the enforcement backstop. The phase-4 kernel-face probes refine the proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedPoolObject {
    /// The concrete `family.ordinal` reference the destroy/prune verbs address.
    pub object: ObjectRef,
    /// The raw MC label column, verbatim (empty for the empty column ⇒ the DPL sentinel).
    pub label: RawLabel,
    /// Whether the row's trailing state token was `plugged` (the drawn proxy) vs
    /// `unplugged` (free).
    pub plugged: bool,
}

/// Pool-custody membership of one observed object, judged by its label against the
/// declared-name set — the individual-level twin of the count-level `born`/`foreign_free`
/// split (pool-objects design D3). It reuses [`judge_label`](crate::core::inventory), the
/// single home of the empty-label⇒DPL idiom, so the classification never drifts from the
/// inventory's (ADR-0010 §4 refined by ADR-0015).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolMembership {
    /// Reconciler-owned: the label is a declared consumer name (the model's `managed`
    /// set) — a free one is a shrink victim, a drawn one counts toward the requirement.
    Managed,
    /// DPL-born boot baseline (the empty-label ⇒ `"dpl"` sentinel; roadmap #14) —
    /// structurally prune-exempt and never a shrink victim.
    DplBorn,
    /// Undeclared and non-DPL-born — the prune target when free (pool-objects design D3).
    Foreign,
}

impl ObservedPoolObject {
    /// Whether the object reads as free — the unplugged proxy for undrawn custody (see the
    /// type-level DPBP-I2 note).
    #[must_use]
    pub fn is_free(&self) -> bool {
        !self.plugged
    }

    /// Judges this object's custody membership against the declared-name set, reusing the
    /// inventory's [`judge_label`](crate::core::inventory) so the empty-label⇒DPL and
    /// declared⇒ours idioms are single-sourced (ADR-0010 §4 refined by ADR-0015). An empty
    /// label is the empty column, judged as the DPL sentinel.
    #[must_use]
    pub fn membership(&self, declared: &BTreeSet<ConstructName>) -> PoolMembership {
        let judged = judge_label(self.label.as_str(), declared);
        // Compare against judge_label's own empty⇒DPL verdict, never re-spelling "dpl" here.
        if judged == judge_label("", declared) {
            PoolMembership::DplBorn
        } else if matches!(judged, Availability::Free) {
            PoolMembership::Managed
        } else {
            PoolMembership::Foreign
        }
    }
}

/// Assembles the count-level [`PoolCensus`] from the raw observed rows of one (container,
/// pool-family) pair — the pure boundary between the adapter's verbatim observation and
/// the model's count vocabulary (pool-objects task 3.1; pool-objects design D2/D3).
///
/// The custody split is the conservative restool proxy (see [`ObservedPoolObject`]):
/// every `plugged` row counts `drawn`, every `unplugged` row `free`; a free row then adds
/// to `born` (DPL) or `foreign_free` (undeclared) per its [`membership`](ObservedPoolObject::membership),
/// or to neither when it is the reconciler's own managed-free. A *plugged* DPL-born row
/// additionally adds to `born_drawn`, netting the board's boot pool out of the draw guard
/// (V-POOL-6; pool-objects design D3). `declared` is the same declared-name recognition set
/// the inventory judges labels against (ADR-0015), so the count-level `born`/`born_drawn`/
/// `foreign_free` match the inventory's per-object verdicts exactly.
#[must_use]
pub fn census_of(rows: &[ObservedPoolObject], declared: &BTreeSet<ConstructName>) -> PoolCensus {
    let population = i64::try_from(rows.len()).unwrap_or(i64::MAX);
    let mut free = 0i64;
    let mut drawn = 0i64;
    let mut born = 0i64;
    let mut foreign_free = 0i64;
    let mut born_drawn = 0i64;
    for row in rows {
        if row.plugged {
            drawn += 1; // conservative proxy: plugged ⇒ drawn (see ObservedPoolObject; DPBP-I2/I3/I4)
            if row.membership(declared) == PoolMembership::DplBorn {
                born_drawn += 1; // a plugged DPL-born nets out of the draw guard (V-POOL-6; pool-objects design D3)
            }
        } else {
            free += 1;
            match row.membership(declared) {
                PoolMembership::DplBorn => born += 1,
                PoolMembership::Foreign => foreign_free += 1,
                PoolMembership::Managed => {}
            }
        }
    }
    PoolCensus::new(population, free, drawn, born, foreign_free, born_drawn)
}

/// The kernel-poolable count of an observed pool family — the plugged rows only. A pool
/// entry is kernel-poolable exactly when it is plugged AND the `fsl_mc_allocator` has bound
/// it, and the allocator binds only plugged objects (an unplugged pool object is visible in
/// `dprc show` but invisible to the allocator; DPBP-I2, `docs/baseline/dpbp.md` :74-77). So
/// the plugged count is the kernel-poolable census the bind-satisfiability judgment reads
/// (ADR-0011: judge against the census) — the count-level twin of the model's `freePool`
/// size in `core/pools.qnt` `drawSatisfiable`, which [`judge_bind_probe`] compares against a
/// family's draw.
///
/// [`judge_bind_probe`]: crate::core::model::judge_bind_probe
#[must_use]
pub fn poolable(rows: &[ObservedPoolObject]) -> i64 {
    i64::try_from(rows.iter().filter(|r| r.plugged).count()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    //! Parity of the census/sizing/convergence surface with
    //! `models/families/pool_lifecycle.qnt` `pool_lifecycle`: the ceiling gate
    //! (`censusRefusesAtCeilingTest`), the population arithmetic, converged and
    //! shrink-below-draw detection, the `Ceiling::Unknown` admit posture, and trio
    //! symmetry (one shape, three tags). The `compile_fail` witness that dpio has no
    //! variant lives on the [`PoolFamily`] rustdoc, not duplicated here.
    //!
    //! The `drift_disposition` tests mirror the phase-1 directed runs of
    //! `pool_lifecycle.qnt` by name: `idempotentReconvergeTest`, `convergenceGrowTest`,
    //! `freeOnlyShrinkTest`, `drawnNeverShrunkTest`, `shrinkBelowDrawRefusedTest` and
    //! `prunePreservesDplBornTest`, plus the ceiling cap, grow+prune co-occurrence, and a
    //! total-function edge sweep the model's simulate-only runs do not enumerate.

    use super::*;

    // ---- linted-enum trio parity (ADR-0014) ----

    #[test]
    fn pool_family_variants_match_the_enum() {
        for f in PoolFamily::POOL_FAMILIES {
            assert!(POOL_FAMILY_VARIANTS.contains(&f.name()), "{}", f.name());
        }
        assert_eq!(PoolFamily::POOL_FAMILIES.len(), POOL_FAMILY_VARIANTS.len());
        let mut seen = POOL_FAMILY_VARIANTS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen.len(),
            POOL_FAMILY_VARIANTS.len(),
            "duplicate variant name"
        );
    }

    #[test]
    fn pool_family_maps_to_the_corpus_family() {
        assert_eq!(PoolFamily::Dpbp.family(), Family::Dpbp);
        assert_eq!(PoolFamily::Dpmcp.family(), Family::Dpmcp);
        assert_eq!(PoolFamily::Dpcon.family(), Family::Dpcon);
    }

    // ---- population arithmetic ----

    #[test]
    fn managed_excludes_the_free_born_and_the_split_holds() {
        // population = free + drawn; managed = population - born - foreign_free;
        // managed_free = free - born - foreign_free.
        let c = PoolCensus::new(5, 4, 1, 1, 1, 0);
        assert_eq!(c.population(), 5);
        assert_eq!(c.free(), 4);
        assert_eq!(c.drawn(), 1);
        assert_eq!(c.born(), 1);
        assert_eq!(c.foreign_free(), 1);
        assert_eq!(c.managed(), 3); // 5 - 1 born - 1 foreign_free
        assert_eq!(c.managed_free(), 2); // 4 free - 1 born - 1 foreign_free
    }

    // ---- the ceiling gate (censusRefusesAtCeilingTest; DPBP-I7, ADR-0011) ----

    #[test]
    fn census_refuses_at_ceiling_admits_below() {
        // At the ceiling a create is disabled; below it is admitted (born + 2 == CEILING 3).
        let ceiling = Ceiling::Counted(3);
        assert!(
            PoolCensus::new(2, 2, 0, 0, 0, 0).admits_create(&ceiling),
            "below admits"
        );
        assert!(
            !PoolCensus::new(3, 3, 0, 0, 0, 0).admits_create(&ceiling),
            "at ceiling refuses"
        );
        // An Observed ceiling gates the same way.
        let obs = Ceiling::Observed {
            n: 3,
            provenance: "test".to_owned(),
        };
        assert!(PoolCensus::new(2, 2, 0, 0, 0, 0).admits_create(&obs));
        assert!(!PoolCensus::new(3, 3, 0, 0, 0, 0).admits_create(&obs));
    }

    #[test]
    fn unknown_ceiling_admits_with_no_refusal() {
        // Ceiling::Unknown has no model counterpart: admit-and-warn, never refuse.
        let c = PoolCensus::new(99, 99, 0, 0, 0, 0);
        assert!(c.admits_create(&Ceiling::Unknown));
        assert!(
            c.grow_enabled(200, &Ceiling::Unknown),
            "grow is never ceiling-blocked when Unknown"
        );
    }

    // ---- convergence predicates (growEnabled/shrinkEnabled/isConverged) ----

    #[test]
    fn grow_enabled_at_a_deficit_the_ceiling_admits() {
        let ceiling = Ceiling::Counted(3);
        // Deficit (managed 0 < req 2) and below ceiling ⇒ grow.
        assert!(PoolCensus::new(1, 1, 0, 1, 0, 0).grow_enabled(2, &ceiling));
        // At the requirement ⇒ no grow.
        assert!(!PoolCensus::new(3, 3, 0, 1, 0, 0).grow_enabled(2, &ceiling)); // managed 2 == req 2
        // Deficit but at ceiling ⇒ no grow (the census gate wins).
        assert!(!PoolCensus::new(3, 3, 0, 0, 0, 0).grow_enabled(5, &ceiling));
    }

    #[test]
    fn shrink_enabled_only_with_a_free_managed_individual() {
        // Surplus (managed 2 > req 1) with a free managed ⇒ shrink.
        assert!(PoolCensus::new(2, 2, 0, 0, 0, 0).shrink_enabled(1));
        // Surplus but every managed is drawn (free is born only) ⇒ no free victim.
        assert!(!PoolCensus::new(3, 1, 2, 1, 0, 0).shrink_enabled(1)); // managed 2 > 1, managed_free 0
        // At the requirement ⇒ no shrink.
        assert!(!PoolCensus::new(2, 2, 0, 0, 0, 0).shrink_enabled(2));
    }

    #[test]
    fn converged_when_managed_meets_the_requirement() {
        // Idempotent target: managed == req, and neither grow nor shrink fires.
        let c = PoolCensus::new(2, 2, 0, 1, 0, 0); // managed 1
        assert!(c.converged(1));
        assert!(!c.grow_enabled(1, &Ceiling::Counted(3)));
        assert!(!c.shrink_enabled(1));
        assert!(!PoolCensus::new(2, 2, 0, 0, 0, 0).converged(1)); // managed 2 != 1
    }

    #[test]
    fn shrink_below_draw_is_requirement_under_drawn() {
        // req below the drawn count surfaces the refusal precondition.
        assert!(PoolCensus::new(2, 0, 2, 0, 0, 0).shrinks_below_draw(1)); // 1 < drawn 2
        assert!(!PoolCensus::new(2, 1, 1, 0, 0, 0).shrinks_below_draw(1)); // 1 == drawn 1, not below
    }

    // ---- trio symmetry: one shape, three tags ----

    #[test]
    fn every_pool_family_obeys_the_same_laws() {
        // The generic shape is tag-invariant: the same census drives the same judgments
        // for every pooled family (pool-objects design D1).
        let ceiling = Ceiling::Counted(3);
        for f in PoolFamily::POOL_FAMILIES {
            assert_eq!(f.family().as_str(), f.name().to_lowercase());
            let deficit = PoolCensus::new(1, 1, 0, 1, 0, 0); // managed 0, req 2
            assert!(deficit.grow_enabled(2, &ceiling), "{}", f.name());
            let surplus = PoolCensus::new(2, 2, 0, 0, 0, 0); // managed 2, req 1
            assert!(surplus.shrink_enabled(1), "{}", f.name());
            let converged = PoolCensus::new(1, 1, 0, 0, 0, 0); // managed 1
            assert!(converged.converged(1), "{}", f.name());
            let over_draw = PoolCensus::new(2, 0, 2, 0, 0, 0);
            assert!(over_draw.shrinks_below_draw(1), "{}", f.name());
        }
    }

    // ---- derived_requirement against a compiled plan ----

    #[test]
    fn derived_requirement_counts_companions_by_container_and_family() {
        use crate::core::model::{DpmacId, MacMode};
        use crate::intent::refuse::compile;
        use crate::intent::{Dataplane, Intent, Isolation, Port, Tenant, TenantRef};
        use crate::testkit::ref_inventory;

        // A userspace-poll consumer with a terminated port compiles to a plan carrying
        // its pooled companions in its own child container.
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

        // The requirement mirrors the plan's companion population, family by family, and
        // is read straight off the plan (ADR-0012, consumed unchanged).
        for f in PoolFamily::POOL_FAMILIES {
            let expected = i64::try_from(
                compiled
                    .plan
                    .objects
                    .iter()
                    .filter(|o| o.container() == &child && o.key().family == f.family())
                    .count(),
            )
            .unwrap();
            assert_eq!(
                derived_requirement(&compiled.plan, &child, f),
                expected,
                "{}",
                f.name()
            );
        }
        // A userspace-poll consumer draws at least one dpbp (ADR-0012), so the plan is
        // non-empty for the trio — the count is a real population, not a vacuous zero.
        assert!(derived_requirement(&compiled.plan, &child, PoolFamily::Dpbp) > 0);
    }

    // ---- drift_disposition: the count-level convergence (mirrors the phase-1 runs) ----

    // idempotentReconvergeTest: a converged census emits nothing, and a second pass over
    // the same census emits nothing again (idempotent + level-triggered).
    #[test]
    fn drift_disposition_is_empty_over_a_converged_census() {
        let c = PoolCensus::new(2, 2, 0, 0, 0, 0); // managed 2, no foreign
        let d = drift_disposition(PoolFamily::Dpbp, c, 2, &Ceiling::Counted(3)).expect("converged");
        assert!(d.is_empty(), "{d:?}");
        let d2 =
            drift_disposition(PoolFamily::Dpbp, c, 2, &Ceiling::Counted(3)).expect("converged");
        assert_eq!(d, d2);
        assert!(d2.is_empty());
    }

    // convergenceGrowTest: a deficit emits creates to the derived count, nothing else.
    #[test]
    fn drift_disposition_grows_to_the_deficit() {
        let c = PoolCensus::new(0, 0, 0, 0, 0, 0); // managed 0
        let d = drift_disposition(PoolFamily::Dpbp, c, 2, &Ceiling::Counted(3)).expect("grows");
        assert_eq!(
            d,
            PoolDeltas {
                create: 2,
                destroy: 0,
                prune: 0
            }
        );
    }

    // The grow is capped at the ceiling headroom; Ceiling::Unknown never caps
    // (admit-and-warn). A capped grow leaves the remainder for a later pass.
    #[test]
    fn drift_disposition_caps_grow_at_headroom_and_unknown_is_uncapped() {
        let c = PoolCensus::new(1, 1, 0, 0, 0, 0); // managed 1, population 1
        // Counted(3): deficit 4, headroom 3-1=2 ⇒ create 2.
        let capped = drift_disposition(PoolFamily::Dpbp, c, 5, &Ceiling::Counted(3)).expect("caps");
        assert_eq!(capped.create, 2);
        let obs = Ceiling::Observed {
            n: 3,
            provenance: "test".to_owned(),
        };
        assert_eq!(
            drift_disposition(PoolFamily::Dpbp, c, 5, &obs)
                .expect("caps")
                .create,
            2
        );
        // Unknown is never ceiling-blocked: the full deficit of 4.
        let uncapped =
            drift_disposition(PoolFamily::Dpbp, c, 5, &Ceiling::Unknown).expect("uncapped");
        assert_eq!(uncapped.create, 4);
    }

    // freeOnlyShrinkTest: a surplus emits destroys, bounded by the free managed count.
    #[test]
    fn drift_disposition_shrinks_the_surplus() {
        let c = PoolCensus::new(3, 3, 0, 0, 0, 0); // managed 3, all free
        let d = drift_disposition(PoolFamily::Dpbp, c, 2, &Ceiling::Counted(9)).expect("shrinks");
        assert_eq!(
            d,
            PoolDeltas {
                create: 0,
                destroy: 1,
                prune: 0
            }
        );
    }

    // drawnNeverShrunkTest: a surplus with some drawn (but requirement >= draw) shrinks only
    // through free individuals — destroy never exceeds the free managed count.
    #[test]
    fn drift_disposition_destroy_is_bounded_by_free_managed() {
        let c = PoolCensus::new(4, 3, 1, 0, 0, 0); // managed 4, free 3, drawn 1
        let d = drift_disposition(PoolFamily::Dpbp, c, 2, &Ceiling::Counted(9)).expect("shrinks");
        assert_eq!(d.destroy, 2); // surplus 2, and 2 <= managed_free 3
        assert!(
            d.destroy <= c.managed_free(),
            "destroy stays within the free managed set"
        );
    }

    // shrinkBelowDrawRefusedTest: a requirement below the drawn count refuses by name and
    // count, emits no deltas, renders, and folds to Error::Config.
    #[test]
    fn drift_disposition_refuses_below_draw() {
        let c = PoolCensus::new(2, 0, 2, 0, 0, 0); // drawn 2
        let err = drift_disposition(PoolFamily::Dpcon, c, 1, &Ceiling::Counted(3)).unwrap_err();
        assert_eq!(
            err,
            ShrinkBelowDraw {
                family: PoolFamily::Dpcon,
                requirement: 1,
                drawn: 2
            }
        );
        let rendered = err.to_string();
        assert!(rendered.contains("Dpcon"), "{rendered}");
        assert!(
            rendered.contains('1') && rendered.contains('2'),
            "{rendered}"
        );
        let e: Error = err.into();
        assert!(matches!(e, Error::Config(_)));
    }

    // prunePreservesDplBornTest: a foreign-free member is pruned, the DPL-born is not (it is
    // not counted foreign, so it never enters the prune emission).
    #[test]
    fn drift_disposition_prunes_foreign_never_born() {
        let c = PoolCensus::new(3, 3, 0, 1, 1, 0); // managed 1, born 1, foreign 1
        let d = drift_disposition(PoolFamily::Dpbp, c, 1, &Ceiling::Counted(9)).expect("prunes");
        assert_eq!(
            d,
            PoolDeltas {
                create: 0,
                destroy: 0,
                prune: 1
            }
        );
        assert_eq!(d.prune, c.foreign_free()); // exactly the foreign-free, the born untouched
        assert!(!d.is_empty());
    }

    // Grow and prune co-occur: a deficit standing alongside foreign free objects emits both
    // in one pass — why the emission is a deltas struct, not a single verdict.
    #[test]
    fn drift_disposition_grows_and_prunes_together() {
        let c = PoolCensus::new(2, 2, 0, 0, 1, 0); // managed 1, one foreign-free
        let d =
            drift_disposition(PoolFamily::Dpbp, c, 3, &Ceiling::Counted(5)).expect("grow+prune");
        assert_eq!(
            d,
            PoolDeltas {
                create: 2,
                destroy: 0,
                prune: 1
            }
        );
    }

    // Trio symmetry: one shape, three tags — the same census drives the same deltas for
    // every pooled family (pool-objects design D1).
    #[test]
    fn drift_disposition_is_tag_invariant() {
        let c = PoolCensus::new(2, 2, 0, 0, 1, 0); // managed 1, one foreign-free
        let mut seen = None;
        for f in PoolFamily::POOL_FAMILIES {
            let d = drift_disposition(f, c, 3, &Ceiling::Counted(5)).expect("ok");
            match seen {
                None => seen = Some(d),
                Some(prev) => assert_eq!(prev, d, "{}", f.name()),
            }
        }
    }

    // Total-function property: a handful of edge censuses return (Ok or the refusal) without
    // panic, and any deltas emitted are non-negative.
    #[test]
    fn drift_disposition_is_total_over_edge_censuses() {
        let edges = [
            PoolCensus::new(0, 0, 0, 0, 0, 0), // all zeros
            PoolCensus::new(2, 0, 2, 0, 0, 0), // requirement 0 with drawn > 0 ⇒ refusal
            PoolCensus::new(2, 2, 0, 0, 2, 0), // foreign-only pool
            PoolCensus::new(3, 3, 0, 1, 1, 0), // born + foreign together
        ];
        for c in edges {
            for ceiling in [Ceiling::Counted(2), Ceiling::Unknown] {
                match drift_disposition(PoolFamily::Dpmcp, c, 0, &ceiling) {
                    Ok(d) => assert!(
                        d.create >= 0 && d.destroy >= 0 && d.prune >= 0,
                        "non-negative deltas for {c:?}"
                    ),
                    Err(refusal) => assert_eq!(refusal.family, PoolFamily::Dpmcp),
                }
            }
        }
    }

    // ---- census_of: raw rows fold into the count vocabulary (pool-objects design D3) ----

    fn declared_set(names: &[&str]) -> BTreeSet<ConstructName> {
        names.iter().map(|n| ConstructName::from(*n)).collect()
    }

    fn pool_row(ord: u32, raw: &str, plugged: bool) -> ObservedPoolObject {
        ObservedPoolObject {
            object: ObjectRef::new(Family::Dpbp, ord),
            label: RawLabel::from(raw),
            plugged,
        }
    }

    // The observation mapping (DPBP-I2/I3/I4), one row per case (see the trailing labels).
    #[test]
    fn census_of_maps_born_foreign_free_and_the_custody_proxy() {
        let declared = declared_set(&["vpp"]);
        let rows = vec![
            pool_row(0, "", false),       // empty ⇒ DPL, free ⇒ born
            pool_row(1, "vpp", false),    // declared ⇒ ours, free ⇒ managed-free
            pool_row(2, "vpp", true),     // declared ⇒ ours, plugged ⇒ drawn
            pool_row(3, "vendor", false), // foreign, free ⇒ foreign_free
            pool_row(4, "vendor", true),  // foreign, plugged ⇒ drawn (proxy)
        ];
        let c = census_of(&rows, &declared);
        assert_eq!(c.population(), 5);
        assert_eq!(c.free(), 3); // rows 0, 1, 3
        assert_eq!(c.drawn(), 2); // rows 2, 4
        assert_eq!(c.born(), 1); // row 0
        assert_eq!(c.foreign_free(), 1); // row 3
        assert_eq!(c.born_drawn(), 0); // no plugged DPL-born row here
        assert_eq!(c.managed_free(), 1); // row 1: free 3 - born 1 - foreign_free 1
        assert_eq!(c.managed(), 3); // population 5 - born 1 - foreign_free 1 - born_drawn 0
        assert_eq!(c.free() + c.drawn(), c.population());
    }

    // V-POOL-6 (board sitting 2026-09-22): the root container's plugged DPL-born boot pool
    // (empty label, plugged) must NOT count toward the draw guard — the managed requirement
    // grows on TOP of it (pool-objects design D3). A census of 52 plugged DPL-born rows with a
    // requirement of 19 plans a grow of 19 and raises NO ShrinkBelowDraw refusal.
    #[test]
    fn plugged_dpl_born_nets_out_of_the_draw_guard() {
        let declared = declared_set(&[]); // the DPL boot pool wears no declared consumer name
        let rows: Vec<ObservedPoolObject> = (0..52).map(|n| pool_row(n, "", true)).collect();
        let c = census_of(&rows, &declared);
        assert_eq!(c.population(), 52);
        assert_eq!(c.drawn(), 52); // the custody proxy still counts every plugged row drawn
        assert_eq!(c.born_drawn(), 52); // ...but all 52 are judged DPL-born and netted out
        assert_eq!(c.drawn_managed(), 0); // so the draw guard sees no live consumer
        assert_eq!(c.managed(), 0); // and the boot pool counts nothing toward the requirement
        assert!(
            !c.shrinks_below_draw(19),
            "the DPL-born pool never forces the refusal"
        );
        let d = drift_disposition(PoolFamily::Dpmcp, c, 19, &Ceiling::Counted(80))
            .expect("a grow, not a refusal");
        assert_eq!(
            d,
            PoolDeltas {
                create: 19,
                destroy: 0,
                prune: 0
            }
        );
    }

    // Companion to V-POOL-6: a plugged FOREIGN pool (undeclared, non-empty label) is NOT netted
    // — count-indistinguishable from a live draw, it stays folded into the draw guard and still
    // biases to the ShrinkBelowDraw refusal (the conservative direction; pool-objects design D2).
    #[test]
    fn plugged_foreign_still_biases_to_the_refusal() {
        let declared = declared_set(&["vpp"]);
        let rows: Vec<ObservedPoolObject> = (0..52).map(|n| pool_row(n, "vendor", true)).collect();
        let c = census_of(&rows, &declared);
        assert_eq!(c.drawn(), 52);
        assert_eq!(c.born_drawn(), 0); // a foreign label is not the DPL sentinel
        assert_eq!(c.drawn_managed(), 52); // foreign-drawn folds in, conservatively
        assert!(c.shrinks_below_draw(19));
        let err = drift_disposition(PoolFamily::Dpmcp, c, 19, &Ceiling::Counted(80)).unwrap_err();
        assert_eq!(
            err,
            ShrinkBelowDraw {
                family: PoolFamily::Dpmcp,
                requirement: 19,
                drawn: 52
            }
        );
    }

    #[test]
    fn census_of_over_an_all_free_managed_pool_is_convergeable() {
        // All rows ours and free ⇒ born/foreign_free zero, so it converges at the managed count.
        let declared = declared_set(&["vpp"]);
        let rows = vec![pool_row(0, "vpp", false), pool_row(1, "vpp", false)];
        let c = census_of(&rows, &declared);
        assert_eq!(c.born(), 0);
        assert_eq!(c.foreign_free(), 0);
        assert_eq!(c.managed(), 2);
        assert!(c.converged(2));
    }

    // poolable counts the plugged rows only — the allocator-bound census (DPBP-I2).
    #[test]
    fn poolable_counts_plugged_only() {
        let rows = vec![
            pool_row(0, "vpp", true),  // plugged ⇒ allocator-bound ⇒ poolable
            pool_row(1, "vpp", false), // unplugged ⇒ invisible to the allocator
            pool_row(2, "", true),     // plugged DPL-born ⇒ still poolable
        ];
        assert_eq!(poolable(&rows), 2);
        assert_eq!(poolable(&[]), 0);
    }

    // membership reuses judge_label: an empty label ⇒ DPL, declared ⇒ ours, else foreign.
    #[test]
    fn membership_reuses_the_label_judgment() {
        let declared = declared_set(&["wan0"]);
        assert_eq!(
            pool_row(0, "", false).membership(&declared),
            PoolMembership::DplBorn
        );
        assert_eq!(
            pool_row(0, "wan0", false).membership(&declared),
            PoolMembership::Managed
        );
        assert_eq!(
            pool_row(0, "vendor", false).membership(&declared),
            PoolMembership::Foreign
        );
    }
}
