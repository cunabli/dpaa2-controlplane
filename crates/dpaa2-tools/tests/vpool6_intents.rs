//! Pins the board-suite V-POOL-6 operands offline (pool-objects task 4.1, bead
//! dpaa2-controlplane-960.10): the two committed TOMLs the sitting runs through
//! `dpaa2ctl` must derive the ROOT pool counts the suite's grow/shrink walk
//! expects, so a drift in either operand fails here instead of on the board
//! mid-sitting. No board is touched — the operands are parsed by the shipped
//! `dpaa2-config` parser, completed exactly as `compile_intent` completes them
//! (the reserved-kernel injection, crates/dpaa2-tools/src/main.rs
//! `complete_kernel`), and compiled against a snapshot inventory.
//!
//! Acceptance (mbt-harness spec "Suite generation renders the pool walks"):
//! intent-a derives strictly MORE root trio capacity than intent-b (the
//! `[extra.kernel]` surplus), while the derived dpio seat count is EQUAL across
//! the pair (dpio is grow-only, pool-objects design D4) — so the a->b delta is a
//! pure free-only trio shrink (pool-objects design D3).
//!
//! The base counts fold the kernel port's dpaa2-eth probe draw into the pool
//! (pool-objects design D9, task 3.7): the one terminated kernel port adds +1
//! dpbp, +1 dpmcp, +`num_queues` dpcon on top of the ADR-0012 companion draw. On
//! the 16-CPU snapshot intent-b derives root dpmcp 18, dpbp 2, dpcon 32, 16 dpio
//! seats; intent-a's `[extra.kernel]` +2 per trio lifts dpmcp to 20 (the
//! board-suite headline, bead: V-POOL-6 intent-a converges dpmcp to 20).

use dpaa2_api::families::dpio::derived_seats;
use dpaa2_api::families::pool_lifecycle::{PoolFamily, derived_requirement};
use dpaa2_api::intent::compiled::Container;
use dpaa2_api::intent::refuse::compile;
use dpaa2_api::intent::{Intent, kernel_tenant};
use dpaa2_api::testkit::ref_inventory;

const CPUS: u32 = 16;

/// Reads, completes and compiles one committed operand from the suite dir — the
/// `compile_intent` pipeline (crates/dpaa2-tools/src/main.rs): parse, inject the
/// reserved kernel when a port names it and none is declared (`complete_kernel`;
/// declaring `[tenant.kernel]` is refused, ADR-0013), then compile.
fn compile_operand(file: &str) -> dpaa2_api::intent::refuse::Compiled {
    let toml = std::fs::read_to_string(format!(
        "{}/../../models/board/V-POOL-6/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("read {file}: {e}"));
    let mut intent: Intent =
        dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("{file}: parse: {e}"));
    // complete_kernel: a port names the kernel and none is declared (bead gqf.19).
    let port_names_kernel = intent.ports.iter().any(|p| p.tenant.is_kernel());
    let kernel_declared = intent.tenants.iter().any(|t| t.name.is_kernel());
    if port_names_kernel && !kernel_declared {
        intent.tenants.insert(0, kernel_tenant(i64::from(CPUS)));
    }
    compile(&intent, &ref_inventory(CPUS)).unwrap_or_else(|e| panic!("{file}: compile: {e:?}"))
}

fn root_req(compiled: &dpaa2_api::intent::refuse::Compiled, family: PoolFamily) -> i64 {
    derived_requirement(&compiled.plan, &Container::Root, family)
}

#[test]
fn intent_a_derives_more_trio_capacity_than_intent_b() {
    let a = compile_operand("intent-a.toml");
    let b = compile_operand("intent-b.toml");

    assert!(
        root_req(&b, PoolFamily::Dpbp) >= 1,
        "intent-b derives a non-empty root dpbp base"
    );

    // The `[extra.kernel]` surplus is +2 per trio family (intent-a.toml); the
    // port is identical across the pair, so the delta is exactly the extras.
    for (family, extra) in [
        (PoolFamily::Dpbp, 2),
        (PoolFamily::Dpcon, 2),
        (PoolFamily::Dpmcp, 2),
    ] {
        let (ra, rb) = (root_req(&a, family), root_req(&b, family));
        assert_eq!(
            ra - rb,
            extra,
            "{family:?}: intent-a ({ra}) must exceed intent-b ({rb}) by the extra ({extra})"
        );
    }

    // dpio is a grow-only per-CPU seat, equal across the pair (pool-objects design D4).
    assert_eq!(
        derived_seats(&a.plan, &Container::Root),
        derived_seats(&b.plan, &Container::Root),
        "dpio seat count is equal across the operand pair"
    );

    // Absolute operands on the 16-CPU snapshot (pool-objects design D9 per-port
    // fold, pool-objects task 3.7): intent-b is the folded base, intent-a lifts
    // dpmcp by the +2 extra to the board-suite headline 20.
    assert_eq!(root_req(&b, PoolFamily::Dpmcp), 18);
    assert_eq!(root_req(&b, PoolFamily::Dpbp), 2);
    assert_eq!(root_req(&b, PoolFamily::Dpcon), 32);
    assert_eq!(derived_seats(&b.plan, &Container::Root), 16);
    assert_eq!(root_req(&a, PoolFamily::Dpmcp), 20);
}
