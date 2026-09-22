//! The pool-lifecycle ITF-replay CI rung (pool-objects task 4.1): every committed pool trace
//! replays green through the task-2.1/2.2 P3 count surface, board-free. The model is the
//! oracle — a regenerated trace whose counts the Rust pure functions no longer reproduce
//! fails here loudly; regenerate with `pnpm model:freeze-pool` and reconcile the
//! transcription.
//!
//! Each trace is a directed run of `models/families/dpbp.qnt` `dpbp_lifecycle` (the
//! representative trio twin; dpmcp/dpcon are byte-identical `CEILING = 3` instantiations).
//! The ITF carries the two state vars (`s`/`conv`) but not the action between them, so the
//! replayer reduces each state to its [`PoolCensus`] count vocabulary and infers the
//! transition from the count delta (mirroring `pool_lifecycle`'s action set). The
//! conformance is threefold: at every state the count↔individual boundary isomorphism
//! (`managed() == managedCount + drawn-foreign`, pool-objects design D2) and the
//! shrink-below-draw duality hold; at every create/grow/shrink/prune transition the enabling
//! predicate ([`PoolCensus::admits_create`]/`grow_enabled`/`shrink_enabled`) that let the
//! model action fire is asserted true; and the two `.fail()` refusals and the
//! `ShrinkBelowDraw` flag are witnessed against the oracle by dedicated tests below.
//!
//! A frozen pool population over the ceiling has no MC-legal state, so the reader has no
//! census for it and the replay fails at decode — a model↔core divergence made loud
//! (DPBP-I7; ADR-0011).

use std::collections::BTreeSet;

use dpaa2_api::core::inventory::Ceiling;
use dpaa2_api::families::pool_lifecycle::{
    PoolCensus, PoolFamily, ShrinkBelowDraw, drift_disposition,
};
use dpaa2_verify::intent::pool_itf::{PoolStep, PoolWorld, parse_pool_trace};

/// The dpbp representative family and its board ceiling (`dpbp.qnt` `CEILING = 3`; DPBP-I7).
const FAMILY: PoolFamily = PoolFamily::Dpbp;
const CEILING: Ceiling = Ceiling::Counted(3);

/// Every committed trace under `models/traces/families/dpbp/`, with the model face it pins.
/// The stems carry the `dpbp_lifecycle::pool_lifecycle::` prefix quint gives an inherited run.
const TRACES: &[(&str, &str)] = &[
    (
        "dpbp_lifecycle::pool_lifecycle::custodyCycleTest",
        "custody cycle: create → free-pool → draw → return",
    ),
    (
        "dpbp_lifecycle::pool_lifecycle::dirtyReturnTest",
        "DPBP-I3: a return restores membership without a reset",
    ),
    (
        "dpbp_lifecycle::pool_lifecycle::censusRefusesAtCeilingTest",
        "DPBP-I7: create-until-refused; at the ceiling the create is disabled",
    ),
    (
        "dpbp_lifecycle::pool_lifecycle::convergenceGrowTest",
        "deficit → grow to the derived count",
    ),
    (
        "dpbp_lifecycle::pool_lifecycle::freeOnlyShrinkTest",
        "surplus → free-only shrink; the drawn survives",
    ),
    (
        "dpbp_lifecycle::pool_lifecycle::drawnNeverShrunkTest",
        "a drawn individual is never a shrink victim",
    ),
    (
        "dpbp_lifecycle::pool_lifecycle::shrinkBelowDrawRefusedTest",
        "ShrinkBelowDraw: requirement below draw surfaces a refusal, not a teardown",
    ),
    (
        "dpbp_lifecycle::pool_lifecycle::prunePreservesDplBornTest",
        "prune removes an undeclared free object; the DPL-born survives",
    ),
    (
        "dpbp_lifecycle::pool_lifecycle::idempotentReconvergeTest",
        "a converged state enables no grow, shrink, or prune",
    ),
];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/traces/families/dpbp/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// A trace/core disagreement — a FINDING, surfaced loudly, never papered over.
fn finding(file: &str, step: usize, msg: &str) -> ! {
    panic!("{file}: trace/core disagreement at step {step}: {msg}");
}

