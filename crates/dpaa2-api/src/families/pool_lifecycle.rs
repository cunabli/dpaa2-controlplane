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
//! pure booleans over the census and requirement, not state transitions. The
//! delta-emitting disposition, prune classification and any plan/`Transition` wiring
//! are the reconciler's (pool-objects task 2.2); these predicates are 2.2's
//! isomorphism surface, not their own.

use crate::core::family::Family;
use crate::core::inventory::Ceiling;
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
/// anchor `born`, seeded free — a boot-baseline object is foreign, roadmap #14). Born
/// members are free (`born <= free`) and are excluded from the count that meets the
/// requirement (`managed = population - born`), exactly as the model's `managedCount`
/// counts only the reconciler's own companions, never the boot object.
///
/// The invariant arithmetic (`free + drawn == population`, `0 <= born <= free`) is a
/// debug assertion in [`PoolCensus::new`]: the adapter is the trust boundary counting
/// its own observation, so a violated invariant is a miscount to catch in test/debug,
/// not untrusted input to reject at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PoolCensus {
    population: i64,
    free: i64,
    drawn: i64,
    born: i64,
}

impl PoolCensus {
    /// Builds a census from an observation. `population` is every plugged member of the
    /// family in the container, split into `free` (undrawn, in the pool) and `drawn`
    /// (claimed by a consumer); `born` is the free DPL-born, prune-exempt subset.
    ///
    /// The custody split (`free + drawn == population`) and the born subset
    /// (`0 <= born <= free`) are debug-asserted — a miscount is an adapter bug, not a
    /// runtime condition (see the type-level note).
    #[must_use]
    pub fn new(population: i64, free: i64, drawn: i64, born: i64) -> Self {
        debug_assert!(
            population >= 0 && free >= 0 && drawn >= 0 && born >= 0,
            "census counts are non-negative"
        );
        debug_assert!(
            free + drawn == population,
            "custody split: free + drawn == population"
        );
        debug_assert!(
            born <= free,
            "DPL-born members are free (born <= free); a drawn born folds into drawn"
        );
        Self {
            population,
            free,
            drawn,
            born,
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

    /// The drawn count (members a consumer holds) — the model's `drawnManaged` at the
    /// count level, since DPL-born members are seeded free.
    #[must_use]
    pub const fn drawn(self) -> i64 {
        self.drawn
    }

    /// The free DPL-born, prune-exempt count (the model anchor `born`; roadmap #14).
    #[must_use]
    pub const fn born(self) -> i64 {
        self.born
    }

    /// The reconciler-owned count — the model's `managedCount`: the population minus the
    /// foreign DPL-born, so the boot object never counts toward the requirement.
    #[must_use]
    pub const fn managed(self) -> i64 {
        self.population - self.born
    }

    /// The free reconciler-owned count — the free pool minus the free DPL-born, so the
    /// prune-exempt boot object is never a shrink victim (the model's "a free *managed*
    /// individual exists" in `shrinkEnabled`; pool-objects design D3).
    #[must_use]
    pub const fn managed_free(self) -> i64 {
        self.free - self.born
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

    /// Converged when the reconciler-owned count meets the requirement — the count half
    /// of the model's `isConverged` (`managedCount == derivedReq`; `pool_lifecycle`
    /// :116). The prune half (`not(anyPrunable)`) needs per-object prune classification,
    /// which is the disposition's (pool-objects task 2.2), so it is not judged here.
    #[must_use]
    pub fn converged(self, requirement: i64) -> bool {
        self.managed() == requirement
    }

    /// Whether the requirement has fallen below the drawn count — the refusal
    /// precondition a free-only shrink cannot meet (the model's `shrinkBelowDrawAt`
    /// guard `derivedReq < drawnManaged`; pool-objects design D3). A requirement below
    /// draw surfaces to the operator, never a teardown of a live consumer; emitting the
    /// typed refusal is the disposition's (pool-objects task 2.2).
    #[must_use]
    pub fn shrinks_below_draw(self, requirement: i64) -> bool {
        requirement < self.drawn
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

#[cfg(test)]
mod tests {
    //! Parity of the census/sizing/convergence surface with
    //! `models/families/pool_lifecycle.qnt` `pool_lifecycle`: the ceiling gate
    //! (`censusRefusesAtCeilingTest`), the population arithmetic, converged and
    //! shrink-below-draw detection, the `Ceiling::Unknown` admit posture, and trio
    //! symmetry (one shape, three tags). The `compile_fail` witness that dpio has no
    //! variant lives on the [`PoolFamily`] rustdoc, not duplicated here.

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
        // population = free + drawn; managed = population - born; managed_free = free - born.
        let c = PoolCensus::new(4, 3, 1, 1);
        assert_eq!(c.population(), 4);
        assert_eq!(c.free(), 3);
        assert_eq!(c.drawn(), 1);
        assert_eq!(c.born(), 1);
        assert_eq!(c.managed(), 3); // 4 - 1 born
        assert_eq!(c.managed_free(), 2); // 3 free - 1 born
    }

    // ---- the ceiling gate (censusRefusesAtCeilingTest; DPBP-I7, ADR-0011) ----

    #[test]
    fn census_refuses_at_ceiling_admits_below() {
        // At the ceiling a create is disabled; below it is admitted (born + 2 == CEILING 3).
        let ceiling = Ceiling::Counted(3);
        assert!(
            PoolCensus::new(2, 2, 0, 0).admits_create(&ceiling),
            "below admits"
        );
        assert!(
            !PoolCensus::new(3, 3, 0, 0).admits_create(&ceiling),
            "at ceiling refuses"
        );
        // An Observed ceiling gates the same way.
        let obs = Ceiling::Observed {
            n: 3,
            provenance: "test".to_owned(),
        };
        assert!(PoolCensus::new(2, 2, 0, 0).admits_create(&obs));
        assert!(!PoolCensus::new(3, 3, 0, 0).admits_create(&obs));
    }

    #[test]
    fn unknown_ceiling_admits_with_no_refusal() {
        // Ceiling::Unknown has no model counterpart: admit-and-warn, never refuse.
        let c = PoolCensus::new(99, 99, 0, 0);
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
        assert!(PoolCensus::new(1, 1, 0, 1).grow_enabled(2, &ceiling));
        // At the requirement ⇒ no grow.
        assert!(!PoolCensus::new(3, 3, 0, 1).grow_enabled(2, &ceiling)); // managed 2 == req 2
        // Deficit but at ceiling ⇒ no grow (the census gate wins).
        assert!(!PoolCensus::new(3, 3, 0, 0).grow_enabled(5, &ceiling));
    }

    #[test]
    fn shrink_enabled_only_with_a_free_managed_individual() {
        // Surplus (managed 2 > req 1) with a free managed ⇒ shrink.
        assert!(PoolCensus::new(2, 2, 0, 0).shrink_enabled(1));
        // Surplus but every managed is drawn (free is born only) ⇒ no free victim.
        assert!(!PoolCensus::new(3, 1, 2, 1).shrink_enabled(1)); // managed 2 > 1, managed_free 0
        // At the requirement ⇒ no shrink.
        assert!(!PoolCensus::new(2, 2, 0, 0).shrink_enabled(2));
    }

    #[test]
    fn converged_when_managed_meets_the_requirement() {
        // Idempotent target: managed == req, and neither grow nor shrink fires.
        let c = PoolCensus::new(2, 2, 0, 1); // managed 1
        assert!(c.converged(1));
        assert!(!c.grow_enabled(1, &Ceiling::Counted(3)));
        assert!(!c.shrink_enabled(1));
        assert!(!PoolCensus::new(2, 2, 0, 0).converged(1)); // managed 2 != 1
    }

    #[test]
    fn shrink_below_draw_is_requirement_under_drawn() {
        // req below the drawn count surfaces the refusal precondition.
        assert!(PoolCensus::new(2, 0, 2, 0).shrinks_below_draw(1)); // 1 < drawn 2
        assert!(!PoolCensus::new(2, 1, 1, 0).shrinks_below_draw(1)); // 1 == drawn 1, not below
    }

    // ---- trio symmetry: one shape, three tags ----

    #[test]
    fn every_pool_family_obeys_the_same_laws() {
        // The generic shape is tag-invariant: the same census drives the same judgments
        // for every pooled family (pool-objects design D1).
        let ceiling = Ceiling::Counted(3);
        for f in PoolFamily::POOL_FAMILIES {
            assert_eq!(f.family().as_str(), f.name().to_lowercase());
            let deficit = PoolCensus::new(1, 1, 0, 1); // managed 0, req 2
            assert!(deficit.grow_enabled(2, &ceiling), "{}", f.name());
            let surplus = PoolCensus::new(2, 2, 0, 0); // managed 2, req 1
            assert!(surplus.shrink_enabled(1), "{}", f.name());
            let converged = PoolCensus::new(1, 1, 0, 0); // managed 1
            assert!(converged.converged(1), "{}", f.name());
            let over_draw = PoolCensus::new(2, 0, 2, 0);
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
}
