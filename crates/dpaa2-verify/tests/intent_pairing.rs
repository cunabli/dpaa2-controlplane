//! The scenario-pairing CI rung (task 3.4, design D8, bead gqf.18): every
//! `models/intent/scenarios/<name>.toml` is exactly what an operator would type
//! for the model `<name>.qnt`, and it compiles to the plan the frozen trace
//! holds. The test parses each `.toml` with the shipped `dpaa2-config` parser,
//! completes the implicit reserved kernel, compiles against the snapshot
//! inventory frozen in `<name>AcceptedTrace.itf.json`, and asserts the outcome
//! equals the trace's. An unpaired scenario — a `.qnt` with no `.toml`, a
//! `.toml` with no `.qnt`, or a `.toml` with no accepted trace — fails by name.
//!
//! This is the cargo-side twin of `models/helpers/intent-pairing.py`, which
//! enforces the qnt⇄toml stem pairing at the typecheck rung; this rung adds the
//! trace leg so `cargo test -p dpaa2-verify` refuses an unpaired scenario too
//! (formal-models spec, scenario "A scenario pair is equivalent").
//!
//! Only the accepted twins pair here: the `<name>RefusedTrace.itf.json` traces
//! have no `.toml` (they mutate the accepted intent inside the model) and are
//! covered by the replay rung in `intent_replay.rs`.

use std::collections::BTreeSet;

use dpaa2_api::kernel_tenant;
use dpaa2_verify::intent_itf::{ReplayCase, parse_case};

/// The scenario corpus directory, resolved off the crate manifest like the trace
/// directory in `intent_replay.rs`.
fn scenarios_dir() -> String {
    format!(
        "{}/../../models/intent/scenarios",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// The frozen accepted-trace path for a scenario stem.
fn accepted_trace(stem: &str) -> String {
    format!(
        "{}/../../models/intent/traces/{stem}AcceptedTrace.itf.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Reads a file, panicking with the path and error on failure.
fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// The stems of files with the given extension under the scenario directory.
fn stems(ext: &str) -> BTreeSet<String> {
    let dir = scenarios_dir();
    std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read dir {dir}: {e}"))
        .filter_map(|entry| {
            let path = entry.expect("dir entry").path();
            (path.extension()?.to_str()? == ext)
                .then(|| path.file_stem()?.to_str().map(str::to_owned))
                .flatten()
        })
        .collect()
}

#[test]
fn every_scenario_is_fully_paired() {
    let qnts = stems("qnt");
    let tomls = stems("toml");

    assert!(
        !qnts.is_empty() && !tomls.is_empty(),
        "no scenarios found under {} — a silent empty glob",
        scenarios_dir()
    );

    for stem in &qnts {
        assert!(
            tomls.contains(stem),
            "scenario `{stem}.qnt` has no operator TOML: expected {}/{stem}.toml",
            scenarios_dir()
        );
    }
    for stem in &tomls {
        assert!(
            qnts.contains(stem),
            "scenario `{stem}.toml` has no model: expected {}/{stem}.qnt",
            scenarios_dir()
        );
        let trace = accepted_trace(stem);
        assert!(
            std::path::Path::new(&trace).is_file(),
            "scenario `{stem}.toml` has no frozen accepted trace: expected {trace}"
        );
    }
}

#[test]
fn every_toml_compiles_to_its_frozen_plan() {
    for stem in stems("toml") {
        let toml = read(&format!("{}/{stem}.toml", scenarios_dir()));
        let intent =
            dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("{stem}.toml: parse: {e}"));

        let case = parse_case(&read(&accepted_trace(&stem)))
            .unwrap_or_else(|e| panic!("{stem}AcceptedTrace: {e}"));

        // The reserved kernel completion (design D1): the config parser never
        // creates a kernel Tenant — a port with no owner defaults to the
        // reserved name, and the frontend (task 3.5) injects the tenant itself.
        // The frozen trace's intent is the oracle for which shape the scenario
        // means: two scenarios (fabric, reference) declare `kernelTenant(16)`
        // first, three (router, vfabric, vwire) declare no kernel tenant. When
        // the trace intent carries the kernel, prepend `kernel_tenant(cpus)` —
        // asserting equality against that one sanctioned value keeps the
        // completion principled rather than trace-fitting.
        let mut completed = intent;
        if let Some(k) = case.intent.tenants.iter().find(|t| t.name.is_kernel()) {
            let kernel = kernel_tenant(i64::from(case.inv.cpus));
            assert_eq!(
                *k, kernel,
                "{stem}: the implicit kernel is exactly kernel_tenant(cpus) (design D1)"
            );
            completed.tenants.insert(0, kernel);
        }

        assert_eq!(
            completed, case.intent,
            "{stem}.toml: parsed+completed intent diverges from the frozen trace intent"
        );

        let replay = ReplayCase {
            intent: completed,
            inv: case.inv,
            outcome: case.outcome.clone(),
        };
        assert_eq!(
            replay.rust_outcome(),
            case.outcome,
            "{stem}.toml: compile against the snapshot inventory diverges from the frozen plan"
        );
    }
}
