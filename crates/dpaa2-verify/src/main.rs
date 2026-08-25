//! `dpaa2-verify` CLI: batch-suite generation and offline result diffing
//! (design D6). The board never runs this tool's generation side; it runs
//! the emitted, operator-reviewed scripts (ADR-0003 §1–2).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};
use dpaa2_verify::generate::{self, Hook, RecoveryGuarantee, SuiteKind, SuiteSpec};
use dpaa2_verify::safety::{RunClass, TrafficClass};
use dpaa2_verify::verdict::{self, Verdict};

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
        /// A step the board is expected to refuse, `<N>=<STATUS NAME>`
        /// (e.g. `1=No privilege`). That step drops its read-back
        /// expectation and is judged by its nonzero exit and the captured
        /// MC status instead. Repeatable.
        #[arg(long, value_name = "N=STATUS")]
        expect_refusal: Vec<String>,
        /// Directory to write `<id>.sh` and `<id>.plan.json` into.
        #[arg(long)]
        out: PathBuf,
    },
    /// Diff a suite's result files against its plan, or an online
    /// transcript, offline — and record a machine-readable verdict.
    #[command(group = clap::ArgGroup::new("source").required(true).args(["plan", "transcript"]))]
    Diff {
        /// The plan file emitted beside a batch script.
        #[arg(long)]
        plan: Option<PathBuf>,
        /// An online transcript (JSON lines) to judge instead of a plan.
        #[arg(long)]
        transcript: Option<PathBuf>,
        /// The results directory the batch script populated (`--plan`).
        #[arg(long)]
        results: Option<PathBuf>,
        /// Suite id override; required when a transcript carries no suite.
        #[arg(long)]
        id: Option<String>,
        /// Suite revision; defaults to the trailing `-rev<N>` of the
        /// results dir name (`--plan`) or transcript stem (`--transcript`).
        #[arg(long)]
        revision: Option<u32>,
        /// Run date `YYYY-MM-DD`; defaults to the newest result-file mtime
        /// (`--plan`) or the transcript's mtime (`--transcript`).
        #[arg(long)]
        date: Option<String>,
        /// Evidence archive path, recorded in the index only.
        #[arg(long)]
        archive: Option<PathBuf>,
        /// Index run-label override; defaults to the results dir name
        /// (`--plan`) or `<dir>/<stem>` (`--transcript`).
        #[arg(long)]
        label: Option<String>,
        /// The per-suite verdict index to upsert.
        #[arg(long, default_value = "models/board/VERDICTS.json")]
        index: PathBuf,
        /// Write the verdict file but leave the index untouched.
        #[arg(long)]
        no_index: bool,
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
            expect_refusal,
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
                expected_refusals: expect_refusal
                    .iter()
                    .map(|s| parse_expect_refusal(s))
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
        Command::Diff {
            plan,
            transcript,
            results,
            id,
            revision,
            date,
            archive,
            label,
            index,
            no_index,
        } => run_diff(&DiffArgs {
            plan,
            transcript,
            results,
            id,
            revision,
            date,
            archive,
            label,
            index,
            no_index,
        }),
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
            // Fold the transcript just written into a verdict beside it.
            // The board runs this, so the index is never touched here.
            let verdict_path = write_transcript_verdict(&transcript, None)?;
            println!(
                "outcome: {outcome:?}; transcript: {}; kernel log: {}; verdict: {}",
                transcript.display(),
                dmesg_path.display(),
                verdict_path.display()
            );
            Ok(match outcome {
                dpaa2_verify::driver::Outcome::Completed => ExitCode::SUCCESS,
                _ => ExitCode::FAILURE,
            })
        }
        Command::Snapshot { what } => run_snapshot(what),
    }
}

