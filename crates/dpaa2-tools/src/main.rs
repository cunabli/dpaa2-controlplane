//! `dpaa2ctl`: the DPAA2 provisioning CLI.
//!
//! Wires the TOML config frontend, the pure reconciler, and the `restool`/sysfs
//! backend into the four operator surfaces: `scan`, `ensure`, `status`, and
//! `dry-run` (proposal). A single `ensure` invocation runs to completion.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};
use dpaa2_api::contract::McControl;
use dpaa2_api::core::error::Error;
use dpaa2_api::families::dpio::SeatDisposition;
use dpaa2_api::intent::refuse::{Compiled, compile};
use dpaa2_api::intent::{Intent, kernel_tenant};
use dpaa2_api::plan::Class;
use dpaa2_api::plan::reconcile::{ReconcileOptions, reconcile_with};
use dpaa2_mc::{RestoolMc, SysfsKernel};
use dpaa2_tools::engine::{
    self, ContainerOutcome, ConvergeConfig, Outcome, PoolOutcome, PoolPass, PopulationOutcome,
    PruneOutcome, RootDpniPruneOutcome,
};
use dpaa2_tools::{StatusReport, link, render};

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

/// The `--allow` gate value (ADR-0015 decision 12): the maximum disruption class an
/// `ensure` run may actuate. A CLI-side mirror of [`Class`] so `dpaa2-api` stays free
/// of the `clap` dependency; disruptive is never implied, so the default is `hitless`.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum AllowArg {
    /// Only set-label relabels and attribute asserts (the default).
    #[default]
    Hitless,
    /// Also an externally-held name change (a netdev rename).
    Boundary,
    /// Also destroy/create, link flap, disconnect, rewire.
    Disruptive,
}

