//! The census' typed capacity face — the two board-proven budgets as a pure value
//! projection the admission (resource-aware-orchestration task 4.2) and CLI (task 5.1)
//! read (ADR-0023 design D2/D8/D11; ADR-0011). The Rust twin of the `pipeline.qnt`
//! budget vars `poolFree` (ADR-0011) and `fdbFree` (V-DPSW-2).
//!
//! Admission reads two independently-moving ledgers (plan-rollback design D2;
//! V-ROLLBACK-1 rev 3): the per-container census pool rows ([`CapacityFace::pools`]) and
//! the MC-global hardware pool budget rows ([`CapacityFace::globals`], bp and dpcon). A
//! create verb a container cannot seat is refused by the global pools too, so a
//! hardware-side drain the census cannot see still refuses against the global row.
//!
//! Pure and stateless: [`capacity_face`] derives the face from the loop's observations
//! alone — the per-family [`PoolCensus`] the pool passes already parse, the observed
//! MC-global free counts, plus the banked FDB bracket — and nothing is carried between
//! two derivations (ADR-0008; the spec scenario "Budgets are re-minted from
//! observation"). The adapter observes; this module only projects.

use crate::core::family::Family;
use crate::core::inventory::Ceiling;
use crate::families::pool_lifecycle::{PoolCensus, PoolFamily};

/// One container-pool budget row's headroom — a reclaimable free count or spent,
/// never-reclaimable seats (ADR-0011; ADR-0003 §7). The variant is the honest data
/// distinction between a pool that frees on teardown and the dpio seats that do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolHeadroom {
    /// Reclaimable free pool entries — the [`PoolCensus::free`] headroom of a pooled
    /// family (dpbp/dpmcp/dpcon; ADR-0011).
    Free(i64),
    /// Spent, never-reclaimable seats — dpio headroom is already spent (ADR-0003 §7):
    /// a live seat cannot be torn down (the ADR-0008 §4 race), so it is reported as
    /// consumed, never as a free count admission could draw against. The sibling of
    /// the dpio [`SeatDisposition`](crate::families::dpio::SeatDisposition) residue.
    Spent(i64),
}

/// One container-pool budget row — a family and its headroom (ADR-0023 design D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolBudget {
    /// The pool family this row budgets.
    pub family: Family,
    /// The row's headroom: reclaimable free, or spent dpio seats.
    pub headroom: PoolHeadroom,
}

/// One MC-global pool budget row — the second ledger admission reads beside the census
/// rows (plan-rollback design D2; ADR-0011). The free count comes from the MC-global
/// hardware pool listing (`dprc show mc.global --resources`), so it moves independently
/// of any one container's census and still refuses a create a hardware-side drain emptied
/// (V-ROLLBACK-1 rev 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalBudget {
    /// The MC-global pool family this row budgets — bp (V-CEIL-1 + V-DPRC-7) or dpcon
    /// (`swpch.2wq`, V-DPCON-2; ADR-0011).
    pub family: Family,
    /// The observed MC-global free count for the family.
    pub free: i64,
}

/// The dpsw FDB budget row — the `pipeline.qnt` `fdbFree` (V-DPSW-2). The FDB table is
/// unobservable through restool (COVERAGE DPSW-I4: the table face stays unobserved), so
/// the budget is the banked V-DPSW-2 bracket, carried as a [`Ceiling`] (ADR-0011's
/// vocabulary): [`Ceiling::Observed`] with provenance `"V-DPSW-2"` when the caller banks
/// the bracket, [`Ceiling::Unknown`] when none is banked — nothing here invents a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FdbBudget {
    /// The banked FDB ceiling — `Observed { n, provenance: "V-DPSW-2" }`, `Unknown`
    /// when none is banked, or `Counted` should an inventory source supply one;
    /// [`FdbBudget::free`] subtracts the draw from `Observed`/`Counted` alike and
    /// yields `None` on `Unknown`.
    pub bracket: Ceiling,
    /// The observed dpsw FDB draw (one unit per dpsw create; `pipeline.qnt` `grossFdbDraw`),
    /// derived by the caller from the census.
    pub draw: i64,
}

impl FdbBudget {
    /// The free count — the bracket minus the observed draw, or `None` when the bracket is
    /// [`Ceiling::Unknown`] (no banked number to subtract from). Admission
    /// (resource-aware-orchestration task 4.2) warns on an unknown budget, never refuses
    /// (ADR-0011's admit-and-warn discipline).
    #[must_use]
    pub fn free(&self) -> Option<i64> {
        match &self.bracket {
            Ceiling::Counted(n) | Ceiling::Observed { n, .. } => Some(n - self.draw),
            Ceiling::Unknown => None,
        }
    }
}

