//! End-to-end child-DPRC (consumer container) convergence through the product pipeline
//! (read → compile → reconcile → dispatch), driven entirely against the in-memory fake
//! backend (design D2/D10; reconciler delta; task 4.2). No board is touched.
//!
//! The acceptance scenario "Consumer declared on an empty board"
//! (`specs/reconciler/spec.md`): a declared consumer converges to exactly its child DPRC
//! — derived options/label/placement, no companion-population steps — the second run
//! plans zero actions, and the dry-run shows per-object provenance.

use std::collections::BTreeMap;

use dpaa2_api::dprc::Options;
use dpaa2_api::dprc_plan::{Attribution, ContainerVerdict, OptionBit};
use dpaa2_api::fake::FakeBackend;
use dpaa2_api::{
    Availability, Ceiling, Class, Compiled, Container, Dataplane, DpmacId, DpmacLinkType,
    DpmacOffer, Error, EthInterface, Family, Intent, Inventory, Isolation, MacMode, McControl,
    Port, Tenant, TenantRef, compile,
};
use dpaa2_tools::engine::{self, ContainerOutcome, ConvergeConfig};
use dpaa2_tools::render;

fn inventory() -> Inventory {
    let dpmacs = BTreeMap::from([(
        DpmacId::new(7),
        DpmacOffer {
            id: DpmacId::new(7),
            max_rate: 10_000,
            eth_if: EthInterface::Xfi,
            link_type: DpmacLinkType::Phy,
            avail: Availability::Free,
        },
    )]);
    let ceilings = BTreeMap::from([
        (Family::Dprc, Ceiling::Unknown),
        (Family::Dpni, Ceiling::Counted(18)),
        (Family::Dpbp, Ceiling::Counted(63)),
        (Family::Dpio, Ceiling::Unknown),
        (Family::Dpcon, Ceiling::Unknown),
        (Family::Dpmcp, Ceiling::Counted(203)),
        (Family::Dpseci, Ceiling::Unknown),
        (Family::Dpsw, Ceiling::Unknown),
    ]);
    Inventory {
        cpus: 16,
        dpmacs,
        labels: BTreeMap::new(),
        ceilings,
    }
}

/// A single isolated userspace-poll consumer ("router") on one 10G port — the smallest
/// intent that derives exactly one consumer child DPRC.
fn compiled_router() -> Compiled {
    let intent = Intent {
        tenants: vec![Tenant {
            name: "router".into(),
            dataplane: Dataplane::UserspacePoll,
            max_cores: 4,
            isolation: Isolation::Isolated,
            renamed: None,
        }],
        ports: vec![Port {
            name: "wan0".into(),
            dpmac: DpmacId::new(7),
            rate: 10_000,
            tenant: TenantRef::from_name("router".into()),
            mac: None,
            mac_mode: MacMode::Assert,
            renamed: None,
        }],
        ..Intent::default()
    };
    compile(&intent, &inventory()).expect("router intent must compile")
}

fn disruptive_cfg() -> ConvergeConfig {
    ConvergeConfig {
        allow: Class::Disruptive,
        ..ConvergeConfig::default()
    }
}

#[test]
fn consumer_on_empty_board_converges_to_only_the_container_and_reruns_clean() {
    let compiled = compiled_router();
    let backend = FakeBackend::new();

    // Empty board: exactly one consumer, planned to create only its child DPRC — no
    // companion/dpni steps — with the derived mask/label/placement (reconciler delta).
    let planned = engine::plan_containers(&compiled.plan, &backend).unwrap();
    assert_eq!(planned.len(), 1);
    let router = &planned[0];
    assert_eq!(router.container.label.as_str(), "router");
    assert_eq!(router.container.options, Options::DEFAULT);
    assert_eq!(router.container.placement, Container::Root);
    assert_eq!(router.plan.steps.len(), 1, "one create, no companion steps");
    assert_eq!(
        router.verdict,
        ContainerVerdict::Diverged(vec![dpaa2_api::dprc_plan::Divergence::Missing])
    );

    // First run: converges the container.
    assert_eq!(
        engine::converge_containers(&compiled.plan, &backend, disruptive_cfg()).unwrap(),
        ContainerOutcome::Converged
    );
    let observed = backend.observe_containers().unwrap();
    assert_eq!(observed.len(), 1, "exactly the child DPRC was created");
    let seen = observed.values().next().unwrap();
    assert_eq!(seen.label.as_str(), "router");
    assert_eq!(seen.options, Options::DEFAULT);
    assert_eq!(seen.placement, Container::Root);

    // Second run: zero planned actions, converged verdict, no new container.
    let replan = engine::plan_containers(&compiled.plan, &backend).unwrap();
    assert!(
        replan.iter().all(|c| c.plan.is_converged()),
        "the second run plans zero actions"
    );
    assert_eq!(replan[0].verdict, ContainerVerdict::Converged);
    assert_eq!(
        engine::converge_containers(&compiled.plan, &backend, disruptive_cfg()).unwrap(),
        ContainerOutcome::Converged
    );
    assert_eq!(
        backend.observe_containers().unwrap().len(),
        1,
        "re-run is idempotent: no second container"
    );
}

#[test]
fn dry_run_shows_container_plan_and_per_object_provenance() {
    // The dry-run text on an empty board: the container-only create step and the derived
    // container's provenance node resolved to its baseline anchor (design D6).
    let compiled = compiled_router();
    let backend = FakeBackend::new();
    let planned = engine::plan_containers(&compiled.plan, &backend).unwrap();
    insta::assert_snapshot!(render::render_container_convergence(
        &compiled.plan,
        &planned
    ));
}

#[test]
fn a_refused_create_is_attributed_and_does_not_converge() {
    // A typed MC status refusal (0x6, SPAWN absent) flows to the typed attribution and
    // the run reports a discriminated permission gap — never a collapsed denial, never a
    // convergence (design D4; the CLI exits non-zero on this outcome).
    let compiled = compiled_router();
    let backend = FakeBackend::new().with_dprc_create_refusal(Error::McStatus { status: 0x6 });
    let outcome = engine::converge_containers(&compiled.plan, &backend, disruptive_cfg()).unwrap();
    assert_eq!(
        outcome,
        ContainerOutcome::Refused {
            label: "router".into(),
            attribution: Attribution::PermissionGap {
                bit: OptionBit::Spawn
            },
        }
    );
    assert!(
        backend.observe_containers().unwrap().is_empty(),
        "a refused create leaves no container"
    );
}
