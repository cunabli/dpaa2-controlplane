//! The read-only fit-check emitter (design D12; ADR-0003 §2, §4).
//!
//! The board milestone is one read-only sitting (design D12): it re-runs
//! the fit check against a live census — the container listing, the pool
//! ceilings, a DPL reconstruction, `dpmac info` on the lifecycle-safe
//! ports — and the shipped `dpaa2ctl dry-run`/`status` on the reference
//! intent, and captures everything for offline diffing. No object is
//! created, moved, destroyed or labelled.
//!
//! Unlike the batch generator ([`crate::generate`]), whose input is a
//! model trace, this emitter's input is a hand-declared ordered list of
//! observation steps in the probe-plan shape ([`crate::driver::ProbePlan`]).
//! That keeps the "generated — do not edit; regenerate instead"
//! invariant and the safety-envelope screening while letting a human
//! author read-only questions no trace can ask.
//!
//! Two guards run before anything is emitted: the input is refused unless
//! every step is read-only (no mutating restool verb, no sysfs write, no
//! operator instruction), and the rendered script is screened by the
//! safety envelope ([`crate::safety`]) exactly as the trace path is.

use std::fmt::Write as _;

use crate::driver::{ExitShape, ProbePlan, ProbeStep, ProbeVerdict, judge_exit};
use crate::generate::{REF_PAIR_ASSERT, TOTAL_DENY_GREP};
use crate::safety::{self, RunClass, TrafficClass};

/// Restool verbs that change board state. A fit check reads only, so a
/// step naming any of these refuses the whole emission (design D12: "no
/// object is created, moved or destroyed").
const MUTATING_VERBS: [&str; 8] = [
    "create",
    "destroy",
    "assign",
    "unassign",
    "connect",
    "disconnect",
    "set-label",
    "sync",
];

/// A read-only fit check runs commands only, and a lifecycle-only run is
/// its class (ADR-0003 §5: queries carry no link semantics, so the
/// mildest class holds). Screening the rendered script under this run
/// keeps a lifecycle sitting off the wired pair, dpmac.7/9 (ADR-0003 §4).
const RUN: RunClass = RunClass {
    class: TrafficClass::ObjectLifecycleOnly,
    flagged: false,
};

/// One step of the offline-judgeable fit-check plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FitStep {
    /// Step index (matches the script's step numbers and result files).
    pub index: usize,
    /// What the step probes, in a few words.
    pub label: String,
    /// The prose expectation, kept beside the step for dispositioning.
    pub expect: String,
    /// The exit shape the step is judged by (`judge_exit`); a `status`
    /// step declares `any`, so a nonzero exit is drift evidence, never a
    /// failure.
    pub exit: ExitShape,
}

/// The offline-judgeable fit-check plan. A fit check is judged on the
/// captured exit against the declared [`ExitShape`], so it carries a
/// different shape from a trace's [`crate::generate::SuitePlan`] (which
/// judges model post-states by read-back). `probes_file` records the
/// source the way a suite plan records its `trace_file`, so the
/// `committed_plan_trace_files_resolve` lint keeps the artifact honest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FitPlan {
    /// Scenario id (e.g. `V-FIT-1`).
    pub id: String,
    /// Declared traffic class (display form).
    pub class: String,
    /// The probe plan the sitting was generated from.
    pub probes_file: String,
    /// The steps, in execution order.
    pub steps: Vec<FitStep>,
}

/// A generated fit check: the reviewable read-only script and its plan.
#[derive(Debug, Clone)]
pub struct FitSuite {
    /// The shell script to review and run on the board.
    pub script: String,
    /// The plan the harness diffs offline against the result files.
    pub plan: FitPlan,
}

