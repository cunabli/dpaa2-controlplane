//! The ITF-replay CI rung (task 3.2): every committed retro trace
//! replays green against the reconciler, with no board attached.
//!
//! The epoch tables transcribe the observation points documented in
//! `models/retro/reconciler.qnt`; a regenerated trace with a different
//! shape fails here loudly — update both together.

use dpaa2_api::{Presence, ReconcileOptions};
use dpaa2_verify::itf::{ModelView, parse_trace};
use dpaa2_verify::replay::{RetroTrace, replay};

const TRACES: &[RetroTrace] = &[
    RetroTrace {
        file: "retroAssociationTest.itf.json",
        port: 7,
        presence: Presence::Present,
        options: ReconcileOptions { prune: false },
        observations: &[0, 15, 16],
    },
    RetroTrace {
        file: "retroTeardownTest.itf.json",
        port: 7,
        presence: Presence::Absent,
        options: ReconcileOptions { prune: true },
        observations: &[16, 19],
    },
];

fn load(file: &str) -> Vec<ModelView> {
    let path = format!("{}/../../models/traces/{file}", env!("CARGO_MANIFEST_DIR"));
    parse_trace(&std::fs::read_to_string(&path).expect("read committed trace"))
        .expect("parse committed trace")
}

#[test]
fn retro_traces_replay_green() {
    for spec in TRACES {
        replay(&load(spec.file), spec).unwrap_or_else(|e| panic!("{e}"));
    }
}

#[test]
fn replay_detects_a_wrong_reconciler_decision() {
    // Same teardown trace, but without prune the reconciler refuses the
    // teardown the model performed — the diff must catch it.
    let spec = RetroTrace {
        options: ReconcileOptions { prune: false },
        ..TRACES[1]
    };
    let err = replay(&load(spec.file), &spec).unwrap_err();
    assert!(err.contains("model expects"), "unexpected error: {err}");
}