/// The per-state conformance: the count↔individual isomorphism, the shrink-below-draw
/// duality, and the drift/refusal agreement (pool-objects design D2/D3).
fn check_state(file: &str, i: usize, w: &PoolWorld) {
    // The model's `managedCount` is the reconciler-owned count; the Rust `managed()` folds in
    // any drawn foreign (count-indistinguishable, the conservative bias). The exact relation
    // is the count↔individual boundary law.
    if w.census.managed() != w.managed_count + w.foreign_drawn {
        finding(
            file,
            i,
            &format!(
                "census.managed() {} != managedCount {} + drawn-foreign {}",
                w.census.managed(),
                w.managed_count,
                w.foreign_drawn
            ),
        );
    }
    assert_eq!(
        w.census.shrinks_below_draw(w.derived_req),
        w.derived_req < w.census.drawn(),
        "{file} step {i}: shrink-below-draw is `requirement < drawn`"
    );
    match drift_disposition(FAMILY, w.census, w.derived_req, &CEILING) {
        Ok(d) => {
            assert!(
                !w.refusal,
                "{file} step {i}: the refusal flag is set but drift accepts"
            );
            assert_eq!(d.prune, w.census.foreign_free(), "{file} step {i}: prune");
            assert!(
                d.destroy <= w.census.managed_free(),
                "{file} step {i}: destroy stays within the free managed set (drawn never a victim)"
            );
            assert!(
                d.create >= 0 && d.destroy >= 0 && d.prune >= 0,
                "{file} step {i}: non-negative deltas"
            );
        }
        Err(sbd) => {
            assert_eq!(
                sbd.requirement, w.derived_req,
                "{file} step {i}: refusal req"
            );
            assert_eq!(
                sbd.drawn,
                w.census.drawn(),
                "{file} step {i}: refusal drawn count"
            );
        }
    }
}

/// The per-transition conformance: the enabling predicate that let the model action fire is
/// driven and asserted true (`pool_lifecycle` `createPooledAt`/`growCreateAt`/
/// `shrinkDestroyAt`/`pruneAt` guards). draw/return/plug/retarget carry no population delta
/// and are covered by [`check_state`] alone.
fn check_transition(file: &str, i: usize, prev: &PoolWorld, w: &PoolWorld) {
    let dpop = w.census.population() - prev.census.population();
    if dpop > 0 {
        // A create fired: the census ceiling gate admitted it (DPBP-I7).
        if !prev.census.admits_create(&CEILING) {
            finding(file, i, "a create fired past the disabled census gate");
        }
        if w.managed_count > prev.managed_count
            && !prev.census.grow_enabled(prev.derived_req, &CEILING)
        {
            finding(file, i, "a managed grow fired without an enabled deficit");
        }
    } else if dpop < 0 {
        if w.managed_count < prev.managed_count {
            // A managed shrink: surplus with a free managed victim.
            if !prev.census.shrink_enabled(prev.derived_req) {
                finding(file, i, "a shrink fired without an enabled surplus");
            }
        } else {
            // A prune: an undeclared free object reclaimed (the census foreign-free).
            let d = drift_disposition(FAMILY, prev.census, prev.derived_req, &CEILING)
                .unwrap_or_else(|_| finding(file, i, "a prune fired at a refusing census"));
            if !(prev.census.foreign_free() > 0 && d.prune > 0) {
                finding(file, i, "a prune fired with no foreign-free object");
            }
        }
    }
}

/// The last decoded world of a run (the state before a `.fail()` sentinel, or the final
/// state).
fn last_world(steps: &[PoolStep]) -> &PoolWorld {
    steps
        .iter()
        .rev()
        .find_map(|s| match s {
            PoolStep::World(w) => Some(w),
            PoolStep::Refused => None,
        })
        .expect("a run has at least one world")
}

#[test]
fn pool_traces_replay_green() {
    for (file, face) in TRACES {
        let steps = parse_pool_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            matches!(steps.first(), Some(PoolStep::World(_))),
            "{file} ({face}): a directed run starts at a world"
        );
        let mut prev: Option<&PoolWorld> = None;
        for (i, step) in steps.iter().enumerate() {
            match step {
                // A `.fail()` step is a disabled guard, always the run's terminal state.
                PoolStep::Refused => assert_eq!(
                    i,
                    steps.len() - 1,
                    "{file} ({face}): a refused (.fail) step must be terminal"
                ),
                PoolStep::World(w) => {
                    check_state(file, i, w);
                    if let Some(p) = prev {
                        check_transition(file, i, p, w);
                    }
                    prev = Some(w);
                }
            }
        }
    }
}

/// DPBP-I7 (ADR-0011): at the ceiling a create is refused as a disabled guard. The frozen
/// `.fail()` tail leaves the ceiling-full world as the last decoded state; the census gate
/// and the grow guard are both off over it.
#[test]
fn census_refuses_at_the_ceiling() {
    let steps = parse_pool_trace(&load(
        "dpbp_lifecycle::pool_lifecycle::censusRefusesAtCeilingTest",
    ))
    .unwrap();
    let w = last_world(&steps);
    assert!(
        !w.census.admits_create(&CEILING),
        "the population is at the ceiling, so a create is disabled"
    );
    assert!(
        !w.census.grow_enabled(w.derived_req, &CEILING),
        "no grow fires past the census gate"
    );
}

