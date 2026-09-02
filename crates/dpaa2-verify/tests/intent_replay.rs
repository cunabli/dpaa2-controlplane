//! The intent ITF-replay CI rung (task 3.2, design D9): every committed intent
//! trace replays green against the Rust `compile`, board-free. The model is the
//! oracle — a regenerated trace whose model outcome no longer equals what
//! `compile` derives fails here loudly; regenerate with `pnpm model:freeze-intent`
//! and reconcile the transcription.
//!
//! Coverage: one Ok per scenario family and every refused twin, so both arms of
//! `compile` replay (models/intent/replay.qnt).

use dpaa2_verify::intent_itf::parse_case;

/// Every committed trace under `models/intent/traces/`, with the arm it exercises.
const TRACES: &[(&str, Arm)] = &[
    ("fabricAcceptedTrace", Arm::Ok),
    ("fabricRefusedTrace", Arm::Refused),
    ("vfabricAcceptedTrace", Arm::Ok),
    ("vfabricRefusedTrace", Arm::Refused),
    ("routerAcceptedTrace", Arm::Ok),
    ("routerRefusedTrace", Arm::Refused),
    ("vwireAcceptedTrace", Arm::Ok),
    ("vwireRefusedTrace", Arm::Refused),
    ("referenceAcceptedTrace", Arm::Ok),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Arm {
    Ok,
    Refused,
}

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/intent/traces/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn intent_traces_replay_green() {
    for (file, arm) in TRACES {
        let case = parse_case(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        let observed = if case.outcome.is_ok() {
            Arm::Ok
        } else {
            Arm::Refused
        };
        assert_eq!(observed, *arm, "{file}: unexpected compile arm");
        assert_eq!(
            case.rust_outcome(),
            case.outcome,
            "{file}: Rust compile diverges from the model's frozen outcome"
        );
    }
}

#[test]
fn replay_detects_a_diverging_outcome() {
    // Mutating the intent under a frozen outcome must break the diff: the
    // fabric accepted intent with its budget starved is refused, not the Ok
    // the trace froze — the comparator must catch the mismatch.
    let mut case = parse_case(&load("fabricAcceptedTrace")).expect("parse");
    for t in &mut case.intent.tenants {
        if t.name.as_str() == "router" {
            t.max_cores = 1;
        }
    }
    assert_ne!(
        case.rust_outcome(),
        case.outcome,
        "a starved budget must diverge from the frozen Ok"
    );
}
