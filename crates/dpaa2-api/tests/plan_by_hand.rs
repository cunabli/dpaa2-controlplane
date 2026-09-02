//! A plan built by hand through the public witness constructors, then reconciled —
//! no [`dpaa2_api::intent::Intent`] and no TOML in sight (design D11).
//!
//! This proves two things the compiler (task 3.2) will lean on: the plan's
//! witness-taking constructors are public and lock the relationships (companions
//! only via a tenant, typed link ends, tenant objects in their own child DPRC), and
//! a [`DesiredTopology`] assembled from a hand-built plan reconciles exactly as a
//! port-only one does.

use dpaa2_api::compiled::{AttachPoint, Container};
use dpaa2_api::{
    CompiledPlan, Dataplane, DesiredPort, DesiredTopology, DpmacId, Family, Isolation, Link,
    LinkType, MacAddr, ObservedDpmac, ObservedTopology, Tenant, Transition, kernel_tenant,
    reconcile,
};

const MAC_7: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);

#[test]
fn hand_built_plan_reconciles_and_locks_relationships() {
    let kernel = kernel_tenant(4);

    // A companion is drawn only through a tenant witness, and the port witness
    // yields the dpni and its dpni<->dpmac edge.
    let dpio = kernel.companion(Family::Dpio, 1);
    let (dpni, iface) = kernel.dpni(1, 4);
    let port_edge = iface.into_port_edge(DpmacId::new(7));

    let mut plan = CompiledPlan::default();
    plan.order.push(dpni.key().clone());
    plan.objects.insert(dpni);
    plan.objects.insert(dpio);
    plan.edges.insert(port_edge);

    // A non-kernel tenant's companion lands in its own child DPRC, never in root.
    let vpp = Tenant {
        name: "vpp".into(),
        dataplane: Dataplane::UserspacePoll,
        max_cores: 8,
        isolation: Isolation::Isolated,
        pool: "".into(),
    };
    assert_eq!(
        vpp.companion(Family::Dpbp, 1).container(),
        &Container::Child("vpp".into())
    );

    // A link between two tenant interfaces is dpni<->dpni: both ends are objects.
    let (_va, va_if) = vpp.dpni(1, 0);
    let (_kb, kb_if) = kernel.dpni(2, 4);
    let wire = Link::wire(va_if, kb_if);
    assert!(
        matches!(
            (wire.a(), wire.b()),
            (AttachPoint::Object { .. }, AttachPoint::Object { .. })
        ),
        "a link end is a dpni, never a dpmac"
    );

    // The hand-built plan reconciles like any port-only one: create, connect, bind.
    let desired =
        DesiredTopology::from_parts(plan, vec![DesiredPort::new(DpmacId::new(7), "lan0")])
            .expect("plan port-edge and port agree on dpmac.7");
    assert_eq!(desired.plan().objects.len(), 2, "dpni + dpio");
    assert_eq!(desired.plan().edges.len(), 1, "the port-edge");

    let observed = ObservedTopology {
        dpnis: vec![],
        dpmacs: vec![ObservedDpmac {
            id: DpmacId::new(7),
            link_type: LinkType::Phy,
            mac: Some(MAC_7),
        }],
    };
    let out = reconcile(&desired, &observed);
    assert_eq!(
        out.transitions,
        vec![
            Transition::Create {
                port: DpmacId::new(7)
            },
            Transition::Connect {
                port: DpmacId::new(7)
            },
            Transition::Bind {
                port: DpmacId::new(7)
            },
        ]
    );
}
