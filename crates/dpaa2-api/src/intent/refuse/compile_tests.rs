//! `compile` against the smallest intent that triggers each refusal variant
//! (one per variant, asserting the *complete* set — the model's scenario-twin
//! style), and one positive test per companion rule (design
//! D3/D4/D5/D6a; 2026-09-06-intent-layer). The
//! reference-board intent is the oracle for the poll-mode draws (ADR-0013 §7,
//! `models/intent/scenarios/reference.qnt`).

use std::collections::BTreeSet;

use super::{Compiled, Referrer, Refusal, Warning, compile};
use crate::core::family::Family;
use crate::core::inventory::{Availability, Ceiling, Inventory};
use crate::core::model::DpmacId;
use crate::intent::compiled::{Attributes, Container, ProvenanceNode};
use crate::intent::{
    Crypto, Dataplane, Extra, Fabric, Intent, Isolation, Link, Member, Port, Switching, Tenant,
    TenantRef, kernel_tenant,
};
// The reference-board inventory is single-sourced in the testkit seam
// (`crate::testkit`); both this unit-test suite and the `compile_props`
// integration suite build it from there (ADR-0013 §7).
use crate::testkit::{RESERVED_3, offer, ref_inventory};

// ---- builders ----

/// The reference board inventory at the reference online-CPU count (16).
fn ref_inv() -> Inventory {
    ref_inventory(16)
}

fn tenant(name: &str, dp: Dataplane, cores: i64, iso: Isolation) -> Tenant {
    Tenant {
        name: name.into(),
        dataplane: dp,
        max_cores: cores,
        isolation: iso,
        renamed: None,
    }
}

/// A restricted tenant pooling `pool` — the `Isolation::Restricted` payload
/// carries the holder name (2026-09-08-vocabulary-v2 D1).
fn restricted(pool: &str) -> Isolation {
    Isolation::Restricted { pool: pool.into() }
}

fn poll(name: &str) -> Tenant {
    tenant(name, Dataplane::UserspacePoll, 16, Isolation::Isolated)
}
fn knl(name: &str) -> Tenant {
    tenant(name, Dataplane::KernelNetlink, 16, Isolation::Isolated)
}
fn port(name: &str, dpmac: u32, rate: i64, tenant: &str) -> Port {
    Port {
        name: name.into(),
        dpmac: DpmacId::new(dpmac),
        rate,
        tenant: TenantRef::from_name(tenant.into()),
        mac: None,
        mac_mode: crate::core::model::MacMode::Assert,
        renamed: None,
    }
}
fn link(name: &str, a: &str, b: &str) -> Link {
    Link {
        name: name.into(),
        interface_a: TenantRef::from_name(a.into()),
        interface_b: TenantRef::from_name(b.into()),
        renamed: None,
    }
}

fn err(intent: &Intent, inv: &Inventory) -> BTreeSet<Refusal> {
    compile(intent, inv).expect_err("intent must be refused")
}
fn ok(intent: &Intent, inv: &Inventory) -> Compiled {
    compile(intent, inv).expect("intent must compile")
}

// ---- plan accessors ----

fn count_fam(c: &Compiled, tenant: &str, family: Family) -> usize {
    c.plan
        .objects
        .iter()
        .filter(|o| o.key().tenant.as_str() == tenant && o.key().family == family)
        .count()
}
fn attributes_of(c: &Compiled, tenant: &str, family: Family, ordinal: u32) -> Attributes {
    c.plan
        .objects
        .iter()
        .find(|o| {
            o.key().tenant.as_str() == tenant
                && o.key().family == family
                && o.key().ordinal == ordinal
        })
        .map(|o| o.attributes().clone())
        .expect("object present")
}
fn container_of(c: &Compiled, tenant: &str, family: Family, ordinal: u32) -> Container {
    c.plan
        .objects
        .iter()
        .find(|o| {
            o.key().tenant.as_str() == tenant
                && o.key().family == family
                && o.key().ordinal == ordinal
        })
        .map(|o| o.container().clone())
        .expect("object present")
}
fn provenance<'a>(
    c: &'a Compiled,
    tenant: &str,
    rule: &str,
    construct: &str,
) -> &'a ProvenanceNode {
    c.plan
        .provenance
        .iter()
        .find(|(k, _)| {
            k.tenant.as_str() == tenant
                && k.rule.as_str() == rule
                && k.construct.as_str() == construct
        })
        .map(|(_, v)| v)
        .expect("provenance node present")
}

