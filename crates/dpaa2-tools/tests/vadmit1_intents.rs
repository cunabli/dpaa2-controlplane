//! Pins the board-suite V-ADMIT-1 operand offline (resource-aware-orchestration
//! task 6.1, bead dpaa2-controlplane-m7a.6): the committed over-budget TOML the
//! sitting runs through `dpaa2ctl` must compile clean and draw decisively over
//! the reference board's dpcon budget, so a drift in the operand (or in the draw
//! derivation) fails here instead of on the board mid-sitting. No board is
//! touched — the operand is parsed by the shipped `dpaa2-config` parser,
//! completed exactly as `compile_intent` completes it (the reserved-kernel
//! injection via `dpaa2_tools::complete_kernel`), compiled against the snapshot
//! inventory, and judged by the shipped `plan_admission` against a hardware-free
//! `FakeBackend` holding that same inventory and no pool objects.
//!
//! Why dpcon, not dpbp — the ceiling-vs-headroom distinction this suite turns on.
//! `compile`'s feasibility check (ADR-0011) refuses a plan whose family count
//! exceeds a *known* ceiling. In `ref_inventory` dpbp carries `Ceiling::Counted(63)`,
//! so a ~200-unit raise on dpbp is refused by compile as `Refusal::Infeasible`
//! before admission ever runs — compile preempts the admission witness. dpcon
//! carries `Ceiling::Unknown`, so feasibility skips it (emitting the
//! `Warning::UnknownCeiling` this test asserts) and the plan compiles clean; the
//! ~200-unit dpcon draw is then refused *purely* by `plan_admission` against the
//! observed free headroom, which is a separate judgment from the ceiling. That
//! admission refusal — not compile's — is the ADR-0023 witness the suite exists
//! for, so the over-budget family must be dpcon.
//!
//! Acceptance (provisioning-cli spec "Ensure refuses before dispatch";
//! ADR-0023 design D5/D7/D8): the gross draw of the pending creates exceeds the
//! observed dpcon free headroom, so admission refuses with an `AdmissionRefusal::Pool`
//! naming the dpcon family and the refusal render carries the `refused: pool`
//! voice the suite's `ensure` leg greps for.

use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::core::family::Family;
use dpaa2_api::intent::Intent;
use dpaa2_api::intent::refuse::Warning;
use dpaa2_api::plan::admission::AdmissionRefusal;
use dpaa2_api::testkit::ref_inventory;

const CPUS: u32 = 16;

/// Reads, completes and compiles the committed operand from the suite dir — the
/// `compile_intent` pipeline (crates/dpaa2-tools/src/main.rs): parse, inject the
/// reserved kernel when a port names it and none is declared
/// (`dpaa2_tools::complete_kernel`), then compile against the snapshot inventory.
fn compile_operand(file: &str) -> dpaa2_api::intent::refuse::Compiled {
    let toml = std::fs::read_to_string(format!(
        "{}/../../models/board/V-ADMIT-1/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("read {file}: {e}"));
    let mut intent: Intent =
        dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("{file}: parse: {e}"));
    dpaa2_tools::complete_kernel(&mut intent, CPUS);
    dpaa2_api::intent::refuse::compile(&intent, &ref_inventory(CPUS))
        .unwrap_or_else(|e| panic!("{file}: compile: {e:?}"))
}

#[test]
fn intent_over_is_refused_naming_the_dpcon_budget() {
    let compiled = compile_operand("intent-over.toml");
    // dpcon's Unknown ceiling skips compile feasibility (ADR-0011): the UnknownCeiling
    // warning is the proof compile deferred the over-budget family to admission.
    assert!(
        compiled.warnings.iter().any(|w| matches!(
            w,
            Warning::UnknownCeiling { family, .. } if *family == Family::Dpcon
        )),
        "compile warns dpcon's ceiling is unknown: {:?}",
        compiled.warnings
    );

    // Fresh board: all companions pending, so the dpcon draw exceeds observed free headroom (ADR-0023 design D5/D8).
    let mc = FakeBackend::new().with_inventory(ref_inventory(CPUS));
    let admission = dpaa2_tools::engine::plan_admission(&compiled.plan, &mc)
        .expect("admission judges the operand");

    let refusals = admission
        .verdict
        .expect_err("the over-budget operand is refused");
    assert!(
        refusals.iter().any(|r| matches!(
            r,
            AdmissionRefusal::Pool { family, .. } if *family == Family::Dpcon
        )),
        "the dpcon budget is named: {refusals:?}"
    );

    // Creates are not audited; the live suite's pre/post census is the real ADR-0023 design D7 witness.
    assert!(
        mc.audit().is_empty(),
        "admission dispatched nothing: {:?}",
        mc.audit()
    );

    let text = dpaa2_tools::render::render_admission_refusal(&refusals);
    assert!(text.contains("refused: pool"), "{text}");
}
