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
    Binding, Cmd, CreateArgs, Drive, Expected, MachineView, MbtTrace, ModelAction, ObjRef, Probe,
    drive, drive_with, expect, readback,
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

/// A hand-written shell file the suite sources after its last trace
/// step: the steps a trace cannot express — frames over a scratch group
/// that has to be standing — run with the script's own variables
/// (`$RESULTS`, the `OBJ_<fam>_<n>` names of created objects, the
/// helpers) and under the same teardown trap.
#[derive(Debug, Clone)]
pub struct Hook {
    /// The path the script sources, spelled as the operator runs it from
    /// the repository root.
    pub path: String,
    /// The file's text, read at generation so the safety envelope
    /// screens it like any other board-touching step.
    pub contents: String,
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
    /// Hand-written steps to source after the last trace step, if any.
    pub hook: Option<Hook>,
    /// Per-family `restool <fam> create` arguments this suite renders
    /// instead of the adapter's default table. Empty renders every
    /// create on the defaults, as every committed suite was generated.
    pub create_args: CreateArgs,
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
    /// The hand-written file the script sources after its last step, if
    /// any. Absent from plans generated without one, so plan files
    /// written before hooks existed still parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook: Option<String>,
    /// The per-family create-argument overrides the suite rendered.
    /// Absent when the suite used the default table, so every plan
    /// committed before overrides existed still parses and re-serializes
    /// byte-identically.
    #[serde(default, skip_serializing_if = "CreateArgs::is_empty")]
    pub create_args: CreateArgs,
}

/// A generated suite: the reviewable script and its plan.
#[derive(Debug, Clone)]
pub struct Suite {
    /// The shell script to review and run on the board.
    pub script: String,
    /// The plan the harness diffs offline against the result files.
    pub plan: SuitePlan,
    /// Recovery-verification suites only: the post-boot companion
    /// script that re-captures state after the operator's reboot and
    /// diffs it against the pre-state capture.
    pub postboot: Option<String>,
}

/// Shell variable holding a created object's board name.
fn shell_var(o: ObjRef) -> String {
    format!("OBJ_{}_{}", o.fam.as_str(), o.num)
}

/// Model-space spelling used inside scripts (`dpni_0`, `dpni_0.1` for
/// endpoints): deliberately distinct from board-name syntax, so the
/// total-deny text scan and the script's grep self-check only ever
/// match real board references — a model-minted `dpni.0` (symbolic,
/// board-named later) must not read as the board's management dpni.0.
fn model_key(o: ObjRef) -> String {
    format!("{}_{}", o.fam.as_str(), o.num)
}

fn model_ep_key(e: crate::adapter::EndpointRef) -> String {
    if e.port == 0 {
        model_key(e.obj)
    } else {
        format!("{}.{}", model_key(e.obj), e.port)
    }
}

/// Renders one command as a script line through the given helper.
fn cmd_line(step: usize, cmd: &Cmd) -> String {
    match cmd {
        Cmd::Restool(argv) => format!("run {step} restool {}", argv.join(" ")),
        Cmd::SysfsWrite { path, value } => format!("sysfs_write {step} {path} {value}"),
    }
}

/// Renders the expectation comment beside a step, in model-space
/// spelling ([`model_key`]).
fn expect_comment(e: &Expected) -> String {
    let mut s = format!("# expect: {}", model_key(e.object));
    if let Some(p) = e.present {
        let _ = write!(s, " present={p}");
    }
    if let Some(p) = e.plugged {
        let _ = write!(s, " plugged={p}");
    }
    if let Some(ref ep) = e.endpoint {
        match ep {
            Some(peer) => {
                let _ = write!(s, " endpoint={}", model_ep_key(*peer));
            }
            None => s.push_str(" endpoint=none"),
        }
    }
    if let Some(b) = e.driver_bound {
        let _ = write!(s, " driver_bound={b}");
    }
    if let Some(l) = e.link_up {
        let _ = write!(s, " link={}", if l { "up" } else { "down" });
    }
    s
}

