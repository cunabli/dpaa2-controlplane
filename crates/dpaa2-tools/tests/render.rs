//! Snapshot tests over the pure render functions (design D9). Inputs are built
//! through the public API — a hand-built [`Intent`] and [`Inventory`] compiled by
//! [`compile`] — so the frozen text is the operator's real dry-run, board-free.

use std::collections::BTreeMap;

use dpaa2_api::{
    Availability, Ceiling, Crypto, Dataplane, DpmacId, DpmacLinkType, DpmacOffer, EthInterface,
    Extra, Family, Intent, Inventory, Isolation, MacMode, Port, ReconcileOptions, Tenant, compile,
    kernel_tenant, reconcile_with,
};
use dpaa2_tools::render::{render_dry_run, render_refusals};

const RESERVED_3: &str = "ADR-0003 §3: total-deny";

fn offer(id: u32, rate: i64, avail: Availability) -> (DpmacId, DpmacOffer) {
    let d = DpmacId::new(id);
    (
        d,
        DpmacOffer {
            id: d,
            max_rate: rate,
            eth_if: EthInterface::Xfi,
            link_type: DpmacLinkType::Phy,
            avail,
        },
    )
}

fn inventory() -> Inventory {
    let dpmacs = BTreeMap::from([
        offer(3, 25_000, Availability::Reserved(RESERVED_3.to_owned())),
        offer(4, 25_000, Availability::Free),
        offer(7, 10_000, Availability::Free),
        offer(9, 10_000, Availability::Free),
    ]);
    let ceilings = BTreeMap::from([
        (Family::Dprc, Ceiling::Unknown),
        (
            Family::Dpni,
            Ceiling::Observed {
                n: 18,
                provenance: "ADR-0011 decision 2".to_owned(),
            },
        ),
        (Family::Dpbp, Ceiling::Counted(63)),
        (Family::Dpio, Ceiling::Unknown),
        (Family::Dpcon, Ceiling::Unknown),
        (
            Family::Dpmcp,
            Ceiling::Observed {
                n: 203,
                provenance: "ADR-0011 decision 3".to_owned(),
            },
        ),
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

fn poll(name: &str) -> Tenant {
    Tenant {
        name: name.into(),
        dataplane: Dataplane::UserspacePoll,
        max_cores: 16,
        isolation: Isolation::Isolated,
        pool: "".into(),
        renamed: None,
    }
}

fn port(name: &str, dpmac: u32, rate: i64, tenant: &str) -> Port {
    Port {
        name: name.into(),
        dpmac: DpmacId::new(dpmac),
        rate,
        tenant: tenant.into(),
        mac: None,
        mac_mode: MacMode::Assert,
        renamed: None,
    }
}

/// A userspace-poll router on two 10G ports with a dpio extra, beside a
/// kernel-owned port (the kernel tenant declared explicitly, as the frontend's
/// completion would inject it): exercises the poll-mode T tree, the raise-only
/// extra, and the kernel per-CPU draw all at once.
#[test]
fn dry_run_reference() {
    let intent = Intent {
        tenants: vec![kernel_tenant(16), poll("router")],
        ports: vec![
            port("wan0", 7, 10_000, "router"),
            port("wan1", 9, 10_000, "router"),
            port("lan0", 4, 25_000, "kernel"),
        ],
        extras: [Extra {
            tenant: "router".into(),
            family: Family::Dpio,
            count: 3,
        }]
        .into_iter()
        .collect(),
        ..Intent::default()
    };
    let compiled = compile(&intent, &inventory()).expect("intent must compile");
    let desired = compiled.desired_topology(&intent);
    // Empty board: the dry-run prints the transitions that would build the ports.
    let observed = dpaa2_api::ObservedTopology {
        dpnis: vec![],
        dpmacs: vec![],
    };
    let plan = reconcile_with(&desired, &observed, ReconcileOptions::default());
    insta::assert_snapshot!(render_dry_run(&compiled.plan, &compiled.warnings, &plan));
}

/// A crypto tenant plus a warning-bearing mix, to freeze the warnings block and a
/// dpseci provenance node.
#[test]
fn dry_run_crypto_and_warning() {
    let intent = Intent {
        tenants: vec![poll("router")],
        ports: vec![
            port("wan0", 7, 10_000, "router"),
            port("wan1", 4, 25_000, "router"),
        ],
        crypto: vec![Crypto {
            tenant: "router".into(),
            flows: 4,
        }],
        ..Intent::default()
    };
    let compiled = compile(&intent, &inventory()).expect("intent must compile");
    let desired = compiled.desired_topology(&intent);
    let observed = dpaa2_api::ObservedTopology {
        dpnis: vec![],
        dpmacs: vec![],
    };
    let plan = reconcile_with(&desired, &observed, ReconcileOptions::default());
    insta::assert_snapshot!(render_dry_run(&compiled.plan, &compiled.warnings, &plan));
}

/// A port anchored on a Reserved dpmac: the compile is refused, and the text names
/// every broken rule with its offending construct (design D5/D10).
#[test]
fn refusal_reserved_anchor() {
    let intent = Intent {
        tenants: vec![kernel_tenant(16)],
        ports: vec![port("wan0", 3, 25_000, "kernel")],
        ..Intent::default()
    };
    let refusals = compile(&intent, &inventory()).expect_err("intent must be refused");
    insta::assert_snapshot!(render_refusals(&refusals));
}
