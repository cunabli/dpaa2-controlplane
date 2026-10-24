//! The gross draw derivation and the admission verdict — the pure judgment the engine
//! runs after `compile` and before dispatch (ADR-0023 design D5/D8; ADR-0011). The Rust
//! twin of the `pipeline.qnt` `grossPoolDraw`/`grossFdbDraw` folds and the `admit` action.
//!
//! Pure and stateless: [`GrossDraw::from_pending`] folds the per-family pending counts — the
//! creates the run's passes would actually dispatch, so the Rust basis equals the model's
//! `pending` fold (ADR-0023 design D8) — routing each family through the one draw source
//! [`TABLE`], never a second match over families. An already-converged object draws nothing,
//! so admission admits the fixpoint its own convergence produced. [`admit`] holds no
//! reservation; the board stays the only ledger and dispatch truth stays re-observation
//! (ADR-0023 design D7). Same face + same draw yield an identical verdict (the spec scenario
//! "Refusals are stable").

use std::collections::BTreeMap;

use crate::core::family::Family;
use crate::plan::capacity::{CapacityFace, PoolHeadroom};
use crate::plan::dispatch::{Disposition, DrawKind, TABLE};

/// An intent's gross draw against the two budgets (ADR-0023 design D8): the pending creates
/// counted, already-converged objects and planned frees not counted — a teardown is a
/// reconcile transition, never a draw here. The pool draw is per family so dpio (spent,
/// non-reclaimable) separates from the reclaimable trio at admission.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GrossDraw {
    /// Per-pool-family draw — the count of planned [`DrawKind::PoolDraw`] creates keyed by
    /// family (dpbp/dpio/dpcon/dpmcp; ADR-0011).
    pub pool: BTreeMap<Family, i64>,
    /// The dpsw FDB draw — the count of planned [`DrawKind::FdbDraw`] creates (V-DPSW-2).
    /// The model's port-wire `+1` (`pipeline.qnt` `grossFdbDraw` `WANT_PORT`) is an edge, not
    /// a dispatch-classified create, so it stays a dispatch-time re-observation (ADR-0023 design D7/D9).
    pub fdb: i64,
}

/// A typed admission refusal — data only, operator terms, no law IDs (ADR-0023 design D5;
/// ADR-0013 vocabulary). A sibling of [`crate::intent::refuse::Refusal`], kept apart so
/// `compile` stays untouched: admission is a separate pure judgment over `(Compiled, face)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AdmissionRefusal {
    /// A pool family's draw exceeds its observed reclaimable free headroom.
    Pool {
        /// The pool family budget overdrawn.
        family: Family,
        /// The gross draw on that family.
        draw: i64,
        /// The observed free headroom it exceeded.
        headroom: i64,
    },
    /// The dpsw FDB draw exceeds the observed V-DPSW-2 bracket free headroom.
    Fdb {
        /// The gross FDB draw.
        draw: i64,
        /// The observed free headroom it exceeded.
        headroom: i64,
    },
}

impl GrossDraw {
    /// Folds the per-family pending creates into the draw (ADR-0023 design D8), routing each
    /// family's count through the one draw source [`TABLE`] by [`Family`] — the Rust twin of
    /// the model's `grossPoolDraw`/`grossFdbDraw` fold over `pending`. The caller supplies the
    /// pending counts from the passes' own convergence reads (the creates the run would
    /// dispatch), so a converged object contributes nothing. A family routed to
    /// `PlainCreate`/`NoCreate` draws nothing budgeted; an `Unimplemented` family draws nothing
    /// (it refuses whole at dispatch, task 3.x). Non-positive counts are skipped.
    #[must_use]
    pub fn from_pending(pending: &BTreeMap<Family, i64>) -> Self {
        let mut draw = Self::default();
        for (&family, &count) in pending {
            if count <= 0 {
                continue;
            }
            if let Some(Disposition::Execute { draw: kind, .. }) =
                TABLE.iter().find(|(f, _)| *f == family).map(|(_, d)| *d)
            {
                match kind {
                    DrawKind::PoolDraw => *draw.pool.entry(family).or_insert(0) += count,
                    DrawKind::FdbDraw => draw.fdb += count,
                    DrawKind::PlainCreate | DrawKind::NoCreate => {}
                }
            }
        }
        draw
    }
}

