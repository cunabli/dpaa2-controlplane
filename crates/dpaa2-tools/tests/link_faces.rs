//! Operator-face tests for dpni↔dpni links (cross-dprc-links task 5.5): the `dry-run` link
//! transitions with their class and provenance, the `status --detail` link and obligation rows
//! (honest-unknown on an unavailable read), and the declined-consent re-run hint. Driven through
//! the `FakeBackend` scenarios of tasks 5.2/5.3 so the rendered text is the operator's real output.

use dpaa2_api::contract::McControl;
use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::core::model::{DprcId, MacMode, ObjectRef};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpni::DpniCfg;
use dpaa2_api::families::pool_lifecycle::RawDriver;
use dpaa2_api::intent::refuse::{Compiled, compile};
use dpaa2_api::intent::{
    Dataplane, Intent, Isolation, Link, Port, Tenant, TenantRef, kernel_tenant,
};
use dpaa2_api::plan::Class;
use dpaa2_api::plan::connect::{CONNECT_ANCESTOR, ChildDeferredVisibility};
use dpaa2_api::testkit::ref_inventory;
use dpaa2_tools::engine::{self, ConvergeConfig};
use dpaa2_tools::render;

const LINK: &str = "l0";

// A root↔root link: the kernel end plus a restricted-to-kernel tenant (dataplane matched to the
// kernel holder), so both link-end dpnis land in dprc.1 sharing the link-name label.
fn compiled_root_link() -> Compiled {
    let secondary = Tenant {
        name: "sec".into(),
        dataplane: Dataplane::KernelNetlink,
        max_cores: 16,
        isolation: Isolation::Restricted {
            pool: "kernel".into(),
        },
        renamed: None,
        priority: None,
    };
    let intent = Intent {
        tenants: vec![kernel_tenant(16), secondary],
        links: vec![Link {
            name: LINK.into(),
            interface_a: TenantRef::Kernel,
            interface_b: TenantRef::from_name("sec".into()),
            renamed: None,
        }],
        ..Intent::empty()
    };
    compile(&intent, &ref_inventory(16)).expect("the root link intent compiles")
}

fn disruptive() -> ConvergeConfig {
    ConvergeConfig {
        allow: Class::Disruptive,
        ..ConvergeConfig::default()
    }
}

fn link_label() -> ConstructName {
    ConstructName::from(LINK)
}

#[test]
fn dry_run_shows_a_fresh_link_connect_with_provenance() {
    // An empty board: both root ends are to be created, so a fresh connect is planned, rendered
    // with its Disruptive class and the link-edge provenance tree.
    let compiled = compiled_root_link();
    let mc = FakeBackend::new();
    let links = engine::plan_links(&compiled.plan, &mc).unwrap();
    insta::assert_snapshot!(render::render_links(&compiled.plan, &links));
}

#[test]
fn dry_run_shows_a_held_end_refusal() {
    // Two root link dpnis exist, but one is wired to a foreign peer: the dry-run renders the typed
    // held-end refusal (DPRC-I5), the exact line ensure would refuse on.
    let compiled = compiled_root_link();
    let mc = FakeBackend::new();
    let a = mc.create_dpni(&link_label(), &DpniCfg::defaults()).unwrap();
    let _b = mc.create_dpni(&link_label(), &DpniCfg::defaults()).unwrap();
    let foreign = mc
        .create_dpni(&ConstructName::from("foreign"), &DpniCfg::defaults())
        .unwrap();
    mc.dprc_connect(
        CONNECT_ANCESTOR,
        a,
        ObjectRef::new(dpaa2_api::core::family::Family::Dpni, foreign.into_inner()),
    )
    .unwrap();
    let links = engine::plan_links(&compiled.plan, &mc).unwrap();
    insta::assert_snapshot!(render::render_links(&compiled.plan, &links));
}

#[test]
fn dry_run_shows_a_b_side_held_end_refusal() {
    // A foreign-held b end with a free a: the dry-run renders the typed refusal naming end b (DPRC-I5).
    let compiled = compiled_root_link();
    let mc = FakeBackend::new();
    let _a = mc.create_dpni(&link_label(), &DpniCfg::defaults()).unwrap();
    let b = mc.create_dpni(&link_label(), &DpniCfg::defaults()).unwrap();
    let foreign = mc
        .create_dpni(&ConstructName::from("foreign"), &DpniCfg::defaults())
        .unwrap();
    mc.dprc_connect(
        CONNECT_ANCESTOR,
        b,
        ObjectRef::new(dpaa2_api::core::family::Family::Dpni, foreign.into_inner()),
    )
    .unwrap();
    let links = engine::plan_links(&compiled.plan, &mc).unwrap();
    insta::assert_snapshot!(render::render_links(&compiled.plan, &links));
}

