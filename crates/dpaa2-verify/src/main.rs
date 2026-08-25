//! `dpaa2-verify` CLI: batch-suite generation and offline result diffing
//! (design D6). The board never runs this tool's generation side; it runs
//! the emitted, operator-reviewed scripts (ADR-0003 §1–2).

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use dpaa2_verify::generate::{self, Hook, RecoveryGuarantee, SuiteKind, SuiteSpec};
use dpaa2_verify::safety::{RunClass, TrafficClass};

/// Model-based-testing harness for the DPAA2 control plane.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Declared traffic class of a run (ADR-0003 §5).
#[derive(Clone, Copy, ValueEnum)]
enum ClassArg {
    /// MC-bus mutations and queries only.
    Lifecycle,
    /// Asserts or observes link state, no frames.
    LinkSignaling,
    /// Frames emitted.
    TrafficBearing,
}

impl From<ClassArg> for TrafficClass {
    fn from(c: ClassArg) -> Self {
        match c {
            ClassArg::Lifecycle => Self::ObjectLifecycleOnly,
            ClassArg::LinkSignaling => Self::LinkSignaling,
            ClassArg::TrafficBearing => Self::TrafficBearing,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Generate a reviewable batch suite from a `quint run --mbt` trace.
    Generate {
        /// The `--mbt` ITF trace to generate from.
        #[arg(long)]
        trace: PathBuf,
        /// Scenario id (names the emitted files, e.g. V-DPRC-1).
        #[arg(long)]
        id: String,
        /// Declared traffic class.
        #[arg(long, value_enum, default_value = "lifecycle")]
        class: ClassArg,
        /// Explicit per-run flag for link/traffic runs on the wired pair.
        #[arg(long)]
        flagged: bool,
        /// Generate the recovery-verification suite itself (exempt from
        /// the recovery gate; must mutate only its own scratch set).
        #[arg(long)]
        recovery_verification: bool,
        /// Marker file whose presence records a passed recovery
        /// verification (committed by task 5.1).
        #[arg(long, default_value = "models/board/RECOVERY-VERIFIED")]
        recovery_marker: PathBuf,
        /// Hand-written shell file the suite sources after its last
        /// step, before teardown (e.g. a traffic face). Path as run from
        /// the repo root.
        #[arg(long)]
        hook: Option<PathBuf>,
        /// Per-family `restool <fam> create` arguments this suite renders
        /// instead of the adapter's default table, e.g.
        /// `dpio=--channel-mode=DPIO_NO_CHANNEL --num-priorities=8`.
        /// Recorded in the plan. Repeatable, once per family.
        #[arg(long, value_name = "FAM=ARGS")]
        create_args: Vec<String>,
        /// Directory to write `<id>.sh` and `<id>.plan.json` into.
        #[arg(long)]
        out: PathBuf,
    },
    /// Diff a suite's result files against its plan, offline.
    Diff {
        /// The plan file emitted beside the script.
        #[arg(long)]
        plan: PathBuf,
        /// The results directory the script populated on the board.
        #[arg(long)]
        results: PathBuf,
    },
    /// Drive a trace or a hand-authored probe plan online against the
    /// live board (operator-supervised; ADR-0003 §3). Run this on the
    /// board, as the operator.
    #[command(group = clap::ArgGroup::new("walked").required(true).args(["trace", "probes"]))]
    Drive {
        /// The `--mbt` ITF trace to walk.
        #[arg(long)]
        trace: Option<PathBuf>,
        /// A hand-authored probe plan to walk instead of a trace, for
        /// the questions no model trace can ask. Always per-step.
        #[arg(long)]
        probes: Option<PathBuf>,
        /// Declared traffic class.
        #[arg(long, value_enum, default_value = "lifecycle")]
        class: ClassArg,
        /// Explicit per-run flag for link/traffic runs on the wired pair.
        #[arg(long)]
        flagged: bool,
        /// Skip per-step confirmation — only for a family the owning
        /// change has promoted after it survived a full batch suite.
        /// Root-container scenarios and probe plans ignore this and
        /// always confirm.
        #[arg(long)]
        promoted: bool,
        /// Transcript file (JSON lines, appended as steps complete).
        #[arg(long)]
        transcript: PathBuf,
    },
    /// Board snapshot: render the read-only capture script, fold a
    /// capture into diffable JSON, or diff two snapshots (task 6.3).
    Snapshot {
        #[command(subcommand)]
        what: SnapshotCmd,
    },
}

/// The three snapshot subcommands.
#[derive(Subcommand)]
enum SnapshotCmd {
    /// Render the read-only capture script the operator runs on the board.
    Render {
        /// File to write the executable script to.
        #[arg(long)]
        out: PathBuf,
    },
    /// Parse a capture directory into a diffable snapshot JSON.
    Parse {
        /// The directory the capture script populated on the board.
        dir: PathBuf,
        /// File to write the snapshot JSON to.
        #[arg(long)]
        out: PathBuf,
    },
    /// Diff two snapshot JSON files; nonzero exit when they differ.
    Diff {
        /// The baseline snapshot (e.g. the committed reference).
        a: PathBuf,
        /// The snapshot to compare against it.
        b: PathBuf,
    },
}

/// Stdin confirmation: enter = step, `p` = pause (ask again), `a` =
/// abort, and on probe steps `s` = skip.
struct StdinPrompt;

impl dpaa2_verify::driver::Prompt for StdinPrompt {
    fn confirm(&mut self, question: &str, skippable: bool) -> dpaa2_verify::driver::Decision {
        let keys = if skippable {
            "[enter=step, s=skip, p=pause, a=abort]"
        } else {
            "[enter=step, p=pause, a=abort]"
        };
        loop {
            eprint!("{question}  {keys} ");
            let mut line = String::new();
            if std::io::stdin().read_line(&mut line).is_err() {
                return dpaa2_verify::driver::Decision::Abort;
            }
            match line.trim() {
                "a" => return dpaa2_verify::driver::Decision::Abort,
                "s" if skippable => return dpaa2_verify::driver::Decision::Skip,
                "p" => {
                    eprintln!("paused — press enter to be asked again");
                    let mut resume = String::new();
                    let _ = std::io::stdin().read_line(&mut resume);
                }
                _ => return dpaa2_verify::driver::Decision::Step,
            }
        }
    }

    fn note(&mut self, text: &str) {
        eprintln!("{text}");
    }
}

/// The live board: restool via `std::process::Command`, sysfs via
/// `std::fs`. The command is run directly rather than through the
/// `dpaa2_mc` runner because that runner discards stderr on success, and
/// stderr is where a refusal's MC status text lands.
struct LiveBoard;

impl dpaa2_verify::driver::BoardIo for LiveBoard {
    fn restool(&mut self, argv: &[String]) -> dpaa2_verify::driver::RestoolRun {
        match std::process::Command::new("restool").args(argv).output() {
            Ok(out) => {
                let ok = out.status.success();
                if !ok {
                    eprintln!("  (exit nonzero, auxiliary: {})", out.status);
                }
                dpaa2_verify::driver::RestoolRun {
                    ok,
                    stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                }
            }
            Err(e) => dpaa2_verify::driver::RestoolRun {
                ok: false,
                stdout: String::new(),
                stderr: format!("failed to spawn restool: {e}"),
            },
        }
    }

    fn exec(&mut self, argv: &[String]) -> (Option<i32>, String) {
        let Some((binary, rest)) = argv.split_first() else {
            return (None, "empty command".to_owned());
        };
        match std::process::Command::new(binary).args(rest).output() {
            // Whole output, stdout then stderr: a probe's finding is the
            // message the board printed, not the status it exited with.
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&out.stderr));
                (out.status.code(), text)
            }
            Err(e) => (None, format!("failed to spawn {binary}: {e}")),
        }
    }

    fn sysfs_write(&mut self, path: &str, value: &str) -> bool {
        std::fs::write(path, value).is_ok()
    }

    fn sysfs_read(&mut self, path: &str) -> Option<String> {
        std::fs::read_link(path)
            .ok()
            .map(|p| p.display().to_string())
            .or_else(|| std::fs::read_to_string(path).ok())
    }

    fn settle(&mut self) {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// Everything the kernel log holds from the first line containing
/// `marker` onward; the whole log when the marker is absent.
fn lines_from_marker(log: &str, marker: &str) -> String {
    match log.lines().position(|l| l.contains(marker)) {
        Some(i) => {
            let mut out = log.lines().skip(i).collect::<Vec<_>>().join("\n");
            out.push('\n');
            out
        }
        None => log.to_owned(),
    }
}

/// Runs `dmesg` and keeps the window from `marker` on. Empty when dmesg
/// cannot be run (off the board, or no privilege): the marker write and
/// this capture are both best-effort.
fn kernel_log_window(marker: &str) -> String {
    let log = std::process::Command::new("dmesg")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    lines_from_marker(&log, marker)
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::too_many_lines)] // one arm per subcommand, by design
fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.command {
        Command::Generate {
            trace,
            id,
            class,
            flagged,
            recovery_verification,
            recovery_marker,
            hook,
            create_args,
            out,
        } => {
            let json = std::fs::read_to_string(&trace)
                .map_err(|e| format!("reading {}: {e}", trace.display()))?;
            let parsed = dpaa2_verify::adapter::parse_mbt_trace(&json)?;
            // The hook's text is read here, not on the board: the
            // envelope screens it before the suite can source it.
            let hook = hook
                .map(|path| {
                    std::fs::read_to_string(&path)
                        .map_err(|e| format!("reading {}: {e}", path.display()))
                        .map(|contents| Hook {
                            path: path.display().to_string(),
                            contents,
                        })
                })
                .transpose()?;
            let spec = SuiteSpec {
                id: id.clone(),
                run: RunClass {
                    class: class.into(),
                    flagged,
                },
                kind: if recovery_verification {
                    SuiteKind::RecoveryVerification
                } else {
                    SuiteKind::Standard
                },
                trace_file: trace.display().to_string(),
                hook,
                create_args: create_args
                    .iter()
                    .map(|f| dpaa2_verify::adapter::CreateArgs::parse_flag(f))
                    .collect::<Result<_, String>>()?,
            };
            let recovery = if recovery_marker.exists() {
                RecoveryGuarantee::Verified
            } else {
                RecoveryGuarantee::Unverified
            };
            let suite = generate::generate(&spec, &parsed, recovery)?;

            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            let script_path = out.join(format!("{id}.sh"));
            std::fs::write(&script_path, &suite.script).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
            }
            let plan_path = out.join(format!("{id}.plan.json"));
            let plan = serde_json::to_string_pretty(&suite.plan).map_err(|e| e.to_string())?;
            std::fs::write(&plan_path, plan).map_err(|e| e.to_string())?;
            let mut wrote = format!(
                "wrote {} ({} steps) and {}",
                script_path.display(),
                suite.plan.steps.len(),
                plan_path.display()
            );
            if let Some(ref postboot) = suite.postboot {
                let post_path = out.join(format!("{id}-postboot.sh"));
                std::fs::write(&post_path, postboot).map_err(|e| e.to_string())?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&post_path, std::fs::Permissions::from_mode(0o755))
                        .map_err(|e| e.to_string())?;
                }
                let _ = write!(wrote, " and {}", post_path.display());
            }
            println!("{wrote}");
            Ok(ExitCode::SUCCESS)
        }
        Command::Diff { plan, results } => {
            let plan: generate::SuitePlan = serde_json::from_str(
                &std::fs::read_to_string(&plan)
                    .map_err(|e| format!("reading {}: {e}", plan.display()))?,
            )
            .map_err(|e| e.to_string())?;
            let reports = generate::diff(&plan, |name| {
                std::fs::read_to_string(results.join(name)).ok()
            })?;
            let mut failed = 0usize;
            for r in &reports {
                match &r.verdict {
                    None => println!("step {:>3}  -     {}", r.index, r.title),
                    Some(v) if v.pass => {
                        let exit = if v.exit.ok {
                            ""
                        } else {
                            "  (exit nonzero, auxiliary)"
                        };
                        println!("step {:>3}  pass  {}{exit}", r.index, r.title);
                    }
                    Some(v) => {
                        failed += 1;
                        println!("step {:>3}  FAIL  {}", r.index, r.title);
                        for m in &v.mismatches {
                            println!("            {m}");
                        }
                        if v.exit.ok {
                            println!(
                                "            (command exited clean; read-back is the observation)"
                            );
                        }
                    }
                }
            }
            println!("{}: {} steps, {} failed", plan.id, reports.len(), failed);
            Ok(if failed == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Command::Drive {
            trace,
            probes,
            class,
            flagged,
            promoted,
            transcript,
        } => {
            let cfg = dpaa2_verify::driver::DriveConfig {
                run: RunClass {
                    class: class.into(),
                    flagged,
                },
                learning: !promoted,
            };
            let mut out = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&transcript)
                .map_err(|e| format!("opening {}: {e}", transcript.display()))?;
            let mut board = LiveBoard;
            // Stamp the kernel log so the window saved after the run
            // starts here (best-effort, like the generated suites do).
            let marker = format!("dpaa2-verify drive pid {} start", std::process::id());
            let _ = std::fs::write("/dev/kmsg", &marker);
            let outcome = if let Some(probes) = probes {
                let json = std::fs::read_to_string(&probes)
                    .map_err(|e| format!("reading {}: {e}", probes.display()))?;
                let plan = dpaa2_verify::driver::parse_probe_plan(&json)?;
                dpaa2_verify::driver::drive_probes(
                    &plan,
                    cfg,
                    &mut board,
                    &mut StdinPrompt,
                    &mut out,
                )?
            } else {
                // clap guarantees one of the two inputs is present.
                let trace = trace.ok_or("--trace or --probes is required")?;
                let json = std::fs::read_to_string(&trace)
                    .map_err(|e| format!("reading {}: {e}", trace.display()))?;
                let parsed = dpaa2_verify::adapter::parse_mbt_trace(&json)?;
                dpaa2_verify::driver::drive_trace(
                    &parsed,
                    cfg,
                    &mut board,
                    &mut StdinPrompt,
                    &mut out,
                )?
            };
            // Whatever the outcome, keep the kernel log from the marker
            // on beside the transcript: the driver's rescan markers and
            // refusals become a file, not operator memory.
            let dmesg_path = transcript
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("dmesg.txt"), |d| d.join("dmesg.txt"));
            std::fs::write(&dmesg_path, kernel_log_window(&marker))
                .map_err(|e| format!("writing {}: {e}", dmesg_path.display()))?;
            println!(
                "outcome: {outcome:?}; transcript: {}; kernel log: {}",
                transcript.display(),
                dmesg_path.display()
            );
            Ok(match outcome {
                dpaa2_verify::driver::Outcome::Completed => ExitCode::SUCCESS,
                _ => ExitCode::FAILURE,
            })
        }
        Command::Snapshot { what } => run_snapshot(what),
    }
}

