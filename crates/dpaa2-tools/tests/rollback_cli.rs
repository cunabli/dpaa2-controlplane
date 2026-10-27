//! Snapshot tests over the mid-run failure-policy preview and the rollback-disposition render
//! (plan-rollback design D5; spec "Rollback outcomes are rendered in operator data terms"). The
//! rendered text is the operator's data vocabulary, board-free: inputs are hand-built public
//! values, no backend. Each disposition render is also asserted free of internal type names
//! (`::`) and process anchors (`ADR`), the operator-data-terms contract.

use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::model::{DpniId, DprcId, ObjectRef};
use dpaa2_api::plan::rollback::{ActuationRecord, FailurePolicy, Residue};
use dpaa2_api::plan::rollback_exec::{FailureDisposition, RollbackReport};
use dpaa2_tools::render::{render_failure_disposition, render_failure_policy};

/// The rollback preview line the dry-run prints last.
#[test]
fn failure_policy_rollback_line() {
    insta::assert_snapshot!(render_failure_policy(FailurePolicy::Rollback));
}

/// The abort preview line the dry-run prints last.
#[test]
fn failure_policy_abort_line() {
    insta::assert_snapshot!(render_failure_policy(FailurePolicy::Abort));
}

/// An overcommit (MC status `0x8`) rollback report with one unwound actuation, a residue of each
/// variant, and one unreversed row: the drained-budget framing and the re-apply-refuses-until-
/// headroom sentence must both appear, in operator data terms (plan-rollback design D5).
#[test]
fn overcommit_rollback_report_is_actionable() {
    let report = RollbackReport {
        unwound: vec![ActuationRecord::DpniCreated {
            dpni: DpniId::new(7),
            container: None,
        }],
        residue: vec![
            Residue::RootPopulation {
                object: ObjectRef::new(Family::Dpbp, 1),
            },
            Residue::DpioSeat {
                object: ObjectRef::new(Family::Dpio, 16),
            },
        ],
        unreversed: vec![ActuationRecord::SwitchPortConnected {
            ancestor: DprcId::ROOT,
            dpsw: ObjectRef::new(Family::Dpsw, 9),
            ifx: 0,
            peer: ObjectRef::new(Family::Dpmac, 7),
        }],
    };
    let failure = Error::McStatus { status: 0x8 };
    let text = render_failure_disposition(&failure, &FailureDisposition::RolledBack(report));
    assert!(
        text.contains("overcommit"),
        "mid-run class is named: {text}"
    );
    assert!(!text.contains("ADR"), "no process anchors: {text}");
    assert!(!text.contains("::"), "no internal type names: {text}");
    assert!(
        text.contains("re-apply refuses until headroom returns."),
        "{text}"
    );
    insta::assert_snapshot!(text);
}

/// A portal-busy give-up (the single-opener collision, bead dpaa2-controlplane-f59) under rollback:
/// the first line is honest mid-run voice naming the busy portal, never the refusal voice, while the
/// disposition lines render exactly as any other rollback (plan-rollback design D5; ADR-0006).
#[test]
fn portal_busy_renders_honest_not_refused() {
    let report = RollbackReport {
        unwound: vec![ActuationRecord::DpniCreated {
            dpni: DpniId::new(7),
            container: None,
        }],
        residue: vec![],
        unreversed: vec![],
    };
    let failure = Error::PortalBusy {
        attempts: 5,
        detail: "open() failed : Invalid argument".to_owned(),
    };
    let text = render_failure_disposition(&failure, &FailureDisposition::RolledBack(report));
    assert!(text.contains("portal"), "names the portal: {text}");
    assert!(text.contains("busy"), "names busy: {text}");
    assert!(!text.contains("refused"), "never the refusal voice: {text}");
    assert!(
        text.contains("unwound: created dpni dpni.7"),
        "disposition lines stay: {text}"
    );
    assert!(!text.contains("ADR"), "no process anchors: {text}");
    assert!(!text.contains("::"), "no internal type names: {text}");
}

/// A non-overcommit failure under abort, with a two-record executed prefix: the plain refusal line
/// and the standing prefix in observed terms, no drained-budget framing (plan-rollback design D5).
#[test]
fn aborted_run_leaves_the_prefix_standing() {
    let prefix = vec![
        ActuationRecord::DpniCreated {
            dpni: DpniId::new(7),
            container: None,
        },
        ActuationRecord::DpniBound {
            dpni: DpniId::new(7),
        },
    ];
    let failure = Error::Backend("the dpni bind verb failed".to_owned());
    let text = render_failure_disposition(&failure, &FailureDisposition::Aborted { prefix });
    assert!(!text.contains("ADR"), "no process anchors: {text}");
    assert!(!text.contains("::"), "no internal type names: {text}");
    insta::assert_snapshot!(text);
}