#[test]
fn converge_links_refuses_a_foreign_held_b_end() {
    // converge_links refuses a foreign-held b end rather than silently rewiring it (DPRC-I5).
    let compiled = compiled_root_link();
    let mc = FakeBackend::new();
    let _a = mc.create_dpni(&link_label(), &DpniCfg::defaults()).unwrap();
    let b = mc.create_dpni(&link_label(), &DpniCfg::defaults()).unwrap();
    let foreign = mc
        .create_dpni(&ConstructName::from("foreign"), &DpniCfg::defaults())
        .unwrap();
    mc.dprc_connect(
        CONNECT_ANCESTOR,
        b,
        ObjectRef::new(dpaa2_api::core::family::Family::Dpni, foreign.into_inner()),
    )
    .unwrap();
    let outcome = engine::converge_links(&compiled.plan, &mc, disruptive()).unwrap();
    assert!(
        matches!(outcome, engine::LinkOutcome::RewireRefused { .. }),
        "a foreign-held b end refuses: {outcome:?}"
    );
}

#[test]
fn status_detail_shows_a_converged_link_from_dprc_get_connection() {
    // After the links converge, status --detail reads each end and the connection from
    // dprc_get_connection and renders the row.
    let compiled = compiled_root_link();
    let mc = FakeBackend::new();
    engine::converge_links(&compiled.plan, &mc, disruptive()).unwrap();
    let rows = engine::link_rows(&compiled.plan, &mc).unwrap();
    insta::assert_snapshot!(render::render_link_detail(&rows, &[]));
}

#[test]
fn status_detail_link_state_is_honest_unknown_when_unresident() {
    // The ends are not yet resident, so the connection cannot be judged: the row renders unknown,
    // and the read itself succeeds (the command still exits success).
    let compiled = compiled_root_link();
    let mc = FakeBackend::new();
    let rows = engine::link_rows(&compiled.plan, &mc).expect("the read succeeds");
    assert!(
        rows.iter()
            .all(|r| r.connection == engine::LinkConnection::Unknown),
        "an unresident link reads unknown, never down or disconnected"
    );
    insta::assert_snapshot!(render::render_link_detail(&rows, &[]));
}

// The reference userspace router terminating two 10G ports, so its child container carries
// residents the bound-child obligation names.
fn compiled_router() -> Compiled {
    let router = Tenant {
        name: "router".into(),
        dataplane: Dataplane::UserspacePoll,
        max_cores: 16,
        isolation: Isolation::Isolated,
        renamed: None,
        priority: None,
    };
    let port = |name: ConstructName, dpmac: u32| Port {
        name,
        dpmac: dpaa2_api::core::model::DpmacId::new(dpmac),
        rate: 10_000,
        tenant: TenantRef::from_name("router".into()),
        mac: None,
        mac_mode: MacMode::Assert,
        renamed: None,
    };
    let intent = Intent {
        tenants: vec![router],
        ports: vec![port("wan0".into(), 7), port("wan1".into(), 9)],
        ..Intent::empty()
    };
    compile(&intent, &ref_inventory(16)).expect("the router intent compiles")
}

#[test]
fn status_detail_shows_a_standing_obligation_row() {
    // A child bound to vfio-fsl-mc with pending residents stands as a display-only obligation row.
    let compiled = compiled_router();
    let mc = FakeBackend::new().with_bound_dprc(DprcId::new(2), RawDriver::from("vfio-fsl-mc"));
    engine::converge_containers(&compiled.plan, &mc, disruptive()).unwrap();
    let obligations = engine::link_obligations(&compiled.plan, &mc, &mc).unwrap();
    assert_eq!(
        obligations.len(),
        1,
        "the bound child stands one obligation"
    );
    insta::assert_snapshot!(render::render_link_detail(&[], &obligations));
}

#[test]
fn declined_consent_output_carries_the_re_run_hint() {
    // The declined-consent face: the typed residue plus the actionable --allow=disruptive hint.
    let residue = ChildDeferredVisibility::new("router".into(), DprcId::new(2));
    let out = render::render_drift_refusal(&residue, Class::Hitless);
    assert!(
        out.contains("--allow=disruptive"),
        "names the consent flag: {out}"
    );
    insta::assert_snapshot!(out);
}
