//! The committed board artifacts under `models/board/` must stay
//! readable by the code that consumes them: the driver's probe-plan
//! parser and envelope, and the adapter's `--mbt` trace parser. A plan
//! or trace that stops parsing — or a plan that stops clearing the
//! safety envelope — fails here rather than on the board, in front of an
//! operator, mid-sitting.

use std::path::PathBuf;

use dpaa2_verify::adapter::{CreateArgs, parse_mbt_trace};
use dpaa2_verify::driver::{check_plan, parse_probe_plan};
use dpaa2_verify::generate::{RecoveryGuarantee, SuiteKind, SuiteSpec, generate};
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
    // V-DPRTC-1/2/3 (plus V-DPRTC-3's postboot half), V-DPDBG-1, the
    // task-5.9 refusal plans V-DPAIOP-1 / V-DPSECI-1 / V-DPNI-2, and the
    // task-4.1 fit-check plan V-FIT-1: a count that drops means a plan was
    // renamed out of the driver's reach, which would pass silently as an
    // empty walk.
    assert!(
        plans.len() >= 9,
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

/// Committed passed scripts predate the per-step stderr and kernel-log
/// capture and are deliberately not rewritten (the README's regenerate
/// rule), so the guarantee is checked on a fresh regeneration of a
/// committed trace: the emitter names both the per-step `step-N-err.txt`
/// and the teardown's `dmesg.txt`.
#[test]
fn a_committed_trace_regenerates_with_stderr_and_kernel_log_capture() {
    let path = format!(
        "{}/../../models/board/V-READBACK-1/vreadback1.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let json = std::fs::read_to_string(&path).expect("read committed trace");
    let trace = parse_mbt_trace(&json).expect("parse trace");
    let spec = SuiteSpec {
        id: "V-READBACK-1".to_owned(),
        run: RunClass {
            class: TrafficClass::ObjectLifecycleOnly,
            flagged: false,
        },
        kind: SuiteKind::Standard,
        trace_file: path.clone(),
        hook: None,
        create_args: CreateArgs::default(),
        expected_refusals: std::collections::BTreeMap::new(),
    };
    let suite = generate(&spec, &trace, RecoveryGuarantee::Verified).expect("generate");
    assert!(suite.script.contains("step-$1-err.txt"), "{}", suite.script);
    assert!(suite.script.contains("dmesg.txt"), "{}", suite.script);
}

/// An expected-refusal probe step parses, carries the status name,
/// clears the envelope, and the parser rejects a refusal on an
/// instruction step or with an unknown status name.
#[test]
fn expected_refusal_probe_steps_parse_and_are_validated() {
    let ok = r#"{
      "suite": "V-DPRTC-1",
      "class": "lifecycle",
      "steps": [
        {
          "label": "second dprtc create refused",
          "expect": "refused with No privilege",
          "cmd": ["restool", "dprtc", "create", "--container=dprc.1"],
          "refusal": "No privilege"
        }
      ]
    }"#;
    let plan = parse_probe_plan(ok).expect("parses");
    assert_eq!(plan.steps[0].refusal.as_deref(), Some("No privilege"));
    let run = RunClass {
        class: plan.class,
        flagged: false,
    };
    check_plan(run, &plan).expect("clears the envelope");

    // A refusal on an instruction step runs no command to refuse.
    let on_instruction = r#"{"suite":"S","class":"lifecycle","steps":[{"label":"l","expect":"e","instruction":"reboot","refusal":"No privilege"}]}"#;
    assert!(
        parse_probe_plan(on_instruction)
            .unwrap_err()
            .contains("instruction")
    );

    // An unknown MC status name is rejected.
    let unknown = r#"{"suite":"S","class":"lifecycle","steps":[{"label":"l","expect":"e","cmd":["restool","x"],"refusal":"Nope"}]}"#;
    assert!(
        parse_probe_plan(unknown)
            .unwrap_err()
            .contains("MC status name")
    );
}

/// Each plan points at the source it was generated from — a trace's
/// `trace_file`, a fit check's `probes_file` (task 4.1). Nothing reads the
/// field after generation, so this is the only thing that keeps it true
/// when a source moves or is renamed.
#[test]
fn committed_plan_trace_files_resolve() {
    let plans = board_files(".plan.json");
    // A count that drops means a plan was renamed out of reach, which
    // would pass silently as an empty walk.
    assert!(
        plans.len() >= 37,
        "expected the committed plans under models/board, found {plans:?}"
    );
    let root = format!("{}/../../", env!("CARGO_MANIFEST_DIR"));
    for path in plans {
        let json = std::fs::read_to_string(&path).expect("read committed plan");
        let plan: serde_json::Value = serde_json::from_str(&json).expect("parse plan");
        let source = plan["trace_file"]
            .as_str()
            .or_else(|| plan["probes_file"].as_str())
            .expect("trace_file or probes_file string");
        let resolved = PathBuf::from(&root).join(source);
        assert!(
            resolved.exists(),
            "{}: source {source} missing at {}",
            path.display(),
            resolved.display()
        );
    }
}

/// The do-not-edit invariant on the fit-check sitting: regenerating from
/// the committed `models/board/V-FIT-1/probes.json` reproduces the
/// committed `V-FIT-1.sh` byte-for-byte (task 4.1, design D12). A hand
/// edit to either the script or the emitter breaks this.
#[test]
fn the_committed_fit_check_regenerates_byte_for_byte() {
    use dpaa2_verify::driver::parse_probe_plan;
    use dpaa2_verify::fitcheck::generate_fit;

    let root = format!("{}/../../", env!("CARGO_MANIFEST_DIR"));
    let probes_file = "models/board/V-FIT-1/probes.json";
    let json =
        std::fs::read_to_string(PathBuf::from(&root).join(probes_file)).expect("read probes.json");
    let plan = parse_probe_plan(&json).expect("parse probe plan");
    let suite = generate_fit(&plan, probes_file).expect("generate fit check");

    let committed =
        std::fs::read_to_string(PathBuf::from(&root).join("models/board/V-FIT-1/V-FIT-1.sh"))
            .expect("read committed V-FIT-1.sh");
    assert_eq!(
        suite.script, committed,
        "V-FIT-1.sh is generated — do not edit; regenerate with `dpaa2-verify generate --probes {probes_file} --out models/board/V-FIT-1`"
    );
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
