//! `dpaa2-verify` CLI: batch-suite generation and offline result diffing
//! (design D6). The board never runs this tool's generation side; it runs
//! the emitted, operator-reviewed scripts (ADR-0003 §1–2).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use dpaa2_verify::generate::{self, RecoveryGuarantee, SuiteKind, SuiteSpec};
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

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.command {
        Command::Generate {
            trace,
            id,
            class,
            flagged,
            recovery_verification,
            recovery_marker,
            out,
        } => {
            let json = std::fs::read_to_string(&trace)
                .map_err(|e| format!("reading {}: {e}", trace.display()))?;
            let parsed = dpaa2_verify::adapter::parse_mbt_trace(&json)?;
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
            println!(
                "wrote {} ({} steps) and {}",
                script_path.display(),
                suite.plan.steps.len(),
                plan_path.display()
            );
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
    }
}
