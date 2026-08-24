//! The online MBT driver (design D6; ADR-0003 §3).
//!
//! Operator-launched, never autonomous: the driver walks a model trace
//! against the live board with step / pause / abort in the operator's
//! hands and a full transcript of every action, expectation, and
//! observation. While a family is in learning mode every step requires
//! explicit confirmation; promotion to free-running (per-block, abort on
//! divergence) follows a family's model surviving a complete batch suite
//! and is recorded in the owning change — the driver takes promotion as
//! an input, it never decides it. Scenarios touching root-container
//! residents (dprtc, dpdbg) force per-step confirmation regardless of
//! promotion, because they cannot be scratch-contained.
//!
//! Beside model traces the driver walks [`ProbePlan`]s: hand-authored
//! step lists for the questions no trace can ask — refusals, and
//! write-only or read-only surfaces the model does not carry. Probe runs
//! are always per-step, and add a skip key because probe outcomes branch.
//!
//! The core is I/O-generic ([`BoardIo`], [`Prompt`]) so the walk, the
//! confirmation flow, and the transcript are testable with no board; the
//! CLI wires the real restool/sysfs/stdin implementations.

use crate::adapter::{
    Binding, Cmd, Drive, ExitEvidence, Expected, Family, MbtTrace, Observed, Probe, StepVerdict,
    drive, expect, readback,
};
use crate::safety::{self, RunClass, TrafficClass};

/// The operator's answer to a confirmation prompt. Pause is not a
/// decision — the prompt simply waits until the operator answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Execute this step.
    Step,
    /// Leave this step unrun and go on to the next. Offered on probe
    /// steps only (`skippable`), where an earlier answer can make a
    /// later step moot; a trace's steps depend on each other and are
    /// not skippable.
    Skip,
    /// Stop the run here; the teardown of what was created is the
    /// operator's session (the transcript lists every created object).
    Abort,
}

/// The operator's channel.
pub trait Prompt {
    /// Asks before a step (learning mode) or after a divergence.
    /// [`Decision::Skip`] is an answer only when `skippable`.
    fn confirm(&mut self, question: &str, skippable: bool) -> Decision;
    /// Shows the operator what a step declares and what the board
    /// answered. Never a question.
    fn note(&mut self, text: &str);
}

/// The board side: command execution and sysfs access.
pub trait BoardIo {
    /// Runs `restool` with `argv`; returns (exit-ok, stdout). The exit
    /// flag is auxiliary evidence only — never an observation.
    fn restool(&mut self, argv: &[String]) -> (bool, String);
    /// Runs a probe step's command — `argv[0]` is the binary — and
    /// returns its exit code (`None` when it never exited: signal or
    /// failed spawn) with stdout and stderr captured whole. Probes ask
    /// about refusals, so the message is the evidence and the exit
    /// status alone is never enough.
    fn exec(&mut self, argv: &[String]) -> (Option<i32>, String);
    /// Writes a sysfs attribute; returns whether the write succeeded.
    fn sysfs_write(&mut self, path: &str, value: &str) -> bool;
    /// Reads a sysfs attribute or link target; `None` when absent.
    fn sysfs_read(&mut self, path: &str) -> Option<String>;
    /// Waits for the board to take an awaited step (kernel probe etc.).
    fn settle(&mut self);
}

/// Driver configuration for one run.
#[derive(Debug, Clone, Copy)]
pub struct DriveConfig {
    /// Declared traffic class and per-run flag (enforced per command).
    pub run: RunClass,
    /// Per-step confirmation. `false` is only honored for scenarios the
    /// owning change has promoted; root-container scenarios override it.
    pub learning: bool,
}

/// One transcript line: the action, what was driven, what the model
/// expected, and what the board's read-back observed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StepRecord {
    /// Step index in the trace.
    pub index: usize,
    /// Human-readable action description.
    pub title: String,
    /// Rendered commands, empty for awaited steps.
    pub commands: Vec<String>,
    /// Reason the step was awaited rather than driven, if so.
    pub awaited: Option<String>,
    /// Board name bound for a created object, if any.
    pub created: Option<String>,
    /// The model's expectation for this step's probes.
    pub expected: Option<Expected>,
    /// What the probes read back.
    pub observed: Option<Observed>,
    /// The judgement (read-back only; exit is auxiliary inside it).
    pub verdict: Option<StepVerdict>,
}

/// How a drive ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Every step ran and conformed.
    Completed,
    /// The operator aborted at the contained step index.
    Aborted(usize),
    /// A read-back diverged at the step index and the run stopped
    /// (promoted mode, or the operator declined to continue).
    Diverged(usize),
}

