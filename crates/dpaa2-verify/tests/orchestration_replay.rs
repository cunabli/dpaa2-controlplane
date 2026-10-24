//! The orchestration ITF-replay CI rung (resource-aware-orchestration task 3.2): the six frozen
//! orchestration traces replay green against the plan-entry family dispatch, and the dispatch's
//! row set reconciles with the model's `TABLE` — board-free. The model is the oracle; a
//! regenerated trace or a drifted dispatch fails here loudly (regenerate with
//! `pnpm model:freeze-orch`).
//!
//! The orchestration model is an interface projection (ADR-0023 design D11), so the replayable
//! Rust truth is the dispatch table plus the judgments its dispositions derive, NOT the engine's
//! IO passes. The ITF carries each census-schema state but not the action between two states, so
//! the replayer reduces each state to its [`OrchState`] and asserts, per consecutive pair, that
//! the dispatch agrees with what the trace shows for the seams Rust owns:
//!
//!   - every family reaching `Converged` (a create or a reconcile) has an `Execute` dispatch row;
//!   - every family refused with its plan drained (the typed Unimplemented refusal) has an
//!     `Unimplemented` row — dpseci, the first such row (ADR-0023 design D10);
//!   - an admission refusal (the verdict turning `AdmissionRefused`) marks exactly the drawing
//!     `Execute` families with pending, never an `Unimplemented` family, and drains no plan
//!     (ADR-0023 design D5/D8 gross draw vs re-observation);
//!   - any trace family with no dispatch row is exactly dpmac, the twin's one sanctioned
//!     asymmetry (ADR-0023 design D10/D11).
//!
//! The registry twin ([`row_set_findings`]) is checked against the real `pipeline.qnt` and the
//! real dispatch table here; its negative (mutated-input) cases live beside the comparator in
//! `src/intent/orchestration_itf.rs`.

use std::collections::BTreeSet;

use dpaa2_api::core::family::Family;
use dpaa2_api::plan::dispatch::{self, Disposition};
use dpaa2_verify::intent::orchestration_itf::{
    OrchState, Status, Verdict, parse_orchestration_trace, row_set_findings,
};

/// Every committed trace under `models/traces/orchestration/`, with the behavior it pins.
const TRACES: &[(&str, &str)] = &[
    (
        "convergeTest",
        "ORCH-I4: admit, create every drawing family in dependency order, refuse dpseci, wire the port",
    ),
    (
        "handoverSafeTest",
        "ORCH-I2: the mac driver holds the dpmac, the wire defers, then fires once it reads back free",
    ),
    (
        "admissionRefusesTest",
        "ORCH-I3/D8: the eaten pool headroom refuses the gross draw whole and typed, no verb dispatches",
    ),
    (
        "stableRefusalTest",
        "ORCH-I3/D5: a refused intent re-observed and re-judged refuses identically — no oscillation",
    ),
    (
        "unimplementedRefusesTest",
        "ORCH-I1/D10: dpseci, the Unimplemented row, refuses typed and drains its plan",
    ),
    (
        "envInterferenceTest",
        "D7: a converged dpni drifts under the environment and the next pass re-converges it",
    ),
];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/traces/orchestration/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// The dispatch row for family `f`, if any (dpmac has none — the sanctioned asymmetry).
fn row_for(f: Family) -> Option<&'static Disposition> {
    dispatch::TABLE
        .iter()
        .find(|(x, _)| *x == f)
        .map(|(_, d)| d)
}