// ======================================================================
// one test per Refusal variant (22) — the smallest triggering intent
// ======================================================================

#[test]
fn refuse_tenant_absent() {
    let intent = Intent {
        ports: vec![port("wan0", 7, 10_000, "ghost")],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::TenantAbsent {
            referrer: Referrer::Port("wan0".into()),
            tenant: "ghost".into(),
        }])
    );
}

#[test]
fn kernel_forwarded_fabric_resolves_without_a_declared_kernel() {
    // A hardware fabric forwarded by the reserved kernel, with tenant-only members
    // and no kernel-owned port, resolves at compile: the forwarder names the
    // kernel, a resolving referent never declared (design D1, 2026-09-06-intent-layer;
    // mirroring
    // parse's `resolves`). Materialisation is owned elsewhere, so the empty
    // refusal set is the whole answer — in particular no `TenantAbsent` for the
    // forwarder.
    let intent = Intent {
        tenants: vec![knl("a")],
        fabrics: vec![Fabric {
            name: "hw".into(),
            switching: Switching::Hardware,
            forwarded_by: "kernel".into(),
            members: vec![Member::Tenant("a".into())],
            renamed: None,
        }],
        ..Intent::default()
    };
    assert_eq!(compile(&intent, &ref_inv()).err(), None);
}

#[test]
fn kernel_crypto_allocation_resolves() {
    // A crypto allocation whose tenant is the reserved kernel, with no kernel port,
    // resolves as the fabric forwarder does (design D1, 2026-09-06-intent-layer): the
    // reserved kernel is a resolving referent without being declared, so the
    // complete refusal set is empty and carries no `TenantAbsent`.
    let intent = Intent {
        crypto: vec![Crypto {
            tenant: "kernel".into(),
            flows: 4,
        }],
        ..Intent::default()
    };
    assert_eq!(compile(&intent, &ref_inv()).err(), None);
}

#[test]
fn refuse_member_unresolved() {
    let intent = Intent {
        tenants: vec![knl("sw")],
        fabrics: vec![Fabric {
            name: "f".into(),
            switching: Switching::Software,
            forwarded_by: "sw".into(),
            members: vec![Member::Tenant("ghost".into())],
            renamed: None,
        }],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::MemberUnresolved {
            fabric: "f".into(),
            member: Member::Tenant("ghost".into()),
        }])
    );
}

#[test]
fn refuse_self_member() {
    let intent = Intent {
        tenants: vec![knl("sw")],
        fabrics: vec![Fabric {
            name: "f".into(),
            switching: Switching::Software,
            forwarded_by: "sw".into(),
            members: vec![Member::Tenant("sw".into())],
            renamed: None,
        }],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::SelfMember {
            fabric: "f".into(),
            member: Member::Tenant("sw".into()),
        }])
    );
}

#[test]
fn refuse_unanchored() {
    let intent = Intent {
        tenants: vec![knl("t")],
        ports: vec![port("wan0", 99, 10_000, "t")],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::Unanchored {
            port: "wan0".into(),
            dpmac: DpmacId::new(99),
        }])
    );
}

#[test]
fn refuse_reserved() {
    let intent = Intent {
        tenants: vec![knl("t")],
        ports: vec![port("wan0", 3, 25_000, "t")],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::Reserved {
            port: "wan0".into(),
            dpmac: DpmacId::new(3),
            why: RESERVED_3.to_owned(),
        }])
    );
}