/// The census' capacity face — the two board-proven budgets as data
/// (ADR-0023 design D2/D8/D11): the per-container pool rows and the dpsw FDB row. A new budget joins as a
/// field with the change that proves its observation, never by reworking this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacityFace {
    /// The four container-pool rows (dpbp/dpmcp/dpcon free, dpio spent).
    pub pools: Vec<PoolBudget>,
    /// The MC-global pool budget rows — the second ledger (plan-rollback design D2). Bp
    /// and dpcon join under the monotonic-growth rule: a dpbp create refuses exactly at the
    /// bp free count and bp moves symmetrically to the object (V-CEIL-1 + V-DPRC-7 rev 2);
    /// a dpcon create draws one `swpch.2wq` unit at the fixed two-priority operand and
    /// destroy credits it back (V-DPCON-2 rev 1). The dpmcp row stays out behind the
    /// ADR-0011 listing-lag question.
    pub globals: Vec<GlobalBudget>,
    /// The dpsw FDB budget row.
    pub fdb: FdbBudget,
}

/// Derives the capacity face from the loop's observations (ADR-0023 design D2: counts
/// re-minted from observation each loop, never ledgered). Pure and total: the pooled trio
/// rows read [`PoolCensus::free`] as reclaimable headroom; the dpio row is the observed
/// seat count rendered [`PoolHeadroom::Spent`] (ADR-0003 §7 non-reclaimable); the
/// `globals` rows are the observed MC-global free counts mapped in the given order (the
/// second ledger, plan-rollback design D2); the FDB row is the banked bracket and the
/// caller-derived draw. Takes only borrowed observation data, so two derivations from the
/// same input yield equal faces with no carried state.
#[must_use]
pub fn capacity_face(
    pools: &[(PoolFamily, PoolCensus)],
    dpio_seats: i64,
    globals: &[(Family, i64)],
    fdb_bracket: &Ceiling,
    fdb_draw: i64,
) -> CapacityFace {
    let mut rows: Vec<PoolBudget> = pools
        .iter()
        .map(|(family, census)| PoolBudget {
            family: family.family(),
            headroom: PoolHeadroom::Free(census.free()),
        })
        .collect();
    rows.push(PoolBudget {
        family: Family::Dpio,
        headroom: PoolHeadroom::Spent(dpio_seats),
    });
    let global_rows: Vec<GlobalBudget> = globals
        .iter()
        .map(|&(family, free)| GlobalBudget { family, free })
        .collect();
    CapacityFace {
        pools: rows,
        globals: global_rows,
        fdb: FdbBudget {
            bracket: fdb_bracket.clone(),
            draw: fdb_draw,
        },
    }
}

#[cfg(test)]
mod tests {
    //! The capacity face is a pure projection of the loop's observations
    //! (ADR-0023 design D2): re-minting from the same input is equal with no carried state, the pooled trio
    //! reads reclaimable free, dpio reads spent, and the FDB row is bracket − draw with the
    //! V-DPSW-2 provenance or an unknown-budget row, never an invented number.

    use super::*;
    use crate::core::model::ObjectRef;
    use crate::core::types::ConstructName;
    use crate::families::pool_lifecycle::{ObservedPoolObject, RawLabel, census_of};
    use std::collections::BTreeSet;

    fn declared(names: &[&str]) -> BTreeSet<ConstructName> {
        names.iter().map(|n| ConstructName::from(*n)).collect()
    }

    // A plugged, undrawn pool row wearing `raw` as its label — the banked-fixture shape
    // census_of's own tests use (pool-objects design D3).
    fn row(family: Family, ord: u32, raw: &str) -> ObservedPoolObject {
        ObservedPoolObject {
            object: ObjectRef::new(family, ord),
            label: RawLabel::from(raw),
            plugged: true,
            drawn: false,
        }
    }

