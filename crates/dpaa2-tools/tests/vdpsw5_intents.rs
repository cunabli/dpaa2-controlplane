//! Pins the board-suite V-DPSW-5 operands offline (dpsw-typestate task 6.2, bead
//! dpaa2-controlplane-dbf.16): the two committed intent files the witness sitting runs
//! through `dpaa2ctl` must derive the switched-domain shape the end-to-end witness expects,
//! so a drift in either operand fails here instead of on the board mid-sitting. No board is
//! touched — each operand is parsed by the shipped `dpaa2-config` parser, completed exactly
//! as `compile_intent` completes it (the reserved-kernel injection via the dpaa2-tools lib
//! `dpaa2_tools::complete_kernel`), and compiled against the reference inventory.
//!
//! The suite converges ONE switched-domain intent (`intent-a.toml`; system-integration
//! delta "The switched domain is witnessed end to end on the board", dpsw-typestate design D9): the `edge`
//! kernel-netlink tenant's dpni joins a kernel-forwarded dpsw whose uplink is the wired
//! dpmac.7. The member is kernel-owned so the board witness gets an enabled, countable port —
//! a userspace-poll member leaves its switch port link-down with no running consumer and the
//! broadcast oracle reads 0 (bead dpaa2-controlplane-dbf.19 / V-DPSW-5 rev 1 finding). The pin
//! asserts the member derives the kernel option profile, lands in the edge tenant's own child
//! dprc, and that the create block is bindable by construction (DPSW-I1: ctrl-if
//! on, flooding+broadcast `PER_FDB`, `max_fdbs` == `num_ifs`), that `mac_table_size` sizes
//! `max_fdb_entries` as the product with `num_ifs` (256 × 2 = 512, dpsw-typestate design D3), that the dpsw
//! draws no dpcon (the DPSW-I3 companion law's no-dpcon half; the exact 1 dpmcp + 1 dpbp +
//! 0 dpio count is the board observation), and that the two fabric-edges wire the uplink
//! dpmac.7 and the member dpni.
//!
//! The interposition leg (`intent-flat.toml`; scenario "An existing port interposes into
//! the domain", dpsw-typestate design D10) stands the SAME `edge` tenant's dpni flat on dpmac.7 first — a
//! dpni↔dpmac.7 port-edge the switched convergence then severs through the typed
//! severed-witness path. The pin asserts the flat operand is exactly that port-edge; the
//! sever itself is a reconcile-time transition the board witnesses, not a compile edge.

use dpaa2_api::core::family::Family;
use dpaa2_api::core::model::DpmacId;
use dpaa2_api::core::types::TenantName;
use dpaa2_api::families::dpni::Profile;
use dpaa2_api::families::pool_lifecycle::{PoolFamily, derived_requirement};
use dpaa2_api::intent::Intent;
use dpaa2_api::intent::compiled::{AttachPoint, Attributes, Container};
use dpaa2_api::intent::refuse::compile;
use dpaa2_api::testkit::ref_inventory;

const CPUS: u32 = 16;