/// The admission verdict (ADR-0023 design D5/D8): the complete set of budgets the gross draw
/// overdraws, else admit. A reclaimable pool row ([`PoolHeadroom::Free`]) refuses when the
/// draw exceeds it; dpio's spent row ([`PoolHeadroom::Spent`], ADR-0003 §7 non-reclaimable)
/// and an unobserved family carry no free count, so their draws take the refusal-free path
/// (the spec's "a budget row joins only with the change that proves its observation"). An
/// Unknown FDB bracket (`free() == None`) admits, never refuses (ADR-0011 warn-on-unknown; the
/// warn rides `compile`'s `Warning::UnknownCeiling` for dpsw). Deterministic: the per-family
/// pool rows iterate in sorted order, the FDB row last, so repeated calls are byte-identical.
///
/// # Errors
///
/// Returns the complete, sorted set of [`AdmissionRefusal`]s the gross draw triggered.
pub fn admit(face: &CapacityFace, draw: &GrossDraw) -> Result<(), Vec<AdmissionRefusal>> {
    let mut refusals = Vec::new();
    for (&family, &d) in &draw.pool {
        if d <= 0 {
            continue;
        }
        if let Some(PoolHeadroom::Free(n)) = face
            .pools
            .iter()
            .find(|p| p.family == family)
            .map(|p| p.headroom)
            && d > n
        {
            refusals.push(AdmissionRefusal::Pool {
                family,
                draw: d,
                headroom: n,
            });
        }
    }
    if draw.fdb > 0
        && let Some(n) = face.fdb.free()
        && draw.fdb > n
    {
        refusals.push(AdmissionRefusal::Fdb {
            draw: draw.fdb,
            headroom: n,
        });
    }
    if refusals.is_empty() {
        Ok(())
    } else {
        refusals.sort();
        Err(refusals)
    }
}

#[cfg(test)]
mod tests {
    //! Admission is a pure judgment of the gross draw against the face (ADR-0023 design D5/D8):
    //! a pool over-draw refuses naming budget/draw/headroom, an FDB over-draw refuses, an
    //! Unknown bracket admits (ADR-0011), a fitting plan admits, and the verdict is identical
    //! across repeated calls (the spec scenario "Refusals are stable").

    use super::*;
    use crate::core::inventory::Ceiling;
    use crate::plan::capacity::{FdbBudget, PoolBudget};

    fn draw(dpbp: i64, dpsw: i64) -> GrossDraw {
        let mut pending = BTreeMap::new();
        if dpbp != 0 {
            pending.insert(Family::Dpbp, dpbp);
        }
        if dpsw != 0 {
            pending.insert(Family::Dpsw, dpsw);
        }
        GrossDraw::from_pending(&pending)
    }

    fn face(dpbp_free: i64, fdb_bracket: Ceiling, fdb_draw: i64) -> CapacityFace {
        CapacityFace {
            pools: vec![
                PoolBudget {
                    family: Family::Dpbp,
                    headroom: PoolHeadroom::Free(dpbp_free),
                },
                PoolBudget {
                    family: Family::Dpio,
                    headroom: PoolHeadroom::Spent(16),
                },
            ],
            fdb: FdbBudget {
                bracket: fdb_bracket,
                draw: fdb_draw,
            },
        }
    }

    fn observed(n: i64) -> Ceiling {
        Ceiling::Observed {
            n,
            provenance: "V-DPSW-2".to_owned(),
        }
    }

    #[test]
    fn pool_over_draw_refuses_naming_budget_draw_headroom() {
        let draw = draw(2, 0);
        assert_eq!(draw.pool.get(&Family::Dpbp), Some(&2));
        let err = admit(&face(1, observed(2), 0), &draw).unwrap_err();
        assert_eq!(
            err,
            vec![AdmissionRefusal::Pool {
                family: Family::Dpbp,
                draw: 2,
                headroom: 1,
            }]
        );
    }

    #[test]
    fn fdb_over_draw_refuses_naming_budget_draw_headroom() {
        let draw = draw(0, 2);
        assert_eq!(draw.fdb, 2);
        // bracket 2 − observed draw 1 ⇒ free 1; the gross draw 2 overruns it.
        let err = admit(&face(4, observed(2), 1), &draw).unwrap_err();
        assert_eq!(
            err,
            vec![AdmissionRefusal::Fdb {
                draw: 2,
                headroom: 1,
            }]
        );
    }

    #[test]
    fn unknown_fdb_bracket_admits_never_refuses() {
        // free() == None (ADR-0011): a draw against an unobserved bracket admits, warn rides compile.
        let draw = draw(0, 3);
        assert!(admit(&face(4, Ceiling::Unknown, 0), &draw).is_ok());
    }

    #[test]
    fn a_fitting_plan_admits() {
        let draw = draw(1, 1);
        assert!(admit(&face(2, observed(2), 0), &draw).is_ok());
    }

    #[test]
    fn a_converged_draw_admits_against_zero_free() {
        // Spec scenario "A converged intent re-admits" (ADR-0023 design D8): zero pending admits at zero free.
        let draw = GrossDraw::from_pending(&BTreeMap::new());
        assert!(draw.pool.is_empty() && draw.fdb == 0);
        assert!(admit(&face(0, observed(2), 2), &draw).is_ok());
    }

    #[test]
    fn dpio_draw_takes_the_refusal_free_path() {
        // A pending dpio seat is a PoolDraw attributed to dpio, but its face row is Spent
        // (non-reclaimable; ADR-0003 §7), so it carries no free count and never refuses.
        let draw = GrossDraw::from_pending(&BTreeMap::from([(Family::Dpio, 1)]));
        assert_eq!(draw.pool.get(&Family::Dpio), Some(&1));
        assert!(admit(&face(0, Ceiling::Unknown, 0), &draw).is_ok());
    }

    #[test]
    fn the_verdict_is_identical_across_repeated_calls() {
        let draw = draw(2, 2);
        let f = face(1, observed(2), 1);
        assert_eq!(admit(&f, &draw), admit(&f, &draw));
    }
}