/// Runs one snapshot subcommand.
fn run_snapshot(what: SnapshotCmd) -> Result<ExitCode, String> {
    use dpaa2_verify::snapshot;
    match what {
        SnapshotCmd::Render { out } => {
            let script = snapshot::render();
            std::fs::write(&out, &script).map_err(|e| format!("writing {}: {e}", out.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
            }
            println!("wrote {}", out.display());
            Ok(ExitCode::SUCCESS)
        }
        SnapshotCmd::Parse { dir, out } => {
            let snap = snapshot::parse(|name| std::fs::read_to_string(dir.join(name)).ok())?;
            let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
            std::fs::write(&out, json).map_err(|e| format!("writing {}: {e}", out.display()))?;
            println!("wrote {}", out.display());
            Ok(ExitCode::SUCCESS)
        }
        SnapshotCmd::Diff { a, b } => {
            let read = |p: &PathBuf| -> Result<snapshot::Snapshot, String> {
                let text = std::fs::read_to_string(p)
                    .map_err(|e| format!("reading {}: {e}", p.display()))?;
                serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", p.display()))
            };
            let deltas = snapshot::diff(&read(&a)?, &read(&b)?);
            for line in &deltas {
                println!("{line}");
            }
            println!("{} deltas", deltas.len());
            Ok(if deltas.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drive run walks exactly one input: a model trace or a probe
    /// plan, never both and never neither.
    #[test]
    fn drive_takes_a_trace_or_a_probe_plan_not_both() {
        assert!(
            Cli::try_parse_from(["v", "drive", "--trace", "t.json", "--transcript", "r"]).is_ok()
        );
        assert!(
            Cli::try_parse_from(["v", "drive", "--probes", "p.json", "--transcript", "r"]).is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "v",
                "drive",
                "--trace",
                "t.json",
                "--probes",
                "p.json",
                "--transcript",
                "r",
            ])
            .is_err()
        );
        assert!(Cli::try_parse_from(["v", "drive", "--transcript", "r"]).is_err());
    }

    /// The kernel-log window keeps everything from the marker on when it
    /// is present, and the whole log when it is absent.
    #[test]
    fn kernel_log_window_keeps_the_suffix_from_the_marker() {
        let log = "a\nb MARK here\nc\n";
        assert_eq!(lines_from_marker(log, "MARK"), "b MARK here\nc\n");
        // Marker absent → whole log unchanged.
        assert_eq!(lines_from_marker("x\ny\n", "MARK"), "x\ny\n");
    }

    /// The snapshot subcommand parses its three forms: `render --out`,
    /// `parse <dir> --out`, and `diff <a> <b>`.
    #[test]
    fn snapshot_parses_render_parse_and_diff() {
        assert!(Cli::try_parse_from(["v", "snapshot", "render", "--out", "s.sh"]).is_ok());
        assert!(Cli::try_parse_from(["v", "snapshot", "parse", "cap", "--out", "s.json"]).is_ok());
        assert!(Cli::try_parse_from(["v", "snapshot", "diff", "a.json", "b.json"]).is_ok());
        // --out is required for render; diff needs both files.
        assert!(Cli::try_parse_from(["v", "snapshot", "render"]).is_err());
        assert!(Cli::try_parse_from(["v", "snapshot", "diff", "a.json"]).is_err());
    }
}