/// Refuses a step that is not read-only: an operator instruction (a fit
/// check runs commands only), a mutating restool verb, or a sysfs write.
fn check_read_only(i: usize, step: &ProbeStep) -> Result<&Vec<String>, String> {
    if step.instruction.is_some() {
        return Err(format!(
            "step {i} ({}): a fit check is read-only and runs commands only, not operator instructions",
            step.label
        ));
    }
    let argv = step
        .cmd
        .as_ref()
        .ok_or_else(|| format!("step {i} ({}): no command to run", step.label))?;
    if let Some(verb) = argv.iter().find(|a| MUTATING_VERBS.contains(&a.as_str())) {
        return Err(format!(
            "step {i} ({}): `{verb}` mutates board state; a fit check reads only (design D12)",
            step.label
        ));
    }
    // ponytail: a sysfs-write heuristic (a `/sys` path and a redirect in
    // the same command). Upgrade to a real shell parse only if a plan
    // ever needs to run `sh -c` against sysfs read-only.
    if argv.iter().any(|a| a.contains("/sys")) && argv.iter().any(|a| a.contains('>')) {
        return Err(format!(
            "step {i} ({}): writes a sysfs attribute; a fit check reads only (design D12)",
            step.label
        ));
    }
    Ok(argv)
}

/// Escapes a label for a double-quoted shell string.
fn sh_dquote(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '"' | '$' | '`') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The read-only script preamble: header, kernel-log window, the
/// independent safety self-check, the read-only helpers, and the
/// reference-pair assertion (ADR-0003 §2, asserted before any command).
fn preamble(id: &str, probes_file: &str) -> String {
    format!(
        r#"#!/bin/sh
# suite: {id}
# class: object-lifecycle-only
# generated by dpaa2-verify from {probes_file} — do not edit; regenerate instead.
# Read-only fit check (design D12; ADR-0003 §2): every command below only
# reads the board — the container census, the pool ceilings, a DPL
# reconstruction, dpmac info on the lifecycle-safe ports, and the shipped
# dpaa2ctl dry-run/status on the reference intent. NOTHING is created,
# moved, destroyed or labelled, so there is NO teardown trap: this sitting
# is idempotent and safe to re-run as-is. Results are captured under the
# results directory for offline diffing and dispositioning:
#   dpaa2-verify diff --plan {id}.plan.json --results <dir>
set -u
RESULTS="${{1:?usage: $0 <results-dir>}}"
mkdir -p "$RESULTS"

# --- kernel-log window ---
# A marker stamps the sitting's start in the kernel log; the footer saves
# everything after it to dmesg.txt (no teardown runs — nothing is created).
KMSG="dpaa2-verify {id} pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {{
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) {{ w = 1 }} w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}}

# --- independent safety self-check (ADR-0003 §4) ---
# The execution side refuses total-deny references even if a script was
# hand-edited after generation.
if grep -nE '{TOTAL_DENY_GREP}' "$0" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- read-only helpers ---
# Each step runs one command, captures its stdout/stderr/exit under the
# results directory, and prints one PASS/FAIL line. Exit is judged in
# place for the operator; the captured output is what 4.2 diffs offline.
# run N cmd...: echo, execute, capture stdout, stderr and the exit code.
run() {{ n="$1"; shift; echo "+ $*"; "$@" >"$RESULTS/step-$n-out.txt" 2>"$RESULTS/step-$n-err.txt"; echo $? > "$RESULTS/step-$n-exit.txt"; }}
# expect_zero N label: PASS iff the step exited zero.
expect_zero() {{ rc=$(cat "$RESULTS/step-$1-exit.txt"); if [ "$rc" = 0 ]; then echo "PASS step $1: $2"; else echo "FAIL step $1: $2 (exit $rc)" >&2; fi; }}
# expect_nonzero N label: PASS iff the step was refused (nonzero exit).
expect_nonzero() {{ rc=$(cat "$RESULTS/step-$1-exit.txt"); if [ "$rc" != 0 ]; then echo "PASS step $1: $2 (exit $rc)"; else echo "FAIL step $1: $2 (exit 0)" >&2; fi; }}
# expect_any N label: informational — the exit is evidence, never a verdict.
expect_any() {{ rc=$(cat "$RESULTS/step-$1-exit.txt"); echo "PASS step $1: $2 (exit $rc — informational)"; }}

