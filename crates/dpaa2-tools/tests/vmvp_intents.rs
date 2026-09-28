//! Pins the board-suite V-MVP-1 operand offline (pool-objects task 4.3, bead
//! dpaa2-controlplane-960.12; system-integration req 1): the one committed
//! `intent.toml` the sitting runs through `dpaa2ctl` must derive the counts the
//! MVP walk expects, so a drift in the operand fails here instead of on the
//! board mid-sitting. No board is touched — the operand is parsed by the shipped
//! `dpaa2-config` parser, completed exactly as `compile_intent` completes it (the
//! reserved-kernel injection, crates/dpaa2-tools/src/main.rs `complete_kernel`),
//! and compiled against a snapshot inventory.
//!
//! The MVP converges TWO regimes from one file (system-integration req 1): the
//! reserved kernel terminates the wired 10G kern0 port, so its dpni + pool
//! companions land in the ROOT container (`Tenant::container` => Root, pool-objects design D5);
//! the isolated `router` userspace-poll tenant terminates the 25G rtr0 port, so
//! its dpni + companions land in its own CHILD container (`Container::Child`).
//!
//! Root operands — kernel regime, 16-CPU snapshot (derive.rs `sizeTenant`, the
//! `is_kernel` arm): a base of per-CPU + per-dpni draws, then the pool-objects
//! design D9 per-port fold (dpaa2-eth's probe draw, pool-objects task 3.7): +1
//! dpbp, +1 dpmcp, +`num_queues` dpcon for the one kern0 port, where the
//! kernel's `num_queues` = cpus. So:
//!   - dpmcp 18 = base (cpus 16 + dpni 1) + fold (1 port) 1
//!   - dpbp   2 = base (dpni 1) + fold (1 port) 1
//!   - dpcon 32 = base dpni·min(cpus, `num_queues`) 16 + fold (1 port · `num_queues` 16) 16
//!   - dpio  16 = cpus (`SeatRegime::KernelSeat`)
//!
//! These equal V-POOL-6 intent-b's board-validated root base to the object: the
//! kernel regime is rate-independent (`num_queues` = cpus, not the 10G/25G worker
//! count), so kern0 on 10G derives the same root pool as V-POOL-6's kern0 on 25G.
//!
//! Child operands — poll-mode arm (ADR-0012 companionDraw): T = 1 main + Σ
//! workers over terminated ports, and rtr0 is 25G ⇒ 5 workers (derive.rs
//! `workers_per_port`), so T = 6, and the poll-mode `num_queues` = T. So:
//!   - dpio  12 = 2·T                (2·threads, `SeatRegime::DpdkSeat`)
//!   - dpcon  6 = dpnis·T = 1·6
//!   - dpbp   2 = poll-mode base
//!   - dpmcp  1 = poll-mode base (one portal per process)

use dpaa2_api::families::dpio::derived_seats;
use dpaa2_api::families::pool_lifecycle::{PoolFamily, derived_requirement};
use dpaa2_api::intent::compiled::Container;
use dpaa2_api::intent::refuse::compile;
use dpaa2_api::intent::{Intent, kernel_tenant};
use dpaa2_api::testkit::ref_inventory;

const CPUS: u32 = 16;

/// Reads, completes and compiles the committed operand from the suite dir — the
/// `compile_intent` pipeline (crates/dpaa2-tools/src/main.rs): parse, inject the
/// reserved kernel when a port names it and none is declared (`complete_kernel`;
/// declaring `[tenant.kernel]` is refused, ADR-0013), then compile.
fn compile_operand(file: &str) -> dpaa2_api::intent::refuse::Compiled {
    let toml = std::fs::read_to_string(format!(
        "{}/../../models/board/V-MVP-1/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("read {file}: {e}"));
    let mut intent: Intent =
        dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("{file}: parse: {e}"));
    let port_names_kernel = intent.ports.iter().any(|p| p.tenant.is_kernel());
    let kernel_declared = intent.tenants.iter().any(|t| t.name.is_kernel());
    if port_names_kernel && !kernel_declared {
        intent.tenants.insert(0, kernel_tenant(i64::from(CPUS)));
    }
    compile(&intent, &ref_inventory(CPUS)).unwrap_or_else(|e| panic!("{file}: compile: {e:?}"))
}

fn req(
    compiled: &dpaa2_api::intent::refuse::Compiled,
    container: &Container,
    family: PoolFamily,
) -> i64 {
    derived_requirement(&compiled.plan, container, family)
}

#[test]
fn intent_derives_the_kernel_root_and_router_child_operands() {
    let c = compile_operand("intent.toml");
    let root = Container::Root;
    let child = Container::Child("router".into());

    // Root: the kernel regime with the pool-objects design D9 per-port fold folded in — equal
    // to V-POOL-6 intent-b's board-validated base (kern0 on 10G, rate-independent).
    assert_eq!(req(&c, &root, PoolFamily::Dpmcp), 18);
    assert_eq!(req(&c, &root, PoolFamily::Dpbp), 2);
    assert_eq!(req(&c, &root, PoolFamily::Dpcon), 32);
    assert_eq!(derived_seats(&c.plan, &root), 16);

    // Child: the router's poll-mode companions in its own container (T = 6).
    assert_eq!(req(&c, &child, PoolFamily::Dpmcp), 1);
    assert_eq!(req(&c, &child, PoolFamily::Dpbp), 2);
    assert_eq!(req(&c, &child, PoolFamily::Dpcon), 6);
    assert_eq!(derived_seats(&c.plan, &child), 12);
}