impl From<AllowArg> for Class {
    fn from(a: AllowArg) -> Self {
        match a {
            AllowArg::Hitless => Class::Hitless,
            AllowArg::Boundary => Class::Boundary,
            AllowArg::Disruptive => Class::Disruptive,
        }
    }
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
        /// Tear down declared-absent ports and, behind `--allow disruptive`,
        /// undeclared containers — the tool's most destructive act
        /// (dprc-hardening design D8).
        #[arg(long)]
        prune: bool,
        /// Maximum disruption class the run may actuate (ADR-0015 decision 12); a
        /// plan whose headline exceeds it is refused, changing nothing. Disruptive is
        /// never implied.
        #[arg(long, value_enum, default_value_t = AllowArg::Hitless)]
        allow: AllowArg,
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
        /// Tear down declared-absent ports and, behind `--allow disruptive`,
        /// undeclared containers — the tool's most destructive act
        /// (dprc-hardening design D8).
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
            let Some((intent, compiled)) = compile_intent(&mc, &cli.config)? else {
                return Ok(ExitCode::FAILURE);
            };
            let desired = compiled.desired_topology_root(&intent);
            let observed = engine::observe(&mc, &kernel)?;
            let report = StatusReport::compute(&desired, &observed);
            print!("{report}");
            // The root-scope pool drift per family, read-only off the board (pool-objects task
            // 3.4): observed-vs-derived counts and the disposition convergence would take.
            // Reported alongside the port lifecycle; the exit code stays port-driven.
            let pools = engine::plan_pools(&compiled.plan, &mc)?;
            print!("{}", render::render_pool_drift(&compiled.plan, &pools));
            // The child-population census the same run would drive, read-only off the board
            // (pool-objects design D11): per-child dpni arity, trio counts, dpio seats, and the
            // VFIO bind. Reported alongside the port lifecycle; the exit code stays port-driven.
            let population = engine::plan_population(&compiled.plan, &mc, &kernel)?;
            print!("{}", render::render_population(&population));
            Ok(if report.has_diverged() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::DryRun { prune } => {
            let Some((intent, compiled)) = compile_intent(&mc, &cli.config)? else {
                return Ok(ExitCode::FAILURE);
            };
            let desired = compiled.desired_topology_root(&intent);
            let observed = engine::observe(&mc, &kernel)?;
            let plan = reconcile_with(
                &desired,
                &observed,
                ReconcileOptions { prune: *prune },
                &std::collections::BTreeSet::new(),
            );
            print!(
                "{}",
                render::render_dry_run(&compiled.plan, &compiled.warnings, &plan)
            );
            // The child-DPRC (consumer container) convergence the same run would drive,
            // re-observed off the board (design D2; ADR-0002; DPRC-I6): container-only steps and
            // per-object provenance, alongside the port families above.
            let containers = engine::plan_containers(&compiled.plan, &mc)?;
            print!(
                "{}",
                render::render_container_convergence(&compiled.plan, &containers)
            );
            // The undeclared-consumer prune report (dprc-encapsulation task 4.3),
            // classified read-only off the board — a dry-run dispatches nothing.
            let prune = engine::plan_prune_report(&compiled.plan, &mc)?;
            print!("{}", render::render_prune(&prune));
            // The root-scope pool convergence the same run would drive, censused read-only off
            // the board (pool-objects task 3.4): per-family observed-vs-derived counts, the
            // class-gated disposition, and each family's provenance — a dry-run dispatches nothing.
            let pools = engine::plan_pools(&compiled.plan, &mc)?;
            print!("{}", render::render_pool_drift(&compiled.plan, &pools));
            // The child-population census the same run would drive, read-only off the board
            // (pool-objects design D11): per-child dpni arity, its trio/dpio counts and the VFIO
            // bind — a dry-run dispatches nothing, so every child a converged board holds renders
            // an empty, hitless plan.
            let population = engine::plan_population(&compiled.plan, &mc, &kernel)?;
            print!("{}", render::render_population(&population));
            Ok(ExitCode::SUCCESS)
        }
        Command::Ensure {
            deadline,
            prune,
            allow,
            no_link,
            link_dir,
        } => ensure(
            &mc, &kernel, cli, *deadline, *prune, *allow, *no_link, link_dir,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn ensure(
    mc: &RestoolMc<dpaa2_mc::RestoolRunner>,
    kernel: &SysfsKernel,
    cli: &Cli,
    deadline: u64,
    prune: bool,
    allow: AllowArg,
    no_link: bool,
    link_dir: &std::path::Path,
) -> Result<ExitCode, Error> {
    let Some((intent, compiled)) = compile_intent(mc, &cli.config)? else {
        return Ok(ExitCode::FAILURE);
    };
    // Warnings are named on stderr so the executed plan on stdout stays clean; the
    // dry-run text carries the same set inline (design D2/D3; ADR-0002).
    let warnings = render::render_warnings(&compiled.warnings);
    if !warnings.is_empty() {
        eprint!("{warnings}");
    }
    // The root slice feeds the port loop: only root-planned dpnis actuate here, and a child
    // tenant's dpni is left for the population pass, never created in dprc.1 (pool-objects
    // design D11; bead dpaa2-controlplane-960.24 routed dry-run/status, this closes ensure).
    let desired = compiled.desired_topology_root(&intent);

    let cfg = ConvergeConfig {
        deadline: Duration::from_secs(deadline),
        prune,
        allow: allow.into(),
        ..ConvergeConfig::default()
    };

    // Grow the root's shared pool capacity FIRST (pool-objects design D3/D10, the grow half):
    // the kernel interface's dpaa2-eth bind draws dpbp/dpio/dpcon and a dpmcp from the root pool,
    // so the companions must exist before the port loop binds. The shrink half runs LAST, after
    // consumer teardown — grow-first, shrink-last (the teardown walk). A refusal exits non-zero.
    if let Some(code) = report_pool_outcome(
        &engine::converge_pools(&compiled.plan, mc, cfg, PoolPass::Grow)?,
        "grow",
    ) {
        return Ok(code);
    }

    let outcome = engine::ensure(&desired, mc, kernel, cfg)?;

    // A refused run changed nothing, so write no `.link` files either.
    if let Outcome::DisruptionRefused { headline, allowed } = &outcome {
        println!(
            "refused: this plan's headline is `{headline}`, but the run allows only up to \
             `{allowed}`.\nre-run with `--allow={headline}` to actuate it (disruptive is never \
             implied)."
        );
        return Ok(ExitCode::FAILURE);
    }

    // Converge the child-DPRC containers declared consumers own (design D2 (ADR-0002); reconciler
    // delta). Container-only: this creates each consumer's DPRC, no companion/dpni
    // steps. A refusal exits non-zero with the discriminated cause, changing nothing.
    match engine::converge_containers(&compiled.plan, mc, cfg)? {
        ContainerOutcome::Converged => {}
        ContainerOutcome::DisruptionRefused { headline, allowed } => {
            println!(
                "refused: a consumer container plan's headline is `{headline}`, but the run allows \
                 only up to `{allowed}`.\nre-run with `--allow={headline}` to actuate it \
                 (disruptive is never implied)."
            );
            return Ok(ExitCode::FAILURE);
        }
        ContainerOutcome::Refused { label, attribution } => {
            println!("refused: the child DPRC for `{label}` was denied: {attribution:?}");
            return Ok(ExitCode::FAILURE);
        }
    }

    // Populate each declared child container after it exists (pool-objects design D11;
    // system-integration req 1): per-port child dpnis with plan-derived arity, their dpmac
    // connect, the derived companions and dpio seats, then the VFIO handoff. A drift inside a
    // bound child or an over-allow headline exits non-zero, changing nothing.
    if let Some(code) = run_population(mc, kernel, &compiled.plan, cfg)? {
        return Ok(code);
    }

    // Prune undeclared consumer containers under the double gate (dprc-encapsulation task 4.3).
    // A separate pass so an empty intent still prunes; the report is printed before the
    // outcome, and a below-disruptive refusal exits non-zero, changing nothing.
    match engine::prune_containers(&compiled.plan, mc, kernel, cfg)? {
        PruneOutcome::Clean => {}
        PruneOutcome::ReportOnly { items } | PruneOutcome::Pruned { items } => {
            print!("{}", render::render_prune(&items));
        }
        PruneOutcome::DisruptionRefused {
            headline,
            allowed,
            items,
        } => {
            print!("{}", render::render_prune(&items));
            println!(
                "refused: pruning an undeclared container is `{headline}`, but the run allows \
                 only up to `{allowed}`.\nre-run with `--prune --allow={headline}` to actuate it \
                 (disruptive is never implied)."
            );
            return Ok(ExitCode::FAILURE);
        }
        // A locked or still-plugged candidate could not be torn down; it survives (docs/baseline/dprc.md DPRC-I11 lock face / DPRC-I2).
        PruneOutcome::Refused { id, attribution } => {
            println!("refused: undeclared container {id} could not be pruned: {attribution:?}");
            return Ok(ExitCode::FAILURE);
        }
    }

    // Prune undeclared managed-labelled root dpnis under the same double gate (pool-objects design D10/D11).
    // A root kernel interface is a consumer that draws the pool, so it is torn down here — after
    // containers, before the pool shrink — so the shrink reclaims the capacity it drew.
    if let Some(code) = report_root_dpni_prune(&engine::prune_root_dpnis(&compiled.plan, mc, cfg)?)
    {
        return Ok(code);
    }

    // Shrink/prune the root pool LAST (pool-objects design D3/D10, the shrink half): every
    // consumer that drew it has been torn down above, so a free-only shrink reclaims the surplus
    // and prunes the foreign objects — the teardown walk reaches the pool it could not before.
    if let Some(code) = report_pool_outcome(
        &engine::converge_pools(&compiled.plan, mc, cfg, PoolPass::Shrink)?,
        "shrink",
    ) {
        return Ok(code);
    }

    // The grow-only dpio seats a teardown cannot reclaim render as the typed reboot-required
    // residue (pool-objects design D4/D10; ADR-0003 §7): observed vs required, reboot-named,
    // never a live destroy — the only census delta a reboot then restores.
    if let SeatDisposition::RebootRequired(residue) =
        engine::plan_pools(&compiled.plan, mc)?.dpio_disposition()
    {
        println!("residue: {residue}");
    }

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
        // Handled above (returns before link application); listed for exhaustiveness.
        Outcome::DisruptionRefused { .. } => Ok(ExitCode::FAILURE),
    }
}

/// Converges the child-container populations, mapping each refusal to its operator message
/// and a non-zero exit (pool-objects design D11). Returns `Some(exit)` on a refusal (nothing
/// further should run), `None` when every child converged.
///
/// # Errors
/// Propagates a backend/kernel read/dispatch error.
fn run_population(
    mc: &RestoolMc<dpaa2_mc::RestoolRunner>,
    kernel: &SysfsKernel,
    plan: &dpaa2_api::intent::compiled::CompiledPlan,
    cfg: ConvergeConfig,
) -> Result<Option<ExitCode>, Error> {
    match engine::converge_population(plan, mc, kernel, cfg)? {
        PopulationOutcome::Converged => Ok(None),
        PopulationOutcome::DisruptionRefused { headline, allowed } => {
            println!(
                "refused: a child population's headline is `{headline}`, but the run allows only \
                 up to `{allowed}`.\nre-run with `--allow={headline}` to actuate it (disruptive is \
                 never implied)."
            );
            Ok(Some(ExitCode::FAILURE))
        }
        PopulationOutcome::DriftRefused { label } => {
            println!(
                "refused: child `{label}` is VFIO-bound but its population drifted; residents \
                 added to a bound child stay invisible until a rebind cycle (ADR-0017). The \
                 rebind policy is roadmap #9's decision (bead dpaa2-controlplane-w01)."
            );
            Ok(Some(ExitCode::FAILURE))
        }
    }
}

/// Maps a pool-convergence outcome to its operator message and a non-zero exit, or `None` when
/// the pass converged (pool-objects design D3/D10). `stage` names the half (grow/shrink) in the
/// refusal so the operator sees which one exceeded the gate.
fn report_pool_outcome(outcome: &PoolOutcome, stage: &str) -> Option<ExitCode> {
    match outcome {
        PoolOutcome::Converged => None,
        PoolOutcome::DisruptionRefused { headline, allowed } => {
            println!(
                "refused: root pool {stage} is `{headline}`, but the run allows only up to \
                 `{allowed}`.\nre-run with `--allow={headline}` to actuate it (disruptive is \
                 never implied)."
            );
            Some(ExitCode::FAILURE)
        }
        PoolOutcome::ShrinkRefused { refusal } => {
            println!("refused: {refusal}");
            Some(ExitCode::FAILURE)
        }
    }
}

/// Maps a root-dpni prune outcome to its operator message and a non-zero exit, or `None` when
/// nothing blocked (pool-objects design D10/D11). A report-only or refused outcome names the
/// candidates and the missing gate; a clean or pruned outcome proceeds.
fn report_root_dpni_prune(outcome: &RootDpniPruneOutcome) -> Option<ExitCode> {
    match outcome {
        RootDpniPruneOutcome::Clean | RootDpniPruneOutcome::Pruned { .. } => None,
        RootDpniPruneOutcome::ReportOnly { candidates } => {
            println!(
                "report-only: {} undeclared managed-labelled root dpni(s) {candidates:?} — \
                 re-run with `--prune --allow disruptive` to tear them down.",
                candidates.len()
            );
            None
        }
        RootDpniPruneOutcome::DisruptionRefused {
            headline,
            allowed,
            candidates,
        } => {
            println!(
                "refused: pruning {} undeclared root dpni(s) {candidates:?} is `{headline}`, but \
                 the run allows only up to `{allowed}`.\nre-run with `--prune --allow={headline}` \
                 to actuate it (disruptive is never implied).",
                candidates.len()
            );
            Some(ExitCode::FAILURE)
        }
    }
}

/// The read → complete → compile pipeline `ensure`, `dry-run`, and `status` share
/// (design D2/D10; ADR-0002, restool-baseline; bead gqf.19): loads the declared [`Intent`], reads the board's
/// hardware offer, completes the reserved kernel, then compiles.
///
/// On refusal it prints every rule with its offending construct and returns `Ok(None)`
/// so the caller exits non-zero having changed nothing — no reconcile, no link files
/// (design D9/D10; ADR-0006, restool-baseline). On success it returns the completed intent beside its [`Compiled`]
/// plan, from which the caller projects the [`dpaa2_api::core::model::DesiredTopology`] the
/// reconciler drives. Every intent now goes through `compile`; a kernel-owned,
/// port-only file behaves as before by construction (design D10; restool-baseline).
///
/// # Errors
///
/// Returns an [`Error`] only when the config is unreadable or the board cannot be
/// queried; a *refused* compile is not an error but the printed `Ok(None)` verdict.
fn compile_intent(
    mc: &RestoolMc<dpaa2_mc::RestoolRunner>,
    path: &std::path::Path,
) -> Result<Option<(Intent, Compiled)>, Error> {
    let mut intent = dpaa2_config::load(path)?;
    let inventory = mc.read_inventory()?;
    complete_kernel(&mut intent, inventory.cpus);
    match compile(&intent, &inventory) {
        Ok(compiled) => Ok(Some((intent, compiled))),
        Err(refusals) => {
            print!("{}", render::render_refusals(&refusals));
            Ok(None)
        }
    }
}

/// Reserved-kernel completion (design D1; restool-baseline): the config parser never creates a kernel
/// [`dpaa2_api::intent::Tenant`] — a port with no tenant defaults to the reserved name — so the
/// frontend injects `kernel_tenant(cpus)` at index 0 when a port terminates the kernel
/// and no kernel tenant is declared. A link naming the kernel is materialised inside
/// `compile`'s `effective_tenants`, so this completes the port case only (the
/// `dpaa2-verify` `intent_pairing` normative note; bead gqf.19).
fn complete_kernel(intent: &mut Intent, cpus: u32) {
    let declared = intent.tenants.iter().any(|t| t.name.is_kernel());
    let port_names_kernel = intent.ports.iter().any(|p| p.tenant.is_kernel());
    if port_names_kernel && !declared {
        intent.tenants.insert(0, kernel_tenant(i64::from(cpus)));
    }
}

fn init_logging() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // Ignore the error if a global subscriber is already installed (e.g. in tests).
    let _ = fmt().with_env_filter(filter).try_init();
}

#[cfg(test)]
mod tests {
    use dpaa2_api::core::model::{DpmacId, MacMode};
    use dpaa2_api::intent::{Intent, Port, TenantRef, kernel_tenant};

