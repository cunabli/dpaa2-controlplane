//! Pins the board-suite V-ROLLBACK-1 operand offline (plan-rollback task 7.1, the
//! rule-R1 offline pre-check): the committed overcommit TOML the sitting runs through
//! `dpaa2ctl`, the draw the suite reads from the dry-run, and the exact report strings
//! the suite greps from `ensure` stdout must all agree with the shipped code, so a drift
//! in the operand, the draw derivation, or the report wording fails here instead of on
//! the board mid-sitting. No board is touched — the operand is parsed by the shipped
//! `dpaa2-config` parser, completed exactly as `compile_intent` completes it (the
//! reserved-kernel injection via `dpaa2_tools::complete_kernel`), compiled against the
//! snapshot inventory, judged by the shipped `plan_admission` against a hardware-free
//! `FakeBackend` holding that same inventory, and its report rendered by the shipped
//! renderers (`render_failure_policy`/`render_failure_disposition`/`render_admission`).
//!
//! What the suite (models/board/V-ROLLBACK-1/V-ROLLBACK-1.sh) assumes, pinned here:
//! - step 1 (dry-run) greps `on failure: rollback`, the row `dpbp draw=<D> headroom=<F>`,
//!   and the overall `admission (...) [admitted]:` header — admission must PASS before the
//!   drain, and D must be a real draw so the writer has a unit to drain past.
//! - step 2 (ensure under the drain) greps the overcommit class line, a `  unwound: ` line,
//!   a `  residue: ` line, and `re-apply refuses until headroom returns.` — the mid-run 0x8
//!   overcommit the rollback policy unwinds, leaving grow-only root-pool residue.
//!
//! The staging premise (why dpbp is the raced family and residue is reachable) lives in the
//! compiled plan's container placement: the kernel port's companions — the dpbp among them —
//! land in ROOT (grow-only residue on unwind), and the remote tenant's dpni + dpbp companion
//! land in its own CHILD container (the drained, child-population verb). The offline twin
//! crates/dpaa2-verify/tests/rollback_overcommit.rs stages the same 0x8 drain over the
//! executor; this file pins the operand and the operator-facing text around it.

use std::collections::{BTreeMap, BTreeSet};

use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::inventory::Ceiling;
use dpaa2_api::core::model::{DpniId, DprcId, ObjectRef};
use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};
use dpaa2_api::intent::Intent;
use dpaa2_api::intent::compiled::Container;
use dpaa2_api::intent::refuse::Warning;
use dpaa2_api::plan::admission::{AdmissionRefusal, GrossDraw};
use dpaa2_api::plan::capacity::{CapacityFace, FdbBudget, GlobalBudget, PoolBudget, PoolHeadroom};
use dpaa2_api::plan::rollback::{ActuationRecord, FailurePolicy, Residue};
use dpaa2_api::plan::rollback_exec::{FailureDisposition, RollbackReport};
use dpaa2_api::testkit::ref_inventory;
use dpaa2_tools::engine::Admission;

const CPUS: u32 = 16;