#[test]
fn refuse_foreign() {
    let mut inv = ref_inv();
    inv.dpmacs
        .extend([offer(11, 25_000, Availability::Foreign("dpl".to_owned()))]);
    let intent = Intent {
        tenants: vec![knl("t")],
        ports: vec![port("wan0", 11, 25_000, "t")],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &inv),
        BTreeSet::from([Refusal::Foreign {
            port: "wan0".into(),
            dpmac: DpmacId::new(11),
            owner: "dpl".to_owned(),
        }])
    );
}

#[test]
fn refuse_double_claimed() {
    let intent = Intent {
        tenants: vec![knl("t")],
        ports: vec![port("wan0", 7, 10_000, "t"), port("wan1", 7, 10_000, "t")],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::DoubleClaimed {
            dpmac: DpmacId::new(7),
            constructs: vec!["wan0".into(), "wan1".into()],
        }])
    );
}

#[test]
fn refuse_over_rate() {
    let intent = Intent {
        tenants: vec![knl("t")],
        ports: vec![port("wan0", 7, 25_000, "t")],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::OverRate {
            port: "wan0".into(),
            rate: 25_000,
            max_rate: 10_000,
        }])
    );
}

#[test]
fn refuse_fabric_not_kernel_forwarded() {
    let intent = Intent {
        tenants: vec![knl("sw")],
        fabrics: vec![Fabric {
            name: "f".into(),
            switching: Switching::Hardware,
            forwarded_by: "sw".into(),
            members: vec![],
            renamed: None,
        }],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::FabricNotKernelForwarded {
            fabric: "f".into(),
            forwarded_by: "sw".into(),
        }])
    );
}

#[test]
fn refuse_port_tenant_mismatch() {
    let intent = Intent {
        tenants: vec![kernel_tenant(16), knl("other")],
        ports: vec![port("p", 7, 10_000, "other")],
        fabrics: vec![Fabric {
            name: "f".into(),
            switching: Switching::Hardware,
            forwarded_by: "kernel".into(),
            members: vec![Member::Port("p".into())],
            renamed: None,
        }],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::PortTenantMismatch {
            fabric: "f".into(),
            port: "p".into(),
            tenant: "other".into(),
        }])
    );
}

#[test]
fn refuse_unsupported_edge() {
    let intent = Intent {
        tenants: vec![kernel_tenant(16)],
        fabrics: vec![
            Fabric {
                name: "f1".into(),
                switching: Switching::Hardware,
                forwarded_by: "kernel".into(),
                members: vec![Member::Fabric("f2".into())],
                renamed: None,
            },
            Fabric {
                name: "f2".into(),
                switching: Switching::Hardware,
                forwarded_by: "kernel".into(),
                members: vec![],
                renamed: None,
            },
        ],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::UnsupportedEdge {
            fabric: "f1".into(),
            member: "f2".into(),
        }])
    );
}

#[test]
fn refuse_unknown_rate_class() {
    let mut inv = ref_inv();
    inv.dpmacs.extend([offer(12, 100_000, Availability::Free)]);
    let intent = Intent {
        tenants: vec![poll("t")],
        ports: vec![port("wan0", 12, 40_000, "t")],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &inv),
        BTreeSet::from([Refusal::UnknownRateClass {
            tenant: "t".into(),
            rates: vec![40_000],
        }])
    );
}

#[test]
fn refuse_core_budget_exceeded() {
    let intent = Intent {
        tenants: vec![tenant(
            "t",
            Dataplane::UserspacePoll,
            1,
            Isolation::Isolated,
        )],
        ports: vec![port("wan0", 7, 10_000, "t")],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::CoreBudgetExceeded {
            tenant: "t".into(),
            t: 3,
            max_cores: 1,
        }])
    );
}

#[test]
fn refuse_extra_not_companion() {
    let mut extras = BTreeSet::new();
    extras.insert(Extra {
        tenant: "t".into(),
        family: Family::Dpni,
        count: 1,
    });
    let intent = Intent {
        tenants: vec![knl("t")],
        extras,
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::ExtraNotCompanion {
            tenant: "t".into(),
            family: Family::Dpni,
        }])
    );
}