{REF_PAIR_ASSERT}"#,
    )
}

/// Generates a read-only fit-check sitting from a hand-authored probe
/// plan (design D12). Renders the reviewable script and the
/// offline-judgeable plan; `probes_file` is the source spelled as the
/// operator runs it from the repository root.
///
/// # Errors
///
/// Refuses (with the reason named) any step that is not read-only — an
/// operator instruction, a mutating restool verb, or a sysfs write — and
/// any rendered command that breaches the safety envelope (ADR-0003 §4).
pub fn generate_fit(plan: &ProbePlan, probes_file: &str) -> Result<FitSuite, String> {
    let id = &plan.suite;
    let mut body = String::new();
    let mut steps = Vec::with_capacity(plan.steps.len());

    for (i, step) in plan.steps.iter().enumerate() {
        let argv = check_read_only(i, step)?;
        let exit = step.exit.unwrap_or(ExitShape::Any);
        let _ = write!(body, "\n# step {i}: {}\n", step.label);
        let _ = writeln!(body, "# expect: {}", step.expect);
        let _ = writeln!(body, "run {i} {}", argv.join(" "));
        let helper = match exit {
            ExitShape::Zero => "expect_zero",
            ExitShape::Nonzero => "expect_nonzero",
            ExitShape::Any => "expect_any",
        };
        let _ = writeln!(body, "{helper} {i} \"{}\"", sh_dquote(&step.label));
        steps.push(FitStep {
            index: i,
            label: step.label.clone(),
            expect: step.expect.clone(),
            exit,
        });
    }

    let footer = format!("\nsave_dmesg\necho \"suite {id} read-only fit check complete\"\n");
    let script = format!("{}{}{}", preamble(id, probes_file), body, footer);

    // The rendered script is screened like the trace path's is (ADR-0003
    // §4): a lifecycle run must stay off the wired pair, and no total-deny
    // reference may reach the board even from a hand-edited plan.
    safety::check_text(RUN, &script).map_err(|v| v.to_string())?;

    Ok(FitSuite {
        script,
        plan: FitPlan {
            id: id.clone(),
            class: RUN.class.to_string(),
            probes_file: probes_file.to_owned(),
            steps,
        },
    })
}

/// One step's offline fit-check outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FitReport {
    /// Step index.
    pub index: usize,
    /// The step's label.
    pub label: String,
    /// The judgement of the declared exit shape against the captured exit.
    pub verdict: ProbeVerdict,
}