/// Renders a command for the transcript.
fn render(cmd: &Cmd) -> String {
    match cmd {
        Cmd::Restool(argv) => format!("restool {}", argv.join(" ")),
        Cmd::SysfsWrite { path, value } => format!("echo {value} > {path}"),
    }
}

/// Walks a model trace against the board.
///
/// Every command passes the safety envelope immediately before it runs;
/// each step's record is appended to `transcript` (JSON lines) as it
/// completes, so an abort still leaves the full history on disk.
///
/// # Errors
///
/// Fails on a safety violation, an unmappable trace, or a transcript
/// write failure. Divergences and aborts are outcomes, not errors.
#[allow(clippy::too_many_lines)] // one linear walk, by design
pub fn drive_trace(
    trace: &MbtTrace,
    cfg: DriveConfig,
    io: &mut dyn BoardIo,
    prompt: &mut dyn Prompt,
    transcript: &mut dyn std::io::Write,
) -> Result<Outcome, String> {
    safety::check_trace(cfg.run, trace).map_err(|v| v.to_string())?;

    // Root-container residents cannot be scratch-contained: per-step
    // confirmation is mandatory there regardless of promotion.
    let root_resident = trace.steps.iter().any(|s| {
        s.action
            .refs()
            .iter()
            .any(|r| matches!(r.fam, Family::Dprtc | Family::Dpdbg))
    });
    let per_step = cfg.learning || root_resident;

    let mut names = Binding::seed(&trace.init);
    let mut pre = trace.init.clone();

    for (i, step) in trace.steps.iter().enumerate() {
        let title = format!("{:?}", step.action);
        let mut record = StepRecord {
            index: i,
            title: title.clone(),
            commands: Vec::new(),
            awaited: None,
            created: None,
            expected: None,
            observed: None,
            verdict: None,
        };

        if per_step && prompt.confirm(&format!("step {i}: {title}"), false) == Decision::Abort {
            writeln!(
                transcript,
                "{}",
                serde_json::to_string(&record).map_err(|e| e.to_string())?
            )
            .map_err(|e| e.to_string())?;
            return Ok(Outcome::Aborted(i));
        }

        let created = crate::adapter::created_object(&pre, &step.post);
        let mut exit_ok = true;
        match drive(&step.action, &pre, &names).map_err(|e| format!("step {i}: {e}"))? {
            Drive::Await(why) => {
                record.awaited = Some(why.to_owned());
                io.settle();
            }
            Drive::Cmds(cmds) => {
                for (n, cmd) in cmds.iter().enumerate() {
                    safety::check_cmd(cfg.run, cmd).map_err(|v| format!("step {i}: {v}"))?;
                    record.commands.push(render(cmd));
                    match cmd {
                        Cmd::Restool(argv) => {
                            let (ok, out) = io.restool(argv);
                            exit_ok &= ok;
                            if n == 0
                                && let Some(c) = created
                            {
                                let name = names
                                    .bind_created(c, &out)
                                    .map_err(|e| format!("step {i}: {e}"))?;
                                record.created = Some(name);
                            }
                        }
                        Cmd::SysfsWrite { path, value } => {
                            exit_ok &= io.sysfs_write(path, value);
                        }
                    }
                }
            }
        }

        let expected =
            expect(&step.action, &pre, &step.post).map_err(|e| format!("step {i}: {e}"))?;
        let probes = readback(&step.action, &pre, &step.post, &names)
            .map_err(|e| format!("step {i}: {e}"))?;
        let outputs: Vec<String> = probes
            .iter()
            .map(|p| match p {
                Probe::Restool(argv) | Probe::RestoolIface { argv, .. } => io.restool(argv).1,
                Probe::SysfsRead { path } => io.sysfs_read(path).unwrap_or_default(),
            })
            .collect();

        let mut diverged = false;
        if let Some(ref e) = expected {
            let object_name = names
                .name(e.object)
                .map_or_else(|_| e.object.to_string(), ToOwned::to_owned);
            let observed = crate::adapter::observe(&probes, &outputs, &object_name)?;
            let verdict =
                crate::adapter::judge(e, &observed, &names, ExitEvidence { ok: exit_ok })?;
            diverged = !verdict.pass;
            record.observed = Some(observed);
            record.verdict = Some(verdict);
        }
        record.expected = expected;
        writeln!(
            transcript,
            "{}",
            serde_json::to_string(&record).map_err(|e| e.to_string())?
        )
        .map_err(|e| e.to_string())?;

        if diverged {
            // A divergence is a discovery, not necessarily the end: in
            // per-step mode the operator decides whether to keep
            // probing; a promoted run stops on its own.
            if !per_step
                || prompt.confirm(
                    &format!("step {i} diverged from the model — continue?"),
                    false,
                ) == Decision::Abort
            {
                return Ok(Outcome::Diverged(i));
            }
        }
        pre = step.post.clone();
    }
    Ok(Outcome::Completed)
}