/// The instruction an awaited step needs a human for, or `None` when the
/// board takes the step on its own (a kernel probe, a pool draw) and a
/// settle is all the script can do.
///
/// Enable, disable and link change are the ones nothing on the board can
/// take: restool enables nothing (§5 step 7) and no command moves a PHY.
/// Batch script and online driver both stop there and say exactly what to
/// do, so the read-back that follows observes the state the trace asked
/// for rather than whatever the port happened to be doing.
pub(crate) fn operator_instruction(
    action: &ModelAction,
    post: &MachineView,
    names: &Binding,
) -> Result<Option<String>, String> {
    let netdev = |o: ObjRef, dir: &str| -> Result<String, String> {
        Ok(format!(
            "bring the consumer {dir}: find the netdev under /sys/bus/fsl-mc/devices/{}/net/ and ip link set <it> {dir}",
            names.name(o)?
        ))
    };
    Ok(match action {
        ModelAction::Enable { obj } => Some(netdev(*obj, "up")?),
        ModelAction::Disable { obj } => Some(netdev(*obj, "down")?),
        ModelAction::LinkChange { obj } => {
            // The cable hangs off the peer (a dpmac), so that is the port
            // the operator is being asked about; an object with no edge
            // can only name itself.
            let wired = names.name(post.peer_of(*obj).map_or(*obj, |p| p.obj))?;
            // The instruction names both faces of the link because a bare
            // ack is not evidence: V-LINK-2's flap-down step was acked and
            // then read the link still up, and a premature ack is
            // indistinguishable from a real firmware finding. Two rev-2
            // findings shape the wording. The stimulus must be physical —
            // an admin-down of the peer interface leaves its transmitter
            // lit, so carrier never drops on this side. And the MC-visible
            // state lags the local carrier flag by longer than the probe
            // delay, so the carrier flag alone still races the read-back;
            // the operator must see restool's own `link status:` move too.
            let dev = names.name(*obj)?;
            let fam = obj.fam.as_str();
            Some(if post.objs.get(obj).is_some_and(|o| o.link_up) {
                format!(
                    "restore the link facing {wired} (reinsert the cable), then verify both faces: cat /sys/class/net/<netdev>/carrier reads 1 (the netdev is under /sys/bus/fsl-mc/devices/{dev}/net/) and restool {fam} info {dev} shows link status: 1, and only then press enter"
                )
            } else {
                format!(
                    "take the link facing {wired} down physically (on this wiring an admin-down of the peer interface does not drop light, only pulling the cable does), then verify both faces: cat /sys/class/net/<netdev>/carrier reads 0 (the netdev is under /sys/bus/fsl-mc/devices/{dev}/net/) and restool {fam} info {dev} shows link status: 0, and only then press enter"
                )
            })
        }
        _ => None,
    })
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

# --- kernel-log window ---
# A marker stamps the sitting's start in the kernel log; the teardown
# saves everything after it to dmesg.txt, so rescan markers (ADR-0008)
# and probe refusals are files, not operator memory.
KMSG="dpaa2-verify {id} pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {{
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) {{ w = 1 }} w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}}

# --- independent safety self-check (ADR-0003 §4) ---
# The execution side refuses total-deny references even if a script was
# hand-edited after generation.
if grep -nE '{deny}' "$0" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- helpers ---
# Exit status stays auxiliary evidence only; conformance is judged on
# read-back (DPNI-I6, DPMAC-I8). stderr is where restool prints the MC
# status text (e.g. `No privilege (status 0x4)`), which until now lived
# only in ADR prose, so it is kept per step in step-N-err.txt.
# keep_err N: append the last command's stderr to step-N-err.txt and still show it
keep_err() {{ tee -a "$RESULTS/step-$1-err.txt" < "$RESULTS/.err" >&2; }}
# run N cmd...: echo, execute, append the exit code to step-N-exit.txt.
run() {{ n="$1"; shift; echo "+ $*"; "$@" 2>"$RESULTS/.err"; rc=$?; keep_err "$n"; echo $rc >> "$RESULTS/step-$n-exit.txt"; }}
# run_create N cmd...: like run, but prints the created object's name.
run_create() {{ n="$1"; shift; echo "+ $*" >&2; out="$("$@" 2>"$RESULTS/.err")"; rc=$?; keep_err "$n"; echo $rc >> "$RESULTS/step-$n-exit.txt"; echo "$out"; }}
# sysfs_write N path value
sysfs_write() {{ n="$1"; echo "+ echo $3 > $2"; sh -c "echo $3 > $2" 2>"$RESULTS/.err"; rc=$?; keep_err "$n"; echo $rc >> "$RESULTS/step-$n-exit.txt"; }}
# probe N M cmd...: capture read-back output, never fail the script.
probe() {{ n="$1"; m="$2"; shift 2; echo "+ (probe) $*"; "$@" > "$RESULTS/step-$n-probe-$m.txt" 2>"$RESULTS/.err" || true; keep_err "$n"; }}
# probe_link N M path: capture a sysfs driver link (empty = unbound).
probe_link() {{ n="$1"; m="$2"; readlink "$3" > "$RESULTS/step-$n-probe-$m.txt" 2>/dev/null || true; }}

{ref_pair}"#,
        id = spec.id,
        class = spec.run.class,
        flag = if spec.run.flagged {
            "  (explicitly flagged run)"
        } else {
            ""
        },
        trace = spec.trace_file,
        deny = TOTAL_DENY_GREP,
        ref_pair = REF_PAIR_ASSERT,
    )
}

/// The total-deny pattern the emitted self-checks grep for, shared by
/// the preamble (which scans the script itself) and the suite hook's
/// gate (which scans the file it is about to source). The bracketed
/// classes keep a self-check from matching its own pattern.
pub(crate) const TOTAL_DENY_GREP: &str =
    "dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)";

/// The suite hook block: a total-deny gate over the file, then the
/// source. Emitted after the last trace step and before the footer, so
/// the hook sees every object the run created and still runs under the
/// teardown trap.
fn hook_block(hook: &Hook) -> String {
    format!(
        r#"
# --- suite hook: {path} ---
# Hand-written steps that need the created objects standing; sourced so
# they see this script's variables and run under the teardown trap.
if grep -nE '{deny}' "{path}" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in the hook" >&2  # safety-self-check
  exit 1
fi
. "{path}"
"#,
        path = hook.path,
        deny = TOTAL_DENY_GREP,
    )
}

