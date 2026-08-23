//! The batch-suite generator (design D6; ADR-0003 §2, §6, §7).
//!
//! Consumes a `--mbt` model trace and emits two artifacts:
//!
//! - a **reviewable shell script** — every restool command visible, the
//!   model's expected post-state as a comment beside each step, the
//!   pinned reference pair asserted before any action, results captured
//!   to files, and an unconditional teardown trap that destroys every
//!   object the run created (reverse order, best-effort) whether the
//!   suite passed, failed, or aborted;
//! - a **plan file** (JSON) the harness diffs offline against the result
//!   files ([`diff`]), judging each step by read-back per the adapter's
//!   law — exit status is auxiliary evidence only.
//!
//! Two gates run before anything is emitted. The safety envelope screens
//! the trace and the rendered script independently
//! ([`crate::safety`]). And the recovery guarantee (ADR-0003 §7) gates
//! mutating suites: while unverified, only the recovery-verification
//! suite itself may be emitted, and that suite is validated to mutate
//! nothing but objects it creates — the scratch set the reboot must
//! erase.

use std::fmt::Write as _;

use crate::adapter::{
    Binding, Cmd, Drive, Expected, MachineView, MbtTrace, ModelAction, ObjRef, Probe, drive,
    expect, readback,
};
use crate::safety::{self, RunClass};

/// The recovery guarantee's verification state (ADR-0003 §7): an
/// assumption until the 5.1 suite has passed on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryGuarantee {
    /// The recovery-verification suite has passed; mutating suites may
    /// be generated.
    Verified,
    /// Still an assumption: only the recovery-verification suite itself
    /// may be emitted.
    Unverified,
}

/// What kind of suite is being generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuiteKind {
    /// A normal scenario suite; gated on the verified recovery guarantee.
    Standard,
    /// The recovery-verification suite: exempt from the gate, but every
    /// mutation must target only objects the trace itself creates.
    RecoveryVerification,
}

/// One suite to generate.
#[derive(Debug, Clone)]
pub struct SuiteSpec {
    /// Scenario id (e.g. `V-DPRC-1`); names the emitted files.
    pub id: String,
    /// Declared traffic class and per-run flag.
    pub run: RunClass,
    /// Standard vs recovery-verification.
    pub kind: SuiteKind,
    /// Where the trace came from, recorded in both artifacts.
    pub trace_file: String,
}

/// One step of the offline-diffable plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanStep {
    /// Step index (matches the script's step numbers and result files).
    pub index: usize,
    /// Human-readable action description.
    pub title: String,
    /// Whether the step runs commands (false = awaited board-side step).
    pub driven: bool,
    /// The read-back probes, rendered with model-space names (used only
    /// to dispatch parsing; the script renders its own runtime names).
    pub probes: Vec<Probe>,
    /// The model id a create step binds from its output, if any.
    pub created: Option<ObjRef>,
    /// The model's expectation for the probes, if the step has one.
    pub expected: Option<Expected>,
}

/// The offline-diffable suite plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SuitePlan {
    /// Scenario id.
    pub id: String,
    /// Declared traffic class (display form).
    pub class: String,
    /// Whether the run was explicitly flagged.
    pub flagged: bool,
    /// The trace the suite was generated from.
    pub trace_file: String,
    /// The steps, in execution order.
    pub steps: Vec<PlanStep>,
}

/// A generated suite: the reviewable script and its plan.
#[derive(Debug, Clone)]
pub struct Suite {
    /// The shell script to review and run on the board.
    pub script: String,
    /// The plan the harness diffs offline against the result files.
    pub plan: SuitePlan,
}

/// Shell variable holding a created object's board name.
fn shell_var(o: ObjRef) -> String {
    format!("OBJ_{}_{}", o.fam.as_str(), o.num)
}

/// Renders one command as a script line through the given helper.
fn cmd_line(step: usize, cmd: &Cmd) -> String {
    match cmd {
        Cmd::Restool(argv) => format!("run {step} restool {}", argv.join(" ")),
        Cmd::SysfsWrite { path, value } => format!("sysfs_write {step} {path} {value}"),
    }
}

