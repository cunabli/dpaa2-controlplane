//! The committed board artifacts under `models/board/` must stay
//! readable by the code that consumes them: the driver's probe-plan
//! parser and envelope, and the adapter's `--mbt` trace parser. A plan
//! or trace that stops parsing — or a plan that stops clearing the
//! safety envelope — fails here rather than on the board, in front of an
//! operator, mid-sitting.

use std::path::PathBuf;

use dpaa2_verify::adapter::parse_mbt_trace;
use dpaa2_verify::driver::{check_plan, parse_probe_plan};
use dpaa2_verify::safety::{RunClass, TrafficClass};

/// Every committed artifact whose name ends in `suffix`, one per
/// scenario directory (`models/board/<id>/`).
fn board_files(suffix: &str) -> Vec<PathBuf> {
    let board = format!("{}/../../models/board", env!("CARGO_MANIFEST_DIR"));
    let mut found: Vec<PathBuf> = std::fs::read_dir(&board)
        .expect("models/board")
        .filter_map(|e| {
            let dir = e.expect("scenario dir").path();
            dir.is_dir().then_some(dir)
        })
        .flat_map(|dir| {
            std::fs::read_dir(&dir)
                .expect("scenario dir")
                .map(|e| e.expect("artifact").path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.ends_with(suffix))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    found.sort();
    found
}

#[test]
fn committed_probe_plans_parse_and_clear_the_envelope() {
    let plans = board_files("probes.json");
    // V-DPRTC-1/2/3 (plus V-DPRTC-3's postboot half) and V-DPDBG-1: a
    // count that drops means a plan was renamed out of the driver's
    // reach, which would pass silently as an empty walk.
    assert!(
        plans.len() >= 5,
        "expected the committed probe plans under models/board, found {plans:?}"
    );
    for path in plans {
        let json = std::fs::read_to_string(&path).expect("read committed plan");
        let plan = parse_probe_plan(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        // The class the plan declares is the class it runs under, and
        // link/traffic classes are refused unflagged (ADR-0003 §5).
        let run = RunClass {
            class: plan.class,
            flagged: plan.class != TrafficClass::ObjectLifecycleOnly,
        };
        check_plan(run, &plan).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}

#[test]
fn committed_board_traces_parse() {
    let traces = board_files(".itf.json");
    assert!(
        traces.len() >= 16,
        "expected the committed board traces under models/board, found {traces:?}"
    );
    for path in traces {
        let json = std::fs::read_to_string(&path).expect("read committed trace");
        parse_mbt_trace(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}