// --- hand-authored probe plans ---------------------------------------

/// What a probe step declares about its command's exit status. Probes
/// exist largely to pin refusals, where the exit status *is* part of the
/// question — but never the whole answer: the captured message is what
/// the finding quotes (DPNI-I6, DPMAC-I8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExitShape {
    /// The command must succeed.
    Zero,
    /// The command must be refused.
    Nonzero,
    /// Either; the step is judged by its read-back or by the operator.
    /// Omitting `exit` says the same thing.
    Any,
}

impl ExitShape {
    /// The shape as the plan spells it.
    fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::Nonzero => "nonzero",
            Self::Any => "any",
        }
    }
}

/// Whether a read-back must find the object in the container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Presence {
    /// The object is listed in the container's `dprc show`.
    Present,
    /// It is not.
    Absent,
}

impl Presence {
    /// The presence as the plan spells it.
    fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
        }
    }
}

/// A probe step's read-back: whether `object` shows up in `container`
/// once the command has run. Same `dprc show` observation the trace path
/// judges presence by ([`crate::adapter::observe`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadbackSpec {
    /// The container to `dprc show`.
    pub container: String,
    /// The board name of the object whose row is looked for.
    pub object: String,
    /// The presence the step expects.
    pub presence: Presence,
}

/// One hand-authored probe step: either a command to run or an
/// instruction for the operator, always with prose saying what it is
/// for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeStep {
    /// What the step probes, in a few words.
    pub label: String,
    /// What is expected of it, in prose: shown before the step and kept
    /// in the transcript, so a finding reads next to its intent.
    pub expect: String,
    /// The command to run, binary first. Exclusive with `instruction`.
    pub cmd: Option<Vec<String>>,
    /// A step the operator performs (a reboot, a cable pull): nothing is
    /// executed, the text is shown and acked. Exclusive with `cmd`.
    pub instruction: Option<String>,
    /// The exit status the command must have; omitted means unjudged.
    pub exit: Option<ExitShape>,
    /// The presence read-back the command must produce, if any.
    pub readback: Option<ReadbackSpec>,
}

/// A hand-authored probe plan: the questions a model trace cannot ask.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbePlan {
    /// Scenario id (e.g. `V-DPRTC-1`), recorded on every transcript line.
    pub suite: String,
    /// The declared traffic class; must be the class the run declares.
    pub class: TrafficClass,
    /// The steps, in order.
    pub steps: Vec<ProbeStep>,
}

/// The judgement of one declared probe expectation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProbeVerdict {
    /// The board did what the step declared.
    pub pass: bool,
    /// What was declared and what happened, in one line.
    pub detail: String,
}

/// One transcript line of a probe run. A probe asserts prose, an exit
/// shape and a presence rather than a model post-state, so it records a
/// different shape from a trace's [`StepRecord`]; `kind` tells the two
/// apart in a transcript that holds both.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProbeRecord {
    /// Always `"probe"`.
    pub kind: String,
    /// The plan's scenario id.
    pub suite: String,
    /// Step index in the plan.
    pub index: usize,
    /// The step's label.
    pub label: String,
    /// The prose expectation the operator was shown.
    pub expect: String,
    /// The command run, if the step had one.
    pub cmd: Option<Vec<String>>,
    /// The operator instruction, if the step was one.
    pub instruction: Option<String>,
    /// The operator skipped the step; nothing ran.
    pub skipped: bool,
    /// stdout and stderr of the command, whole and verbatim.
    pub output: Option<String>,
    /// The command's exit code; `None` when it never exited.
    pub exit_code: Option<i32>,
    /// Judgement of the declared exit shape, if the step declared one.
    pub exit_verdict: Option<ProbeVerdict>,
    /// What the read-back observed, if the step asked for one.
    pub observed: Option<Observed>,
    /// Judgement of the read-back.
    pub readback_verdict: Option<ProbeVerdict>,
}

impl ProbeRecord {
    /// The record of a step before it runs: everything the plan declared,
    /// nothing observed yet.
    fn of(suite: &str, index: usize, step: &ProbeStep) -> Self {
        Self {
            kind: "probe".to_owned(),
            suite: suite.to_owned(),
            index,
            label: step.label.clone(),
            expect: step.expect.clone(),
            cmd: step.cmd.clone(),
            instruction: step.instruction.clone(),
            skipped: false,
            output: None,
            exit_code: None,
            exit_verdict: None,
            observed: None,
            readback_verdict: None,
        }
    }
}