#[test]
fn refuse_extra_not_positive() {
    let mut extras = BTreeSet::new();
    extras.insert(Extra {
        tenant: "t".into(),
        family: Family::Dpio,
        count: 0,
    });
    let intent = Intent {
        tenants: vec![knl("t")],
        extras,
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::ExtraNotPositive {
            tenant: "t".into(),
            family: Family::Dpio,
            count: 0,
        }])
    );
}

#[test]
fn refuse_crypto_flows_not_positive() {
    let intent = Intent {
        tenants: vec![knl("t")],
        crypto: vec![Crypto {
            tenant: "t".into(),
            flows: 0,
        }],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::CryptoFlowsNotPositive {
            tenant: "t".into(),
            ordinal: 1,
            flows: 0,
        }])
    );
}

#[test]
fn refuse_crypto_flows_over_device() {
    let intent = Intent {
        tenants: vec![knl("t")],
        crypto: vec![Crypto {
            tenant: "t".into(),
            flows: 17,
        }],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::CryptoFlowsOverDevice {
            tenant: "t".into(),
            ordinal: 1,
            flows: 17,
            max_flows: 16,
        }])
    );
}

#[test]
fn refuse_infeasible() {
    let mut inv = ref_inv();
    inv.ceilings.insert(Family::Dpni, Ceiling::Counted(0));
    let intent = Intent {
        tenants: vec![knl("t")],
        ports: vec![port("wan0", 7, 10_000, "t")],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &inv),
        BTreeSet::from([Refusal::Infeasible {
            family: Family::Dpni,
            needed: 1,
            available: 0,
        }])
    );
}

#[test]
fn refuse_unpriced_dataplane() {
    let intent = Intent {
        tenants: vec![tenant(
            "t",
            Dataplane::UserspaceEvent,
            16,
            Isolation::Isolated,
        )],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::UnpricedDataplane {
            tenant: "t".into(),
            dataplane: Dataplane::UserspaceEvent,
        }])
    );
}

#[test]
fn refuse_holder_not_public() {
    let intent = Intent {
        tenants: vec![
            tenant("t", Dataplane::KernelNetlink, 16, restricted("h")),
            knl("h"), // Isolated, not Public
        ],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::HolderNotPublic {
            tenant: "t".into(),
            holder: "h".into(),
        }])
    );
}

#[test]
fn refuse_pool_chain() {
    // t -> h, and h itself pools g (h is Restricted): h is not Public, so t's
    // holder is both not-public and pooled — HolderNotPublic and PoolChain
    // co-fire. h itself pools the Public g cleanly, so h is not refused.
    let intent = Intent {
        tenants: vec![
            tenant("g", Dataplane::KernelNetlink, 16, Isolation::Public),
            tenant("h", Dataplane::KernelNetlink, 16, restricted("g")),
            tenant("t", Dataplane::KernelNetlink, 16, restricted("h")),
        ],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([
            Refusal::HolderNotPublic {
                tenant: "t".into(),
                holder: "h".into(),
            },
            Refusal::PoolChain {
                tenant: "t".into(),
                holder: "h".into(),
            },
        ])
    );
}

/// Spec scenario "A port named `pool` cannot collide in refusal rendering"
/// (2026-09-08-vocabulary-v2 D3, PASS3-F13): an operator legally declares a port named
/// `pool` and a restricted tenant whose holder is undeclared. The missing-holder
/// refusal identifies the drawing tenant through [`Referrer::Pool`] — a typed
/// site, not a `ConstructName` carrying the literal `"pool"` — so it is
/// distinguishable from anything referring to the port. Before 2026-09-08-vocabulary-v2 D3 both rendered a
/// `construct: "pool"` and collided.
#[test]
fn port_named_pool_does_not_collide_with_pool_referrer() {
    let intent = Intent {
        tenants: vec![
            kernel_tenant(16),
            tenant("t", Dataplane::KernelNetlink, 16, restricted("ghost")),
        ],
        // A perfectly legal port an operator named `pool`, owned by the kernel.
        ports: vec![port("pool", 7, 10_000, "kernel")],
        ..Intent::default()
    };
    // The only refusal is the missing pool holder, keyed by the DRAWING tenant `t`
    // via the typed referrer — never a construct string that a port could match.
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::TenantAbsent {
            referrer: Referrer::Pool("t".into()),
            tenant: "ghost".into(),
        }])
    );
}

