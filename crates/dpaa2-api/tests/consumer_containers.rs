//! The consumer→container derivation (intent-compiler spec, `dprc-encapsulation`
//! task 4.1): a declared consumer runtime derives exactly one typed child-DPRC
//! realization, the reserved kernel derives none, and the derivation is container-only.
//!
//! These run the whole compile pipeline (`compile` → [`derive_consumer_containers`]) so
//! the realization is proved against real derivation output, not a hand-built plan. The
//! full plan a consumer compiles to still carries its companions and dpnis (the intent
//! layer sizes them); this change's derivation surface projects only the container —
//! `topology`, sizing and the dpni option surface stay tiles #5/#6.

use std::collections::BTreeMap;

use dpaa2_api::compiled::Container as Placement;
use dpaa2_api::dprc::{Container, Options};
use dpaa2_api::dprc_plan::{ConsumerContainer, derive_consumer_containers};
use dpaa2_api::testkit::ref_inventory;
use dpaa2_api::{
    Compiled, Dataplane, DpmacId, Family, Intent, Isolation, Link, MacMode, Port, Tenant,
    TenantName, TenantRef, compile,
};

// ---- fixtures (mirroring the `refuse.rs` unit-test helpers) ----

fn poll(name: &str) -> Tenant {
    Tenant {
        name: name.into(),
        dataplane: Dataplane::UserspacePoll,
        max_cores: 16,
        isolation: Isolation::Isolated,
        renamed: None,
    }
}

fn port(name: &str, dpmac: u32, tenant: &str) -> Port {
    Port {
        name: name.into(),
        dpmac: DpmacId::new(dpmac),
        rate: 10_000,
        tenant: TenantRef::from_name(tenant.into()),
        mac: None,
        mac_mode: MacMode::Assert,
        renamed: None,
    }
}

fn compiled(intent: &Intent) -> Compiled {
    compile(intent, &ref_inventory(16)).expect("intent compiles")
}

fn count_family(c: &Compiled, tenant: &str, family: Family) -> usize {
    c.plan
        .objects
        .iter()
        .filter(|o| o.key().tenant.as_str() == tenant && o.key().family == family)
        .count()
}

// ---- Scenario: Consumer container derivation ----

#[test]
fn a_declared_consumer_derives_exactly_one_container_with_the_default_mask() {
    // One consumer with a terminated port: the derivation yields exactly one child-DPRC
    // realization, keyed by the consumer, with the board-verified default mask, root
    // placement, and the consumer's name as its label (ADR-0005; ADR-0015).
    let intent = Intent {
        tenants: vec![poll("vpp")],
        ports: vec![port("wan0", 7, "vpp")],
        ..Intent::default()
    };
    let c = compiled(&intent);
    let containers = derive_consumer_containers(&c.plan);

    assert_eq!(containers.len(), 1, "exactly one container per consumer");
    let cc = &containers[&TenantName::from("vpp")];
    assert_eq!(cc.options, Options::DEFAULT, "the DPRC-I4 default mask");
    assert_eq!(cc.placement, Placement::Root, "root placement (dprc.1)");
    assert_eq!(cc.label.as_str(), "vpp", "name-keyed label");
    assert_eq!(cc.tenant, TenantName::from("vpp"));
}

#[test]
fn every_public_holder_and_isolated_consumer_derives_its_own_container() {
    // Two container-owning consumers derive two containers, each keyed by its own name.
    let intent = Intent {
        tenants: vec![
            poll("vpp"),
            Tenant {
                isolation: Isolation::Public,
                ..poll("holder")
            },
        ],
        ports: vec![port("wan0", 7, "vpp")],
        ..Intent::default()
    };
    let containers = derive_consumer_containers(&compiled(&intent).plan);
    let names: Vec<&str> = containers.keys().map(TenantName::as_str).collect();
    assert_eq!(
        names,
        ["holder", "vpp"],
        "one container per consumer, by name"
    );
    assert!(containers.values().all(|cc| cc.options == Options::DEFAULT));
    assert!(
        containers
            .values()
            .all(|cc| cc.placement == Placement::Root)
    );
}

// ---- Scenario: Kernel tenant derives no container ----