/// Parses a hand-authored probe plan.
///
/// # Errors
///
/// Returns the JSON error, or the first structural breach: a step must
/// carry exactly one of `cmd` (non-empty) and `instruction`, and `exit` /
/// `readback` judge a command, so they may not sit on an instruction.
pub fn parse_probe_plan(json: &str) -> Result<ProbePlan, String> {
    let plan: ProbePlan = serde_json::from_str(json).map_err(|e| e.to_string())?;
    for (i, step) in plan.steps.iter().enumerate() {
        let at = format!("probe step {i} ({})", step.label);
        match (&step.cmd, &step.instruction) {
            (Some(_), Some(_)) => {
                return Err(format!(
                    "{at}: carries both `cmd` and `instruction`; a step is one or the other"
                ));
            }
            (None, None) => {
                return Err(format!("{at}: carries neither `cmd` nor `instruction`"));
            }
            (Some(cmd), None) if cmd.is_empty() => return Err(format!("{at}: `cmd` is empty")),
            (None, Some(_)) if step.exit.is_some() || step.readback.is_some() => {
                return Err(format!(
                    "{at}: `exit` and `readback` judge a command, and an instruction step runs none"
                ));
            }
            _ => {}
        }
    }
    Ok(plan)
}

/// Every board-touching command line of a probe step: the step's own
/// (binary included — a probe plan is hand-authored argv, not a rendered
/// [`Cmd`]) and the `dprc show` its read-back needs. The safety envelope
/// sees this list twice — once for the whole plan before step 1, once
/// per step immediately before it runs.
fn step_texts(step: &ProbeStep) -> Vec<String> {
    let mut texts = Vec::new();
    if let Some(argv) = &step.cmd {
        texts.push(argv.join(" "));
    }
    if let Some(rb) = &step.readback {
        texts.push(show_argv(&rb.container).join(" "));
    }
    texts
}

/// The read-back probe's argv (after the binary name).
fn show_argv(container: &str) -> Vec<String> {
    vec!["dprc".to_owned(), "show".to_owned(), container.to_owned()]
}

/// Judges an exit status against the shape the step declared.
fn judge_exit(shape: ExitShape, code: Option<i32>) -> ProbeVerdict {
    let got = code.map_or_else(|| "no exit code".to_owned(), |c| format!("exit {c}"));
    let pass = match (shape, code) {
        (ExitShape::Any, _) => true,
        (ExitShape::Zero, Some(c)) => c == 0,
        (ExitShape::Nonzero, Some(c)) => c != 0,
        (ExitShape::Zero | ExitShape::Nonzero, None) => false,
    };
    ProbeVerdict {
        pass,
        detail: format!("expected {} exit, got {got}", shape.as_str()),
    }
}

/// Judges a presence read-back against what `dprc show` reported.
fn judge_presence(rb: &ReadbackSpec, observed: Option<bool>) -> ProbeVerdict {
    let want = rb.presence == Presence::Present;
    let got = match observed {
        Some(true) => "present",
        Some(false) => "absent",
        None => "not observed",
    };
    ProbeVerdict {
        pass: observed == Some(want),
        detail: format!(
            "expected {} {} in {}, read back {got}",
            rb.object,
            rb.presence.as_str(),
            rb.container
        ),
    }
}

/// Screens every command a plan would run — the steps' own and the
/// `dprc show` of their read-backs — against the safety envelope, before
/// step 1 executes. A forbidden command refuses the whole plan rather
/// than the step that carries it, so a hand-authored plan is checkable
/// (and is checked in CI) without a board.
///
/// # Errors
///
/// Returns the first breach, naming the suite, the step, and the
/// violation.
pub fn check_plan(run: RunClass, plan: &ProbePlan) -> Result<(), String> {
    for (i, step) in plan.steps.iter().enumerate() {
        for text in step_texts(step) {
            safety::check_text(run, &text)
                .map_err(|v| format!("{} step {i} ({}): {v}", plan.suite, step.label))?;
        }
    }
    Ok(())
}

/// Appends one record to the transcript as JSON.
fn transcribe(transcript: &mut dyn std::io::Write, record: &ProbeRecord) -> Result<(), String> {
    writeln!(
        transcript,
        "{}",
        serde_json::to_string(record).map_err(|e| e.to_string())?
    )
    .map_err(|e| e.to_string())
}