/// Parses a `--expect-refusal <N>=<STATUS NAME>` flag into a step index
/// and a status name. The name itself is validated against the MC status
/// table by `generate`, which also knows the trace length to reject an
/// out-of-range index.
fn parse_expect_refusal(spec: &str) -> Result<(usize, String), String> {
    let (n, name) = spec
        .split_once('=')
        .ok_or_else(|| format!("--expect-refusal expects `<N>=<STATUS NAME>`, got {spec:?}"))?;
    let idx = n
        .trim()
        .parse::<usize>()
        .map_err(|_| format!("--expect-refusal step index {n:?} is not a number"))?;
    Ok((idx, name.to_owned()))
}

/// The parsed `diff` arguments, shared by its plan and transcript arms.
struct DiffArgs {
    plan: Option<PathBuf>,
    transcript: Option<PathBuf>,
    results: Option<PathBuf>,
    id: Option<String>,
    revision: Option<u32>,
    date: Option<String>,
    archive: Option<PathBuf>,
    label: Option<String>,
    index: PathBuf,
    no_index: bool,
}

/// The last path component as a `String`, empty when there is none.
fn base_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned()
}

/// The file stem as a `String`, `transcript` when there is none.
fn stem_of(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("transcript")
        .to_owned()
}

/// Seconds since the Unix epoch, now.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A file's mtime in seconds since the epoch, 0 when unavailable.
fn mtime_secs(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// The newest mtime among a directory's files, 0 when empty/unavailable.
fn newest_mtime(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter_map(|m| m.modified().ok())
        .filter_map(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .max()
        .unwrap_or(0)
}

/// Writes a verdict to `path` as pretty JSON.
fn write_verdict_file(path: &Path, v: &Verdict) -> Result<(), String> {
    let json = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// Upserts `v` into the index unless `--no-index`; returns the index note
/// for the summary line.
fn maybe_upsert(args: &DiffArgs, v: &Verdict, label: &str) -> Result<String, String> {
    if args.no_index {
        return Ok("index untouched".to_owned());
    }
    let existing = std::fs::read_to_string(&args.index).unwrap_or_default();
    let mut index = verdict::parse_index(&existing)?;
    let archive = args.archive.as_ref().map(|p| p.display().to_string());
    verdict::upsert(&mut index, &v.suite, label, v, archive);
    if let Some(parent) = args.index.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&args.index, verdict::render_index(&index))
        .map_err(|e| format!("writing {}: {e}", args.index.display()))?;
    Ok(format!(
        "index {} [{} rev {}]",
        args.index.display(),
        v.suite,
        v.revision
    ))
}

/// Prints the one-line verdict summary.
fn print_verdict_line(v: &Verdict, verdict_path: &Path, idx_note: &str) {
    println!(
        "verdict: {} {}/{} → {}; {idx_note}",
        if v.pass { "pass" } else { "FAIL" },
        v.passed,
        v.judged,
        verdict_path.display()
    );
}

/// Runs `diff`, dispatching on `--plan` vs `--transcript`.
fn run_diff(args: &DiffArgs) -> Result<ExitCode, String> {
    match (&args.plan, &args.transcript) {
        (Some(plan), _) => run_diff_plan(&plan.clone(), args),
        (_, Some(transcript)) => run_diff_transcript(&transcript.clone(), args),
        // clap's required ArgGroup guarantees one of the two.
        _ => Err("--plan or --transcript is required".to_owned()),
    }
}

/// The `diff --plan` arm: diff result files, print the report, write the
/// verdict, upsert the index.
fn run_diff_plan(plan_path: &Path, args: &DiffArgs) -> Result<ExitCode, String> {
    let results = args.results.as_deref().ok_or("--plan requires --results")?;
    let plan_text = std::fs::read_to_string(plan_path)
        .map_err(|e| format!("reading {}: {e}", plan_path.display()))?;
    let plan: generate::SuitePlan = serde_json::from_str(&plan_text).map_err(|e| e.to_string())?;
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
                    println!("            (command exited clean; read-back is the observation)");
                }
            }
        }
    }
    println!("{}: {} steps, {} failed", plan.id, reports.len(), failed);

    let revision = args
        .revision
        .unwrap_or_else(|| verdict::revision_of(&base_name(results)));
    let date = args
        .date
        .clone()
        .unwrap_or_else(|| verdict::civil_date(newest_mtime(results)));
    let v = verdict::from_batch(
        &plan,
        &plan_text,
        &reports,
        |name| std::fs::read_to_string(results.join(name)).ok(),
        || {
            std::fs::read_dir(results)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        },
        revision,
        date,
    );
    let verdict_path = results.join("verdict.json");
    write_verdict_file(&verdict_path, &v)?;
    let label = args.label.clone().unwrap_or_else(|| base_name(results));
    let idx_note = maybe_upsert(args, &v, &label)?;
    print_verdict_line(&v, &verdict_path, &idx_note);

    // Exit follows the verdict's overall pass, so a hook FAIL (which the
    // step report does not count) still fails the process; the printed
    // report above is unchanged.
    Ok(if v.pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// The `diff --transcript` arm: judge a transcript, print a per-step
/// table, write the verdict, upsert the index.
fn run_diff_transcript(transcript: &Path, args: &DiffArgs) -> Result<ExitCode, String> {
    let text = std::fs::read_to_string(transcript)
        .map_err(|e| format!("reading {}: {e}", transcript.display()))?;
    let stem = stem_of(transcript);
    let revision = args.revision.unwrap_or_else(|| verdict::revision_of(&stem));
    let date = args
        .date
        .clone()
        .unwrap_or_else(|| verdict::civil_date(mtime_secs(transcript)));
    let v = verdict::from_transcript(
        args.id.as_deref(),
        &base_name(transcript),
        &text,
        revision,
        date,
    )?;

    for s in &v.steps {
        let mark = match s.conform {
            Some(true) => "pass",
            Some(false) => "FAIL",
            None => "-",
        };
        println!("step {:>3}  {mark:<4}  {}", s.index, s.title);
        for m in &s.mismatches {
            println!("            {m}");
        }
    }
    println!("{}: {} steps, {} judged", v.suite, v.steps.len(), v.judged);

    let verdict_path = transcript.with_file_name(format!("{stem}.verdict.json"));
    write_verdict_file(&verdict_path, &v)?;
    let label = args.label.clone().unwrap_or_else(|| {
        let dir = transcript.parent().map(base_name).unwrap_or_default();
        if dir.is_empty() {
            stem.clone()
        } else {
            format!("{dir}/{stem}")
        }
    });
    let idx_note = maybe_upsert(args, &v, &label)?;
    print_verdict_line(&v, &verdict_path, &idx_note);

    Ok(if v.pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Folds a transcript into a verdict written beside it (the `drive`
/// path); a suite-less trace transcript falls back to its stem so a
/// verdict is still emitted. The index is never touched here.
fn write_transcript_verdict(
    transcript: &Path,
    suite_override: Option<&str>,
) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(transcript)
        .map_err(|e| format!("reading {}: {e}", transcript.display()))?;
    let name = base_name(transcript);
    let stem = stem_of(transcript);
    let revision = verdict::revision_of(&stem);
    let date = verdict::civil_date(now_secs());
    let v = verdict::from_transcript(suite_override, &name, &text, revision, date.clone())
        .or_else(|_| verdict::from_transcript(Some(&stem), &name, &text, revision, date))?;
    let path = transcript.with_file_name(format!("{stem}.verdict.json"));
    write_verdict_file(&path, &v)?;
    Ok(path)
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

    /// A `diff` run judges exactly one input: a plan or a transcript,
    /// never both and never neither.
    #[test]
    fn diff_takes_a_plan_or_a_transcript_not_both() {
        assert!(Cli::try_parse_from(["v", "diff", "--plan", "p.json", "--results", "r"]).is_ok());
        assert!(Cli::try_parse_from(["v", "diff", "--transcript", "t.jsonl"]).is_ok());
        assert!(
            Cli::try_parse_from([
                "v",
                "diff",
                "--plan",
                "p.json",
                "--transcript",
                "t.jsonl",
                "--results",
                "r",
            ])
            .is_err()
        );
        assert!(Cli::try_parse_from(["v", "diff"]).is_err());
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