/// Renders the expectation comment beside a step.
fn expect_comment(e: &Expected) -> String {
    let mut s = format!("# expect: {}", e.object);
    if let Some(p) = e.present {
        let _ = write!(s, " present={p}");
    }
    if let Some(p) = e.plugged {
        let _ = write!(s, " plugged={p}");
    }
    if let Some(ref ep) = e.endpoint {
        match ep {
            Some(peer) => {
                let _ = write!(s, " endpoint={peer}");
            }
            None => s.push_str(" endpoint=none"),
        }
    }
    if let Some(b) = e.driver_bound {
        let _ = write!(s, " driver_bound={b}");
    }
    s
}

/// The script preamble: header, self-check, helpers, reference-pair
/// assertion (ADR-0003 §2 — asserted before any action; the bracketed
/// grep classes keep the self-check from matching its own pattern).
fn preamble(spec: &SuiteSpec) -> String {
    format!(
        r#"#!/bin/sh
# suite: {id}
# class: {class}{flag}
# generated by dpaa2-verify from {trace} — do not edit; regenerate instead.
# Every board-touching command is visible below; the model's expected
# post-state sits as a comment beside each step (ADR-0003 §2). Results
# are captured under the results directory for offline diffing:
#   dpaa2-verify diff --plan {id}.plan.json --results <dir>
set -u
RESULTS="${{1:?usage: $0 <results-dir>}}"
mkdir -p "$RESULTS"

# --- independent safety self-check (ADR-0003 §4) ---
# The execution side refuses total-deny references even if a script was
# hand-edited after generation.
if grep -nE 'dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)' "$0" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- helpers ---
# run N cmd...: echo, execute, append the exit code to step-N-exit.txt.
# Exit status is recorded as auxiliary evidence only; conformance is
# judged on read-back (DPNI-I6, DPMAC-I8).
run() {{ n="$1"; shift; echo "+ $*"; "$@"; echo $? >> "$RESULTS/step-$n-exit.txt"; }}
# run_create N cmd...: like run, but prints the created object's name.
run_create() {{ n="$1"; shift; echo "+ $*" >&2; out="$("$@")"; echo $? >> "$RESULTS/step-$n-exit.txt"; echo "$out"; }}
# sysfs_write N path value
sysfs_write() {{ n="$1"; echo "+ echo $3 > $2"; sh -c "echo $3 > $2"; echo $? >> "$RESULTS/step-$n-exit.txt"; }}
# probe N M cmd...: capture read-back output, never fail the script.
probe() {{ n="$1"; m="$2"; shift 2; echo "+ (probe) $*"; "$@" > "$RESULTS/step-$n-probe-$m.txt" 2>/dev/null || true; }}
# probe_link N M path: capture a sysfs driver link (empty = unbound).
probe_link() {{ n="$1"; m="$2"; readlink "$3" > "$RESULTS/step-$n-probe-$m.txt" 2>/dev/null || true; }}

# --- reference pair assertion (ADR-0003 §2) ---
# Evidence is only valid against the stamped pair; refuse anything else.
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac
"#,
        id = spec.id,
        class = spec.run.class,
        flag = if spec.run.flagged {
            "  (explicitly flagged run)"
        } else {
            ""
        },
        trace = spec.trace_file,
    )
}

/// Validates that a recovery-verification trace mutates only what it
/// creates: containers may be created anywhere, objects only inside
/// created containers, and every other driven action may reference
/// created objects only (the scratch set the reboot must erase).
fn check_scratch_only(trace: &MbtTrace) -> Result<(), String> {
    let mut created: Vec<ObjRef> = Vec::new();
    let mut pre = trace.init.clone();
    for (i, step) in trace.steps.iter().enumerate() {
        match &step.action {
            ModelAction::CreateContainer { .. } | ModelAction::CreateObject { .. } => {
                if let ModelAction::CreateObject { container, .. } = &step.action
                    && !created.contains(container)
                {
                    return Err(format!(
                        "step {i}: recovery verification creates an object in {container}, outside the scratch set"
                    ));
                }
                if let Some(c) = crate::adapter::created_object(&pre, &step.post) {
                    created.push(c);
                }
            }
            other => {
                // Awaited steps mutate nothing from userspace.
                let is_driven = !matches!(
                    other,
                    ModelAction::KernelBind { .. }
                        | ModelAction::ChildIrqRefresh { .. }
                        | ModelAction::Allocate { .. }
                        | ModelAction::Free { .. }
                        | ModelAction::Enable { .. }
                        | ModelAction::Disable { .. }
                        | ModelAction::LinkChange { .. }
                );
                if is_driven
                    && let Some(outside) = other.refs().iter().find(|r| !created.contains(r))
                {
                    return Err(format!(
                        "step {i}: recovery verification mutates {outside}, outside the scratch set it created"
                    ));
                }
            }
        }
        pre = step.post.clone();
    }
    Ok(())
}

