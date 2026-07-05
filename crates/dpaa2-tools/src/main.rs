//! `dpaa2ctl`: the DPAA2 provisioning CLI.
//!
//! Wires the TOML config frontend, the pure reconciler, and the `restool`/sysfs
//! backend into the four operator surfaces: `scan`, `ensure`, `status`, and
//! `dry-run` (proposal). A single `ensure` invocation runs to completion.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use dpaa2_api::{DesiredTopology, Error, ReconcileOptions, reconcile_with};
use dpaa2_mc::{RestoolMc, SysfsKernel};
use dpaa2_tools::engine::{self, ConvergeConfig, Outcome};
use dpaa2_tools::{StatusReport, link};

/// Declarative DPAA2 (DPNI↔DPMAC) provisioning for the LX2160A.
#[derive(Parser, Debug)]
#[command(name = "dpaa2ctl", version, about)]
struct Cli {
    /// Path to the desired-topology file.
    #[arg(long, default_value = "/etc/dpaa2/topology.toml", global = true)]
    config: PathBuf,

    /// The fsl-mc root container to operate on.
    #[arg(long, default_value = dpaa2_mc::DEFAULT_CONTAINER, global = true)]
    container: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Observe and print the current MC/kernel state.
    Scan,
    /// Reconcile the system toward the desired topology.
    Ensure {
        /// Overall convergence budget, in seconds.
        #[arg(long, default_value_t = 30)]
        deadline: u64,
        /// Tear down ports declared absent (opt-in).
        #[arg(long)]
        prune: bool,
        /// Skip generating and reloading `systemd.link` files.
        #[arg(long)]
        no_link: bool,
        /// Directory to write generated `.link` files into.
        #[arg(long, default_value = link::RUNTIME_NETWORK_DIR)]
        link_dir: PathBuf,
    },
    /// Block until the MC firmware answers an MC command, or timeout.
    WaitReady {
        /// Maximum seconds to wait for the MC to become responsive.
        #[arg(long, default_value_t = 60)]
        timeout: u64,
    },
    /// Print each managed port's lifecycle and the delta from desired.
    Status,
    /// Print the plan reconcile would execute; change nothing.
    DryRun {
        /// Tear down ports declared absent (opt-in).
        #[arg(long)]
        prune: bool,
    },
}

fn main() -> ExitCode {
    init_logging();
    let cli = Cli::parse();

    match run(&cli) {
        Ok(code) => code,
        Err(e) => {
            tracing::error!(error = %e, "fatal");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<ExitCode, Error> {
    let mc = RestoolMc::with_runner(dpaa2_mc::RestoolRunner::new(), cli.container.clone());
    let kernel = SysfsKernel::new(cli.container.clone());

    match &cli.command {
        Command::Scan => {
            let observed = engine::observe(&mc, &kernel)?;
            println!("{observed:#?}");
            Ok(ExitCode::SUCCESS)
        }
        Command::WaitReady { timeout } => {
            let ready = engine::wait_ready(
                &mc,
                Duration::from_secs(*timeout),
                Duration::from_millis(500),
            )?;
            Ok(if ready {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Command::Status => {
            let desired = load_config(&cli.config)?;
            let observed = engine::observe(&mc, &kernel)?;
            let report = StatusReport::compute(&desired, &observed);
            print!("{report}");
            Ok(if report.has_diverged() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::DryRun { prune } => {
            let desired = load_config(&cli.config)?;
            let observed = engine::observe(&mc, &kernel)?;
            let plan = reconcile_with(&desired, &observed, ReconcileOptions { prune: *prune });
            println!("{} planned transition(s):", plan.transitions.len());
            for t in &plan.transitions {
                println!("  {t:?}");
            }
            for d in &plan.drift {
                println!("  DRIFT {} {}: {}", d.dpni, d.attribute, d.detail);
            }
            for a in &plan.assertions {
                println!("  ASSERT {} {}: {}", a.port, a.field, a.detail);
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Ensure {
            deadline,
            prune,
            no_link,
            link_dir,
        } => ensure(&mc, &kernel, cli, *deadline, *prune, *no_link, link_dir),
    }
}

fn ensure(
    mc: &RestoolMc<dpaa2_mc::RestoolRunner>,
    kernel: &SysfsKernel,
    cli: &Cli,
    deadline: u64,
    prune: bool,
    no_link: bool,
    link_dir: &std::path::Path,
) -> Result<ExitCode, Error> {
    let desired = load_config(&cli.config)?;

    let cfg = ConvergeConfig {
        deadline: Duration::from_secs(deadline),
        prune,
        ..ConvergeConfig::default()
    };
    let outcome = engine::ensure(&desired, mc, kernel, cfg)?;

    // Apply stable names *after* convergence: the matchable MAC lives on the DPNI,
    // which does not exist until provisioning creates it. `link::apply` writes the
    // `.link` files from the now-known DPNI MACs and re-triggers udev so the rename
    // takes effect this boot (system-integration spec).
    if !no_link {
        let observed = engine::observe(mc, kernel)?;
        link::apply(&desired, &observed, link_dir)?;
    }

    match outcome {
        Outcome::Converged => Ok(ExitCode::SUCCESS),
        Outcome::DeadlineExceeded { unconverged } => {
            tracing::error!(?unconverged, "did not converge before deadline");
            Ok(ExitCode::FAILURE)
        }
    }
}

fn load_config(path: &std::path::Path) -> Result<DesiredTopology, Error> {
    dpaa2_config::load(path)
}

fn init_logging() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // Ignore the error if a global subscriber is already installed (e.g. in tests).
    let _ = fmt().with_env_filter(filter).try_init();
}
