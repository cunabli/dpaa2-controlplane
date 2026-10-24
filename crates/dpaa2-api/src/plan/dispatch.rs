//! The plan-entry family dispatch: one compile-checked total table over the
//! construct families (design D10; ADR-0023), the Rust twin of the family `TABLE`
//! in `models/orchestration/pipeline.qnt`.
//!
//! Every plan entry an intent compiles to flows through [`disposition`], a `match`
//! over the plan vocabulary [`Attributes`] with no wildcard arm: a new construct
//! family cannot reach a plan without stating its row here, or the workspace fails
//! to build (the reconciler spec's "a new family without a pass does not compile"
//! scenario). A runtime registry map is refused — a missing insertion compiles, a
//! missing `match` arm does not (ADR-0023 design D10).
//!
//! Shape (ADR-0023 design D10 leaves it to this parcel): a plain exhaustive `match`, not a
//! visitor trait. A visitor with one implementor is an abstraction this seam does
//! not earn; the `match` is the interim `engine::reify_guard` (dpsw-typestate task
//! 6.4) upgraded to return the typed row, so the completeness guard and this table
//! are one place, not two (the guard is absorbed, not duplicated).
//!
//! The row values live once as the `const` rows below; [`disposition`] and the
//! enumerable [`TABLE`] both reference them, so "which families lack an executor"
//! is stated in exactly one place. [`TABLE`] is keyed by [`Family`] so the
//! resource-aware-orchestration task 3.2 `dpaa2-verify` twin can compare the row set against the model's `TABLE` by
//! [`Family::variant_name`] without reaching into `dpaa2-tools`. `Attributes` is
//! payload-carrying and cannot be iterated like [`crate::core::family::ALL_FAMILIES`]
//! ([`crate::intent::refuse::REFUSAL_VARIANTS`] is the precedent); [`TABLE`] stands
//! in for iteration, kept honest by the exhaustive `match` and the
//! `dispatch_agrees_with_the_table` test.

use crate::core::family::Family;
use crate::intent::compiled::Attributes;

/// The resource a family's create draws (the model's `ResKind`): keyed into the two
/// budgets at admission (ADR-0023 design D8). Variant names mirror `pipeline.qnt` verbatim
/// so the resource-aware-orchestration task 3.2 twin compares them directly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawKind {
    /// Draws the container pool (dpio/dpbp/dpcon/dpmcp companions; ADR-0011).
    PoolDraw,
    /// Draws a dpsw FDB entry (V-DPSW-2).
    FdbDraw,
    /// A plain create, no budgeted draw.
    PlainCreate,
    /// No runtime create — the object's occupancy is the handover face (dpmac).
    NoCreate,
}

/// A family-table row (ADR-0023 design D10), the model's `Disposition`: `Execute` names the
/// pass that consumes the family's plan entries and the resource its create draws;
/// `Unimplemented` is the typed whole-intent refusal naming the retirement trigger
/// that ships the pass.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Disposition {
    /// A delivered family: the pass consuming it and the resource its create draws.
    Execute {
        /// The resource the create draws.
        draw: DrawKind,
        /// The engine pass that consumes the family's plan entries.
        pass: &'static str,
    },
    /// No consuming pass today: the whole intent is refused typed before any verb,
    /// naming the change that retires the gap (ADR-0023 design D10; D5 refusal voice).
    Unimplemented {
        /// The roadmap item or change that ships the pass, verbatim the model's row
        /// so the resource-aware-orchestration task 3.2 twin compares the trigger string unchanged.
        trigger: &'static str,
    },
}

// Row values, single-sourced: `disposition` and `TABLE` both reference them.
const POOL: Disposition = Disposition::Execute {
    draw: DrawKind::PoolDraw,
    pass: "converge_pools",
};
const DPNI: Disposition = Disposition::Execute {
    draw: DrawKind::PlainCreate,
    pass: "converge_population",
};
const DPSW: Disposition = Disposition::Execute {
    draw: DrawKind::FdbDraw,
    pass: "converge_switches",
};
const DPRC: Disposition = Disposition::Execute {
    draw: DrawKind::PlainCreate,
    pass: "converge_containers",
};
// dpseci is the first `Unimplemented` row (ADR-0023 design D10): crypto rides the VPP scripts.
const DPSECI: Disposition = Disposition::Unimplemented {
    trigger: "crypto-provisioning change (ADR-0023 D10)",
};

/// The plan-entry family's disposition (ADR-0023 design D10): the pass that consumes it and its
/// draw, or the typed refusal. The exhaustive `match` has no wildcard arm, so adding or
/// removing an [`Attributes`] variant fails the build here — the reconciler spec's
/// second scenario. The pool companions (dpio/dpbp/dpcon/dpmcp) share one pool-drawing
/// row ([`Attributes::Unsized`]).
#[must_use]
pub fn disposition(attrs: &Attributes) -> Disposition {
    match attrs {
        Attributes::Unsized => POOL,
        Attributes::Dpni { .. } => DPNI,
        Attributes::Dpsw { .. } => DPSW,
        Attributes::Dprc { .. } => DPRC,
        Attributes::Dpseci { .. } => DPSECI,
    }
}