/// Generates one suite from a model trace.
///
/// # Errors
///
/// Refuses (with the reason named) on a safety-envelope violation, on a
/// mutating suite while the recovery guarantee is unverified, on a
/// recovery-verification trace that mutates outside its scratch set,
/// and on traces the adapter cannot map (e.g. actions in containers
/// restool cannot reach).
#[allow(clippy::too_many_lines)] // one linear emission walk, by design
pub fn generate(
    spec: &SuiteSpec,
    trace: &MbtTrace,
    recovery: RecoveryGuarantee,
) -> Result<Suite, String> {
    safety::check_trace(spec.run, trace).map_err(|v| v.to_string())?;

    let mutating = trace.steps.iter().any(|s| {
        !matches!(
            drive(&s.action, &MachineView::default(), &Binding::default()),
            Ok(Drive::Await(_))
        )
    });
    match (spec.kind, recovery) {
        (SuiteKind::Standard, RecoveryGuarantee::Unverified) if mutating => {
            return Err(
                "refusing to emit a mutating suite: the recovery guarantee (ADR-0003 §7) is \
                 unverified — run the recovery-verification suite first"
                    .to_owned(),
            );
        }
        (SuiteKind::RecoveryVerification, _) => check_scratch_only(trace)?,
        _ => {}
    }

    // Two bindings over one walk: symbolic names render the script
    // (created objects are shell variables until the board names them);
    // model-space names render the plan's probes for offline dispatch.
    let mut sym = Binding::seed(&trace.init);
    let mut model_names = Binding::seed(&trace.init);
    let mut pre = trace.init.clone();
    let mut body = String::new();
    let mut teardown: Vec<(ObjRef, String)> = Vec::new();
    let mut steps = Vec::new();

    for (i, step) in trace.steps.iter().enumerate() {
        let title = format!("{:?}", step.action);
        let _ = write!(body, "\n# step {i}: {title}\n");

        let created = crate::adapter::created_object(&pre, &step.post);
        let d = drive(&step.action, &pre, &sym).map_err(|e| format!("step {i}: {e}"))?;
        let driven = match &d {
            Drive::Await(why) => {
                let _ = writeln!(body, "# awaited: {why}");
                if !matches!(step.action, ModelAction::LinkChange { .. }) {
                    // Give the kernel a moment before probing its work.
                    let _ = writeln!(body, "sleep 1");
                }
                false
            }
            Drive::Cmds(cmds) => {
                if let (Some(c), Some(Cmd::Restool(argv))) = (created, cmds.first()) {
                    let var = shell_var(c);
                    let _ = writeln!(
                        body,
                        "{var}=\"$(run_create {i} restool {})\"",
                        argv.join(" ")
                    );
                    let _ = writeln!(body, "echo \"{c} ${{{var}}}\" >> \"$RESULTS/created.txt\"");
                    sym.bind_symbolic(c, format!("${{{var}}}"));
                    model_names.bind_symbolic(c, c.to_string());
                    teardown.push((c, var));
                    for extra in &cmds[1..] {
                        let _ = writeln!(body, "{}", cmd_line(i, extra));
                    }
                } else {
                    for cmd in cmds {
                        let _ = writeln!(body, "{}", cmd_line(i, cmd));
                    }
                }
                true
            }
        };

        let expected =
            expect(&step.action, &pre, &step.post).map_err(|e| format!("step {i}: {e}"))?;
        if let Some(ref e) = expected {
            let _ = writeln!(body, "{}", expect_comment(e));
        }
        let sym_probes =
            readback(&step.action, &pre, &step.post, &sym).map_err(|e| format!("step {i}: {e}"))?;
        for (m, probe) in sym_probes.iter().enumerate() {
            match probe {
                Probe::Restool(argv) => {
                    let _ = writeln!(body, "probe {i} {m} restool {}", argv.join(" "));
                }
                Probe::SysfsRead { path } => {
                    let _ = writeln!(body, "probe_link {i} {m} {path}");
                }
            }
        }
        let plan_probes = readback(&step.action, &pre, &step.post, &model_names)
            .map_err(|e| format!("step {i}: {e}"))?;

        steps.push(PlanStep {
            index: i,
            title,
            driven,
            probes: plan_probes,
            created,
            expected,
        });
        pre = step.post.clone();
    }

    // Unconditional teardown (ADR-0003 §6): destroy everything this run
    // created, newest first, best-effort — pass, fail, or abort alike.
    let mut trap = String::from("\n# --- unconditional teardown (ADR-0003 §6) ---\nteardown() {\n");
    for (obj, var) in teardown.iter().rev() {
        // `:-` defaults keep the trap alive under `set -u` when the run
        // aborted before this object was ever created.
        let _ = writeln!(
            trap,
            "  [ -n \"${{{var}:-}}\" ] && restool {} destroy \"${{{var}}}\" 2>/dev/null || true",
            obj.fam.as_str()
        );
    }
    trap.push_str("}\ntrap teardown EXIT\n");

    let script = format!(
        "{}{}{}\necho \"suite {} complete\"\n",
        preamble(spec),
        trap,
        body,
        spec.id
    );
    safety::check_text(spec.run, &script).map_err(|v| v.to_string())?;

    Ok(Suite {
        script,
        plan: SuitePlan {
            id: spec.id.clone(),
            class: spec.run.class.to_string(),
            flagged: spec.run.flagged,
            trace_file: spec.trace_file.clone(),
            steps,
        },
    })
}