/// Walks a hand-authored probe plan against the board.
///
/// Always per-step, whatever `cfg.learning` says: a probe plan exists
/// because the model cannot predict the answer, which is the same reason
/// the Dprtc/Dpdbg latch forces confirmation on trace runs. Each step may
/// also be skipped, since one probe's answer can make a later one moot.
///
/// Every command of the plan passes the safety envelope before step 1
/// runs — a forbidden command refuses the plan rather than the step that
/// carries it — and again immediately before it executes. A divergence
/// asks the operator whether to continue; nothing here aborts by itself.
///
/// # Errors
///
/// Fails on a safety violation, a plan whose declared class is not the
/// run's, or a transcript write failure. Divergences and aborts are
/// outcomes, not errors.
pub fn drive_probes(
    plan: &ProbePlan,
    cfg: DriveConfig,
    io: &mut dyn BoardIo,
    prompt: &mut dyn Prompt,
    transcript: &mut dyn std::io::Write,
) -> Result<Outcome, String> {
    if plan.class != cfg.run.class {
        return Err(format!(
            "{} declares {} but the run declares {}: the class a plan is written for is the class it runs under (ADR-0003 §5)",
            plan.suite, plan.class, cfg.run.class
        ));
    }
    check_plan(cfg.run, plan)?;

    for (i, step) in plan.steps.iter().enumerate() {
        let mut record = ProbeRecord::of(&plan.suite, i, step);
        let doing = match (&step.cmd, &step.instruction) {
            (Some(argv), _) => format!("cmd: {}", argv.join(" ")),
            (None, Some(text)) => format!("operator: {text}"),
            (None, None) => String::new(),
        };
        prompt.note(&format!(
            "step {i}: {}\n  expect: {}\n  {doing}",
            step.label, step.expect
        ));
        match prompt.confirm(&format!("step {i}: {}", step.label), true) {
            Decision::Abort => {
                transcribe(transcript, &record)?;
                return Ok(Outcome::Aborted(i));
            }
            Decision::Skip => {
                record.skipped = true;
                transcribe(transcript, &record)?;
                continue;
            }
            Decision::Step => {}
        }

        let mut diverged = false;
        if let Some(argv) = &step.cmd {
            for text in step_texts(step) {
                safety::check_text(cfg.run, &text).map_err(|v| format!("step {i}: {v}"))?;
            }
            let (code, output) = io.exec(argv);
            prompt.note(&output);
            record.exit_code = code;
            record.output = Some(output);
            if let Some(shape) = step.exit {
                let verdict = judge_exit(shape, code);
                diverged |= !verdict.pass;
                record.exit_verdict = Some(verdict);
            }

            if let Some(rb) = &step.readback {
                let probes = [Probe::Restool(show_argv(&rb.container))];
                let outputs = [io.restool(&show_argv(&rb.container)).1];
                let observed = crate::adapter::observe(&probes, &outputs, &rb.object)?;
                let verdict = judge_presence(rb, observed.present);
                prompt.note(&verdict.detail);
                diverged |= !verdict.pass;
                record.observed = Some(observed);
                record.readback_verdict = Some(verdict);
            }
        }
        transcribe(transcript, &record)?;

        if diverged
            && prompt.confirm(
                &format!("step {i} diverged from its expectation — continue?"),
                false,
            ) == Decision::Abort
        {
            return Ok(Outcome::Diverged(i));
        }
    }
    Ok(Outcome::Completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{BindView, MachineView, MbtStep, ObjRef, ObjView};
    use crate::safety::TrafficClass;

    const LIFECYCLE: DriveConfig = DriveConfig {
        run: RunClass {
            class: TrafficClass::ObjectLifecycleOnly,
            flagged: false,
        },
        learning: true,
    };

    /// Prompt double: scripted decisions, counts questions, keeps what
    /// the operator was shown.
    struct Scripted {
        decisions: Vec<Decision>,
        asked: usize,
        notes: Vec<String>,
    }

    impl Scripted {
        fn new(decisions: Vec<Decision>) -> Self {
            Self {
                decisions,
                asked: 0,
                notes: Vec::new(),
            }
        }
    }

    impl Prompt for Scripted {
        fn confirm(&mut self, _q: &str, _skippable: bool) -> Decision {
            let d = self
                .decisions
                .get(self.asked)
                .copied()
                .unwrap_or(Decision::Step);
            self.asked += 1;
            d
        }

        fn note(&mut self, text: &str) {
            self.notes.push(text.to_owned());
        }
    }

    /// Board double: names creates sequentially, answers probes from a
    /// fixed map of canned outputs.
    struct FakeBoard {
        next_id: u32,
        show: String,
    }

    impl BoardIo for FakeBoard {
        fn restool(&mut self, argv: &[String]) -> (bool, String) {
            if argv.first().map(String::as_str) == Some("--script") {
                let name = format!("{}.{}", argv[1], self.next_id);
                self.next_id += 1;
                (true, name)
            } else if argv.get(1).map(String::as_str) == Some("create") {
                let name = format!("dprc.{}", self.next_id);
                self.next_id += 1;
                (true, name)
            } else if argv.get(1).map(String::as_str) == Some("show") {
                (true, self.show.clone())
            } else {
                (true, String::new())
            }
        }

        fn exec(&mut self, _argv: &[String]) -> (Option<i32>, String) {
            unreachable!("a trace-driven run executes no hand-authored command")
        }

        fn sysfs_write(&mut self, _path: &str, _value: &str) -> bool {
            true
        }

        fn sysfs_read(&mut self, _path: &str) -> Option<String> {
            None
        }

        fn settle(&mut self) {}
    }

    fn dprc(num: u32) -> ObjRef {
        ObjRef {
            fam: Family::Dprc,
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

    /// init: dprc.1; one step: create a scratch container.
    fn one_create() -> MbtTrace {
        let mut init = MachineView::default();
        init.objs.insert(dprc(1), obj(None, true));
        let mut post = init.clone();
        post.objs.insert(dprc(100), obj(Some(dprc(1)), false));
        MbtTrace {
            init,
            steps: vec![MbtStep {
                action: crate::adapter::ModelAction::CreateContainer { parent: dprc(1) },
                post,
            }],
        }
    }

    #[test]
    fn learning_mode_confirms_every_step_and_transcribes() {
        let mut prompt = Scripted::new(vec![Decision::Step]);
        let mut board = FakeBoard {
            next_id: 2,
            show: "dprc.2   unplugged\n".to_owned(),
        };
        let mut transcript = Vec::new();
        let outcome = drive_trace(
            &one_create(),
            LIFECYCLE,
            &mut board,
            &mut prompt,
            &mut transcript,
        )
        .unwrap();
        assert_eq!(outcome, Outcome::Completed);
        assert_eq!(prompt.asked, 1);

        let line: StepRecord = serde_json::from_str(
            std::str::from_utf8(&transcript)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(line.commands, vec!["restool --script dprc create dprc.1"]);
        assert_eq!(line.created.as_deref(), Some("dprc.2"));
        assert!(line.verdict.unwrap().pass);
    }

    #[test]
    fn abort_stops_the_run_and_keeps_the_transcript() {
        let mut prompt = Scripted::new(vec![Decision::Abort]);
        let mut board = FakeBoard {
            next_id: 2,
            show: String::new(),
        };
        let mut transcript = Vec::new();
        let outcome = drive_trace(
            &one_create(),
            LIFECYCLE,
            &mut board,
            &mut prompt,
            &mut transcript,
        )
        .unwrap();
        assert_eq!(outcome, Outcome::Aborted(0));
        // The aborted step is still on record, with nothing executed.
        let line: StepRecord = serde_json::from_str(
            std::str::from_utf8(&transcript)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        assert!(line.commands.is_empty());
    }

    #[test]
    fn promoted_runs_stop_on_divergence_learning_may_continue() {
        // The board lies: the created container never shows up.
        let mut board = FakeBoard {
            next_id: 2,
            show: String::new(),
        };
        let promoted = DriveConfig {
            learning: false,
            ..LIFECYCLE
        };
        let mut prompt = Scripted::new(vec![]);
        let mut transcript = Vec::new();
        let outcome = drive_trace(
            &one_create(),
            promoted,
            &mut board,
            &mut prompt,
            &mut transcript,
        )
        .unwrap();
        assert_eq!(outcome, Outcome::Diverged(0));
        assert_eq!(prompt.asked, 0, "promoted runs do not prompt");

        // Learning mode: the operator chooses to continue past it.
        let mut board = FakeBoard {
            next_id: 2,
            show: String::new(),
        };
        let mut prompt = Scripted::new(vec![Decision::Step, Decision::Step]);
        let mut transcript = Vec::new();
        let outcome = drive_trace(
            &one_create(),
            LIFECYCLE,
            &mut board,
            &mut prompt,
            &mut transcript,
        )
        .unwrap();
        assert_eq!(outcome, Outcome::Completed);
        assert_eq!(prompt.asked, 2, "step confirm + divergence confirm");
    }

    #[test]
    fn root_container_scenarios_force_per_step_confirmation() {
        // A dprtc-touching step under a promoted config still prompts.
        let mut init = MachineView::default();
        init.objs.insert(dprc(1), obj(None, true));
        let rtc = ObjRef {
            fam: Family::Dprtc,
            num: 0,
        };
        init.objs.insert(rtc, obj(Some(dprc(1)), true));
        let mut post = init.clone();
        post.objs.get_mut(&rtc).unwrap().plugged = false;
        let trace = MbtTrace {
            init,
            steps: vec![MbtStep {
                action: crate::adapter::ModelAction::Unplug { obj: rtc },
                post,
            }],
        };
        let promoted = DriveConfig {
            learning: false,
            ..LIFECYCLE
        };
        let mut prompt = Scripted::new(vec![Decision::Abort]);
        let mut board = FakeBoard {
            next_id: 2,
            show: String::new(),
        };
        let mut transcript = Vec::new();
        let outcome =
            drive_trace(&trace, promoted, &mut board, &mut prompt, &mut transcript).unwrap();
        assert_eq!(outcome, Outcome::Aborted(0));
        assert_eq!(prompt.asked, 1);
    }

    // --- probe plans -------------------------------------------------

    /// Board double for probe runs: canned answers per exec, in order,
    /// and a record of what actually ran.
    struct ProbeBoard {
        answers: Vec<(Option<i32>, String)>,
        ran: Vec<Vec<String>>,
        show: String,
    }

    impl ProbeBoard {
        fn new(answers: Vec<(Option<i32>, String)>, show: &str) -> Self {
            Self {
                answers,
                ran: Vec::new(),
                show: show.to_owned(),
            }
        }
    }

    impl BoardIo for ProbeBoard {
        fn restool(&mut self, _argv: &[String]) -> (bool, String) {
            (true, self.show.clone())
        }

        fn exec(&mut self, argv: &[String]) -> (Option<i32>, String) {
            let answer = self
                .answers
                .get(self.ran.len())
                .cloned()
                .unwrap_or((Some(0), String::new()));
            self.ran.push(argv.to_vec());
            answer
        }

        fn sysfs_write(&mut self, _path: &str, _value: &str) -> bool {
            true
        }

        fn sysfs_read(&mut self, _path: &str) -> Option<String> {
            None
        }

        fn settle(&mut self) {}
    }

    /// The V-DPRTC-1 shape: a refusal probe with a presence read-back,
    /// then an operator-only step.
    const DPRTC_PLAN: &str = r#"{
      "suite": "V-DPRTC-1",
      "class": "lifecycle",
      "steps": [
        {
          "label": "second dprtc create refused",
          "cmd": ["restool", "dprtc", "create", "--container=dprc.1"],
          "expect": "nonzero exit; capture the exact MC status string",
          "exit": "nonzero",
          "readback": { "container": "dprc.1", "object": "dprtc.1", "presence": "absent" }
        },
        {
          "label": "reboot the board",
          "instruction": "Reboot now; after boot run the postboot plan.",
          "expect": "operator reboots after acking"
        }
      ]
    }"#;

    fn records(transcript: &[u8]) -> Vec<ProbeRecord> {
        std::str::from_utf8(transcript)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn a_probe_step_is_a_command_or_an_instruction_never_both() {
        let plan = parse_probe_plan(DPRTC_PLAN).unwrap();
        assert_eq!(plan.suite, "V-DPRTC-1");
        assert_eq!(plan.class, TrafficClass::ObjectLifecycleOnly);
        assert_eq!(plan.steps[0].exit, Some(ExitShape::Nonzero));
        assert_eq!(plan.steps[1].exit, None, "unjudged exit is the default");

        let step = |body: &str| format!(r#"{{"suite":"S","class":"lifecycle","steps":[{body}]}}"#);
        let err = |body: &str| parse_probe_plan(&step(body)).unwrap_err();

        assert!(
            err(r#"{"expect":"e","cmd":["restool","dprc","info","dprc.1"]}"#).contains("label"),
            "a step must say what it probes"
        );
        assert!(
            err(r#"{"label":"l","expect":"e","cmd":["restool"],"instruction":"do it"}"#)
                .contains("one or the other")
        );
        assert!(err(r#"{"label":"l","expect":"e"}"#).contains("neither"));
        assert!(err(r#"{"label":"l","expect":"e","cmd":[]}"#).contains("empty"));
        assert!(
            err(r#"{"label":"l","expect":"e","instruction":"reboot","exit":"zero"}"#)
                .contains("judge a command")
        );
        // A mistyped field is a silent wrong expectation otherwise.
        assert!(err(r#"{"label":"l","expects":"e","cmd":["restool"]}"#).contains("unknown field"));
    }

    #[test]
    fn exit_shapes_judge_the_status_the_step_declared() {
        assert!(judge_exit(ExitShape::Nonzero, Some(1)).pass);
        assert!(!judge_exit(ExitShape::Nonzero, Some(0)).pass);
        assert!(judge_exit(ExitShape::Zero, Some(0)).pass);
        assert!(!judge_exit(ExitShape::Zero, Some(3)).pass);
        assert!(judge_exit(ExitShape::Any, Some(3)).pass);
        assert!(judge_exit(ExitShape::Any, None).pass);
        // A command that never exited answered neither shape.
        assert!(!judge_exit(ExitShape::Nonzero, None).pass);
        assert_eq!(
            judge_exit(ExitShape::Nonzero, Some(1)).detail,
            "expected nonzero exit, got exit 1"
        );
    }

    #[test]
    fn probe_runs_confirm_every_step_capture_output_and_may_skip() {
        // Promoted config: a probe plan confirms every step anyway.
        let promoted = DriveConfig {
            learning: false,
            ..LIFECYCLE
        };
        let mut prompt = Scripted::new(vec![Decision::Step, Decision::Skip]);
        let mut board = ProbeBoard::new(
            vec![(
                Some(1),
                "error: dprtc create failed (status 0x1a)\n".to_owned(),
            )],
            "dprtc.0   plugged\n",
        );
        let mut transcript = Vec::new();
        let outcome = drive_probes(
            &parse_probe_plan(DPRTC_PLAN).unwrap(),
            promoted,
            &mut board,
            &mut prompt,
            &mut transcript,
        )
        .unwrap();

        assert_eq!(outcome, Outcome::Completed);
        assert_eq!(prompt.asked, 2, "one confirmation per step, no divergence");
        assert_eq!(
            board.ran,
            vec![vec![
                "restool".to_owned(),
                "dprtc".to_owned(),
                "create".to_owned(),
                "--container=dprc.1".to_owned(),
            ]],
            "the skipped step runs nothing"
        );

        let lines = records(&transcript);
        assert_eq!(lines[0].kind, "probe");
        assert_eq!(lines[0].suite, "V-DPRTC-1");
        assert_eq!(lines[0].exit_code, Some(1));
        assert!(
            lines[0].output.as_ref().unwrap().contains("status 0x1a"),
            "the refusal message is the finding, not the exit status"
        );
        assert!(lines[0].exit_verdict.as_ref().unwrap().pass);
        assert!(lines[0].readback_verdict.as_ref().unwrap().pass);
        assert_eq!(lines[0].observed.as_ref().unwrap().present, Some(false));
        assert!(!lines[0].skipped);
        assert!(lines[1].skipped);
        assert!(lines[1].instruction.is_some());
        assert!(lines[1].output.is_none());
        // The operator saw the prose expectation before answering.
        assert!(prompt.notes[0].contains("nonzero exit; capture the exact MC status string"));
    }

    #[test]
    fn a_probe_divergence_asks_rather_than_aborting() {
        // The step declares a clean exit; the board refuses the command.
        let plan = parse_probe_plan(&DPRTC_PLAN.replace(r#""nonzero""#, r#""zero""#)).unwrap();
        let answers = vec![(Some(1), "error: refused\n".to_owned())];

        let mut prompt = Scripted::new(vec![Decision::Step, Decision::Step, Decision::Step]);
        let mut board = ProbeBoard::new(answers.clone(), "dprtc.0   plugged\n");
        let mut transcript = Vec::new();
        let outcome =
            drive_probes(&plan, LIFECYCLE, &mut board, &mut prompt, &mut transcript).unwrap();
        assert_eq!(outcome, Outcome::Completed, "the operator kept probing");
        assert_eq!(prompt.asked, 3, "step, divergence, step");
        assert!(!records(&transcript)[0].exit_verdict.as_ref().unwrap().pass);

        let mut prompt = Scripted::new(vec![Decision::Step, Decision::Abort]);
        let mut board = ProbeBoard::new(answers, "dprtc.0   plugged\n");
        let mut transcript = Vec::new();
        let outcome =
            drive_probes(&plan, LIFECYCLE, &mut board, &mut prompt, &mut transcript).unwrap();
        assert_eq!(outcome, Outcome::Diverged(0));
    }

    #[test]
    fn a_forbidden_command_refuses_the_plan_before_step_one() {
        let plan = parse_probe_plan(
            r#"{
              "suite": "V-DPDBG-1",
              "class": "lifecycle",
              "steps": [
                {"label":"dpdbg info","expect":"prints the object","cmd":["restool","dpdbg","info","dpdbg.0"]},
                {"label":"console reroute","expect":"never runs","cmd":["restool","dpdbg","set","dpdbg.0","--uart=1"]}
              ]
            }"#,
        )
        .unwrap();
        let mut prompt = Scripted::new(vec![]);
        let mut board = ProbeBoard::new(vec![], "");
        let mut transcript = Vec::new();
        let err =
            drive_probes(&plan, LIFECYCLE, &mut board, &mut prompt, &mut transcript).unwrap_err();
        assert!(err.contains("--uart"), "{err}");
        assert!(board.ran.is_empty(), "the safe step 0 never ran either");
        assert_eq!(prompt.asked, 0);
    }

    #[test]
    fn a_plan_runs_under_the_class_it_was_written_for() {
        let plan =
            parse_probe_plan(&DPRTC_PLAN.replace(r#""lifecycle""#, r#""link-signaling""#)).unwrap();
        let mut prompt = Scripted::new(vec![]);
        let mut board = ProbeBoard::new(vec![], "");
        let mut transcript = Vec::new();
        let err =
            drive_probes(&plan, LIFECYCLE, &mut board, &mut prompt, &mut transcript).unwrap_err();
        assert!(err.contains("link-signaling"), "{err}");
    }
}
