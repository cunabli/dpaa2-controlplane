//! End-to-end child-DPRC (consumer container) convergence through the product pipeline
//! (read → compile → reconcile → dispatch), driven entirely against the in-memory fake
//! backend (design D2/D10; ADR-0002, restool-baseline; reconciler delta; task 4.2). No board is touched.
//!
//! The acceptance scenario "Consumer declared on an empty board"
//! (`specs/reconciler/spec.md`): a declared consumer converges to exactly its child DPRC
//! — derived options/label/placement, no companion-population steps — the second run
//! plans zero actions, and the dry-run shows per-object provenance.

use std::collections::BTreeMap;

use dpaa2_api::contract::McControl;
use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::model::{DpmacId, DpniId, DprcId, MacMode, ObjectRef, ObservedDpni};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dprc::{ContainerState, ObservedResident, Options, ResidentKind};
use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawDriver, RawLabel};
use dpaa2_api::intent::compiled::Container;
use dpaa2_api::intent::refuse::{Compiled, compile};
use dpaa2_api::intent::{Dataplane, Intent, Isolation, Port, Tenant, TenantRef};
use dpaa2_api::plan::Class;
use dpaa2_api::plan::dprc::{
    Attribution, ContainerVerdict, ObservedContainer, OptionBit, PruneBucket, plan_prune,
};
use dpaa2_api::testkit::ref_inventory;
use dpaa2_tools::engine::{
    self, ContainerOutcome, ConvergeConfig, PoolOutcome, PoolPass, PopulationOutcome, PruneOutcome,
    RootDpniPruneOutcome,
};
use dpaa2_tools::render;

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
        ..Intent::empty()
    };
    compile(&intent, &ref_inventory(16)).expect("router intent must compile")
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
        ContainerVerdict::Diverged(vec![dpaa2_api::plan::dprc::Divergence::Missing])
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
    // container's provenance node resolved to its baseline anchor (design D6; ADR-0004).
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
    // convergence (design D4; ADR-0003; the CLI exits non-zero on this outcome).
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

#[test]
fn observe_container_returns_present_and_absent() {
    // The per-candidate seam (dpni-typestate task 4.2): a seeded id reads back Some; an unseeded id is honest absence (None).
    let backend = FakeBackend::new().with_container(
        DprcId::new(5),
        orphan_container("foreign", Options::DEFAULT, Container::Root),
    );
    let seen = backend
        .observe_container(DprcId::new(5))
        .unwrap()
        .expect("seeded container reads back present");
    assert_eq!(seen.label.as_str(), "foreign");
    assert!(backend.observe_container(DprcId::new(9)).unwrap().is_none());
}

#[test]
fn converge_reobserves_the_created_candidate_per_id() {
    // The post-dispatch verdict re-observes the created container by id (dpni-typestate design D5): the fake mints dprc.2, which reads back converged.
    let compiled = compiled_router();
    let backend = FakeBackend::new();
    assert_eq!(
        engine::converge_containers(&compiled.plan, &backend, disruptive_cfg()).unwrap(),
        ContainerOutcome::Converged
    );
    let seen = backend
        .observe_container(DprcId::new(2))
        .unwrap()
        .expect("the created dprc.2 is re-observable by its id");
    assert_eq!(seen.label.as_str(), "router");
}

// ---- undeclared-consumer prune under the double gate (dprc-encapsulation task 4.3) ----

fn prune_disruptive_cfg() -> ConvergeConfig {
    ConvergeConfig {
        prune: true,
        allow: Class::Disruptive,
        ..ConvergeConfig::default()
    }
}

/// An empty intent: no declared consumer, so every observed child container is undeclared.
fn compiled_empty() -> Compiled {
    compile(&Intent::empty(), &ref_inventory(16)).expect("empty intent must compile")
}