// ---- 2026-09-08-vocabulary-v2 D4: the three programmatic-parity refusals ----

/// Spec scenario "A programmatic self-loop link is refused" (2026-09-08-vocabulary-v2 D4): an
/// `Intent` built in Rust (never parsed) with a link whose two ends name the same
/// tenant is refused `LinkSelfLoop`, and the refused compile hands out no plan.
#[test]
fn programmatic_self_loop_link_is_refused() {
    let intent = Intent {
        tenants: vec![kernel_tenant(16)],
        links: vec![link("wire", "kernel", "kernel")],
        ..Intent::default()
    };
    // `compile` returns the refusal set (Err), so derivation's plan is never
    // handed out; the self-loop is named.
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::LinkSelfLoop {
            link: "wire".into()
        }])
    );
}

/// Spec scenario "A programmatic kernel declaration is refused" (2026-09-08-vocabulary-v2 D4):
/// an `Intent` built in Rust declaring a tenant named `kernel` of a NON-reserved
/// shape is refused `KernelDeclared`. The reserved `kernel_tenant` is exempt — it
/// is materialised, not declared — so the many intents carrying it still compile.
#[test]
fn programmatic_kernel_declaration_is_refused() {
    let intent = Intent {
        tenants: vec![tenant(
            "kernel",
            Dataplane::UserspacePoll,
            8,
            Isolation::Isolated,
        )],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::KernelDeclared])
    );
}

/// Spec scenario "A programmatic rename double-claim is refused" (2026-09-08-vocabulary-v2 D4):
/// an `Intent` built in Rust whose port `e0` renames from a currently-declared,
/// not-itself-renamed port `wan0` is refused `RenameDoubleClaim` naming both.
#[test]
fn programmatic_rename_double_claim_is_refused() {
    let mk = |name: &str, dpmac: u32, from: Option<&str>| Port {
        name: name.into(),
        dpmac: DpmacId::new(dpmac),
        rate: 10_000,
        tenant: TenantRef::from_name("router".into()),
        mac: None,
        mac_mode: crate::core::model::MacMode::Assert,
        renamed: from.map(Into::into),
    };
    let intent = Intent {
        tenants: vec![poll("router")],
        ports: vec![mk("e0", 7, Some("wan0")), mk("wan0", 8, None)],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::RenameDoubleClaim {
            construct: "e0".into(),
            from: "wan0".into(),
        }])
    );
}

#[test]
fn refuse_pool_dataplane_mismatch() {
    let intent = Intent {
        tenants: vec![tenant(
            "t",
            Dataplane::UserspacePoll,
            16,
            restricted("kernel"),
        )],
        ..Intent::default()
    };
    assert_eq!(
        err(&intent, &ref_inv()),
        BTreeSet::from([Refusal::PoolDataplaneMismatch {
            tenant: "t".into(),
            drawer: Dataplane::UserspacePoll,
            holder: Dataplane::KernelNetlink,
        }])
    );
}

// ======================================================================
// one positive test per companion rule (design D3/D4/D6a; 2026-09-06-intent-layer)
// ======================================================================

/// The reference board intent: poll-mode per-thread draws and dpcon per polled
/// queue (ADR-0012; `dpcon.md` DPCON-I1; reference.qnt `referencePlanShapeTest`).
fn reference_intent() -> Intent {
    Intent {
        tenants: vec![kernel_tenant(16), poll("router")],
        ports: vec![
            port("wan0", 7, 10_000, "router"),
            port("wan1", 9, 10_000, "router"),
        ],
        ..Intent::default()
    }
}

