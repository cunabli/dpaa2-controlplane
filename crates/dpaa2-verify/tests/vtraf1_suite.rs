//! V-TRAF-1 suite generation (cross-dprc-links task 6.1): the six cross-dprc-links design D10 faces
//! stitched into one object-lifecycle suite, rendered via the generator the CLI
//! drives. The committed `V-TRAF-1.sh`/`V-TRAF-1.plan.json` are generated — do
//! not edit; regenerate by running this test with `WRITE_VTRAF1=1`.
//!
//! Faithful to the vtraf1.qnt header: the frozen face traces are core-machine
//! `--mbt` traces (the adapter parses them unchanged), the two teardown-law
//! refusals ride a directed-run hook, and the child-issued-connect refusal is
//! the banked V-DPCI-1 witness cited, not re-run.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dpaa2_verify::board::adapter::{CreateArgs, MbtTrace, ModelAction, parse_mbt_trace};
use dpaa2_verify::board::generate::{
    Hook, RecoveryGuarantee, ReferenceStep, Stitched, Suite, SuiteKind, SuiteSpec, generate,
    stitch_faces,
};
use dpaa2_verify::board::safety::{RunClass, TrafficClass};

fn board_dir() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../models/board/V-TRAF-1",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// The six faces in cross-dprc-links design D10 order, each read from its frozen core-machine trace.
fn faces() -> Vec<(String, MbtTrace)> {
    let labels = [
        "1 root<->root converge",
        "2 cross-container populate->connect->bind",
        "3 child<->child generality",
        "4 create-side heal (consented rebind)",
        "5 destroy-mirror",
        "6 teardown laws (positive half)",
    ];
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let p = board_dir().join(format!("vtraf1.face{}.itf.json", i + 1));
            let json =
                std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            let trace =
                parse_mbt_trace(&json).unwrap_or_else(|e| panic!("parse face {}: {e}", i + 1));
            ((*label).to_owned(), trace)
        })
        .collect()
}

/// The face step index of the heal's kernel-bound rebind — face 4's last
/// `kernelBind`, where the dmesg law is recorded (cross-dprc-links design D5;
/// face 2 has no kernel-bound end).
fn face4_rebind_index(stitched: &Stitched) -> usize {
    let start = *stitched
        .face_markers
        .iter()
        .find(|(_, l)| l.starts_with('4'))
        .expect("face 4 marker")
        .0;
    let end = stitched
        .face_markers
        .iter()
        .find(|(_, l)| l.starts_with('5'))
        .map_or(stitched.trace.steps.len(), |(k, _)| *k);
    (start..end)
        .rfind(|&i| {
            matches!(
                stitched.trace.steps[i].action,
                ModelAction::KernelBind { .. }
            )
        })
        .expect("a kernelBind in face 4")
}

fn build() -> (Suite, Stitched) {
    let stitched = stitch_faces(&faces()).expect("stitch the six faces");
    let hook_path = "models/board/V-TRAF-1/vtraf1.hook.sh";
    let hook_contents =
        std::fs::read_to_string(board_dir().join("vtraf1.hook.sh")).expect("read hook");
    let rebind = face4_rebind_index(&stitched);
    let mut step_notes = BTreeMap::new();
    step_notes.insert(
        rebind,
        "a post-bind connect on this kernel-bound end logs ENDPOINT_CHANGED then -EPERM \
         (discarded) — the dmesg law recorded here, the face with an explicit kernel-bound end \
         (cross-dprc-links design D4); face 2 carries no kernelBind and is judged by \
         dprc_get_connection from root"
            .to_owned(),
    );
    let references = vec![ReferenceStep {
        title: "child-issued connect refused No privilege (0x4)".to_owned(),
        cites: "models/board/V-DPCI-1 sitting: a dprc connect issued from a child container \
                lacking TOPOLOGY_CHANGES_ALLOWED returns MC status 0x4 (No privilege); this \
                suite connects at the root ancestor instead (cross-dprc-links design D2), so the \
                refusal is cited, not re-run (cross-dprc-links design D10)"
            .to_owned(),
    }];
    let spec = SuiteSpec {
        id: "V-TRAF-1".to_owned(),
        run: RunClass {
            class: TrafficClass::ObjectLifecycleOnly,
            flagged: false,
        },
        kind: SuiteKind::Standard,
        trace_file: "models/board/V-TRAF-1/vtraf1.qnt".to_owned(),
        hook: Some(Hook {
            path: hook_path.to_owned(),
            contents: hook_contents,
        }),
        create_args: CreateArgs::default(),
        expected_refusals: BTreeMap::new(),
        pool_record: false,
        face_markers: stitched.face_markers.clone(),
        step_notes,
        references,
    };
    let suite =
        generate(&spec, &stitched.trace, RecoveryGuarantee::Verified).expect("generate V-TRAF-1");
    (suite, stitched)
}