/// The reference-pair assertion (ADR-0003 §2): evidence is only valid
/// against the stamped pair; refuse anything else. Shared by every
/// emitted script, asserted before any action.
pub(crate) const REF_PAIR_ASSERT: &str = r#"# --- reference pair assertion (ADR-0003 §2) ---
# Evidence is only valid against the stamped pair; refuse anything else.
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac
"#;

/// The pre-state capture a recovery-verification suite takes before any
/// mutation (mbt-harness spec, "Recovery check runs first"): the state
/// the reboot must restore. `generate-dpl` is a read-only query walk of
/// live MC state (the same probe the reference capture uses); its output
/// stays in the results directory — operator material, never committed.
const PRE_STATE_CAPTURE: &str = r#"
# --- pre-state capture (recovery check runs first) ---
# Take this suite from a fresh boot with no MC-mutating services
# started: the pre-state is the recovery-diff reference, so it must
# itself be the boot state.
restool dprc list                > "$RESULTS/pre-dprc-list.txt"
restool dprc show dprc.1         > "$RESULTS/pre-dprc1-show.txt"
restool dprc generate-dpl dprc.1 > "$RESULTS/pre-dpl.dts"
"#;

/// The no-teardown footer of a recovery-verification suite: the reboot
/// is the teardown (ADR-0003 §7) — destroying the scratch set here
/// would leave the reboot nothing to erase and the diff vacuous.
fn recovery_footer(id: &str) -> String {
    format!(
        "\n# --- no teardown: the reboot is the teardown (ADR-0003 \u{a7}7) ---\n\
         # The scratch set above is deliberately left in place. Now:\n\
         #   1. reboot the board\n\
         #   2. run {id}-postboot.sh with the same results directory\n\
         save_dmesg\n\
         echo \"suite {id} mutations complete - scratch set left in place; reboot now\"\n"
    )
}