/// The per-pair obligations: the dispatch agrees step by step with what the trace shows.
fn check_pair(file: &str, i: usize, prev: &OrchState, cur: &OrchState) {
    for (f, disp) in dispatch::TABLE {
        let name = f.variant_name();

        // A create or reconcile — a family reaching Converged — needs an Execute row.
        if matches!(prev.status[f], Status::Missing | Status::Drifted)
            && cur.status[f] == Status::Converged
        {
            assert!(
                matches!(disp, Disposition::Execute { .. }),
                "{file} step {i}: {name} reaches Converged but its dispatch row is {disp:?}, not Execute"
            );
        }

        // The typed Unimplemented refusal: newly refused with its plan drained to zero.
        if !prev.refused[f] && cur.refused[f] && prev.pending[f] > 0 && cur.pending[f] == 0 {
            assert!(
                matches!(disp, Disposition::Unimplemented { .. }),
                "{file} step {i}: {name} is refused with its plan drained but its dispatch row is {disp:?}, not Unimplemented"
            );
        }
    }

    // An admission refusal (the verdict turning AdmissionRefused): the refusal set is exactly the
    // drawing Execute families with pending, no Unimplemented family, and no plan is drained.
    if prev.verdict != Verdict::AdmissionRefused && cur.verdict == Verdict::AdmissionRefused {
        for (f, disp) in dispatch::TABLE {
            let name = f.variant_name();
            assert_eq!(
                cur.pending[f], prev.pending[f],
                "{file} step {i}: admission drained {name}'s plan (ADR-0023 design D8 is draw-not-dispatch)"
            );
            match disp {
                Disposition::Execute { .. } => {
                    if prev.pending[f] > 0 && !prev.refused[f] {
                        assert!(
                            cur.refused[f],
                            "{file} step {i}: admission refused but left drawing family {name} unrefused"
                        );
                    }
                }
                Disposition::Unimplemented { .. } => assert_eq!(
                    cur.refused[f], prev.refused[f],
                    "{file} step {i}: admission refused the Unimplemented family {name}"
                ),
            }
        }
    }
}

#[test]
fn orchestration_traces_replay_green() {
    for (file, face) in TRACES {
        let states =
            parse_orchestration_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            states.len() >= 2,
            "{file} ({face}): a directed run has at least two populated states"
        );

        // The run starts at init: unjudged, nothing wired, no family refused.
        let init = &states[0];
        assert_eq!(
            init.verdict,
            Verdict::Unjudged,
            "{file} ({face}): init is Unjudged"
        );
        assert!(
            !init.port_wired,
            "{file} ({face}): init leaves the port unwired"
        );
        for (f, _) in dispatch::TABLE {
            assert!(
                !init.refused[f],
                "{file} ({face}): init refuses {}",
                f.variant_name()
            );
        }

        // Any trace family with no dispatch row is exactly dpmac (ADR-0023 design D10/D11).
        let seen: BTreeSet<Family> = states
            .iter()
            .flat_map(|s| s.status.keys().copied())
            .collect();
        for f in seen {
            if row_for(f).is_none() {
                assert_eq!(
                    f,
                    Family::Dpmac,
                    "{file} ({face}): trace family {} has no dispatch row and is not the dpmac exemption",
                    f.variant_name()
                );
            }
        }

        for (i, pair) in states.windows(2).enumerate() {
            check_pair(file, i + 1, &pair[0], &pair[1]);
        }
    }
}

/// The registry twin is green on the real model and the real dispatch table — the dispatch's row
/// set reconciles with `pipeline.qnt`'s `TABLE`, dpmac the one sanctioned asymmetry
/// (resource-aware-orchestration task 3.2; ADR-0023 design D10/D11).
#[test]
fn row_set_twin_agrees_with_the_model() {
    let qnt = {
        let path = format!(
            "{}/../../models/orchestration/pipeline.qnt",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
    };
    let out = row_set_findings(&qnt, dpaa2_api::plan::dispatch::TABLE);
    assert!(out.is_empty(), "the row-set twin found drift: {out:?}");
}

/// The committed trace set is exactly `TRACES` — a dropped or unlisted trace fails CI rather than
/// silently shrinking coverage (the `dpsw_replay.rs` discipline).
#[test]
fn every_committed_trace_is_listed() {
    let dir = format!(
        "{}/../../models/traces/orchestration",
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