/// An orphan child container seeded with two residents (created + assigned-in) so a
/// teardown predicts a real eviction post-state (ADR-0007 §3).
fn orphan_container(
    label: impl Into<ConstructName>,
    options: Options,
    placement: Container,
) -> ObservedContainer {
    // Keyed by family-qualified ObjectRef: a created dpbp + an assigned-in dpmcp (review M1; PASS3-F14; ADR-0007 §3).
    let mut residents = BTreeMap::new();
    residents.insert(
        ObjectRef::new(Family::Dpbp, 1),
        ObservedResident {
            origin: Some(ResidentKind::CreatedIn),
            plugged: false,
        },
    );
    residents.insert(
        ObjectRef::new(Family::Dpmcp, 2),
        ObservedResident {
            origin: Some(ResidentKind::AssignedIn),
            plugged: false,
        },
    );
    ObservedContainer {
        state: ContainerState::Populated,
        options,
        label: label.into(),
        placement,
        residents,
    }
}

#[test]
fn full_fingerprint_orphan_is_pruned_and_reruns_clean() {
    // Full fingerprint + empty intent + double gate: destroyed, absent on re-observation,
    // and the second pass plans zero and reports Clean (idempotent).
    let compiled = compiled_empty();
    let backend = FakeBackend::new().with_container(
        DprcId::new(5),
        orphan_container("foreign", Options::DEFAULT, Container::Root),
    );
    assert!(matches!(
        engine::prune_containers(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
            .unwrap(),
        PruneOutcome::Pruned { .. }
    ));
    assert!(
        backend.observe_containers().unwrap().is_empty(),
        "the orphan was torn down"
    );
    assert_eq!(
        engine::prune_containers(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
            .unwrap(),
        PruneOutcome::Clean
    );
}

#[test]
fn prune_verdict_reobserves_destroyed_id_as_absent() {
    // The prune verdict re-checks each destroyed id via observe_container, expecting None (dpni-typestate design D5); a survivor would be an error.
    let compiled = compiled_empty();
    let backend = FakeBackend::new().with_container(
        DprcId::new(5),
        orphan_container("foreign", Options::DEFAULT, Container::Root),
    );
    assert!(matches!(
        engine::prune_containers(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
            .unwrap(),
        PruneOutcome::Pruned { .. }
    ));
    assert!(backend.observe_container(DprcId::new(5)).unwrap().is_none());
}

#[test]
fn partial_fingerprint_orphan_is_pruned_under_the_double_gate() {
    // A non-default mask (seeded via with_container, which dprc_create's default cannot
    // produce) is a partial candidate; the double gate still tears it down.
    let compiled = compiled_empty();
    let partial = Options {
        spawn: false,
        ..Options::DEFAULT
    };
    let backend = FakeBackend::new().with_container(
        DprcId::new(5),
        orphan_container("foreign", partial, Container::Root),
    );
    assert!(matches!(
        engine::prune_containers(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
            .unwrap(),
        PruneOutcome::Pruned { .. }
    ));
    assert!(backend.observe_containers().unwrap().is_empty());
}

#[test]
fn empty_label_container_is_report_only_and_untouched() {
    // The report-only fence (ADR-0001 §4): an unlabeled container is never dispatched,
    // even under --prune --allow disruptive, and survives.
    let compiled = compiled_empty();
    let backend = FakeBackend::new().with_container(
        DprcId::new(5),
        orphan_container("", Options::DEFAULT, Container::Root),
    );
    match engine::prune_containers(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
        .unwrap()
    {
        PruneOutcome::ReportOnly { items } => {
            assert_eq!(
                items[&DprcId::new(5)].classification.bucket,
                PruneBucket::ReportOnly
            );
        }
        other => panic!("expected report-only, got {other:?}"),
    }
    assert_eq!(
        backend.observe_containers().unwrap().len(),
        1,
        "the report-only container is never touched"
    );
}

#[test]
fn candidate_without_prune_is_reported_not_dispatched() {
    // First gate withheld: --allow disruptive but no --prune reports the candidate and
    // dispatches nothing.
    let compiled = compiled_empty();
    let backend = FakeBackend::new().with_container(
        DprcId::new(5),
        orphan_container("foreign", Options::DEFAULT, Container::Root),
    );
    let cfg = ConvergeConfig {
        allow: Class::Disruptive,
        ..ConvergeConfig::default()
    };
    assert!(matches!(
        engine::prune_containers(&compiled.plan, &backend, &backend, cfg).unwrap(),
        PruneOutcome::ReportOnly { .. }
    ));
    assert_eq!(backend.observe_containers().unwrap().len(), 1);
}

#[test]
fn candidate_below_disruptive_is_refused_not_dispatched() {
    // Second gate withheld: --prune but --allow below disruptive refuses, changing nothing.
    let compiled = compiled_empty();
    let backend = FakeBackend::new().with_container(
        DprcId::new(5),
        orphan_container("foreign", Options::DEFAULT, Container::Root),
    );
    let cfg = ConvergeConfig {
        prune: true,
        allow: Class::Hitless,
        ..ConvergeConfig::default()
    };
    match engine::prune_containers(&compiled.plan, &backend, &backend, cfg).unwrap() {
        PruneOutcome::DisruptionRefused {
            headline, allowed, ..
        } => {
            assert_eq!(headline, Class::Disruptive);
            assert_eq!(allowed, Class::Hitless);
        }
        other => panic!("expected disruption-refused, got {other:?}"),
    }
    assert_eq!(backend.observe_containers().unwrap().len(), 1);
}

#[test]
fn declared_consumer_is_torn_down_once_intent_empties() {
    // Acceptance: converge a declared consumer (existing flow), then re-run with an empty
    // intent under the double gate — its now-undeclared container is torn down end to end,
    // and a second re-run is Clean.
    let compiled = compiled_router();
    let backend = FakeBackend::new();
    assert_eq!(
        engine::converge_containers(&compiled.plan, &backend, disruptive_cfg()).unwrap(),
        ContainerOutcome::Converged
    );
    assert_eq!(backend.observe_containers().unwrap().len(), 1);

    let empty = compiled_empty();
    assert!(matches!(
        engine::prune_containers(&empty.plan, &backend, &backend, prune_disruptive_cfg()).unwrap(),
        PruneOutcome::Pruned { .. }
    ));
    assert!(backend.observe_containers().unwrap().is_empty());
    assert_eq!(
        engine::prune_containers(&empty.plan, &backend, &backend, prune_disruptive_cfg()).unwrap(),
        PruneOutcome::Clean
    );
}

#[test]
fn plugged_resident_orphan_prune_is_refused_not_aborted() {
    // Plugged resident → destroy is -EBUSY/MC 0x10 (docs/baseline/dprc.md DPRC-I2); sound per-candidate refusal is dprc-hardening task 3.3 / PASS3-F5.
    let compiled = compiled_empty();
    let mut orphan = orphan_container("plugged-orphan", Options::DEFAULT, Container::Root);
    orphan
        .residents
        .get_mut(&ObjectRef::new(Family::Dpbp, 1))
        .expect("seed resident 1")
        .plugged = true;
    let backend = FakeBackend::new().with_container(DprcId::new(5), orphan);

    let outcome =
        engine::prune_containers(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
            .expect(
                "a plugged-resident candidate must be a per-candidate refusal, not an aborting Err",
            );
    assert!(
        !matches!(outcome, PruneOutcome::Clean),
        "a refused candidate is reported, not silently clean"
    );
    assert_eq!(
        backend.observe_containers().unwrap().len(),
        1,
        "a refused teardown leaves the plugged orphan in place (destroy is -EBUSY)"
    );
}

#[test]
fn locked_orphan_prune_is_lock_gated_and_survives() {
    // Locked → destroy is lock-stripped (docs/baseline/dprc.md DPRC-I11); sound lock-gate is dprc-hardening task 3.3.
    let compiled = compiled_empty();
    let mut orphan = orphan_container("locked-orphan", Options::DEFAULT, Container::Root);
    orphan.state = ContainerState::Locked;
    let backend = FakeBackend::new().with_container(DprcId::new(5), orphan);

    let outcome =
        engine::prune_containers(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
            .unwrap();
    assert!(
        !matches!(outcome, PruneOutcome::Pruned { .. }),
        "a locked candidate is lock-gated, not torn down"
    );
    assert_eq!(
        backend.observe_containers().unwrap().len(),
        1,
        "a Locked orphan survives the prune: destroy is lock-stripped (dprc.md DPRC-I11)"
    );
}

#[test]
fn render_prune_shows_candidate_partial_and_report_only() {
    // The dry-run block: a full candidate, a partial candidate, and a report-only fence
    // rendered together with buckets, fingerprint fields, steps and predicted post-state.
    let observed = BTreeMap::from([
        (
            DprcId::new(2),
            orphan_container("full", Options::DEFAULT, Container::Root),
        ),
        (
            DprcId::new(3),
            orphan_container(
                "partial",
                Options {
                    spawn: false,
                    ..Options::DEFAULT
                },
                Container::Root,
            ),
        ),
        (
            DprcId::new(4),
            orphan_container("", Options::DEFAULT, Container::Root),
        ),
    ]);
    let items = plan_prune(&observed, &BTreeMap::new());
    insta::assert_snapshot!(render::render_prune(&items));
}

#[test]
fn render_prune_empty_says_none() {
    insta::assert_snapshot!(render::render_prune(&BTreeMap::new()));
}

// ---- child population after container convergence (pool-objects design D11) ----

/// The reference router (two 10G ports, T = 5): its child derives TWO dpnis plus its
/// poll-mode companions, the arity the population reads from the plan (never a constant).
fn compiled_reference() -> Compiled {
    let port = |name: &str, dpmac: u32| Port {
        name: name.into(),
        dpmac: DpmacId::new(dpmac),
        rate: 10_000,
        tenant: TenantRef::from_name("router".into()),
        mac: None,
        mac_mode: MacMode::Assert,
        renamed: None,
    };
    let intent = Intent {
        tenants: vec![Tenant {
            name: "router".into(),
            dataplane: Dataplane::UserspacePoll,
            max_cores: 16,
            isolation: Isolation::Isolated,
            renamed: None,
        }],
        ports: vec![port("wan0", 7), port("wan1", 9)],
        ..Intent::empty()
    };
    compile(&intent, &ref_inventory(16)).expect("reference intent must compile")
}

#[test]
fn population_converges_the_child_then_reruns_clean() {
    // pool-objects design D11 / system-integration req 1: converge the container, then populate
    // it — two dpnis (plan arity), connected to their dpmac peers, plus the derived companions
    // and dpio seats. A second population pass creates and binds nothing (idempotent; the
    // vfio_handoff no-op on re-run rides the whole pass being empty).
    let compiled = compiled_reference();
    let backend = FakeBackend::new();

    assert_eq!(
        engine::converge_containers(&compiled.plan, &backend, disruptive_cfg()).unwrap(),
        ContainerOutcome::Converged
    );
    assert_eq!(
        engine::converge_population(&compiled.plan, &backend, &backend, disruptive_cfg()).unwrap(),
        PopulationOutcome::Converged
    );

    let child = DprcId::new(2);
    let count = |f: Family| backend.observe_pool(Some(child), f).unwrap().len();
    assert_eq!(
        count(Family::Dpni),
        2,
        "arity from the plan, not a constant"
    );
    assert_eq!(count(Family::Dpbp), 2);
    assert_eq!(count(Family::Dpmcp), 1);
    assert_eq!(count(Family::Dpcon), 10);
    assert_eq!(count(Family::Dpio), 10);

    // The plan re-reads converged and a second pass leaves the census untouched.
    let plans = engine::plan_population(&compiled.plan, &backend, &backend).unwrap();
    assert_eq!(plans.len(), 1);
    assert!(plans[0].is_converged(), "the re-plan is converged");
    assert_eq!(
        engine::converge_population(&compiled.plan, &backend, &backend, disruptive_cfg()).unwrap(),
        PopulationOutcome::Converged
    );
    assert_eq!(count(Family::Dpni), 2, "re-run creates no third dpni");
    assert_eq!(count(Family::Dpio), 10, "re-run creates no extra seats");
}

#[test]
fn population_refuses_drift_inside_a_bound_child() {
    // ADR-0017: a child already bound to vfio-fsl-mc whose plan still needs residents is a typed
    // drift refusal — residents added while bound stay invisible until a rebind — and nothing is
    // populated or healed here.
    let compiled = compiled_reference();
    // dprc_create mints dprc.2; seed it bound before it is created so the empty child reads bound.
    let backend =
        FakeBackend::new().with_bound_dprc(DprcId::new(2), RawDriver::from("vfio-fsl-mc"));
    assert_eq!(
        engine::converge_containers(&compiled.plan, &backend, disruptive_cfg()).unwrap(),
        ContainerOutcome::Converged
    );

    assert_eq!(
        engine::converge_population(&compiled.plan, &backend, &backend, disruptive_cfg()).unwrap(),
        PopulationOutcome::DriftRefused {
            label: "router".into()
        }
    );
    assert!(
        backend
            .observe_pool(Some(DprcId::new(2)), Family::Dpni)
            .unwrap()
            .is_empty(),
        "a bound-child drift refusal actuates nothing"
    );
}

#[test]
fn render_population_shows_the_converged_child() {
    // The dry-run/status block for a converged child: two present, connected dpnis, the trio
    // and dpio seats all hitless — the idempotent run's zero-action proof, printed.
    let compiled = compiled_reference();
    let backend = FakeBackend::new();
    engine::converge_containers(&compiled.plan, &backend, disruptive_cfg()).unwrap();
    engine::converge_population(&compiled.plan, &backend, &backend, disruptive_cfg()).unwrap();
    let plans = engine::plan_population(&compiled.plan, &backend, &backend).unwrap();
    insta::assert_snapshot!(render::render_population(&plans));
}

// ---- pool-objects task 3.14: teardown-to-baseline (design D10/D11) ----

/// A root topology dpni fixture (pool-objects design D10/D11 root-dpni prune): id, an optional
/// managed label, and an optional dpmac connection. An empty (`None`) label is the DPL/foreign
/// sentinel the one-label law exempts.
fn root_dpni(id: u32, label: Option<ConstructName>, connected: Option<u32>) -> ObservedDpni {
    ObservedDpni {
        id: DpniId::new(id),
        label,
        connected_to: connected.map(DpmacId::new),
        mac: None,
        netdev: None,
        attributes: BTreeMap::new(),
        cfg_observation: None,
    }
}

/// A root pool row wearing the kernel interface's construct label (pool-objects design D10): a
/// plugged, census-free row a consumer holds via the hidden in-use set.
fn kern_row(family: Family, ord: u32) -> ObservedPoolObject {
    ObservedPoolObject {
        object: ObjectRef::new(family, ord),
        label: RawLabel::from("kern0"),
        plugged: true,
        drawn: false,
    }
}

#[test]
fn undeclared_managed_labelled_root_dpni_prunes_under_the_double_gate() {
    // pool-objects design D10/D11: compiled_router declares "wan0"; the board carries three root
    // dpnis — the declared "wan0" (kept), an undeclared managed-labelled "kern0" (the target),
    // and an empty-label DPL-born one (exempt by the one-label law).
    let compiled = compiled_router();
    let backend = FakeBackend::new()
        .with_dpni(root_dpni(1, Some(ConstructName::from("wan0")), Some(7)))
        .with_dpni(root_dpni(2, Some(ConstructName::from("kern0")), Some(4)))
        .with_dpni(root_dpni(3, None, Some(17)));

    // First gate: `--prune` withheld ⇒ report-only, nothing torn down.
    match engine::prune_root_dpnis(&compiled.plan, &backend, &backend, disruptive_cfg()).unwrap() {
        RootDpniPruneOutcome::ReportOnly { candidates } => {
            assert_eq!(
                candidates,
                vec![DpniId::new(2)],
                "only the undeclared managed-labelled dpni"
            );
        }
        other => panic!("expected report-only, got {other:?}"),
    }
    assert_eq!(
        backend.observe().unwrap().dpnis.len(),
        3,
        "report-only actuates nothing"
    );
    assert!(
        !backend.audit().iter().any(|a| a.starts_with("unbind:")),
        "report-only never touches the kernel seam: {:?}",
        backend.audit()
    );

    // Both gates held ⇒ kern0 is torn down in ADR-0008 §8 order (disconnect, unbind, destroy); the declared "wan0" and the empty-label DPL dpni survive.
    match engine::prune_root_dpnis(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
        .unwrap()
    {
        RootDpniPruneOutcome::Pruned { pruned } => assert_eq!(pruned, vec![DpniId::new(2)]),
        other => panic!("expected pruned, got {other:?}"),
    }
    let surviving: Vec<u32> = backend
        .observe()
        .unwrap()
        .dpnis
        .iter()
        .map(|d| d.id.into_inner())
        .collect();
    assert_eq!(
        surviving,
        vec![1, 3],
        "declared and DPL-born dpnis are untouched"
    );
    let audit = backend.audit();
    let pos = |needle: &str| {
        audit
            .iter()
            .position(|a| a == needle)
            .unwrap_or_else(|| panic!("{needle} not audited in {audit:?}"))
    };
    assert!(
        pos("disconnect:dpni.2") < pos("unbind:dpni.2")
            && pos("unbind:dpni.2") < pos("destroy:dpni.2"),
        "kern0 is disconnected, then unbound, then destroyed: {audit:?}"
    );
}

#[test]
fn unconnected_candidate_still_unbinds_and_destroys() {
    // ADR-0008 §8: a candidate with no peer skips the disconnect but is still unbound before the
    // destroy — restool refuses a destroy of a driver-bound dpni client-side.
    let compiled = compiled_empty();
    let backend =
        FakeBackend::new().with_dpni(root_dpni(2, Some(ConstructName::from("kern0")), None));

    match engine::prune_root_dpnis(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
        .unwrap()
    {
        RootDpniPruneOutcome::Pruned { pruned } => assert_eq!(pruned, vec![DpniId::new(2)]),
        other => panic!("expected pruned, got {other:?}"),
    }
    let audit = backend.audit();
    assert!(
        !audit.iter().any(|a| a == "disconnect:dpni.2"),
        "an unconnected candidate emits no disconnect: {audit:?}"
    );
    assert!(
        audit.iter().position(|a| a == "unbind:dpni.2")
            < audit.iter().position(|a| a == "destroy:dpni.2"),
        "unbind still precedes destroy: {audit:?}"
    );
}

#[test]
fn root_dpni_prune_gates_never_touch_the_kernel_seam() {
    // pool-objects design D10/D11: `--prune` withheld ⇒ ReportOnly; a below-disruptive allow ⇒
    // DisruptionRefused. Neither dispatches, so the kernel unbind seam stays untouched.
    let compiled = compiled_empty();
    let backend =
        FakeBackend::new().with_dpni(root_dpni(2, Some(ConstructName::from("kern0")), Some(4)));

    assert!(matches!(
        engine::prune_root_dpnis(&compiled.plan, &backend, &backend, disruptive_cfg()).unwrap(),
        RootDpniPruneOutcome::ReportOnly { .. }
    ));
    let refuse_cfg = ConvergeConfig {
        prune: true,
        allow: Class::Hitless,
        ..ConvergeConfig::default()
    };
    match engine::prune_root_dpnis(&compiled.plan, &backend, &backend, refuse_cfg).unwrap() {
        RootDpniPruneOutcome::DisruptionRefused {
            headline, allowed, ..
        } => {
            assert_eq!(headline, Class::Disruptive);
            assert_eq!(allowed, Class::Hitless);
        }
        other => panic!("expected disruption-refused, got {other:?}"),
    }
    assert!(
        !backend.audit().iter().any(|a| a.starts_with("unbind:")),
        "neither gate touches the kernel seam: {:?}",
        backend.audit()
    );
}

#[test]
fn empty_intent_reaches_prune_once_the_consumer_is_torn_down() {
    // pool-objects design D10 teardown walk: a kernel interface holds root pool objects it drew.
    // The old order (shrink first) bounces the in-use probe; the reorder (grow, consumer
    // teardown, shrink) reaches prune and reclaims the pool.
    let compiled = compiled_empty();
    let backend = FakeBackend::new()
        .with_dpni(root_dpni(1, Some(ConstructName::from("kern0")), Some(4)))
        .with_in_use_pool_object(DprcId::ROOT, kern_row(Family::Dpbp, 1))
        .with_in_use_pool_object(DprcId::ROOT, kern_row(Family::Dpmcp, 2));

    // Old-order regression: a pool shrink before the consumer teardown hits the in-use probe.
    assert!(
        engine::converge_pools(
            &compiled.plan,
            &backend,
            prune_disruptive_cfg(),
            PoolPass::Shrink
        )
        .is_err(),
        "a pool shrink before consumer teardown bounces the in-use unplug probe"
    );

    // Reordered walk: grow-first defers, prune tears the consumer down, shrink-last reclaims.
    assert_eq!(
        engine::converge_pools(
            &compiled.plan,
            &backend,
            prune_disruptive_cfg(),
            PoolPass::Grow
        )
        .unwrap(),
        PoolOutcome::Converged
    );
    assert!(matches!(
        engine::prune_root_dpnis(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
            .unwrap(),
        RootDpniPruneOutcome::Pruned { .. }
    ));
    assert_eq!(
        engine::converge_pools(
            &compiled.plan,
            &backend,
            prune_disruptive_cfg(),
            PoolPass::Shrink
        )
        .unwrap(),
        PoolOutcome::Converged
    );
    assert!(
        backend.observe_pool(None, Family::Dpbp).unwrap().is_empty(),
        "the drawn pool reclaims once its consumer is gone"
    );
    assert!(
        backend
            .observe_pool(None, Family::Dpmcp)
            .unwrap()
            .is_empty()
    );
    assert!(
        backend.observe().unwrap().dpnis.is_empty(),
        "the kernel dpni was pruned"
    );
}

#[test]
fn container_prune_unbinds_and_disconnects_before_destroy() {
    // pool-objects design D11 teardown walk: a VFIO-bound child with a connected child dpni is
    // unbound and its dpni disconnected BEFORE its residents are destroyed.
    let compiled = compiled_empty();
    let backend = FakeBackend::new()
        .with_container(
            DprcId::new(5),
            orphan_container("foreign", Options::DEFAULT, Container::Root),
        )
        .with_bound_dprc(DprcId::new(5), RawDriver::from("vfio-fsl-mc"))
        .with_pool_object(
            DprcId::new(5),
            ObservedPoolObject {
                object: ObjectRef::new(Family::Dpni, 10),
                label: RawLabel::from("foreign"),
                plugged: true,
                drawn: false,
            },
        );
    // The child dpni is connected to a root dpmac from the common ancestor (DPNI-I9 form).
    backend
        .connect_in(
            DprcId::ROOT,
            DpniId::new(10),
            ObjectRef::new(Family::Dpmac, 4),
        )
        .unwrap();

    assert!(matches!(
        engine::prune_containers(&compiled.plan, &backend, &backend, prune_disruptive_cfg())
            .unwrap(),
        PruneOutcome::Pruned { .. }
    ));
    let audit = backend.audit();
    let pos = |needle: &str| {
        audit
            .iter()
            .position(|a| a.contains(needle))
            .unwrap_or_else(|| panic!("{needle} not audited in {audit:?}"))
    };
    assert!(
        pos("vfio_unbind") < pos("disconnect"),
        "unbind before disconnect: {audit:?}"
    );
    assert!(
        pos("disconnect") < pos("dprc_destroy"),
        "disconnect before destroy: {audit:?}"
    );
}

#[test]
fn dpio_seat_residue_renders_as_reboot_required() {
    // pool-objects design D4/D10: an observed dpio seat count above the required renders the
    // typed reboot-required residue (observed vs required, ADR-0003 §7) in the pool block the
    // dry-run and status surfaces print — never a live destroy.
    let compiled = compiled_empty(); // derived dpio requirement 0
    let backend = FakeBackend::new()
        .with_pool_object(
            DprcId::ROOT,
            ObservedPoolObject {
                object: ObjectRef::new(Family::Dpio, 0),
                label: RawLabel::from("kern0"),
                plugged: true,
                drawn: false,
            },
        )
        .with_pool_object(
            DprcId::ROOT,
            ObservedPoolObject {
                object: ObjectRef::new(Family::Dpio, 1),
                label: RawLabel::from("kern0"),
                plugged: true,
                drawn: false,
            },
        );
    let drift = engine::plan_pools(&compiled.plan, &backend).unwrap();
    assert_eq!(drift.dpio_observed, 2);
    assert_eq!(drift.dpio_required, 0);
    let text = render::render_pool_drift(&compiled.plan, &drift);
    assert!(text.contains("reboot-required"), "{text}");
    assert!(text.contains("ADR-0003 §7"), "{text}");
}