/// The post-boot companion of a recovery-verification suite: re-capture
/// the same views after the operator's reboot and diff against the
/// pre-state capture. A clean diff is what marks the recovery guarantee
/// verified (the operator then commits the marker file); any difference
/// stops the board program (design D7 step 1).
fn postboot_script(id: &str) -> String {
    let ref_pair = REF_PAIR_ASSERT;
    format!(
        r#"#!/bin/sh
# suite: {id} post-boot diff
# Run AFTER the reboot that follows {id}.sh, with the same results
# directory. Diffs post-boot state against the pre-mutation capture: the
# reboot must have erased the scratch set and restored the DPL boot
# state. Whether the change-#1 DPL baseline capture also matches
# pre-dpl.dts settles the design's open question on the diff reference;
# if that capture is at hand, compare it too.
set -u
RESULTS="${{1:?usage: $0 <results-dir>}}"
[ -r "$RESULTS/pre-dpl.dts" ] || {{ echo "refusing: no pre-state capture in $RESULTS" >&2; exit 1; }}

{ref_pair}
restool dprc list                > "$RESULTS/post-dprc-list.txt"
restool dprc show dprc.1         > "$RESULTS/post-dprc1-show.txt"
restool dprc generate-dpl dprc.1 > "$RESULTS/post-dpl.dts"

status=0
diff -u "$RESULTS/pre-dpl.dts" "$RESULTS/post-dpl.dts"               > "$RESULTS/recovery-diff.txt" || status=1
diff -u "$RESULTS/pre-dprc-list.txt" "$RESULTS/post-dprc-list.txt"  >> "$RESULTS/recovery-diff.txt" || status=1
diff -u "$RESULTS/pre-dprc1-show.txt" "$RESULTS/post-dprc1-show.txt" >> "$RESULTS/recovery-diff.txt" || status=1
if [ "$status" = 0 ]; then
  echo "recovery diff clean: the reboot restored the pre-mutation state"
  echo "guarantee verified - commit models/board/RECOVERY-VERIFIED to unblock mutating suites"
else
  echo "RECOVERY DIFF NOT CLEAN - board program stops here (design D7); see $RESULTS/recovery-diff.txt" >&2
fi
exit "$status"
"#
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
/// Refuses (with the reason named) on a safety-envelope violation in
/// the trace or in a suite hook's text, on a hook asked of a
/// recovery-verification suite, on a mutating suite while the recovery
/// guarantee is unverified, on a recovery-verification trace that
/// mutates outside its scratch set, and on traces the adapter cannot
/// map (e.g. actions in containers restool cannot reach).
#[allow(clippy::too_many_lines)] // one linear emission walk, by design
pub fn generate(
    spec: &SuiteSpec,
    trace: &MbtTrace,
    recovery: RecoveryGuarantee,
) -> Result<Suite, String> {
    safety::check_trace(spec.run, trace).map_err(|v| v.to_string())?;
    if spec.kind == SuiteKind::RecoveryVerification && spec.hook.is_some() {
        return Err(
            "refusing: a recovery-verification suite takes no hook — the reboot is its \
             teardown, so hook steps would land in the recovery diff as state nothing \
             explains"
                .to_owned(),
        );
    }
    // The hook is board-touching text like any step, so it is screened
    // before it can be sourced.
    if let Some(ref hook) = spec.hook {
        safety::check_text(spec.run, &hook.contents)
            .map_err(|v| format!("suite hook {}: {v}", hook.path))?;
    }

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
    // (model id, shell var, parent's rendered name) per created object;
    // the parent is needed for the trap's best-effort unplug.
    let mut teardown: Vec<(ObjRef, String, Option<String>)> = Vec::new();
    // Boot edges the suite tears down: both ends were there at boot, so
    // nothing this run destroys will restore them and the trap must.
    let mut severed: Vec<(crate::adapter::EndpointRef, crate::adapter::EndpointRef)> = Vec::new();
    let mut steps = Vec::new();

    for (i, step) in trace.steps.iter().enumerate() {
        let title = format!("{:?}", step.action);
        let _ = write!(body, "\n# step {i}: {title}\n");

        if let ModelAction::DisconnectEdge { e } = &step.action
            && let Some(peer) = pre.peer_of(e.obj)
            && trace.init.objs.contains_key(&e.obj)
            && trace.init.objs.contains_key(&peer.obj)
        {
            severed.push((*e, peer));
        }

        let created = crate::adapter::created_object(&pre, &step.post);
        let d = drive_with(&step.action, &pre, &sym, &spec.create_args)
            .map_err(|e| format!("step {i}: {e}"))?;
        let driven = match &d {
            Drive::Await(why) => {
                let _ = writeln!(body, "# awaited: {why}");
                match operator_instruction(&step.action, &step.post, &sym)
                    .map_err(|e| format!("step {i}: {e}"))?
                {
                    // The instruction is double-quoted so a created
                    // object's runtime name expands into it.
                    Some(what) => {
                        let _ = writeln!(body, "printf '>> operator action: %s\\n' \"{what}\"");
                        let _ = writeln!(body, "printf '   press enter when done: '");
                        let _ = writeln!(body, "read -r _ack");
                    }
                    // Give the kernel a moment before probing its work.
                    None => {
                        let _ = writeln!(body, "sleep 1");
                    }
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
                    let _ = writeln!(
                        body,
                        "echo \"{} ${{{var}}}\" >> \"$RESULTS/created.txt\"",
                        model_key(c)
                    );
                    let parent = match step.post.objs.get(&c).and_then(|o| o.parent) {
                        Some(p) => Some(
                            sym.name(p)
                                .map_err(|e| format!("step {i}: {e}"))?
                                .to_owned(),
                        ),
                        None => None,
                    };
                    sym.bind_symbolic(c, format!("${{{var}}}"));
                    model_names.bind_symbolic(c, c.to_string());
                    teardown.push((c, var, parent));
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
                Probe::Restool(argv) | Probe::RestoolIface { argv, .. } => {
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

    // The recovery-verification suite keeps its scratch set: the reboot
    // is the teardown, and the pre-state capture is the diff reference.
    // Every other suite tears down unconditionally (ADR-0003 §6):
    // destroy everything this run created, newest first, best-effort —
    // pass, fail, or abort alike.
    let (capture, trap, footer, postboot) = if spec.kind == SuiteKind::RecoveryVerification {
        (
            PRE_STATE_CAPTURE.to_owned(),
            String::new(),
            recovery_footer(&spec.id),
            Some(postboot_script(&spec.id)),
        )
    } else {
        // Teardown stderr is evidence, not noise: a refused unplug is
        // exactly how a leaked object goes unnoticed, so it lands in a
        // log instead of /dev/null. stdout stays on the console.
        let log = "2>>\"$RESULTS/teardown.log\"";
        let root = sym.name(crate::adapter::ROOT_DPRC).ok().map(str::to_owned);
        let mut trap =
            String::from("\n# --- unconditional teardown (ADR-0003 §6) ---\nteardown() {\n");
        for (obj, var, parent) in teardown.iter().rev() {
            // `:-` defaults keep the trap alive under `set -u` when the
            // run aborted before this object was ever created. Non-dprc
            // objects are unplugged first, best-effort: a root-container
            // object still holding a kernel driver refuses destroy, and
            // the unplug is what triggers its unbind.
            if obj.fam != crate::adapter::Family::Dprc
                && let Some(parent) = parent
            {
                // A root-container object may be driver-bound by teardown
                // time — after `dprc sync` the kernel's fsl_mc_allocator
                // claims free pool companions — and restool refuses
                // `--plugged=0` on anything holding a driver. Unbind it
                // first if it is bound; child-container objects never are
                // (DPRC-I6), so they skip this.
                if root.as_deref() == Some(parent.as_str()) {
                    let dev = format!("/sys/bus/fsl-mc/devices/${{{var}}}");
                    let _ = writeln!(
                        trap,
                        "  [ -n \"${{{var}:-}}\" ] && [ -e \"{dev}/driver\" ] && echo \"${{{var}}}\" > \"{dev}/driver/unbind\" {log} || true"
                    );
                }
                let _ = writeln!(
                    trap,
                    "  [ -n \"${{{var}:-}}\" ] && restool dprc assign {parent} --object=\"${{{var}}}\" --plugged=0 {log} || true"
                );
            }
            // The trap renders its own destroys rather than going
            // through `drive`, so restool's dpdbg exception lands here
            // too: `dpdbg destroy` names no object (restool destroys id
            // 0 by definition and rejects an argument). The guard still
            // keys off the create having happened.
            let target = if obj.fam == crate::adapter::Family::Dpdbg {
                String::new()
            } else {
                format!(" \"${{{var}}}\"")
            };
            let _ = writeln!(
                trap,
                "  [ -n \"${{{var}:-}}\" ] && restool {} destroy{target} {log} || true",
                obj.fam.as_str()
            );
            // Settle after every destroy, and only after a destroy. A
            // destroy is what makes the firmware raise an object event,
            // and the bus answers that by re-walking the container in
            // its interrupt thread. A second destroy arriving mid-walk
            // makes the walk read a stale descriptor and silently detach
            // an unrelated resident's driver (ADR-0008 §4) — observed on
            // the board, three bystanders at once. Waiting lets each
            // walk finish over a container nobody is still changing.
            trap.push_str("  sleep 2\n");
        }
        // Last, once the scratch objects that took the port are gone:
        // put the boot wiring back. A severed reference-environment edge
        // outlives the run otherwise, and the next boot is not a
        // teardown the suite is allowed to assume.
        for (a, b) in &severed {
            let (Some(root), Ok(a), Ok(b)) = (root.as_ref(), sym.endpoint(*a), sym.endpoint(*b))
            else {
                return Err("cannot restore a severed boot edge: unresolved endpoint".to_owned());
            };
            let _ = writeln!(
                trap,
                "  # restore boot wiring severed by this suite (reference-environment edge)\n  \
                 restool dprc connect {root} --endpoint1={a} --endpoint2={b} {log}\n  sleep 2"
            );
        }
        // The rescans the destroys and the severed-edge restore trigger
        // are the window's point, so the kernel log is saved last.
        trap.push_str("  save_dmesg\n}\ntrap teardown EXIT\n");
        let footer = format!("\necho \"suite {} complete\"\n", spec.id);
        (String::new(), trap, footer, None)
    };

    let hook = spec.hook.as_ref().map_or_else(String::new, hook_block);
    let script = format!(
        "{}{}{}{}{}{}",
        preamble(spec),
        capture,
        trap,
        body,
        hook,
        footer
    );
    safety::check_text(spec.run, &script).map_err(|v| v.to_string())?;
    if let Some(ref p) = postboot {
        safety::check_text(spec.run, p).map_err(|v| v.to_string())?;
    }

    Ok(Suite {
        script,
        postboot,
        plan: SuitePlan {
            id: spec.id.clone(),
            class: spec.run.class.to_string(),
            flagged: spec.run.flagged,
            trace_file: spec.trace_file.clone(),
            steps,
            hook: spec.hook.as_ref().map(|h| h.path.clone()),
            create_args: spec.create_args.clone(),
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
            // Model ids appear in scripts in their model-space spelling
            // (`dpni_0`, [`model_key`]); family names carry no
            // underscore, so the first one is the separator.
            names.bind_symbolic(model.replacen('_', ".", 1).parse::<ObjRef>()?, board.trim());
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
            hook: None,
            create_args: CreateArgs::default(),
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
    fn teardown_destroys_a_dpdbg_without_naming_it() {
        // The trap renders its own destroys rather than going through
        // `drive`, so restool's dpdbg law — no object name, ever — has
        // to hold here too or the teardown leaks the object.
        let dpdbg = ObjRef {
            fam: Family::Dpdbg,
            num: 0,
        };
        let mut init = MachineView::default();
        init.objs.insert(dprc(1), obj(None, true));
        let mut post = init.clone();
        post.objs.insert(dpdbg, obj(Some(dprc(1)), false));
        let trace = MbtTrace {
            init,
            steps: vec![MbtStep {
                action: ModelAction::CreateObject {
                    fam: Family::Dpdbg,
                    container: dprc(1),
                },
                post,
            }],
        };
        let s = generate(
            &spec(SuiteKind::Standard),
            &trace,
            RecoveryGuarantee::Verified,
        )
        .unwrap()
        .script;

        // Nothing between the verb and the end of the command: no
        // --container, no object name.
        assert!(
            s.contains("run_create 0 restool --script dpdbg create)"),
            "{s}"
        );
        let teardown = s.split("teardown() {").nth(1).unwrap();
        assert!(teardown.contains("restool dpdbg destroy 2>>"), "{teardown}");
        sh_parses(&s);
    }

    /// A root-resident object may be driver-bound when the trap runs, and
    /// restool refuses to unplug anything holding a driver — so teardown
    /// unbinds first, and its stderr is kept rather than discarded.
    #[test]
    fn teardown_unbinds_root_residents_before_unplugging_and_keeps_stderr() {
        let mut init = MachineView::default();
        init.objs.insert(dprc(1), obj(None, true));
        let mut s1 = init.clone();
        s1.objs.insert(dpbp(101), obj(Some(dprc(1)), false));
        let mut s2 = s1.clone();
        s2.objs.get_mut(&dpbp(101)).unwrap().plugged = true;
        let trace = MbtTrace {
            init,
            steps: vec![
                MbtStep {
                    action: ModelAction::CreateObject {
                        fam: Family::Dpbp,
                        container: dprc(1),
                    },
                    post: s1,
                },
                MbtStep {
                    action: ModelAction::Plug { obj: dpbp(101) },
                    post: s2,
                },
            ],
        };
        let s = generate(
            &spec(SuiteKind::Standard),
            &trace,
            RecoveryGuarantee::Verified,
        )
        .unwrap()
        .script;

        let unbind = s.find("/driver/unbind").unwrap();
        let unplug = s.find("--plugged=0").unwrap();
        assert!(unbind < unplug, "unbind must precede the unplug");
        assert!(!s.contains("--plugged=0 2>/dev/null"));
        assert!(s.contains("2>>\"$RESULTS/teardown.log\""));

        // Every destroy is followed by a settle, and nothing else is:
        // the wait exists to keep the bus's own rescan from overlapping
        // the next destroy (ADR-0008 §4).
        let teardown = s.split("teardown() {").nth(1).unwrap();
        let teardown = teardown.split("}\ntrap").next().unwrap();
        let settled: Vec<_> = teardown
            .lines()
            .zip(teardown.lines().skip(1))
            .filter(|(_, next)| next.trim() == "sleep 2")
            .map(|(line, _)| line)
            .collect();
        let destroys: Vec<_> = teardown
            .lines()
            .filter(|l| l.contains(" destroy "))
            .collect();
        assert!(!destroys.is_empty());
        assert_eq!(settled, destroys, "settle follows destroys, and only those");
        assert_eq!(teardown.matches("sleep 2").count(), destroys.len());

        sh_parses(&s);
    }

    /// A flap prompt must ask for evidence, not an ack: V-LINK-2's
    /// flap-down step was acked and then read the link still up, and a
    /// premature ack is indistinguishable from a real firmware finding.
    /// Both directions name the carrier file under the object's own
    /// netdev *and* the restool read-back, because the MC-visible state
    /// lags the carrier flag; the down direction also says the stimulus
    /// has to be physical. The emitted script still parses as sh — the
    /// prompt is interpolated into a double-quoted printf argument.
    #[test]
    fn link_flap_prompts_demand_a_local_carrier_read() {
        let dpni = ObjRef {
            fam: Family::Dpni,
            num: 2,
        };
        let dpmac = ObjRef {
            fam: Family::Dpmac,
            num: 7,
        };
        let ep = |o| crate::adapter::EndpointRef { obj: o, port: 0 };

        let mut init = MachineView::default();
        init.objs.insert(dprc(1), obj(None, true));
        init.objs.insert(dpni, obj(Some(dprc(1)), true));
        init.objs.insert(dpmac, obj(Some(dprc(1)), true));
        init.edges.insert((ep(dpni), ep(dpmac)));
        // One scratch create, so the trap the emitter always writes has a
        // body: a flap-only trace is not a shape any suite has.
        let mut made = init.clone();
        made.objs.insert(dpbp(101), obj(Some(dprc(1)), false));
        let mut up = made.clone();
        up.objs.get_mut(&dpni).unwrap().link_up = true;
        let mut down = up.clone();
        down.objs.get_mut(&dpni).unwrap().link_up = false;

        let mut spec = spec(SuiteKind::Standard);
        spec.run = RunClass {
            class: TrafficClass::LinkSignaling,
            flagged: true,
        };
        let trace = MbtTrace {
            init,
            steps: vec![
                MbtStep {
                    action: ModelAction::CreateObject {
                        fam: Family::Dpbp,
                        container: dprc(1),
                    },
                    post: made,
                },
                MbtStep {
                    action: ModelAction::LinkChange { obj: dpni },
                    post: up,
                },
                MbtStep {
                    action: ModelAction::LinkChange { obj: dpni },
                    post: down,
                },
            ],
        };
        let s = generate(&spec, &trace, RecoveryGuarantee::Verified)
            .unwrap()
            .script;

        assert!(s.contains(
            "restore the link facing dpmac.7 (reinsert the cable), then verify both faces: \
             cat /sys/class/net/<netdev>/carrier reads 1 \
             (the netdev is under /sys/bus/fsl-mc/devices/dpni.2/net/) \
             and restool dpni info dpni.2 shows link status: 1, and only then press enter"
        ));
        assert!(s.contains(
            "take the link facing dpmac.7 down physically (on this wiring an admin-down of the \
             peer interface does not drop light, only pulling the cable does), then verify both \
             faces: cat /sys/class/net/<netdev>/carrier reads 0 \
             (the netdev is under /sys/bus/fsl-mc/devices/dpni.2/net/) \
             and restool dpni info dpni.2 shows link status: 0, and only then press enter"
        ));

        sh_parses(&s);
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
        assert!(s.contains("run_create 0 restool --script dprc create dprc.1"));
        assert!(s.contains("--plugged=1"));
        assert!(s.contains("# expect: dpbp_101 present=true plugged=true"));
        // Created objects recorded for the offline diff, then torn down
        // unconditionally in reverse order.
        assert!(s.contains("created.txt"));
        let bp = s.find("restool dpbp destroy").unwrap();
        let rc = s.find("restool dprc destroy").unwrap();
        assert!(bp < rc, "teardown must be newest-first");
        assert!(s.contains("trap teardown EXIT"));
        // The independent total-deny self-check is present.
        assert!(s.contains("dpmac[.]3"));
        // Standard suites have no post-boot companion.
        assert!(suite.postboot.is_none());

        sh_parses(s);
    }

    /// Every step keeps its stderr in a per-step file and the teardown
    /// saves the kernel-log window: the MC status text of a refusal and
    /// the ADR-0008 rescan markers become files, not operator memory.
    /// The recovery suite has no trap, so its footer saves the log.
    #[test]
    fn steps_keep_stderr_and_the_trap_saves_the_kernel_log() {
        let s = generate(
            &spec(SuiteKind::Standard),
            &scratch_trace(),
            RecoveryGuarantee::Verified,
        )
        .unwrap()
        .script;
        assert!(s.contains("step-$1-err.txt"), "{s}");
        assert!(s.contains("/dev/kmsg"), "{s}");
        let trap = s.find("teardown() {").unwrap();
        let trap_end = s.find("}\ntrap teardown EXIT").unwrap();
        let save = s.find("save_dmesg\n}").unwrap();
        assert!(
            trap < save && save < trap_end,
            "save_dmesg is the trap's last line"
        );
        sh_parses(&s);

        // The recovery-verification suite has no trap; its footer saves
        // the kernel log before the reboot that is its teardown.
        let mut scratch = scratch_trace();
        scratch.steps.pop();
        let rec = generate(
            &spec(SuiteKind::RecoveryVerification),
            &scratch,
            RecoveryGuarantee::Unverified,
        )
        .unwrap()
        .script;
        assert!(rec.contains("save_dmesg"), "{rec}");
        sh_parses(&rec);
    }

    /// A hook carries the steps a trace cannot express (V-TRAF-0's
    /// frames), so it must run where the created objects are still
    /// standing and the trap still owns their destruction: after the
    /// last step, before the footer, inside the trap's reach.
    #[test]
    fn a_hook_is_sourced_after_the_last_step_and_under_the_trap() {
        const FACE: &str = "models/board/V-TEST-1/face.sh";
        let mut hooked = spec(SuiteKind::Standard);
        hooked.hook = Some(Hook {
            path: FACE.to_owned(),
            contents: "echo \"face over ${OBJ_dpbp_101}\" > \"$RESULTS/face.txt\"\n".to_owned(),
        });
        let suite = generate(&hooked, &scratch_trace(), RecoveryGuarantee::Verified).unwrap();
        let s = &suite.script;

        let trap = s.find("trap teardown EXIT").unwrap();
        let last_step = s.find("# step 3").unwrap();
        let source = s.find(&format!(". \"{FACE}\"")).unwrap();
        let footer = s.find("suite V-TEST-1 complete").unwrap();
        assert!(trap < source, "the hook runs under the teardown trap");
        assert!(last_step < source, "the hook runs after the last step");
        assert!(source < footer, "the hook runs before the footer");
        // The sourced file is gated the same way the script gates itself.
        assert!(s.contains("refusing: total-deny object referenced in the hook"));
        assert_eq!(suite.plan.hook.as_deref(), Some(FACE));
        sh_parses(s);

        // Without a hook the script is what it always was.
        let plain = generate(
            &spec(SuiteKind::Standard),
            &scratch_trace(),
            RecoveryGuarantee::Verified,
        )
        .unwrap();
        assert!(!plain.script.contains("suite hook"));
        assert!(plain.plan.hook.is_none());
    }

    /// The hook is board-touching text like any step, so the envelope
    /// screens it at generation — and the recovery-verification suite,
    /// whose teardown is the reboot, takes none at all.
    #[test]
    fn a_hook_is_screened_and_refused_on_a_recovery_suite() {
        let mut hooked = spec(SuiteKind::Standard);
        hooked.hook = Some(Hook {
            path: "models/board/V-TEST-1/face.sh".to_owned(),
            contents: "restool dpni info dpni.5 # peer of dpmac.3\n".to_owned(),
        });
        let err = generate(&hooked, &scratch_trace(), RecoveryGuarantee::Verified).unwrap_err();
        assert!(err.contains("dpmac.3"), "{err}");

        hooked.kind = SuiteKind::RecoveryVerification;
        let err = generate(&hooked, &scratch_trace(), RecoveryGuarantee::Unverified).unwrap_err();
        assert!(err.contains("takes no hook"), "{err}");
    }

    /// Asserts a script is valid shell (`sh -n` parses, no exec).
    fn sh_parses(script: &str) {
        let path = std::env::temp_dir().join(format!(
            "dpaa2-verify-test-{:x}.sh",
            std::hash::BuildHasher::hash_one(&std::hash::RandomState::new(), script)
        ));
        std::fs::write(&path, script).unwrap();
        let ok = std::process::Command::new("sh")
            .arg("-n")
            .arg(&path)
            .status()
            .unwrap()
            .success();
        std::fs::remove_file(&path).ok();
        assert!(ok, "emitted script does not parse as sh");
    }

    /// Severing a boot edge is the one mutation destroying the scratch
    /// set cannot undo, so the trap reconnects it — last, after every
    /// destroy has released the port, and rendered from the endpoints the
    /// trace actually cut.
    #[test]
    fn teardown_restores_a_severed_boot_edge() {
        let dpni = ObjRef {
            fam: Family::Dpni,
            num: 1,
        };
        let dpmac = ObjRef {
            fam: Family::Dpmac,
            num: 7,
        };
        let ep = |o| crate::adapter::EndpointRef { obj: o, port: 0 };

        let mut init = MachineView::default();
        init.objs.insert(dprc(1), obj(None, true));
        init.objs.insert(dpni, obj(Some(dprc(1)), true));
        init.objs.insert(dpmac, obj(Some(dprc(1)), true));
        init.edges.insert((ep(dpni), ep(dpmac)));
        let mut s1 = init.clone();
        s1.edges.clear();
        let mut s2 = s1.clone();
        s2.objs.insert(dpbp(101), obj(Some(dprc(1)), false));

        let trace = MbtTrace {
            init,
            steps: vec![
                MbtStep {
                    action: ModelAction::DisconnectEdge { e: ep(dpni) },
                    post: s1,
                },
                MbtStep {
                    action: ModelAction::CreateObject {
                        fam: Family::Dpbp,
                        container: dprc(1),
                    },
                    post: s2,
                },
            ],
        };
        let mut spec = spec(SuiteKind::Standard);
        spec.run = RunClass {
            class: TrafficClass::LinkSignaling,
            flagged: true,
        };
        let s = generate(&spec, &trace, RecoveryGuarantee::Verified)
            .unwrap()
            .script;

        let restore = s
            .find("restool dprc connect dprc.1 --endpoint1=dpni.1 --endpoint2=dpmac.7")
            .expect("trap must reconnect the severed boot edge");
        let destroy = s.find("restool dpbp destroy").unwrap();
        let trap_end = s.find("}\ntrap teardown EXIT").unwrap();
        assert!(destroy < restore, "restore comes after the destroys");
        assert!(restore < trap_end, "restore stays inside the trap");
        // Only a boot edge is restored: the disconnect is driven with
        // restool's own singular option.
        assert!(s.contains("restool dprc disconnect dprc.1 --endpoint=dpni.1"));

        sh_parses(&s);
    }

    #[test]
    fn recovery_suite_keeps_its_scratch_set_for_the_reboot() {
        // The real recovery scenario mutates without destroying: the
        // reboot erases the scratch set. Drop the fixture's destroy.
        let mut trace = scratch_trace();
        trace.steps.pop();
        let suite = generate(
            &spec(SuiteKind::RecoveryVerification),
            &trace,
            RecoveryGuarantee::Unverified,
        )
        .unwrap();
        let s = &suite.script;
        // Pre-state capture comes before any mutation.
        let capture = s.find("pre-dpl.dts").unwrap();
        let first_step = s.find("# step 0").unwrap();
        assert!(capture < first_step);
        // No teardown: the reboot is the teardown; the operator is told so.
        assert!(!s.contains("trap teardown"));
        assert!(!s.contains("destroy"));
        assert!(s.contains("reboot"));

        // The post-boot companion re-captures, diffs against the
        // pre-state, and still asserts the reference pair.
        let post = suite.postboot.as_deref().unwrap();
        assert!(post.contains("10.39.0"));
        assert!(post.contains("post-dpl.dts"));
        assert!(post.contains("recovery-diff.txt"));

        sh_parses(s);
        sh_parses(post);
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
                "created.txt" => Some("dprc_100 dprc.2\ndpbp_101 dpbp.5\n".to_owned()),
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

    /// A create-argument override reaches both artifacts — the rendered
    /// create line and the recorded plan — while a suite without one
    /// serializes no `create_args` key, so committed plans stay
    /// byte-identical.
    #[test]
    fn create_args_override_reaches_script_and_plan() {
        let dpio = |num| ObjRef {
            fam: Family::Dpio,
            num,
        };
        let mut init = MachineView::default();
        init.objs.insert(dprc(1), obj(None, true));
        let mut post = init.clone();
        post.objs.insert(dpio(101), obj(Some(dprc(1)), false));
        let trace = MbtTrace {
            init,
            steps: vec![MbtStep {
                action: ModelAction::CreateObject {
                    fam: Family::Dpio,
                    container: dprc(1),
                },
                post,
            }],
        };

        let mut over_spec = spec(SuiteKind::Standard);
        over_spec.create_args = [(
            Family::Dpio,
            vec![
                "--channel-mode=DPIO_NO_CHANNEL".to_owned(),
                "--num-priorities=8".to_owned(),
            ],
        )]
        .into_iter()
        .collect();
        let suite = generate(&over_spec, &trace, RecoveryGuarantee::Verified).unwrap();

        // The override renders the create line.
        assert!(
            suite.script.contains(
                "restool --script dpio create --channel-mode=DPIO_NO_CHANNEL --num-priorities=8 --container=dprc.1"
            ),
            "{}",
            suite.script
        );
        // And it is recorded in the plan, round-tripping through JSON.
        let json = serde_json::to_string(&suite.plan).unwrap();
        assert!(json.contains("create_args"), "{json}");
        let back: SuitePlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, suite.plan);

        // A suite without overrides serializes no key at all, so plans
        // written before overrides existed stay byte-identical.
        let plain = generate(
            &spec(SuiteKind::Standard),
            &scratch_trace(),
            RecoveryGuarantee::Verified,
        )
        .unwrap();
        let json = serde_json::to_string(&plain.plan).unwrap();
        assert!(!json.contains("create_args"), "{json}");
        assert!(plain.plan.create_args.is_empty());
    }
}