#[test]
fn pollmode_per_thread_draws_and_dpcon_per_polled_queue() {
    let c = ok(&reference_intent(), &ref_inv());
    // T = 1 + 2 + 2 = 5, visibly unmeasured.
    let t = provenance(&c, "router", "T", "");
    assert_eq!(t.value, 5);
    assert_eq!(t.mark, crate::intent::compiled::Measurement::Unmeasured);
    assert_eq!(count_fam(&c, "router", Family::Dpni), 2);
    assert_eq!(
        attributes_of(&c, "router", Family::Dpni, 1),
        Attributes::Dpni {
            cfg: crate::intent::derive::dpni_cfg(Dataplane::UserspacePoll, 5)
        }
    );
    assert_eq!(count_fam(&c, "router", Family::Dpio), 10); // 2·T
    assert_eq!(count_fam(&c, "router", Family::Dpbp), 2);
    assert_eq!(count_fam(&c, "router", Family::Dpmcp), 1);
    assert_eq!(count_fam(&c, "router", Family::Dpcon), 10); // dpnis·T
}

#[test]
fn dpni_option_profile_follows_the_binding_consumer() {
    // dpni-typestate design D3: two tenants differing only in dataplane, same construct
    // (a physical port) — poll consumer's dpni gets the PMD mask, kernel-netlink the kernel.
    use crate::families::dpni::Profile;
    let intent = Intent {
        tenants: vec![kernel_tenant(16), poll("app")],
        ports: vec![
            port("k7", 7, 10_000, "kernel"),
            port("a9", 9, 10_000, "app"),
        ],
        ..Intent::default()
    };
    let c = ok(&intent, &ref_inv());
    let Attributes::Dpni { cfg: kernel_cfg } = attributes_of(&c, "kernel", Family::Dpni, 1) else {
        panic!("kernel dpni")
    };
    assert_eq!(kernel_cfg.options, Profile::Kernel.mask());
    let Attributes::Dpni { cfg: app_cfg } = attributes_of(&c, "app", Family::Dpni, 1) else {
        panic!("app dpni")
    };
    assert_eq!(app_cfg.options, Profile::Pmd.mask());
}

#[test]
fn dpni_cfg_overlays_num_queues_and_custom_cg() {
    // dpni-typestate design D3 / `dpni.qnt` `dpniCfg`: a poll consumer at 5 queues overlays
    // num_queues=5, num_cgs=5+8=13 under CUSTOM_CG at PMD's 16 TCs; a kernel one takes the count at 1 TC, default num_cgs.
    use crate::families::dpni::DpniOpt;
    let pmd = crate::intent::derive::dpni_cfg(Dataplane::UserspacePoll, 5);
    assert_eq!(pmd.num_queues.get(), 5);
    assert_eq!(pmd.num_cgs.get(), 13);
    assert_eq!(pmd.num_tcs.get(), 16);
    assert!(pmd.options.contains(DpniOpt::CustomCg));

    let knl = crate::intent::derive::dpni_cfg(Dataplane::KernelNetlink, 16);
    assert_eq!(knl.num_queues.get(), 16);
    assert_eq!(knl.num_tcs.get(), 1);
    assert_eq!(knl.num_cgs.get(), 0);
}

#[test]
fn dpni_options_provenance_node_carries_the_profile_anchor() {
    // dpni-typestate design D3: the dry-run plan carries a dpni-options node whose value
    // is the flag + escape count of the derived mask and whose anchor names the profile.
    let c = ok(&reference_intent(), &ref_inv());
    let opts = provenance(&c, "router", "dpni-options", "");
    assert_eq!(opts.value, 6); // 5 PMD flags + PfdrInPeb
    assert!(opts.anchor.contains("PMD profile"), "{}", opts.anchor);
    let kopts = provenance(&c, "kernel", "dpni-options", "");
    assert_eq!(kopts.value, 1); // HAS_KEY_MASKING only
    assert!(kopts.anchor.contains("kernel profile"), "{}", kopts.anchor);
}