/// Diffs a fit check's result files against its plan, judging each step's
/// captured exit against its declared shape (`judge_exit`). `read` maps
/// a result file name (e.g. `step-3-exit.txt`) to its content, or `None`
/// when the file does not exist. The captured stdout/stderr is the
/// operator's to disposition (design D12); only the exit shape is machine
/// judged here.
pub fn fit_diff(plan: &FitPlan, read: impl Fn(&str) -> Option<String>) -> Vec<FitReport> {
    plan.steps
        .iter()
        .map(|step| {
            let code = read(&format!("step-{}-exit.txt", step.index))
                .and_then(|s| s.lines().next().map(str::trim).and_then(|l| l.parse().ok()));
            FitReport {
                index: step.index,
                label: step.label.clone(),
                verdict: judge_exit(step.exit, code),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::parse_probe_plan;

    const READONLY_PLAN: &str = r#"{
      "suite": "V-FIT-TEST",
      "class": "lifecycle",
      "steps": [
        {"label":"census","expect":"the containers","cmd":["restool","dprc","list"],"exit":"zero"},
        {"label":"drift","expect":"nonzero means drift, evidence not failure","cmd":["dpaa2ctl","status"],"exit":"any"}
      ]
    }"#;

    #[test]
    fn a_read_only_plan_renders_a_teardown_free_reference_sitting() {
        let plan = parse_probe_plan(READONLY_PLAN).unwrap();
        let suite = generate_fit(&plan, "models/board/V-FIT-TEST/probes.json").unwrap();
        let s = &suite.script;

        // The do-not-edit header names the probe source, not a trace.
        assert!(
            s.contains(
                "generated by dpaa2-verify from models/board/V-FIT-TEST/probes.json — do not edit; regenerate instead."
            ),
            "{s}"
        );
        // The reference pair is asserted before any command (ADR-0003 §2).
        assert!(s.contains("MC firmware is not 10.39.0"), "{s}");
        assert!(s.contains("kernel is not 6.6.52"), "{s}");
        // PASS/FAIL rendering per step, keyed on the declared exit shape.
        assert!(s.contains("expect_zero 0 \"census\""), "{s}");
        assert!(s.contains("expect_any 1 \"drift\""), "{s}");
        // Nothing is created, so there is no teardown trap and no mutating
        // verb anywhere in the script.
        assert!(!s.contains("trap teardown"), "{s}");
        assert!(!s.contains("teardown()"), "{s}");
        for verb in MUTATING_VERBS {
            assert!(!s.contains(&format!(" {verb} ")), "verb {verb} leaked: {s}");
        }
        // The plan records the probe source for the resolve lint.
        assert_eq!(
            suite.plan.probes_file,
            "models/board/V-FIT-TEST/probes.json"
        );
        assert_eq!(suite.plan.class, "object-lifecycle-only");
    }

    #[test]
    fn a_mutating_verb_is_refused() {
        let plan = parse_probe_plan(
            r#"{
              "suite": "V-BAD",
              "class": "lifecycle",
              "steps": [
                {"label":"census","expect":"ok","cmd":["restool","dprc","list"],"exit":"zero"},
                {"label":"makes an object","expect":"never rendered","cmd":["restool","dpbp","create","--container=dprc.1"]}
              ]
            }"#,
        )
        .unwrap();
        let err = generate_fit(&plan, "p.json").unwrap_err();
        assert!(
            err.contains("create") && err.contains("reads only"),
            "{err}"
        );
    }

    #[test]
    fn an_operator_instruction_step_is_refused() {
        let plan = parse_probe_plan(
            r#"{
              "suite": "V-BAD",
              "class": "lifecycle",
              "steps": [
                {"label":"reboot","expect":"never rendered","instruction":"reboot the board"}
              ]
            }"#,
        )
        .unwrap();
        let err = generate_fit(&plan, "p.json").unwrap_err();
        assert!(err.contains("runs commands only"), "{err}");
    }

    #[test]
    fn a_sysfs_write_is_refused() {
        let plan = parse_probe_plan(
            r#"{
              "suite": "V-BAD",
              "class": "lifecycle",
              "steps": [
                {"label":"override","expect":"never rendered","cmd":["sh","-c","echo vfio-fsl-mc > /sys/bus/fsl-mc/devices/dpbp.1/driver_override"]}
              ]
            }"#,
        )
        .unwrap();
        let err = generate_fit(&plan, "p.json").unwrap_err();
        assert!(err.contains("sysfs") && err.contains("reads only"), "{err}");
    }

    #[test]
    fn fit_diff_judges_each_step_by_its_captured_exit() {
        let plan = parse_probe_plan(READONLY_PLAN).unwrap();
        let plan = generate_fit(&plan, "p.json").unwrap().plan;
        // Step 0 wants zero and got 3 → fail; step 1 is `any` → pass.
        let reports = fit_diff(&plan, |name| match name {
            "step-0-exit.txt" => Some("3\n".to_owned()),
            "step-1-exit.txt" => Some("1\n".to_owned()),
            _ => None,
        });
        assert!(!reports[0].verdict.pass, "{:?}", reports[0]);
        assert!(reports[1].verdict.pass, "an `any` exit is never a failure");

        // Clean census passes.
        let reports = fit_diff(&plan, |name| {
            (name == "step-0-exit.txt").then(|| "0\n".to_owned())
        });
        assert!(reports[0].verdict.pass);
    }
}
