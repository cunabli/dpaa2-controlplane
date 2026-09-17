//! Pins the board-suite V-DPRC-9 operands offline (dprc-encapsulation task 5.3,
//! bead dpaa2-controlplane-cd3.12): the two committed TOMLs the sitting runs
//! through `dpaa2ctl` must compile to the derivation the suite's read-back is
//! diffed against, so a drift in either operand fails here instead of on the
//! board mid-sitting. No board is touched — the operands are parsed by the
//! shipped `dpaa2-config` parser and compiled against a snapshot inventory,
//! exactly the pipeline `compile_intent` drives (crates/dpaa2-tools/src/main.rs).
//!
//! Acceptance (system-integration spec "End-to-end convergence diffs clean"):
//! intent-a declares one consumer that derives exactly its child DPRC —
//! container-only, no residents, no dpnis; intent-b is declared-empty and
//! derives nothing, the prune leg's operand.

use dpaa2_api::core::family::Family;
use dpaa2_api::dprc_plan::{ContainerStep, derive_consumer_containers, plan_consumer_container};
use dpaa2_api::families::dprc::Options;
use dpaa2_api::testkit::ref_inventory;
use dpaa2_api::{Attributes, Container, compile};

/// Reads and compiles one committed operand from the suite directory.
fn compile_operand(file: &str) -> dpaa2_api::Compiled {
    let toml = std::fs::read_to_string(format!(
        "{}/../../models/board/V-DPRC-9/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("read {file}: {e}"));
    let intent = dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("{file}: parse: {e}"));
    compile(&intent, &ref_inventory(16)).unwrap_or_else(|e| panic!("{file}: compile: {e:?}"))
}

#[test]
fn intent_a_derives_exactly_one_container_only_consumer() {
    let compiled = compile_operand("intent-a.toml");

    // Exactly one derived consumer container, with the derived label/placement/mask.
    let derived = derive_consumer_containers(&compiled.plan);
    assert_eq!(
        derived.len(),
        1,
        "one declared consumer derives one container"
    );
    let container = derived.values().next().expect("one container");
    assert_eq!(container.label.as_str(), "vdprc9");
    assert_eq!(container.placement, Container::Root);
    assert_eq!(container.options, Options::DEFAULT);

    // Zero residents: the container-only plan against a fresh (absent) observation is
    // a single CreateContainer with no companion/populate step (reconciler delta).
    let child = compiled
        .plan
        .objects
        .iter()
        .find(|o| matches!(o.attributes(), Attributes::Dprc { .. }))
        .expect("the child DPRC object");
    let plan = plan_consumer_container(child, None);
    assert_eq!(
        plan.steps.len(),
        1,
        "container-only: one create, no residents"
    );
    assert!(matches!(
        plan.steps[0],
        ContainerStep::CreateContainer { .. }
    ));

    // No dpnis: a portless tenant terminates no port, so the derivation mints none.
    assert!(
        compiled
            .plan
            .objects
            .iter()
            .all(|o| o.key().family != Family::Dpni),
        "a portless consumer derives no dpnis"
    );
}

#[test]
fn intent_b_derives_nothing() {
    let compiled = compile_operand("intent-b.toml");
    assert!(
        derive_consumer_containers(&compiled.plan).is_empty(),
        "the declared-empty prune operand derives zero consumer containers"
    );
    // `compile` always emits the reserved-kernel dprtc.0 singleton (the ever-present
    // real-time clock, root-placed and DPL-born), so the declared-empty operand
    // derives no *consumer* object: every object is that one kernel singleton.
    let consumer_objects: Vec<_> = compiled
        .plan
        .objects
        .iter()
        .filter(|o| o.key().family != Family::Dprtc)
        .collect();
    assert!(
        consumer_objects.is_empty(),
        "the declared-empty prune operand derives no consumer objects, found {consumer_objects:?}"
    );
}