#[test]
fn undeclared_reserved_kernel_full_percpu_draw_in_root() {
    // A link names the kernel without declaring it: it is materialised in Root
    // with the full per-CPU dpio draw (design D6a).
    let intent = Intent {
        tenants: vec![poll("app")],
        links: vec![link("up", "app", "kernel")],
        ..Intent::default()
    };
    let c = ok(&intent, &ref_inv());
    assert_eq!(count_fam(&c, "kernel", Family::Dpio), 16); // one per online CPU
    assert_eq!(container_of(&c, "kernel", Family::Dpio, 1), Container::Root);
    // the reserved kernel lives in Root itself — it derives no child dprc.
    assert_eq!(count_fam(&c, "kernel", Family::Dprc), 0);
    assert_eq!(provenance(&c, "kernel", "cpus", "").value, 16);
}

#[test]
fn kernel_netlink_namespace_child_resident_draws_dpio_zero() {
    // A declared kernel-netlink namespace with a wire to the kernel: child-
    // resident, so zero extra dpio, but its dpni still runs `cpus` queues and
    // prices `cpus` dpcons (design D6a; the vwire scenario).
    let intent = Intent {
        tenants: vec![knl("ns")],
        links: vec![link("veth", "ns", "kernel")],
        ..Intent::default()
    };
    let c = ok(&intent, &ref_inv());
    assert_eq!(count_fam(&c, "ns", Family::Dpio), 0); // child-resident
    assert_eq!(count_fam(&c, "ns", Family::Dpni), 1);
    assert_eq!(
        attributes_of(&c, "ns", Family::Dpni, 1),
        // cpus transmit queues, kernel profile
        Attributes::Dpni {
            cfg: crate::intent::derive::dpni_cfg(Dataplane::KernelNetlink, 16)
        }
    );
    assert_eq!(count_fam(&c, "ns", Family::Dpcon), 16); // dpnis·cpus
    // a namespace is isolated: its own kernel-bound child dprc.
    assert_eq!(count_fam(&c, "ns", Family::Dprc), 1);
    assert_eq!(
        container_of(&c, "ns", Family::Dpni, 1),
        Container::Child("ns".into())
    );
}

#[test]
fn dpseci_per_crypto_block_sized_by_its_own_flows() {
    let intent = Intent {
        tenants: vec![knl("sec")],
        crypto: vec![
            Crypto {
                tenant: "sec".into(),
                flows: 4,
            },
            Crypto {
                tenant: "sec".into(),
                flows: 8,
            },
        ],
        ..Intent::default()
    };
    let c = ok(&intent, &ref_inv());
    assert_eq!(count_fam(&c, "sec", Family::Dpseci), 2);
    assert_eq!(
        attributes_of(&c, "sec", Family::Dpseci, 1),
        Attributes::Dpseci {
            num_queues: 4,
            has_cg: true,
        }
    );
    assert_eq!(
        attributes_of(&c, "sec", Family::Dpseci, 2),
        Attributes::Dpseci {
            num_queues: 8,
            has_cg: true,
        }
    );
    // one dpseci provenance node per tenant; its value is the accelerator count.
    assert_eq!(provenance(&c, "sec", "dpseci", "").value, 2);
}

#[test]
fn dpsw_hardware_fabric() {
    let intent = Intent {
        tenants: vec![kernel_tenant(16)],
        ports: vec![
            port("p7", 7, 10_000, "kernel"),
            port("p8", 8, 10_000, "kernel"),
        ],
        fabrics: vec![Fabric {
            name: "br".into(),
            switching: Switching::Hardware,
            forwarded_by: "kernel".into(),
            members: vec![Member::Port("p7".into()), Member::Port("p8".into())],
            renamed: None,
        }],
        ..Intent::default()
    };
    let c = ok(&intent, &ref_inv());
    assert_eq!(count_fam(&c, "kernel", Family::Dpsw), 1);
    assert_eq!(
        attributes_of(&c, "kernel", Family::Dpsw, 1),
        Attributes::Dpsw {
            num_ifs: 2,
            max_fdbs: 2,
            per_fdb_flooding: true,
            per_fdb_broadcast: true,
            ctrl_if: true,
        }
    );
    assert_eq!(provenance(&c, "kernel", "dpsw", "br").value, 2);
    // the two member ports are dpsw interfaces (fabric-edges), not port-edges.
    assert_eq!(c.plan.edges.len(), 2);
}