#[test]
fn vtraf1_regenerates_byte_for_byte() {
    let (suite, _) = build();
    let dir = board_dir();
    let sh = dir.join("V-TRAF-1.sh");
    let plan = dir.join("V-TRAF-1.plan.json");
    let plan_json = serde_json::to_string_pretty(&suite.plan).expect("serialize plan");
    if std::env::var("WRITE_VTRAF1").is_ok() {
        std::fs::write(&sh, &suite.script).expect("write sh");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&sh, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        std::fs::write(&plan, &plan_json).expect("write plan");
        return;
    }
    let committed_sh = std::fs::read_to_string(&sh).expect("read committed V-TRAF-1.sh");
    assert_eq!(
        suite.script, committed_sh,
        "V-TRAF-1.sh is generated — do not edit; regenerate with `WRITE_VTRAF1=1 cargo test -p dpaa2-verify --test vtraf1_suite`"
    );
    let committed_plan = std::fs::read_to_string(&plan).expect("read committed V-TRAF-1.plan.json");
    assert_eq!(
        plan_json, committed_plan,
        "V-TRAF-1.plan.json is generated — do not edit"
    );
}

#[test]
fn vtraf1_covers_six_faces_in_order() {
    let (_, stitched) = build();
    let labels: Vec<&str> = stitched.face_markers.values().map(String::as_str).collect();
    assert_eq!(
        stitched.face_markers.len(),
        6,
        "all six cross-dprc-links design D10 faces present"
    );
    // The markers are keyed by step index (a BTreeMap), so values() is already in face order.
    for (i, label) in labels.iter().enumerate() {
        assert!(
            label.starts_with(&(i + 1).to_string()),
            "face {} out of order: {label}",
            i + 1
        );
    }
}

#[test]
fn vtraf1_tears_down_banks_vdpci1_and_records_dmesg_on_face4() {
    let (suite, stitched) = build();
    // The unconditional teardown trap (ADR-0003 §6) destroys every created face object.
    assert!(
        suite.script.contains("trap teardown EXIT"),
        "teardown trap present"
    );
    // The banked V-DPCI-1 witness is cited as a command-less reference step.
    let reference = suite
        .plan
        .steps
        .iter()
        .find(|s| s.reference.is_some())
        .expect("a banked reference step");
    assert!(!reference.driven, "a reference runs no board command");
    assert!(reference.probes.is_empty(), "a reference carries no probe");
    assert!(
        reference.reference.as_deref().unwrap().contains("V-DPCI-1"),
        "the reference cites the V-DPCI-1 bank"
    );
    assert!(
        suite.script.contains("banked witnesses (cited, not run)"),
        "the script carries the banked-witness block"
    );
    // The dmesg law rides exactly one step, the face-4 kernel-bound rebind.
    let rebind = face4_rebind_index(&stitched);
    let noted: Vec<usize> = suite
        .plan
        .steps
        .iter()
        .filter(|s| s.note.is_some())
        .map(|s| s.index)
        .collect();
    assert_eq!(
        noted,
        vec![rebind],
        "the dmesg record rides the face-4 rebind only"
    );
    assert!(
        suite.plan.steps[rebind]
            .note
            .as_deref()
            .unwrap()
            .contains("ENDPOINT_CHANGED"),
        "the dmesg law is recorded"
    );
    // The two directed-run refusals ride the hook (sourced after the last step),
    // not the forward trace; their text lives in the sourced hook file.
    assert_eq!(
        suite.plan.hook.as_deref(),
        Some("models/board/V-TRAF-1/vtraf1.hook.sh")
    );
    assert!(
        suite
            .script
            .contains(". \"models/board/V-TRAF-1/vtraf1.hook.sh\""),
        "the suite sources the refusal hook"
    );
    let hook = std::fs::read_to_string(board_dir().join("vtraf1.hook.sh")).expect("read hook");
    assert!(
        hook.contains("LINK-I1"),
        "the hook probes disconnect-before-destroy"
    );
    assert!(
        hook.contains("DPRC-I5"),
        "the hook probes the double-connect refusal"
    );
}

#[test]
fn vtraf1_renders_no_dpmac_token() {
    // Finding 49: no PHY dpmac participates in any face edge — the rendered steps,
    // hook, teardown, and references name no dpmac. The only dpmac tokens the script
    // may carry are the fixed total-deny safety self-check greps (every suite's
    // preamble), which reference nothing.
    let (suite, _) = build();
    let offending: Vec<&str> = suite
        .script
        .lines()
        .filter(|l| l.contains("dpmac") && !l.contains("safety-self-check"))
        .collect();
    assert!(
        offending.is_empty(),
        "the rendered V-TRAF-1 suite names no dpmac (finding 49): {offending:?}"
    );
}