/// Reads, completes and compiles a committed operand from the suite dir — the
/// `compile_intent` pipeline (crates/dpaa2-tools/src/main.rs): parse, inject the reserved
/// kernel when a port names it and none is declared (`dpaa2_tools::complete_kernel`;
/// declaring `[tenant.kernel]` is refused, ADR-0013), then compile.
fn compile_operand(file: &str) -> dpaa2_api::intent::refuse::Compiled {
    let toml = std::fs::read_to_string(format!(
        "{}/../../models/board/V-DPSW-5/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("read {file}: {e}"));
    let mut intent: Intent =
        dpaa2_config::parse_str(&toml).unwrap_or_else(|e| panic!("{file}: parse: {e}"));
    dpaa2_tools::complete_kernel(&mut intent, CPUS);
    compile(&intent, &ref_inventory(CPUS)).unwrap_or_else(|e| panic!("{file}: compile: {e:?}"))
}

#[test]
fn switched_domain_derives_a_bindable_dpsw_with_two_fabric_edges() {
    let c = compile_operand("intent-a.toml");

    // One dpsw in the kernel-regime root container — the kernel is its only steering owner (DPSW-I6).
    let dpsws: Vec<_> = c
        .plan
        .objects
        .iter()
        .filter(|o| o.key().family == Family::Dpsw)
        .collect();
    assert_eq!(dpsws.len(), 1, "one dpsw for the one hardware fabric");
    let dpsw = dpsws[0];
    assert_eq!(dpsw.container(), &Container::Root);

    // Bindable by construction (DPSW-I1; dpsw-typestate design D2/D3): ctrl-if on, PER_FDB
    // flooding+broadcast, max_fdbs == num_ifs, max_fdb_entries = mac_table_size × num_ifs (256×2).
    assert_eq!(
        dpsw.attributes(),
        &Attributes::Dpsw {
            num_ifs: 2,
            max_fdbs: 2,
            max_fdb_entries: 512,
            per_fdb_flooding: true,
            per_fdb_broadcast: true,
            ctrl_if: true,
        }
    );

    // DPSW-I3 companion law, no-dpcon half: a kernel-bound dpsw draws 0 dpcon; the exact
    // 1 dpmcp + 1 dpbp + 0 dpio draw in the dpsw's own container is the board observation.
    assert_eq!(
        derived_requirement(&c.plan, &Container::Root, PoolFamily::Dpcon),
        0
    );
    assert!(derived_requirement(&c.plan, &Container::Root, PoolFamily::Dpmcp) >= 1);
    assert!(derived_requirement(&c.plan, &Container::Root, PoolFamily::Dpbp) >= 1);

    let dpsw_edges: Vec<_> = c
        .plan
        .edges
        .iter()
        .filter(|e| {
            matches!(e.a(), AttachPoint::Object { key, .. } if key.family == Family::Dpsw)
                || matches!(e.b(), AttachPoint::Object { key, .. } if key.family == Family::Dpsw)
        })
        .collect();
    assert_eq!(dpsw_edges.len(), 2, "two dpsw interfaces: uplink + member");

    let far = |e: &dpaa2_api::intent::compiled::Edge| {
        if matches!(e.a(), AttachPoint::Object { key, .. } if key.family == Family::Dpsw) {
            e.b().clone()
        } else {
            e.a().clone()
        }
    };
    let uplink = dpsw_edges
        .iter()
        .filter(|e| far(e) == AttachPoint::Mac(DpmacId::new(7)))
        .count();
    assert_eq!(uplink, 1, "the uplink interface faces bare dpmac.7");
    let member = dpsw_edges
        .iter()
        .filter(|e| {
            matches!(far(e), AttachPoint::Object { key, .. }
                if key.family == Family::Dpni && key.tenant.as_str() == "edge")
        })
        .count();
    assert_eq!(
        member, 1,
        "the member interface faces the edge tenant's dpni"
    );

    // The kernel-owned member (kernel-netlink, bead dpaa2-controlplane-dbf.19) lands in the edge
    // tenant's own child dprc and derives the KERNEL profile — a userspace-poll revert fails here.
    let member_dpni = c
        .plan
        .objects
        .iter()
        .find(|o| o.key().family == Family::Dpni && o.key().tenant.as_str() == "edge")
        .expect("the edge tenant's member dpni is planned");
    assert_eq!(
        member_dpni.container(),
        &Container::Child(TenantName::from("edge")),
        "the member dpni lands in the edge tenant's own child dprc"
    );
    let Attributes::Dpni { cfg } = member_dpni.attributes() else {
        panic!("the member is a dpni: {:?}", member_dpni.attributes());
    };
    assert_eq!(
        cfg.options,
        Profile::Kernel.mask(),
        "the kernel-netlink member derives the kernel option profile"
    );
}

#[test]
fn flat_operand_is_the_pre_interposition_dpni_on_dpmac7() {
    let c = compile_operand("intent-flat.toml");

    // No dpsw in the flat topology — the switch does not exist yet.
    assert!(
        !c.plan
            .objects
            .iter()
            .any(|o| o.key().family == Family::Dpsw),
        "the flat operand has no switch"
    );

    // One dpni↔dpmac.7 port-edge — the edge the switched convergence severs (the sever is a
    // reconcile-time transition the board witnesses, not a compile edge; dpsw-typestate design D10).
    let flat: Vec<_> = c
        .plan
        .edges
        .iter()
        .filter_map(|e| e.port_edge_dpni())
        .collect();
    assert_eq!(flat.len(), 1, "one flat dpni↔dpmac port-edge");
    let (dpni, dpmac) = flat[0];
    assert_eq!(
        dpmac,
        DpmacId::new(7),
        "the flat dpni faces the wired dpmac.7"
    );
    assert_eq!(
        dpni.tenant.as_str(),
        "edge",
        "the flat dpni is the edge tenant's"
    );

    // Same tenant, same regime: the flat member is kernel-netlink too (bead dpaa2-controlplane-dbf.19).
    let flat_dpni = c
        .plan
        .objects
        .iter()
        .find(|o| o.key().family == Family::Dpni && o.key().tenant.as_str() == "edge")
        .expect("the flat edge dpni is planned");
    let Attributes::Dpni { cfg } = flat_dpni.attributes() else {
        panic!(
            "the flat edge object is a dpni: {:?}",
            flat_dpni.attributes()
        );
    };
    assert_eq!(
        cfg.options,
        Profile::Kernel.mask(),
        "the flat kernel-netlink dpni derives the kernel option profile"
    );
}
