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
//! The core is I/O-generic ([`BoardIo`], [`Prompt`]) so the walk, the
//! confirmation flow, and the transcript are testable with no board; the
//! CLI wires the real restool/sysfs/stdin implementations.

use crate::adapter::{
    Binding, Cmd, Drive, ExitEvidence, Expected, Family, MbtTrace, Observed, Probe, StepVerdict,
    drive, expect, readback,
};
use crate::safety::{self, RunClass};

/// The operator's answer to a confirmation prompt. Pause is not a
/// decision — the prompt simply waits until the operator answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Execute this step.
    Step,
    /// Stop the run here; the teardown of what was created is the
    /// operator's session (the transcript lists every created object).
    Abort,
}

/// The operator's confirmation channel.
pub trait Prompt {
    /// Asks before a step (learning mode) or after a divergence.
    fn confirm(&mut self, question: &str) -> Decision;
}

/// The board side: command execution and sysfs access.
pub trait BoardIo {
    /// Runs `restool` with `argv`; returns (exit-ok, stdout). The exit
    /// flag is auxiliary evidence only — never an observation.
    fn restool(&mut self, argv: &[String]) -> (bool, String);
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

        if per_step && prompt.confirm(&format!("step {i}: {title}")) == Decision::Abort {
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
                Probe::Restool(argv) => io.restool(argv).1,
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
                || prompt.confirm(&format!("step {i} diverged from the model — continue?"))
                    == Decision::Abort
            {
                return Ok(Outcome::Diverged(i));
            }
        }
        pre = step.post.clone();
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

    /// Prompt double: scripted decisions, counts questions.
    struct Scripted {
        decisions: Vec<Decision>,
        asked: usize,
    }

    impl Prompt for Scripted {
        fn confirm(&mut self, _q: &str) -> Decision {
            let d = self
                .decisions
                .get(self.asked)
                .copied()
                .unwrap_or(Decision::Step);
            self.asked += 1;
            d
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
        let mut prompt = Scripted {
            decisions: vec![Decision::Step],
            asked: 0,
        };
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
        assert_eq!(line.commands, vec!["restool dprc create dprc.1"]);
        assert_eq!(line.created.as_deref(), Some("dprc.2"));
        assert!(line.verdict.unwrap().pass);
    }

    #[test]
    fn abort_stops_the_run_and_keeps_the_transcript() {
        let mut prompt = Scripted {
            decisions: vec![Decision::Abort],
            asked: 0,
        };
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
        let mut prompt = Scripted {
            decisions: vec![],
            asked: 0,
        };
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
        let mut prompt = Scripted {
            decisions: vec![Decision::Step, Decision::Step],
            asked: 0,
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
        let mut prompt = Scripted {
            decisions: vec![Decision::Abort],
            asked: 0,
        };
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
}
