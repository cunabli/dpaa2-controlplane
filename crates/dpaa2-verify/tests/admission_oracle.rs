//! The offline admission oracle (resource-aware-orchestration task 4.2): the pure
//! `dpaa2-api` gross draw and admission verdict exercised end to end against hand-built
//! fixtures — board-free, no IO. The oracle is the spec's four "Admission judges the gross
//! draw before dispatch" scenarios plus one idempotence test (five tests): an over-budget pool
//! intent and an over-budget FDB intent each refuse typed naming budget/draw/headroom, a rebuild
//! against an exhausted budget resolves in two runs (the pending draw counts the recreate, never
//! the planned free; ADR-0023 design D8), and a converged intent re-admits (the pending draw is
//! empty); the idempotence test (`refusals_are_idempotent`,
//! resource-aware-orchestration task 4.2) adds that repeated
//! judgments are byte-stable (ADR-0023 design D5).

use std::collections::BTreeMap;

use dpaa2_api::core::family::Family;
use dpaa2_api::core::inventory::Ceiling;
use dpaa2_api::plan::admission::{AdmissionRefusal, GrossDraw, admit};
use dpaa2_api::plan::capacity::{CapacityFace, FdbBudget, PoolBudget, PoolHeadroom};

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

// A banked V-DPSW-2 bracket of `n` with `draw` observed, so free() == n − draw.
fn bracket(n: i64) -> Ceiling {
    Ceiling::Observed {
        n,
        provenance: "V-DPSW-2".to_owned(),
    }
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

/// An over-budget pool intent refuses whole, naming the budget, the draw, and the headroom
/// (the spec scenario "An over-budget intent is refused naming the shortfall").
#[test]
fn over_budget_pool_refuses_typed() {
    let draw = draw(3, 0);
    let err = admit(&face(1, bracket(4), 0), &draw).unwrap_err();
    assert_eq!(
        err,
        vec![AdmissionRefusal::Pool {
            family: Family::Dpbp,
            draw: 3,
            headroom: 1,
        }]
    );
}

/// An over-budget FDB intent refuses typed, naming the FDB budget, the draw, and the headroom.
#[test]
fn over_budget_fdb_refuses_typed() {
    let draw = draw(0, 3);
    // bracket 4 − observed draw 2 ⇒ free 2; the gross FDB draw 3 overruns it.
    let err = admit(&face(4, bracket(4), 2), &draw).unwrap_err();
    assert_eq!(
        err,
        vec![AdmissionRefusal::Fdb {
            draw: 3,
            headroom: 2,
        }]
    );
}

/// A rebuild against an exhausted budget resolves in two runs (the spec scenario "A rebuild
/// against an exhausted budget takes two runs"): the recreate is the one pending create, the
/// planned free is not counted, so the first run — judged against the exhausted face — refuses;
/// judged against the post-teardown face, the same pending draw admits (ADR-0023 design D8).
#[test]
fn rebuild_against_exhausted_budget_takes_two_runs() {
    // The immutable-drift object's recreate is a pending create of one — its planned free is not.
    let draw = draw(1, 0);
    // Run one: the drifted object still occupies its slot, so free is zero — refused.
    let refused = admit(&face(0, bracket(4), 0), &draw).unwrap_err();
    assert_eq!(
        refused,
        vec![AdmissionRefusal::Pool {
            family: Family::Dpbp,
            draw: 1,
            headroom: 0,
        }]
    );
    assert!(admit(&face(1, bracket(4), 0), &draw).is_ok());
}

/// A converged intent re-admits (the spec scenario "A converged intent re-admits"): every
/// planned object is observed converged, so the pending draw is empty, and admission admits even
/// when every drawn budget shows zero free headroom (ADR-0023 design D8).
#[test]
fn converged_intent_re_admits_against_zero_free() {
    let draw = GrossDraw::from_pending(&BTreeMap::new());
    assert!(draw.pool.is_empty() && draw.fdb == 0);
    assert!(admit(&face(0, bracket(4), 4), &draw).is_ok());
}

/// Judging the same face and plan twice yields byte-identical refusals (the spec scenario
/// "Refusals are stable"): admission holds no state and never oscillates (ADR-0023 design D5/D7).
#[test]
fn refusals_are_idempotent() {
    let draw = draw(3, 3);
    let f = face(1, bracket(4), 3);
    let first = admit(&f, &draw);
    let second = admit(&f, &draw);
    assert_eq!(first, second);
    assert!(first.is_err());
}