/// One step's offline-diff outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepReport {
    /// Step index.
    pub index: usize,
    /// Human-readable action description.
    pub title: String,
    /// The judgement, or `None` for steps with nothing to observe.
    pub verdict: Option<crate::adapter::StepVerdict>,
}

/// Diffs a suite's result files against its plan. `read` maps a result
/// file name (e.g. `step-3-probe-0.txt`, `created.txt`) to its content,
/// or `None` when the file does not exist.
///
/// # Errors
///
/// Fails on a malformed `created.txt` or an expectation whose object
/// cannot be resolved — result files from a different suite or a
/// truncated run.
pub fn diff(
    plan: &SuitePlan,
    read: impl Fn(&str) -> Option<String>,
) -> Result<Vec<StepReport>, String> {
    // Board names of created objects, recorded by the run.
    let mut names = Binding::default();
    if let Some(created) = read("created.txt") {
        for line in created.lines().filter(|l| !l.trim().is_empty()) {
            let (model, board) = line
                .trim()
                .split_once(' ')
                .ok_or_else(|| format!("malformed created.txt line: `{line}`"))?;
            names.bind_symbolic(model.parse::<ObjRef>()?, board.trim());
        }
    }

    let mut reports = Vec::new();
    for step in &plan.steps {
        let Some(ref expected) = step.expected else {
            reports.push(StepReport {
                index: step.index,
                title: step.title.clone(),
                verdict: None,
            });
            continue;
        };
        // Boot-born objects keep their literal names; anything else must
        // have been recorded by its create step.
        let resolve = |o: ObjRef| -> String {
            names
                .name(o)
                .map_or_else(|_| o.to_string(), ToOwned::to_owned)
        };
        let object_name = resolve(expected.object);
        if let Some(Some(peer)) = expected.endpoint {
            let n = resolve(peer.obj);
            names.bind_symbolic(peer.obj, n);
        }
        names.bind_symbolic(expected.object, object_name.clone());

        let outputs: Vec<String> = (0..step.probes.len())
            .map(|m| read(&format!("step-{}-probe-{m}.txt", step.index)).unwrap_or_default())
            .collect();
        let observed = crate::adapter::observe(&step.probes, &outputs, &object_name)?;
        // Exit codes are auxiliary; a step that recorded none (awaited)
        // reports ok.
        let exit_ok = read(&format!("step-{}-exit.txt", step.index))
            .is_none_or(|s| s.lines().all(|l| l.trim() == "0" || l.trim().is_empty()));
        let verdict = crate::adapter::judge(
            expected,
            &observed,
            &names,
            crate::adapter::ExitEvidence { ok: exit_ok },
        )?;
        reports.push(StepReport {
            index: step.index,
            title: step.title.clone(),
            verdict: Some(verdict),
        });
    }
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{BindView, Family, MbtStep, ObjView};
    use crate::safety::TrafficClass;

    const LIFECYCLE: RunClass = RunClass {
        class: TrafficClass::ObjectLifecycleOnly,
        flagged: false,
    };

    fn spec(kind: SuiteKind) -> SuiteSpec {
        SuiteSpec {
            id: "V-TEST-1".to_owned(),
            run: LIFECYCLE,
            kind,
            trace_file: "test.itf.json".to_owned(),
        }
    }

    fn dprc(num: u32) -> ObjRef {
        ObjRef {
            fam: Family::Dprc,
            num,
        }
    }

    fn dpbp(num: u32) -> ObjRef {
        ObjRef {
            fam: Family::Dpbp,
            num,
        }
    }

    fn obj(parent: Option<ObjRef>, plugged: bool) -> ObjView {
        ObjView {
            parent,
            plugged,
            bus_visible: true,
            bind: BindView::Unbound,
            link_up: false,
        }
    }

    /// init: dprc.1. Steps: create dprc.100 in dprc.1, create dpbp.101
    /// inside it, plug dpbp.101, destroy dpbp.101.
    fn scratch_trace() -> MbtTrace {
        let mut init = MachineView::default();
        init.objs.insert(dprc(1), obj(None, true));
        let mut s1 = init.clone();
        s1.objs.insert(dprc(100), obj(Some(dprc(1)), false));
        let mut s2 = s1.clone();
        s2.objs.insert(dpbp(101), obj(Some(dprc(100)), false));
        let mut s3 = s2.clone();
        s3.objs.get_mut(&dpbp(101)).unwrap().plugged = true;
        let mut s4 = s3.clone();
        s4.objs.remove(&dpbp(101));
        MbtTrace {
            init,
            steps: vec![
                MbtStep {
                    action: ModelAction::CreateContainer { parent: dprc(1) },
                    post: s1,
                },
                MbtStep {
                    action: ModelAction::CreateObject {
                        fam: Family::Dpbp,
                        container: dprc(100),
                    },
                    post: s2,
                },
                MbtStep {
                    action: ModelAction::Plug { obj: dpbp(101) },
                    post: s3,
                },
                MbtStep {
                    action: ModelAction::Destroy { obj: dpbp(101) },
                    post: s4,
                },
            ],
        }
    }

    #[test]
    fn mutating_suites_are_gated_on_the_recovery_guarantee() {
        let err = generate(
            &spec(SuiteKind::Standard),
            &scratch_trace(),
            RecoveryGuarantee::Unverified,
        )
        .unwrap_err();
        assert!(err.contains("recovery guarantee"), "{err}");

        // Verified: the same suite generates.
        assert!(
            generate(
                &spec(SuiteKind::Standard),
                &scratch_trace(),
                RecoveryGuarantee::Verified
            )
            .is_ok()
        );
        // The recovery-verification suite itself passes the gate — it is
        // scratch-only by construction here.
        assert!(
            generate(
                &spec(SuiteKind::RecoveryVerification),
                &scratch_trace(),
                RecoveryGuarantee::Unverified
            )
            .is_ok()
        );
    }

    #[test]
    fn recovery_verification_must_stay_inside_its_scratch_set() {
        // Plugging a boot-born object is outside the scratch set.
        let mut init = MachineView::default();
        init.objs.insert(dprc(1), obj(None, true));
        init.objs.insert(dpbp(0), obj(Some(dprc(1)), false));
        let mut post = init.clone();
        post.objs.get_mut(&dpbp(0)).unwrap().plugged = true;
        let trace = MbtTrace {
            init,
            steps: vec![MbtStep {
                action: ModelAction::Plug { obj: dpbp(0) },
                post,
            }],
        };
        let err = generate(
            &spec(SuiteKind::RecoveryVerification),
            &trace,
            RecoveryGuarantee::Unverified,
        )
        .unwrap_err();
        assert!(err.contains("outside the scratch set"), "{err}");
    }

    #[test]
    fn scripts_are_reviewable_and_guarded() {
        let suite = generate(
            &spec(SuiteKind::Standard),
            &scratch_trace(),
            RecoveryGuarantee::Verified,
        )
        .unwrap();
        let s = &suite.script;
        // Reference pair asserted before any step.
        let assert_pos = s.find("10.39.0").unwrap();
        let first_step = s.find("# step 0").unwrap();
        assert!(assert_pos < first_step);
        assert!(s.contains("6.6.52"));
        // Every command visible; expectations beside steps.
        assert!(s.contains("run_create 0 restool dprc create dprc.1"));
        assert!(s.contains("--plugged=1"));
        assert!(s.contains("# expect: dpbp.101 present=true plugged=true"));
        // Created objects recorded for the offline diff, then torn down
        // unconditionally in reverse order.
        assert!(s.contains("created.txt"));
        let bp = s.find("restool dpbp destroy").unwrap();
        let rc = s.find("restool dprc destroy").unwrap();
        assert!(bp < rc, "teardown must be newest-first");
        assert!(s.contains("trap teardown EXIT"));
        // The independent total-deny self-check is present.
        assert!(s.contains("dpmac[.]3"));

        // The emitted script is valid shell (`sh -n` parses, no exec).
        let path = std::env::temp_dir().join("dpaa2-verify-test-suite.sh");
        std::fs::write(&path, s).unwrap();
        let ok = std::process::Command::new("sh")
            .arg("-n")
            .arg(&path)
            .status()
            .unwrap()
            .success();
        std::fs::remove_file(&path).ok();
        assert!(ok, "emitted script does not parse as sh");
    }

    #[test]
    fn offline_diff_judges_by_readback() {
        let suite = generate(
            &spec(SuiteKind::Standard),
            &scratch_trace(),
            RecoveryGuarantee::Verified,
        )
        .unwrap();
        // Fabricated result files: the board named dprc.100 -> dprc.2
        // and dpbp.101 -> dpbp.5; every read-back conforms.
        let results = |name: &str| -> Option<String> {
            match name {
                "created.txt" => Some("dprc.100 dprc.2\ndpbp.101 dpbp.5\n".to_owned()),
                "step-0-probe-0.txt" => Some("dprc.2    unplugged\n".to_owned()),
                "step-1-probe-0.txt" | "step-2-probe-0.txt" => {
                    let plugged = if name.starts_with("step-1") {
                        "unplugged"
                    } else {
                        "plugged"
                    };
                    Some(format!("dpbp.5    {plugged}\n"))
                }
                // Post-destroy show: the dpbp is gone.
                "step-3-probe-0.txt" => Some("dprc.2    plugged\n".to_owned()),
                n if n.ends_with("-exit.txt") => Some("0\n".to_owned()),
                _ => None,
            }
        };
        let reports = diff(&suite.plan, results).unwrap();
        assert_eq!(reports.len(), 4);
        for r in &reports {
            let v = r.verdict.as_ref().unwrap();
            assert!(v.pass, "step {} failed: {:?}", r.index, v.mismatches);
        }

        // A clean exit cannot rescue a diverging read-back: report the
        // dpbp still present after its destroy.
        let lying = |name: &str| -> Option<String> {
            if name == "step-3-probe-0.txt" {
                Some("dprc.2    plugged\ndpbp.5    plugged\n".to_owned())
            } else {
                results(name)
            }
        };
        let reports = diff(&suite.plan, lying).unwrap();
        let v = reports[3].verdict.as_ref().unwrap();
        assert!(!v.pass);
        assert!(v.exit.ok, "exit stays auxiliary evidence");
    }
}
