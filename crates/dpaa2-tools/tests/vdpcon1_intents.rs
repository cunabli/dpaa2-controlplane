//! Pins the dpcon priority knob's offline footprint (`cross-dprc-links` task 7.6).
//! `Tenant.priority` has NO effect on any MC operand today — the dpcon create
//! operand is the fixed default and the WQ priority binds only at consumer
//! registration (a later consumer-rig task). Its one and only artifact is a
//! single provenance node emitted by the `dpcon-priority` derivation rule
//! (crates/dpaa2-api/src/intent/derive.rs). This suite pins that footprint
//! through the shipped `dpaa2-config` parser and `compile` pipeline, the same
//! way the sibling `v*_intents` suites pin their operands offline: a stated
//! `Some(p)` adds exactly one provenance node over the baseline and nothing
//! else; an unset knob derives byte-identically to the baseline.
//!
//! TRIPWIRE: this MUST break the day `Tenant.priority` gains a load-bearing
//! consumer (consumer registration / the dpaa2-verify userspace consumer rig),
//! forcing a deliberate reconciliation of the COVERAGE DPCON-I3 row.

use dpaa2_api::intent::Intent;
use dpaa2_api::intent::compiled::ProvenanceKey;
use dpaa2_api::intent::refuse::{Compiled, compile};
use dpaa2_api::testkit::ref_inventory;

const CPUS: u32 = 16;

// A userspace-poll tenant with a port, so its dpcon node exists; `prio` is the one differing line.
fn intent_toml(prio: &str) -> String {
    format!(
        r#"[intent]
schema = 1

[tenant.vpp]
dataplane = "userspace-poll"
max_cores = 16
{prio}
[port.wan0]
dpmac = "dpmac.5"
rate = 25000
tenant = "vpp"
"#
    )
}

// Parse, inject the inert reserved kernel, then compile, as `compile_intent` does (src/main.rs).
fn compile_operand(prio: &str) -> Compiled {
    let toml = intent_toml(prio);
    let mut intent: Intent =
        dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("parse: {e}"));
    dpaa2_tools::complete_kernel(&mut intent, CPUS);
    compile(&intent, &ref_inventory(CPUS)).unwrap_or_else(|e| panic!("compile: {e:?}"))
}

#[test]
fn priority_knob_emits_one_provenance_node_and_nothing_else() {
    let base = compile_operand("");
    let withp = compile_operand("dpcon_priority = 3");

    let key = ProvenanceKey::new("vpp", "dpcon-priority", "");
    let node = withp
        .plan
        .provenance
        .get(&key)
        .expect("the stated priority emits one node");
    assert_eq!(node.value, 3);
    assert_eq!(node.request, 3);

    // Byte-identical to the baseline apart from that one node.
    let mut expected = base;
    expected.plan.provenance.insert(key, node.clone());
    assert_eq!(
        expected, withp,
        "Some(p) is the baseline plus exactly the one node"
    );
}

#[test]
fn unset_knob_derives_byte_identically() {
    let base = compile_operand("");
    let key = ProvenanceKey::new("vpp", "dpcon-priority", "");
    assert!(
        !base.plan.provenance.contains_key(&key),
        "an unset knob adds no node — the derivation stays at today's behavior"
    );
}
