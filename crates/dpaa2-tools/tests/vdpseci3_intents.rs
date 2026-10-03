//! Pins the board-suite V-DPSECI-3 operand offline (dpseci-typestate task 5.1, bead
//! dpaa2-controlplane-lbk.9): the one committed `intent-a.toml` the sitting runs through
//! `dpaa2ctl` must derive the dpseci create block the dual-transport read-back expects, so a
//! drift in the operand fails here instead of on the board mid-sitting. No board is touched —
//! the operand is parsed by the shipped `dpaa2-config` parser, completed exactly as
//! `compile_intent` completes it (the reserved-kernel injection via the dpaa2-tools lib
//! `dpaa2_tools::complete_kernel` — a no-op here, the scratch intent names no kernel port), and compiled
//! against a snapshot inventory.
//!
//! The suite converges ONE crypto intent (dpseci-typestate design D6 "Suite A"): the scratch
//! `userspace-poll` tenant carries a single `[[crypto]]` block sized by its own flows, so its
//! dpseci lands in the tenant's own CHILD container (`Container::Child`) and VFIO-binds — the
//! `RemoteOwned` arrangement the suite's cross-boundary leg reads. The derived create block is
//! the whole point of the pin (dpseci-typestate design D3/D4): `num_queues` = flows, priorities
//! all-2 (the verified deployed profile; no intent knob — kernel-vs-2 semantics are baseline
//! unknown #4), options `HAS_CG` only (ADR-0013; `HAS_OPR`/`OPR_SHARED` bake in bits whose cost
//! is baseline unknown #6).

use dpaa2_api::core::family::Family;
use dpaa2_api::families::dpseci::{DpseciCfg, DpseciOpt, OptionMask};
use dpaa2_api::intent::Intent;
use dpaa2_api::intent::compiled::{Attributes, Container};
use dpaa2_api::intent::refuse::compile;
use dpaa2_api::testkit::ref_inventory;

const CPUS: u32 = 16;

/// Reads, completes and compiles the committed operand from the suite dir — the
/// `compile_intent` pipeline (crates/dpaa2-tools/src/main.rs): parse, inject the reserved
/// kernel when a port names it and none is declared (`dpaa2_tools::complete_kernel`), then compile. The
/// scratch intent names no kernel port, so the injection is inert; it is kept here so the pin
/// mirrors the shipped pipeline byte-for-byte.
fn compile_operand(file: &str) -> dpaa2_api::intent::refuse::Compiled {
    let toml = std::fs::read_to_string(format!(
        "{}/../../models/board/V-DPSECI-3/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("read {file}: {e}"));
    let mut intent: Intent =
        dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("{file}: parse: {e}"));
    dpaa2_tools::complete_kernel(&mut intent, CPUS);
    compile(&intent, &ref_inventory(CPUS)).unwrap_or_else(|e| panic!("{file}: compile: {e:?}"))
}

#[test]
fn intent_derives_the_scratch_child_dpseci_cfg() {
    let c = compile_operand("intent-a.toml");

    // Exactly one dpseci, in the scratch tenant's own child container — the VFIO/cross-container
    // arrangement the suite's RemoteOwned leg reads (dpseci-typestate design D6).
    let dpsecis: Vec<_> = c
        .plan
        .objects
        .iter()
        .filter(|o| o.key().family == Family::Dpseci)
        .collect();
    assert_eq!(dpsecis.len(), 1, "one dpseci for the one crypto block");
    let dpseci = dpsecis[0];
    assert_eq!(dpseci.container(), &Container::Child("scratch".into()));

    // The derived create block: num_queues = flows (2), priorities all-2, options HAS_CG only
    // (dpseci-typestate design D3/D4; convergence signature D9; V-DPSECI-2 hook read-back).
    assert_eq!(
        dpseci.attributes(),
        &Attributes::Dpseci {
            cfg: DpseciCfg::new(
                OptionMask::empty().with_flag(DpseciOpt::HasCg),
                2,
                vec![2, 2]
            )
            .unwrap(),
        }
    );
}