/// A requirement below the drawn count surfaces the typed [`ShrinkBelowDraw`] refusal — never
/// a teardown of a live consumer (pool-objects design D3). The refusal-flagged world drives
/// the oracle refusal, matching by requirement and drawn count.
#[test]
fn shrink_below_draw_refuses_by_name_and_count() {
    let steps = parse_pool_trace(&load(
        "dpbp_lifecycle::pool_lifecycle::shrinkBelowDrawRefusedTest",
    ))
    .unwrap();
    let w = steps
        .iter()
        .find_map(|s| match s {
            PoolStep::World(w) if w.refusal => Some(w),
            _ => None,
        })
        .expect("the run reaches the refusal flag");
    let err = drift_disposition(FAMILY, w.census, w.derived_req, &CEILING)
        .expect_err("a requirement below draw refuses");
    assert_eq!(
        err,
        ShrinkBelowDraw {
            family: FAMILY,
            requirement: w.derived_req,
            drawn: w.census.drawn(),
        }
    );
}

/// A drawn individual is never a shrink victim (pool-objects design D3): even at a surplus the
/// oracle's destroy count stays within the free managed set, so the drawn object the model
/// refused to tear down (`shrinkDestroyAt(first).fail()`) survives.
#[test]
fn drawn_individual_is_never_a_shrink_victim() {
    let steps = parse_pool_trace(&load(
        "dpbp_lifecycle::pool_lifecycle::drawnNeverShrunkTest",
    ))
    .unwrap();
    let w = last_world(&steps);
    assert!(w.census.drawn() > 0, "the run holds a drawn individual");
    let d = drift_disposition(FAMILY, w.census, w.derived_req, &CEILING)
        .expect("a free shrink, not a refusal");
    assert!(d.destroy > 0, "the surplus shrinks");
    assert!(
        d.destroy <= w.census.managed_free(),
        "the shrink takes only free managed individuals, so the drawn one survives"
    );
}

/// A tampered census must break the diff — the replay is load-bearing, not a rubber stamp
/// (mirrors `dpni_replay.rs`'s divergence guard). The converged final world drifts to nothing;
/// a census carrying one extra foreign-free object drifts to a prune.
#[test]
fn replay_detects_a_diverging_world() {
    let steps =
        parse_pool_trace(&load("dpbp_lifecycle::pool_lifecycle::convergenceGrowTest")).unwrap();
    let w = last_world(&steps);
    let frozen = drift_disposition(FAMILY, w.census, w.derived_req, &CEILING);
    let c = w.census;
    // One extra undeclared free object (population/free/foreign_free all +1 keeps the census
    // invariants) — the oracle must now emit a prune, diverging from the converged frozen world.
    let tampered = PoolCensus::new(
        c.population() + 1,
        c.free() + 1,
        c.drawn(),
        c.born(),
        c.foreign_free() + 1,
        c.born_drawn(),
    );
    assert_ne!(
        frozen,
        drift_disposition(FAMILY, tampered, w.derived_req, &CEILING),
        "an extra foreign-free object must diverge from the converged world"
    );
}

/// The committed trace set is exactly the `TRACES` table — a dropped or unlisted trace file
/// fails CI rather than silently shrinking coverage.
#[test]
fn every_committed_trace_is_listed() {
    let dir = format!(
        "{}/../../models/traces/families/dpbp",
        env!("CARGO_MANIFEST_DIR")
    );
    let on_disk: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {dir}: {e}"))
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".itf.json").map(str::to_owned))
        .collect();
    for (file, _) in TRACES {
        assert!(
            on_disk.contains(*file),
            "listed trace {file} is missing on disk"
        );
    }
    assert_eq!(
        on_disk.len(),
        TRACES.len(),
        "an unlisted trace file is present"
    );
}

/// The decoder rejects a pool population over the ceiling — the DPBP-I7 envelope made loud (a
/// hand-built trace stands in for a regenerated one the core no longer reproduces).
#[test]
fn decode_rejects_a_population_over_the_ceiling() {
    let obj = |n: u32| {
        format!(
            r##"[{{"fam":{{"tag":"Dpbp","value":{{"#tup":[]}}}},"num":{{"#bigint":"{n}"}}}},{{"parent":{{"tag":"Some","value":{{"fam":{{"tag":"Dprc","value":{{"#tup":[]}}}},"num":{{"#bigint":"1"}}}}}},"allocatedBy":{{"tag":"None","value":{{"#tup":[]}}}}}}]"##
        )
    };
    let objs = (0..4).map(obj).collect::<Vec<_>>().join(",");
    let json = format!(
        r##"{{"states":[{{"conv":{{"derivedReq":{{"#bigint":"0"}},"managed":{{"#set":[]}},"refusal":false}},"s":{{"objs":{{"#map":[{objs}]}}}}}}]}}"##
    );
    let err = parse_pool_trace(&json).expect_err("population 4 over the ceiling must fail");
    assert!(err.contains("ceiling"), "{err}");
}