    // Three trio censuses with distinct free counts: dpbp 2 (two declared-free), dpmcp 1,
    // dpcon 0 (the DPL boot row is `born`, not managed-free, but still free).
    fn trio_censuses() -> Vec<(PoolFamily, PoolCensus)> {
        let d = declared(&["vpp"]);
        let dpbp = census_of(
            &[row(Family::Dpbp, 0, "vpp"), row(Family::Dpbp, 1, "vpp")],
            &d,
        );
        let dpmcp = census_of(&[row(Family::Dpmcp, 0, "vpp")], &d);
        let dpcon: Vec<ObservedPoolObject> = vec![];
        let dpcon = census_of(&dpcon, &d);
        vec![
            (PoolFamily::Dpbp, dpbp),
            (PoolFamily::Dpmcp, dpmcp),
            (PoolFamily::Dpcon, dpcon),
        ]
    }

    #[test]
    fn budgets_are_reminted_from_observation() {
        // ADR-0023 design D2 / spec scenario: two derivations from the same borrowed
        // observation are equal — the face carries no allocation state between loops.
        let trio = trio_censuses();
        let bracket = Ceiling::Observed {
            n: 2,
            provenance: "V-DPSW-2".to_owned(),
        };
        let globals = [(Family::Dpbp, 63)];
        let first = capacity_face(&trio, 16, &globals, &bracket, 1);
        let second = capacity_face(&trio, 16, &globals, &bracket, 1);
        assert_eq!(first, second);
    }

    #[test]
    fn pool_rows_show_free_per_family() {
        let trio = trio_censuses();
        let face = capacity_face(&trio, 16, &[], &Ceiling::Unknown, 0);
        assert_eq!(
            face.pools[0],
            PoolBudget {
                family: Family::Dpbp,
                headroom: PoolHeadroom::Free(2),
            }
        );
        assert_eq!(
            face.pools[1],
            PoolBudget {
                family: Family::Dpmcp,
                headroom: PoolHeadroom::Free(1),
            }
        );
        assert_eq!(
            face.pools[2],
            PoolBudget {
                family: Family::Dpcon,
                headroom: PoolHeadroom::Free(0),
            }
        );
    }

    #[test]
    fn dpio_reads_as_spent_not_reclaimable() {
        // dpio seats are non-reclaimable (ADR-0003 §7): the row renders Spent, never Free.
        let face = capacity_face(&trio_censuses(), 16, &[], &Ceiling::Unknown, 0);
        let dpio = face
            .pools
            .iter()
            .find(|p| p.family == Family::Dpio)
            .expect("dpio row present");
        assert_eq!(dpio.headroom, PoolHeadroom::Spent(16));
        assert!(
            !matches!(dpio.headroom, PoolHeadroom::Free(_)),
            "dpio headroom is spent, never a reclaimable free count"
        );
    }

    #[test]
    fn fdb_free_is_bracket_minus_draw_with_provenance() {
        // V-DPSW-2 bracket 2, one dpsw drawn ⇒ free 1, provenance preserved.
        let bracket = Ceiling::Observed {
            n: 2,
            provenance: "V-DPSW-2".to_owned(),
        };
        let face = capacity_face(&trio_censuses(), 16, &[], &bracket, 1);
        assert_eq!(face.fdb.free(), Some(1));
        assert_eq!(face.fdb.bracket, bracket);
        assert_eq!(face.fdb.draw, 1);
    }

    #[test]
    fn unknown_bracket_yields_an_unknown_budget_row() {
        // No banked bracket ⇒ an Unknown row with no free number (never an invented one);
        // admission (task 4.2) warns on Unknown, never refuses (ADR-0011).
        let face = capacity_face(&trio_censuses(), 16, &[], &Ceiling::Unknown, 3);
        assert_eq!(face.fdb.bracket, Ceiling::Unknown);
        assert_eq!(face.fdb.free(), None);
    }

    #[test]
    fn global_bp_row_is_present_and_leaves_census_rows_unchanged() {
        // The second ledger (plan-rollback design D2): a passed bp free count is the lone
        // globals row and the census pool rows are untouched by its presence.
        let trio = trio_censuses();
        let without = capacity_face(&trio, 16, &[], &Ceiling::Unknown, 0);
        let with = capacity_face(&trio, 16, &[(Family::Dpbp, 63)], &Ceiling::Unknown, 0);
        assert_eq!(
            with.globals,
            vec![GlobalBudget {
                family: Family::Dpbp,
                free: 63,
            }]
        );
        assert_eq!(with.pools, without.pools);
    }

    #[test]
    fn empty_globals_yields_no_global_rows() {
        // A budget row joins only with the change that proves its observation (plan-rollback design D2).
        let face = capacity_face(&trio_censuses(), 16, &[], &Ceiling::Unknown, 0);
        assert_eq!(face.globals, Vec::<GlobalBudget>::new());
    }
}