/// The dispatch's row set as data, keyed by [`Family`] (relatable to the model's `TABLE`
/// via [`Family::variant_name`]). TABLE keys the full derived vocabulary
/// ([`crate::core::family::DERIVED_FAMILIES`]); the four pool companions
/// (dpbp/dpio/dpcon/dpmcp) share the one pool-drawing row ([`Attributes::Unsized`]). The
/// model's extra `Dpmac` (`NoCreate`) row — DPC-born, no plan entry — is the
/// resource-aware-orchestration task 3.2 twin's single sanctioned asymmetry
/// (ADR-0023 design D10/D11). Enumerable because `Attributes` is payload-carrying and
/// cannot be iterated directly (the [`crate::intent::refuse::REFUSAL_VARIANTS`] precedent).
pub const TABLE: &[(Family, Disposition)] = &[
    (Family::Dpbp, POOL),
    (Family::Dpio, POOL),
    (Family::Dpcon, POOL),
    (Family::Dpmcp, POOL),
    (Family::Dpni, DPNI),
    (Family::Dpsw, DPSW),
    (Family::Dprc, DPRC),
    (Family::Dpseci, DPSECI),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::family::DERIVED_FAMILIES;
    use crate::families::dpni::DpniCfg;
    use crate::families::dpseci::{DpseciCfg, OptionMask};
    use std::collections::BTreeSet;

    // One representative per TABLE row, so the dispatch is exercised the way a compiled plan
    // carries it. The four pool companions all carry `Unsized`, so each maps through the one
    // pool row to its own key.
    fn representatives() -> Vec<(Family, Attributes)> {
        vec![
            (Family::Dpbp, Attributes::Unsized),
            (Family::Dpio, Attributes::Unsized),
            (Family::Dpcon, Attributes::Unsized),
            (Family::Dpmcp, Attributes::Unsized),
            (
                Family::Dpni,
                Attributes::Dpni {
                    cfg: DpniCfg::defaults(),
                },
            ),
            (
                Family::Dpsw,
                Attributes::Dpsw {
                    num_ifs: 1,
                    max_fdbs: 1,
                    max_fdb_entries: 1,
                    per_fdb_flooding: false,
                    per_fdb_broadcast: false,
                    ctrl_if: false,
                },
            ),
            (
                Family::Dprc,
                Attributes::Dprc {
                    options: BTreeSet::new(),
                },
            ),
            (
                Family::Dpseci,
                Attributes::Dpseci {
                    cfg: DpseciCfg::new(OptionMask::empty(), 1, vec![1]).unwrap(),
                },
            ),
        ]
    }

    fn row_for(family: Family) -> Option<Disposition> {
        TABLE.iter().find(|(f, _)| *f == family).map(|(_, d)| *d)
    }

    #[test]
    fn every_unimplemented_row_names_its_trigger() {
        // Every `Unimplemented` row must name its retirement trigger (reconciler spec).
        for (family, disp) in TABLE {
            if let Disposition::Unimplemented { trigger } = disp {
                assert!(
                    !trigger.is_empty(),
                    "{} is Unimplemented but names no retirement trigger",
                    family.variant_name()
                );
            }
        }
    }

    #[test]
    fn dpseci_is_the_first_unimplemented_row() {
        // Reconciler spec scenario 1: dpseci is refused typed; trigger copied from pipeline.qnt.
        assert_eq!(
            row_for(Family::Dpseci),
            Some(Disposition::Unimplemented {
                trigger: "crypto-provisioning change (ADR-0023 D10)"
            })
        );
    }

    #[test]
    fn dispatch_agrees_with_the_table() {
        // The enumerable TABLE is a copy of what the exhaustive `match` decides; this keeps
        // the two honest for every plan vocabulary variant (the REFUSAL_VARIANTS discipline).
        for (family, attrs) in representatives() {
            assert_eq!(
                Some(disposition(&attrs)),
                row_for(family),
                "{} dispatch disagrees with its TABLE row",
                family.variant_name()
            );
        }
        // TABLE keys all of DERIVED_FAMILIES (ADR-0023 D11); see resource-aware-orchestration task 3.2.
        let keys: BTreeSet<Family> = TABLE.iter().map(|(f, _)| *f).collect();
        let derived: BTreeSet<Family> = DERIVED_FAMILIES.iter().copied().collect();
        assert_eq!(keys, derived, "TABLE keys drifted from DERIVED_FAMILIES");
    }
}