    use super::complete_kernel;

    fn port(tenant: &str) -> Port {
        Port {
            name: "wan0".into(),
            dpmac: DpmacId::new(7),
            rate: 10_000,
            tenant: TenantRef::from_name(tenant.into()),
            mac: None,
            mac_mode: MacMode::Assert,
            renamed: None,
        }
    }

    #[test]
    fn injects_kernel_tenant_at_index_zero_when_a_port_owns_it() {
        let mut intent = Intent {
            tenants: vec![],
            ports: vec![port("kernel")],
            ..Intent::empty()
        };
        complete_kernel(&mut intent, 16);
        assert_eq!(intent.tenants, vec![kernel_tenant(16)]);
    }

    #[test]
    fn does_nothing_when_the_kernel_is_already_declared() {
        // No duplicate, and the declared cpus budget is never overwritten.
        let mut intent = Intent {
            tenants: vec![kernel_tenant(16)],
            ports: vec![port("kernel")],
            ..Intent::empty()
        };
        complete_kernel(&mut intent, 8);
        assert_eq!(intent.tenants, vec![kernel_tenant(16)]);
    }

    #[test]
    fn does_nothing_when_no_port_names_the_kernel() {
        let mut intent = Intent {
            tenants: vec![],
            ports: vec![port("app")],
            ..Intent::empty()
        };
        complete_kernel(&mut intent, 16);
        assert!(intent.tenants.is_empty());
    }
}