/// Reads, completes and compiles the committed operand from the suite dir — the
/// `compile_intent` pipeline (crates/dpaa2-tools/src/main.rs): parse, inject the reserved
/// kernel when a port names it and none is declared (`dpaa2_tools::complete_kernel`), then
/// compile against the snapshot inventory. Mirrors the vadmit1 exemplar, pointed here.
fn compile_operand(file: &str) -> dpaa2_api::intent::refuse::Compiled {
    let toml = std::fs::read_to_string(format!(
        "{}/../../models/board/V-ROLLBACK-1/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("read {file}: {e}"));
    let mut intent: Intent =
        dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("{file}: parse: {e}"));
    dpaa2_tools::complete_kernel(&mut intent, CPUS);
    dpaa2_api::intent::refuse::compile(&intent, &ref_inventory(CPUS))
        .unwrap_or_else(|e| panic!("{file}: compile: {e:?}"))
}

/// The compile accepts and carries only unknown-ceiling admit-and-warn warnings (ADR-0011):
/// the reserved kernel and the remote userspace-poll tenant draw dprc/dpio/dpcon objects whose
/// reference ceilings are Unknown, so feasibility cannot check them and compile defers them
/// (the vadmit1 precedent). Every warning is this shape — a NEW warning class is operand drift.
#[test]
fn the_committed_operand_compiles_clean_against_the_snapshot_inventory() {
    let compiled = compile_operand("intent-overcommit.toml");

    assert!(
        compiled
            .warnings
            .iter()
            .all(|w| matches!(w, Warning::UnknownCeiling { .. })),
        "every compile warning is an unknown-ceiling admit-and-warn: {:?}",
        compiled.warnings
    );
    let deferred: BTreeSet<Family> = compiled
        .warnings
        .iter()
        .filter_map(|w| match w {
            Warning::UnknownCeiling { family, .. } => Some(*family),
            Warning::UnmeasuredCombination { .. } => None,
        })
        .collect();
    assert_eq!(
        deferred,
        BTreeSet::from([Family::Dprc, Family::Dpio, Family::Dpcon]),
        "the unknown-ceiling families compile deferred are exactly dprc, dpio, dpcon: {:?}",
        compiled.warnings
    );
}

/// The gross dpbp draw D the suite's step-1 sed reads from the dry-run row — inventory-independent,
/// derived from the plan's pending creates. The suite needs D >= 1 so the writer has a unit to drain
/// past the admitted draw.
fn dpbp_draw(admission: &Admission) -> i64 {
    admission.draw.pool.get(&Family::Dpbp).copied().unwrap_or(0)
}

/// The honest offline observable, and the suite header's own precondition narrative: the
/// reference board as recorded is pool-tight (dpbp near-dry, dpcon dry), so `plan_admission`
/// against the snapshot inventory REFUSES every tight drawn budget — dpbp among them — at
/// planning time. The operator stages free headroom LIVE before the sitting (the step-1
/// PRECONDITION gate); the snapshot cannot represent that staged board. This pins the
/// necessity of that precondition: a snapshot that silently went flush would fail here.
#[test]
fn the_recorded_snapshot_refuses_the_pool_tight_intent_naming_dpbp() {
    let compiled = compile_operand("intent-overcommit.toml");
    let snapshot = FakeBackend::new().with_inventory(ref_inventory(CPUS));
    let (admission, _drift, _switches) =
        dpaa2_tools::engine::plan_admission(&compiled.plan, &snapshot)
            .expect("admission judges the operand");

    let refusals = admission
        .verdict
        .as_ref()
        .expect_err("the pool-tight snapshot refuses at planning time")
        .clone();
    assert!(
        refusals.iter().any(|r| matches!(
            r,
            AdmissionRefusal::Pool { family, .. } if *family == Family::Dpbp
        )),
        "the tight dpbp budget is named in the snapshot refusal: {refusals:?}"
    );

    assert!(
        dpbp_draw(&admission) >= 1,
        "the intent draws at least one dpbp (D >= 1)"
    );

    let table = dpaa2_tools::render::render_admission(&admission);
    assert!(
        table
            .lines()
            .any(|l| { l.trim_start().starts_with("dpbp draw=") && l.contains(" headroom=") }),
        "the dpbp draw/headroom row shape the suite's sed keys on is rendered:\n{table}"
    );
}

/// Mirrors the suite's step-1 operator PRECONDITION offline: stage free root-pool headroom
/// F = D + 1 > D for every drawn reclaimable budget (the exact `F > D` arithmetic the suite
/// checks), driven by the real draw so this never drifts from the operand. The staged objects
/// are DPL-born (empty label), so they count as free headroom and are prune-exempt — no pool
/// churn. On this staged board admission ADMITS, exactly as the operator-staged live board
/// does, and `render_admission` emits the `[admitted]:` header and `dpbp draw=D headroom=F` row
/// the suite greps.
#[test]
fn the_operator_staged_board_admits_the_intent_and_renders_the_admitted_row() {
    let compiled = compile_operand("intent-overcommit.toml");

    let snapshot = FakeBackend::new().with_inventory(ref_inventory(CPUS));
    let (snap_admission, _d, _s) = dpaa2_tools::engine::plan_admission(&compiled.plan, &snapshot)
        .expect("admission judges the operand");
    let draw = snap_admission.draw.pool.clone();

    // Stage F = draw + 1 free DPL-born objects per reclaimable drawn family (dpio is a
    // non-reclaimable seat, headroom Spent — it never refuses and needs no staging).
    let mut staged = FakeBackend::new().with_inventory(ref_inventory(CPUS));
    let mut ordinal = 1000;
    for (&family, &d) in &draw {
        if d > 0 && family != Family::Dpio {
            for _ in 0..=d {
                staged = staged.with_pool_object(
                    DprcId::ROOT,
                    ObservedPoolObject {
                        object: ObjectRef::new(family, ordinal),
                        label: RawLabel::from(""),
                        plugged: true,
                        drawn: false,
                    },
                );
                ordinal += 1;
            }
        }
    }

    let (admission, _drift, _switches) =
        dpaa2_tools::engine::plan_admission(&compiled.plan, &staged)
            .expect("admission judges the operand");
    assert!(
        admission.verdict.is_ok(),
        "the operator-staged board admits every drawn budget: {:?}",
        admission.verdict
    );
    assert!(
        dpbp_draw(&admission) >= 1,
        "the intent draws at least one dpbp (D >= 1)"
    );

    // The shapes the suite's step-1 grep keys on: the admitted overall header and a `dpbp draw=`
    // row carrying headroom and the [admitted] marker, with F > D (the suite's precondition).
    let table = dpaa2_tools::render::render_admission(&admission);
    assert!(
        table
            .lines()
            .any(|l| l.starts_with("admission (") && l.ends_with(") [admitted]:")),
        "the admitted-verdict header the suite greps for is present:\n{table}"
    );
    let dpbp_row = table
        .lines()
        .find(|l| l.trim_start().starts_with("dpbp draw="))
        .unwrap_or_else(|| panic!("a dpbp budget row is rendered:\n{table}"));
    assert!(
        dpbp_row.contains(" headroom=") && dpbp_row.trim_end().ends_with("[admitted]"),
        "the dpbp budget row carries headroom and the [admitted] marker: {dpbp_row:?}"
    );
}

#[test]
fn the_compiled_plan_stages_the_kernel_companions_in_root_and_the_tenant_in_a_child() {
    let compiled = compile_operand("intent-overcommit.toml");
    let objects = &compiled.plan.objects;

    // The kernel port's dpbp companion lands in ROOT: on unwind this is grow-only reboot-named
    // residue (ADR-0020), the `  residue: ` line the suite grades.
    assert!(
        objects.iter().any(|o| o.key().family == Family::Dpbp
            && *o.container() == Container::Root
            && o.key().tenant.is_kernel()),
        "a kernel-owned dpbp companion is staged in the root container:\n{objects:#?}"
    );

    // The remote tenant's dpni lands in its own CHILD container (RemoteOwned, VFIO).
    assert!(
        objects.iter().any(|o| o.key().family == Family::Dpni
            && matches!(o.container(), Container::Child(t) if t.as_str() == "remote")),
        "the remote tenant's dpni is staged in its child container:\n{objects:#?}"
    );

    // The remote tenant's dpbp companion lands in the SAME child container: this is the
    // child-population dpbp create the suite drains out from under (the drained verb), distinct
    // from the root residue above.
    assert!(
        objects.iter().any(|o| o.key().family == Family::Dpbp
            && matches!(o.container(), Container::Child(t) if t.as_str() == "remote")),
        "the remote tenant's dpbp companion is staged in its child container:\n{objects:#?}"
    );
}

#[test]
fn the_suite_grep_strings_re_derive_from_the_shipped_renderers() {
    // The literals are copied verbatim from models/board/V-ROLLBACK-1/V-ROLLBACK-1.sh, so a
    // drift on EITHER the renderer side or the suite side breaks this assertion.
    const DRY_RUN_POLICY: &str = "on failure: rollback";
    const OVERCOMMIT_CLASS: &str = "refused: overcommit — admission passed, but a concurrent writer drained the resource budget mid-run";
    const UNWOUND_PREFIX: &str = "  unwound: ";
    const RESIDUE_PREFIX: &str = "  residue: ";
    const REAPPLY_CLOSE: &str = "re-apply refuses until headroom returns.";
    // Step 1 (rev 4) also parses the MC-global bp row; its sed keys on this shape.
    const GLOBAL_ROW_SHAPE: &str = "mc-global dpbp draw=";
    // Step 4 (rev 4) greps the global-budget planning-time refusal in this voice.
    const STEP4_GLOBAL_REFUSAL: &str =
        "refused: mc-global pool dpbp draw 2 exceeds global free headroom 0";

    // Step 1 previews the default (rollback) policy; the dry-run greps this line.
    let policy = dpaa2_tools::render::render_failure_policy(FailurePolicy::Rollback);
    assert!(
        policy.contains(DRY_RUN_POLICY),
        "the rollback policy preview carries the dry-run grep string: {policy:?}"
    );

    // Step 2 renders the mid-run 0x8 overcommit under rollback: the class line, at least one
    // unwound record, the typed root-population residue, and the re-apply closing sentence.
    let report = RollbackReport {
        unwound: vec![ActuationRecord::DpniCreated {
            dpni: DpniId::new(7),
            container: Some(DprcId::new(2)),
        }],
        residue: vec![Residue::RootPopulation {
            object: ObjectRef::new(Family::Dpbp, 1),
        }],
        unreversed: vec![],
    };
    let failure = Error::McStatus { status: 0x8 };
    let text = dpaa2_tools::render::render_failure_disposition(
        &failure,
        &FailureDisposition::RolledBack(report),
    );

    for needle in [
        OVERCOMMIT_CLASS,
        UNWOUND_PREFIX,
        RESIDUE_PREFIX,
        REAPPLY_CLOSE,
    ] {
        assert!(
            text.contains(needle),
            "the overcommit rollback report carries the suite grep string {needle:?}:\n{text}"
        );
    }

    // A proven GlobalBudget bp row renders beside the census row (plan-rollback design D7).
    let global_adm = Admission {
        face: CapacityFace {
            pools: vec![PoolBudget {
                family: Family::Dpbp,
                headroom: PoolHeadroom::Free(5),
            }],
            globals: vec![GlobalBudget {
                family: Family::Dpbp,
                free: 1,
            }],
            fdb: FdbBudget {
                bracket: Ceiling::Unknown,
                draw: 0,
            },
        },
        draw: GrossDraw {
            pool: BTreeMap::from([(Family::Dpbp, 2)]),
            fdb: 0,
        },
        verdict: Err(vec![AdmissionRefusal::GlobalPool {
            family: Family::Dpbp,
            draw: 2,
            headroom: 1,
        }]),
    };
    let table = dpaa2_tools::render::render_admission(&global_adm);
    assert!(
        table
            .lines()
            .any(|l| l.trim_start().starts_with(GLOBAL_ROW_SHAPE) && l.contains(" headroom=")),
        "the mc-global dpbp row shape the suite's step-1 sed keys on is rendered:\n{table}"
    );

    // The step-4 line re-derives from render_admission_refusal on a dpbp GlobalPool refusal.
    let refusal = dpaa2_tools::render::render_admission_refusal(&[AdmissionRefusal::GlobalPool {
        family: Family::Dpbp,
        draw: 2,
        headroom: 0,
    }]);
    assert!(
        refusal.contains(STEP4_GLOBAL_REFUSAL),
        "the global-budget refusal voice carries the suite's step-4 grep string {STEP4_GLOBAL_REFUSAL:?}:\n{refusal}"
    );
}