#[test]
fn the_reserved_kernel_derives_no_container() {
    // A link names the kernel, materialising it in the root container with its per-CPU
    // draw; the derivation still yields no child DPRC for it (ADR-0005), only for the
    // declared consumer.
    let intent = Intent {
        tenants: vec![poll("app")],
        links: vec![Link {
            name: "up".into(),
            interface_a: TenantRef::from_name("app".into()),
            interface_b: TenantRef::Kernel,
            renamed: None,
        }],
        ..Intent::default()
    };
    let c = compiled(&intent);
    // The kernel is materialised (its dpio draw is in the full plan) yet owns no DPRC.
    assert!(
        count_family(&c, "kernel", Family::Dpio) > 0,
        "kernel materialised"
    );
    assert_eq!(
        count_family(&c, "kernel", Family::Dprc),
        0,
        "no kernel DPRC"
    );

    let containers = derive_consumer_containers(&c.plan);
    assert!(
        !containers.contains_key(&TenantName::from("kernel")),
        "the kernel derives no container"
    );
    assert!(
        containers.contains_key(&TenantName::from("app")),
        "the declared consumer derives one"
    );
}

// ---- Scenario: Derivation is container-only ----

#[test]
fn the_derivation_is_container_only_dropping_companions_and_dpnis() {
    // The full plan sizes the consumer's companions and its port dpni — proving the
    // sizing rules are active elsewhere — while the derivation surface carries only the
    // container: no companion (dpio/dpbp/dpcon/dpmcp) or dpni is realized here (tiles
    // #5/#6). `ConsumerContainer` has no member for one, so the companion set is empty
    // by construction; the assertions below pin that against the live plan.
    let intent = Intent {
        tenants: vec![poll("vpp")],
        ports: vec![port("wan0", 7, "vpp")],
        ..Intent::default()
    };
    let c = compiled(&intent);
    // Precondition: the full plan DOES carry the companions and the dpni this surface drops.
    assert!(count_family(&c, "vpp", Family::Dpio) > 0);
    assert!(count_family(&c, "vpp", Family::Dpbp) > 0);
    assert!(count_family(&c, "vpp", Family::Dpmcp) > 0);
    assert!(count_family(&c, "vpp", Family::Dpni) > 0);

    let containers = derive_consumer_containers(&c.plan);
    // The consumer maps to a single container, and nothing else — companions and dpni
    // are absent from the derivation surface.
    assert_eq!(containers.len(), 1);
    let realized: BTreeMap<TenantName, ConsumerContainer> = containers;
    assert!(realized.contains_key(&TenantName::from("vpp")));
}

// ---- Per-object rule provenance cites the baseline anchor ----

#[test]
fn the_container_provenance_cites_the_baseline_anchor() {
    // The realization carries the derived object's provenance key; its node in the plan's
    // DAG cites the DPRC baseline (`docs/baseline/dprc.md` "Intent mapping": the
    // restool-default child options), reusing the intent layer's provenance idiom exactly.
    let intent = Intent {
        tenants: vec![poll("vpp")],
        ports: vec![port("wan0", 7, "vpp")],
        ..Intent::default()
    };
    let c = compiled(&intent);
    let cc = &derive_consumer_containers(&c.plan)[&TenantName::from("vpp")];

    assert_eq!(cc.provenance.rule.as_str(), "dprc", "the dprc rule");
    let node = c
        .plan
        .provenance
        .get(&cc.provenance)
        .expect("the derived container's provenance node resolves in the plan DAG");
    assert!(
        node.anchor.contains("dprc.md"),
        "the anchor cites the DPRC baseline: {}",
        node.anchor
    );
    assert_eq!(node.value, 1, "one child DPRC per consumer");
}

// ---- Model gate: entry-state pairing (DoD) ----

#[test]
fn the_derived_mask_pairs_with_the_dprc_lifecycle_entry_state() {
    // Binds the derivation output to the task-1.1 model's entry state (`dprc.qnt` `init`
    // / [`Container::declare`]): the derived option mask IS the lifecycle entry-state
    // mask, and realizing the container through create carries it unchanged (DPRC-I10,
    // create-time-immutable). This is the parity/pairing idiom of task 2.1, extended
    // from the state sum to the derived container.
    let intent = Intent {
        tenants: vec![poll("vpp")],
        ports: vec![port("wan0", 7, "vpp")],
        ..Intent::default()
    };
    let c = compiled(&intent);
    let cc = &derive_consumer_containers(&c.plan)[&TenantName::from("vpp")];

    // The declared (entry) container of the lifecycle carries the default mask; the
    // derivation output must equal it.
    let entry = Container::declare();
    assert_eq!(
        entry.options(),
        cc.options,
        "the derived mask is the lifecycle entry-state mask"
    );
    assert_eq!(cc.options, Options::DEFAULT);

    // Realizing the derived container (declare → create with the derived mask) preserves
    // the mask across the transition — the plan and the typestate agree.
    let created = Container::declare().create(cc.options);
    assert_eq!(created.options(), Options::DEFAULT);
}
