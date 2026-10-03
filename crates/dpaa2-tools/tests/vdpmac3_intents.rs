//! Pins the board-suite V-DPMAC-3 operand offline (dpmac-typestate task 5.1, bead
//! dpaa2-controlplane-0xu.10): the one committed `intent-a.toml` the sitting runs
//! through `dpaa2ctl` must derive the operands both arbitration faces need, so a
//! drift in the operand fails here instead of on the board mid-sitting. No board
//! is touched — the operand is parsed by the shipped `dpaa2-config` parser,
//! completed exactly as `compile_intent` completes it (the reserved-kernel
//! injection via the dpaa2-tools lib `dpaa2_tools::complete_kernel`), and compiled
//! against a snapshot inventory.
//!
//! The suite converges TWO regimes from one file (dpmac-typestate design D7
//! "Suite A"): the reserved kernel terminates the wired 10G kern0 port, so its
//! dpni + pool companions land in the ROOT container (`Tenant::container` => Root,
//! pool-objects design D5) and read `KernelOwned` on the board; the isolated `remote`
//! userspace-poll tenant terminates the 25G rmt0 port, so its dpni + companions
//! land in its own CHILD container (`Container::Child`) and VFIO-bind — the
//! `RemoteOwned` arrangement the suite's cross-boundary leg reads.
//!
//! Root operands — kernel regime, 16-CPU snapshot (derive.rs `sizeTenant`, the
//! `is_kernel` arm) with the pool-objects design D9 per-port fold for the one
//! kern0 port (dpaa2-eth's probe draw): dpmcp 18, dpbp 2, dpcon 32, dpio 16 —
//! the kernel regime is rate-independent, so kern0 on 10G derives the same root
//! pool V-MVP-1/V-POOL-6 board-validated for a kernel port.
//!
//! Child operands — poll-mode arm (ADR-0012 companionDraw): T = 1 main + Σ
//! workers over terminated ports, and rmt0 is 25G ⇒ 5 workers (derive.rs
//! `workers_per_port`), so T = 6 and poll-mode `num_queues` = T: dpio 12 (2·T),
//! dpcon 6 (dpnis·T = 1·6), dpbp 2 and dpmcp 1 (poll-mode base).

use dpaa2_api::families::dpio::derived_seats;
use dpaa2_api::families::pool_lifecycle::{PoolFamily, derived_requirement};
use dpaa2_api::intent::Intent;
use dpaa2_api::intent::compiled::Container;
use dpaa2_api::intent::refuse::compile;
use dpaa2_api::testkit::ref_inventory;

const CPUS: u32 = 16;

/// Reads, completes and compiles the committed operand from the suite dir — the
/// `compile_intent` pipeline (crates/dpaa2-tools/src/main.rs): parse, inject the
/// reserved kernel when a port names it and none is declared (`dpaa2_tools::complete_kernel`;
/// declaring `[tenant.kernel]` is refused, ADR-0013), then compile.
fn compile_operand(file: &str) -> dpaa2_api::intent::refuse::Compiled {
    let toml = std::fs::read_to_string(format!(
        "{}/../../models/board/V-DPMAC-3/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("read {file}: {e}"));
    let mut intent: Intent =
        dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("{file}: parse: {e}"));
    dpaa2_tools::complete_kernel(&mut intent, CPUS);
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
fn intent_derives_the_kernel_root_and_remote_child_operands() {
    let c = compile_operand("intent-a.toml");
    let root = Container::Root;
    let child = Container::Child("remote".into());

    // KernelOwned face: the kernel regime on dpmac.7 with the pool-objects design D9
    // per-port fold folded into the ROOT pool (rate-independent, 10G).
    assert_eq!(req(&c, &root, PoolFamily::Dpmcp), 18);
    assert_eq!(req(&c, &root, PoolFamily::Dpbp), 2);
    assert_eq!(req(&c, &root, PoolFamily::Dpcon), 32);
    assert_eq!(derived_seats(&c.plan, &root), 16);

    // RemoteOwned face: the remote tenant's poll-mode companions in its own child
    // container (T = 6) — the VFIO/cross-container arrangement the suite reads.
    assert_eq!(req(&c, &child, PoolFamily::Dpmcp), 1);
    assert_eq!(req(&c, &child, PoolFamily::Dpbp), 2);
    assert_eq!(req(&c, &child, PoolFamily::Dpcon), 6);
    assert_eq!(derived_seats(&c.plan, &child), 12);
}