#[test]
fn extras_only_raise_a_count() {
    let mut extras = BTreeSet::new();
    extras.insert(Extra {
        tenant: "router".into(),
        family: Family::Dpio,
        count: 3,
    });
    let intent = Intent {
        tenants: vec![kernel_tenant(16), poll("router")],
        ports: vec![port("wan0", 7, 10_000, "router")],
        extras,
        ..Intent::default()
    };
    let c = ok(&intent, &ref_inv());
    // request = 2·T = 2·3 = 6; extra 3 raises it to 9 (raise-only).
    let node = provenance(&c, "router", "dpio", "");
    assert_eq!(node.request, 6);
    assert_eq!(node.extra, Some(3));
    assert_eq!(node.value, 9);
    assert_eq!(count_fam(&c, "router", Family::Dpio), 9);
}

#[test]
fn restricted_tenant_pools_into_its_holders_container() {
    // A DPDK secondary (restricted) pools a userspace-poll primary (public): it
    // derives no dprc of its own and its objects sit in the holder's container,
    // but it keeps its own dpmcp draw (design D6a; the reference 3-vs-1 dpmcp).
    let intent = Intent {
        tenants: vec![
            tenant("prim", Dataplane::UserspacePoll, 16, Isolation::Public),
            tenant("sec", Dataplane::UserspacePoll, 16, restricted("prim")),
        ],
        ..Intent::default()
    };
    let c = ok(&intent, &ref_inv());
    assert_eq!(count_fam(&c, "sec", Family::Dprc), 0); // no dprc of its own
    assert_eq!(count_fam(&c, "prim", Family::Dprc), 1);
    assert_eq!(count_fam(&c, "sec", Family::Dpmcp), 1); // its own portal draw
    assert_eq!(
        container_of(&c, "sec", Family::Dpmcp, 1),
        Container::Child("prim".into())
    );
}

// ---- the DesiredTopology seam (design D10/D11; 2026-09-06-intent-layer) ----

#[test]
fn compile_pairs_a_coherent_desired_topology() {
    let intent = reference_intent();
    let c = ok(&intent, &ref_inv());
    let topology = c.desired_topology(&intent);
    assert_eq!(topology.ports().len(), 2);
    // the two terminated ports are the whole port-edge set.
    assert_eq!(topology.plan().edges.len(), 2);
}

#[test]
fn desired_topology_keeps_the_operators_mac_intent() {
    // The port's MAC and mode are actuation-only facts the derivation never
    // reads, but the projection must carry them (design D9, 2026-09-06-intent-layer).
    let mac = crate::core::model::MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);
    let intent = Intent {
        tenants: vec![kernel_tenant(16)],
        ports: vec![Port {
            mac: Some(mac),
            mac_mode: crate::core::model::MacMode::Actuate,
            ..port("wan0", 7, 10_000, "kernel")
        }],
        ..Intent::default()
    };
    let c = ok(&intent, &ref_inv());
    let topology = c.desired_topology(&intent);
    let projected = &topology.ports()[0];
    assert_eq!(projected.mac, Some(mac));
    assert_eq!(projected.mac_mode, crate::core::model::MacMode::Actuate);
}

#[test]
fn compile_is_deterministic() {
    let intent = reference_intent();
    let inv = ref_inv();
    assert_eq!(compile(&intent, &inv), compile(&intent, &inv));
}

#[test]
fn warnings_flag_unmeasured_cross_class_mix() {
    // A userspace-poll tenant terminating a 10G and a 25G port: the formula prices
    // the mix but flags it unmeasured (design D3, 2026-09-06-intent-layer).
    let intent = Intent {
        tenants: vec![poll("router")],
        ports: vec![
            port("wan0", 7, 10_000, "router"),
            port("wan1", 4, 25_000, "router"),
        ],
        ..Intent::default()
    };
    let c = ok(&intent, &ref_inv());
    assert!(c.warnings.contains(&Warning::UnmeasuredCombination {
        tenant: "router".into(),
        rates: vec![10_000, 25_000],
    }));
}
