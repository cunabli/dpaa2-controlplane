//! The pure derivation `derive(intent, inventory) -> CompiledPlan` (design
//! D3/D4/D6; ADR-0013 §4).
//!
//! Transcribed from `models/intent/derive.qnt`: intent plus the observed hardware
//! offer become the complete object plan — every object keyed by
//! `(tenant, family, ordinal)`, every derived count carrying its rule, its source
//! construct, and its evidence anchor as a provenance DAG. The numbers live in the
//! ADRs, the family modules, and `companions.qnt`; they are read here by reference,
//! never restated.
//!
//! The plan is built *through* the witness constructors of [`crate::compiled`]
//! ([`Tenant::companion`], [`Tenant::dpni`], [`Tenant::dpseci`], [`Port::terminate`],
//! [`Link::wire`], [`Fabric::dpsw`], [`Fabric::edge`], [`Fabric::wire`]), so the D6
//! relationship locks hold on the compile path exactly as on a hand-built plan: a
//! companion is drawn only through a tenant, a link end is a dpni never a dpmac, and
//! a non-kernel tenant's object can never land in the root dprc.
//!
//! This module assumes an intent the refusals of [`crate::refuse`] have not rejected
//! (the total function's other half): a claimed dpmac is anchored and single-owner, a
//! hardware fabric is kernel-forwarded, a rate class is seeded. The derivation of a
//! refused intent is *defined* (total, never a panic), not wrong — `refusals` runs
//! `derive` for its feasibility count on any intent.

use std::collections::{BTreeMap, BTreeSet};

use crate::compiled::{
    AttachPoint, CompiledPlan, Measurement, ObjectKey, PlannedObject, ProvenanceKey, ProvenanceNode,
};
use crate::family::Family;
use crate::intent::{
    Crypto, Dataplane, Fabric, Intent, KERNEL, Link, Member, Port, Switching, Tenant, kernel_tenant,
};
use crate::inventory::Inventory;
use crate::types::{ConstructName, TenantName};

// ---- knobs the gate resolved (design open questions) ----

/// Open question 2 (design.md): whether the kernel's dpio budget is the online-CPU
/// count (ADR-0012) or `max_cores`. The gate picks online CPUs, so this is `false`
/// (`derive.qnt` `KERNEL_BUDGET_IS_DPIO_COUNT`).
const KERNEL_BUDGET_IS_DPIO_COUNT: bool = false;

// The per-family census draws a kernel consumer adds beyond its base companion draw
// (`models/families/dpsw.qnt`, `models/families/dpseci.qnt`; ADR-0012 §3): each dpsw
// draws a dpbp for its control interface and a dpmcp; each dpseci draws a dpmcp.
const DPSW_DRAW_DPBP: i64 = 1;
const DPSW_DRAW_DPMCP: i64 = 1;
const DPSECI_DRAW_DPBP: i64 = 0;
const DPSECI_DRAW_DPMCP: i64 = 1;

/// Workers a single port of the given rate class draws (design D3; `derive.qnt`
/// `WORKER_TABLE`/`workersPerPort`): 10G ⇒ 2 (the seed, decomposed from the verified
/// `T = 5 = 1 + 2·2` configuration), 25G ⇒ 5 (declared linear-in-rate, unmeasured,
/// signed off at gate close). A rate class absent here has no worker count, so its
/// tenant's `T` is undefined ([`thread_count`] returns `None`) and [`crate::refuse`]
/// refuses it `UnknownRateClass`.
#[must_use]
pub(crate) fn workers_per_port(rate: i64) -> Option<i64> {
    match rate {
        10_000 => Some(2),
        25_000 => Some(5),
        _ => None,
    }
}

/// `T = 1 main + Σ workers(rate)` over the ports the tenant terminates (design D3, the
/// explicit formula; `derive.qnt` `threadCount`). A portless tenant is main-only ⇒
/// `Some(1)`; a port whose rate class has no worker row poisons the whole sum ⇒
/// `None`, which `refuse.qnt` turns into `UnknownRateClass`.
#[must_use]
pub(crate) fn thread_count(ports: &[&Port]) -> Option<i64> {
    let mut t = 1;
    for p in ports {
        t += workers_per_port(p.rate)?;
    }
    Some(t)
}

fn imin(a: i64, b: i64) -> i64 {
    a.min(b)
}

/// Narrows a derived i64 count to the u32 an ordinal/queue field carries; a negative
/// count (only reachable on a refused intent's undefined derivation) reads as 0.
fn u(n: i64) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

// ---- fabric member helpers (design D6; DPAA2 UM §2.2.2 fig. 6) ----

#[must_use]
pub(crate) fn fabric_by_name<'a>(intent: &'a Intent, name: &ConstructName) -> Option<&'a Fabric> {
    intent.fabrics.iter().find(|f| &f.name == name)
}

#[must_use]
pub(crate) fn port_by_name<'a>(intent: &'a Intent, name: &ConstructName) -> Option<&'a Port> {
    intent.ports.iter().find(|p| &p.name == name)
}

fn member_is_port(m: &Member, pn: &ConstructName) -> bool {
    matches!(m, Member::Port(p) if p == pn)
}

fn member_is_fabric(m: &Member, fname: &ConstructName) -> bool {
    matches!(m, Member::Fabric(h) if h == fname)
}

/// The tenant a member resolves to: a tenant member, or a software fabric member's
/// owner (its bridging). Ports and hardware fabrics resolve to no tenant.
fn member_tenant(intent: &Intent, m: &Member) -> Option<TenantName> {
    match m {
        Member::Tenant(c) => Some(c.clone()),
        Member::Fabric(h) => match fabric_by_name(intent, h) {
            Some(f) if f.switching == Switching::Software => Some(f.forwarded_by.clone()),
            _ => None,
        },
        Member::Port(_) => None,
    }
}

/// A port is hardware-switched (yields no dpni and no port-edge; its dpmac is a dpsw
/// interface instead) iff some hardware fabric names it.
#[must_use]
pub(crate) fn is_hw_switched_port(intent: &Intent, port_name: &ConstructName) -> bool {
    intent.fabrics.iter().any(|f| {
        f.switching == Switching::Hardware && f.members.iter().any(|m| member_is_port(m, port_name))
    })
}

/// The tenant's terminated ports (owned and not switched into a hardware fabric), in
/// declaration order.
#[must_use]
pub(crate) fn terminated_ports<'a>(intent: &'a Intent, name: &TenantName) -> Vec<&'a Port> {
    intent
        .ports
        .iter()
        .filter(|p| &p.tenant == name && !is_hw_switched_port(intent, &p.name))
        .collect()
}

/// The distinct seeded rate classes a tenant's terminated ports span; a set larger
/// than one is a cross-class mix the worker formula prices but flags as unmeasured
/// (design D3; `derive.qnt` `seededRateClasses`).
#[must_use]
pub(crate) fn seeded_rate_classes(intent: &Intent, name: &TenantName) -> BTreeSet<i64> {
    terminated_ports(intent, name)
        .iter()
        .filter(|p| workers_per_port(p.rate).is_some())
        .map(|p| p.rate)
        .collect()
}

/// Whether X is attached to hardware fabric `f` through `f`'s own members: a tenant
/// member X, or a software-fabric member owned by X.
fn owner_attached_via_members(intent: &Intent, f: &Fabric, x: &TenantName) -> bool {
    f.members.iter().any(|m| match m {
        Member::Tenant(c) => c == x,
        Member::Fabric(h) => matches!(
            fabric_by_name(intent, h),
            Some(g) if g.switching == Switching::Software && &g.forwarded_by == x
        ),
        Member::Port(_) => false,
    })
}

/// X attaches to hardware fabric `f` iff a member of `f` resolves to X, or a software
/// fabric X owns lists `f` as a member (a chain of switches).
fn attaches_to_hw_fabric(intent: &Intent, x: &TenantName, f: &Fabric) -> bool {
    let via_software_chain = intent.fabrics.iter().any(|g| {
        g.switching == Switching::Software
            && &g.forwarded_by == x
            && g.members.iter().any(|m| member_is_fabric(m, &f.name))
    });
    owner_attached_via_members(intent, f, x) || via_software_chain
}

fn hw_owned_fabrics<'a>(intent: &'a Intent, name: &TenantName) -> Vec<&'a Fabric> {
    intent
        .fabrics
        .iter()
        .filter(|f| f.switching == Switching::Hardware && &f.forwarded_by == name)
        .collect()
}

fn crypto_blocks_of<'a>(intent: &'a Intent, name: &TenantName) -> Vec<&'a Crypto> {
    intent.crypto.iter().filter(|k| &k.tenant == name).collect()
}

// ---- dpni origins (design D6; object-model.md §2; UM §2.2.2 fig. 6) ----

/// Each dpni source a tenant terminates, in the concatenation order that fixes its
/// ordinal (`derive.qnt` `Origin`/`originList`).
enum Origin {
    Port(Port),
    Link {
        link: Link,
        side: u8,
    },
    Attach(ConstructName),
    WireOwner {
        fabric: ConstructName,
        peer: TenantName,
    },
    WireMember(ConstructName),
}

fn is_port(o: &Origin, pn: &ConstructName) -> bool {
    matches!(o, Origin::Port(p) if &p.name == pn)
}
fn is_link_side(o: &Origin, ln: &ConstructName, sd: u8) -> bool {
    matches!(o, Origin::Link { link, side } if &link.name == ln && *side == sd)
}
fn is_attach(o: &Origin, fname: &ConstructName) -> bool {
    matches!(o, Origin::Attach(n) if n == fname)
}
fn is_wire_owner_of(o: &Origin, g: &ConstructName, peer: &TenantName) -> bool {
    matches!(o, Origin::WireOwner { fabric, peer: p } if fabric == g && p == peer)
}
fn is_wire_member_of(o: &Origin, g: &ConstructName) -> bool {
    matches!(o, Origin::WireMember(n) if n == g)
}

/// The ordered dpni sources of a tenant: terminated ports, link ends,
/// hardware-fabric attachments, then software-fabric wire ends (owner then member).
/// Position + 1 is the dpni ordinal.
fn origin_list(intent: &Intent, name: &TenantName) -> Vec<Origin> {
    let mut out = Vec::new();
    // terminated ports (6a)
    for p in terminated_ports(intent, name) {
        out.push(Origin::Port(p.clone()));
    }
    // link ends (6b); a self-link yields two
    for l in &intent.links {
        if &l.interface_a == name {
            out.push(Origin::Link {
                link: l.clone(),
                side: 0,
            });
        }
        if &l.interface_b == name {
            out.push(Origin::Link {
                link: l.clone(),
                side: 1,
            });
        }
    }
    // one attach per hardware fabric the tenant attaches to (6c)
    for f in &intent.fabrics {
        if f.switching == Switching::Hardware && attaches_to_hw_fabric(intent, name, f) {
            out.push(Origin::Attach(f.name.clone()));
        }
    }
    // owner's end of each software-fabric pseudo-wire (6b)
    for g in &intent.fabrics {
        if g.switching == Switching::Software && &g.forwarded_by == name {
            for m in &g.members {
                if let Some(c) = member_tenant(intent, m)
                    && c != *name
                {
                    out.push(Origin::WireOwner {
                        fabric: g.name.clone(),
                        peer: c,
                    });
                }
            }
        }
    }
    // member's end of each software fabric the tenant is wired into (6b)
    for g in &intent.fabrics {
        if g.switching == Switching::Software
            && &g.forwarded_by != name
            && g.members
                .iter()
                .any(|m| member_tenant(intent, m).as_ref() == Some(name))
        {
            out.push(Origin::WireMember(g.name.clone()));
        }
    }
    out
}

/// 1-based ordinal of the first origin the predicate accepts (0 if none;
/// `derive.qnt` `ordinalWhere`).
fn ordinal_where(list: &[Origin], pred: impl Fn(&Origin) -> bool) -> u32 {
    list.iter()
        .position(pred)
        .map_or(0, |i| u32::try_from(i + 1).unwrap_or(0))
}

fn terminated_port_names(intent: &Intent, name: &TenantName) -> BTreeSet<ConstructName> {
    terminated_ports(intent, name)
        .iter()
        .map(|p| p.name.clone())
        .collect()
}

/// The construct names of a tenant's dpnis: a port name, a link name, or the fabric
/// name for an attachment or either wire end (`derive.qnt` `dpniConstructs`).
fn dpni_constructs(intent: &Intent, name: &TenantName) -> BTreeSet<ConstructName> {
    origin_list(intent, name)
        .iter()
        .map(|o| match o {
            Origin::Port(p) => p.name.clone(),
            Origin::Link { link, .. } => link.name.clone(),
            Origin::Attach(fname) | Origin::WireMember(fname) => fname.clone(),
            Origin::WireOwner { fabric, .. } => fabric.clone(),
        })
        .collect()
}

// ---- hardware-fabric interfaces (dpsw.if numbering, 0-based) ----

/// A dpni endpoint by tenant + attaching fabric: that tenant's attach dpni on the
/// fabric's dpsw (`derive.qnt` `attachPoint`).
fn attach_point(intent: &Intent, tenant: &TenantName, fabric_name: &ConstructName) -> AttachPoint {
    let ordinal = ordinal_where(&origin_list(intent, tenant), |o| is_attach(o, fabric_name));
    AttachPoint::object(ObjectKey::new(tenant, Family::Dpni, ordinal), 0)
}

/// The ordered endpoints of a hardware fabric's dpsw interfaces (`derive.qnt`
/// `hwFabricAttachPoints`): members in list order — a member port's dpmac, a member tenant's
/// or member software-fabric-owner's attach dpni, hardware-in-hardware skipped
/// (refused at [`crate::refuse`]); then one interface per software fabric listing `f`
/// whose owner is not already attached through `f`'s own members.
fn hw_fabric_attach_points(intent: &Intent, f: &Fabric) -> Vec<AttachPoint> {
    let mut ends = Vec::new();
    for m in &f.members {
        match m {
            Member::Port(p) => {
                if let Some(port) = port_by_name(intent, p) {
                    ends.push(AttachPoint::mac(port.dpmac));
                }
            }
            Member::Tenant(c) => ends.push(attach_point(intent, c, &f.name)),
            Member::Fabric(h) => {
                if let Some(g) = fabric_by_name(intent, h)
                    && g.switching == Switching::Software
                {
                    ends.push(attach_point(intent, &g.forwarded_by, &f.name));
                }
            }
        }
    }
    for g in &intent.fabrics {
        if g.switching == Switching::Software
            && g.members.iter().any(|m| member_is_fabric(m, &f.name))
            && !owner_attached_via_members(intent, f, &g.forwarded_by)
        {
            ends.push(attach_point(intent, &g.forwarded_by, &f.name));
        }
    }
    ends
}

fn dpsw_ordinal_of(intent: &Intent, f: &Fabric) -> u32 {
    hw_owned_fabrics(intent, &f.forwarded_by)
        .iter()
        .position(|g| g.name == f.name)
        .map_or(0, |i| u32::try_from(i + 1).unwrap_or(0))
}

// ---- per-tenant sizing (design D3/D4/D5) ----

/// A derived count and the additive extra that raised it (`derive.qnt` `EffectiveDemand`).
#[derive(Clone)]
struct EffectiveDemand {
    request: i64,
    extra: Option<i64>,
    value: i64,
}

/// The extra count declared for `(tenant, family)`, 0 when none; several extras on
/// one pair sum (`derive.qnt` `extraOf`).
fn extra_of(intent: &Intent, name: &TenantName, fam: Family) -> i64 {
    intent
        .extras
        .iter()
        .filter(|e| &e.tenant == name && e.family == fam)
        .map(|e| e.count)
        .sum()
}

/// The request/extra idiom (design D5; `derive.qnt` `effective`): a matching
/// per-`(tenant, family)` extra adds its count to the request; else the request
/// stands.
fn effective(intent: &Intent, name: &TenantName, fam: Family, request: i64) -> EffectiveDemand {
    let ev = extra_of(intent, name, fam);
    if ev == 0 {
        EffectiveDemand {
            request,
            extra: None,
            value: request,
        }
    } else {
        EffectiveDemand {
            request,
            extra: Some(ev),
            value: request + ev,
        }
    }
}

/// ADR-0012 prices only `KernelNetlink` and `UserspacePoll`; a `UserspaceEvent` tenant
/// has no companion draw, so the derivation never sizes it — [`crate::refuse`] refuses it
/// `UnpricedDataplane` (`derive.qnt` `hasPricing`).
#[must_use]
pub(crate) fn has_pricing(c: &Tenant) -> bool {
    c.dataplane != Dataplane::UserspaceEvent
}

fn kernel_cores(inv: &Inventory, c: &Tenant) -> i64 {
    if KERNEL_BUDGET_IS_DPIO_COUNT {
        c.max_cores
    } else {
        i64::from(inv.cpus)
    }
}

/// Everything the object, order, provenance and edge builders read for one tenant
/// (`derive.qnt` `Sizing`).
struct Sizing {
    name: TenantName,
    is_kernel: bool,
    is_root_kernel: bool,
    is_restricted: bool,
    dpnis: i64,
    t: i64,
    cpus: i64,
    num_queues: i64,
    effective_dpio: EffectiveDemand,
    effective_dpbp: EffectiveDemand,
    effective_dpmcp: EffectiveDemand,
    effective_dpcon: EffectiveDemand,
    num_dpseci: i64,
    num_dpsw: i64,
}

/// Companion draws by reference to ADR-0012 (`companions.qnt`). Poll-mode: dpio 2·T,
/// dpbp 2, dpmcp one per process, dpni transmit queues ≥ T. `KernelNetlink`: dpio one
/// per online CPU, dpbp/dpmcp one per consuming object plus one per dpio, each
/// dpsw/dpseci adding its own family census row; dpcon is one per polled queue
/// (`dpcon.md`). A declared kernel-netlink namespace (name ≠ `kernel`) is
/// child-resident: `draw_cpus = 0`, so it draws zero extra dpio (dpio services are one
/// kernel-global per-CPU list every container shares) while its dpnis still run `cpus`
/// transmit queues and price `cpus` dpcons each (design D6a; `derive.qnt` `sizeTenant`).
fn size_tenant(intent: &Intent, inv: &Inventory, c: &Tenant) -> Sizing {
    let nm = c.name.clone();
    let is_kernel = c.dataplane == Dataplane::KernelNetlink;
    let is_root_kernel = c.name.is_kernel();
    let is_restricted = !c.pool.is_empty();
    let dpnis = i64::try_from(origin_list(intent, &nm).len()).unwrap_or(0);
    let t = thread_count(&terminated_ports(intent, &nm)).unwrap_or(0);
    let cpus = kernel_cores(inv, c);
    let draw_cpus = if is_root_kernel { cpus } else { 0 };
    let num_queues = if is_kernel { cpus } else { t };
    let num_dpsw = i64::try_from(hw_owned_fabrics(intent, &nm).len()).unwrap_or(0);
    let num_dpseci = i64::try_from(crypto_blocks_of(intent, &nm).len()).unwrap_or(0);

    // base companion draw (ADR-0012 companionDraw)
    let (base_dpio, base_dpbp, base_dpmcp) = if is_kernel {
        (draw_cpus, dpnis, draw_cpus + dpnis)
    } else {
        (2 * t, 2, 1)
    };
    let req_dpio = base_dpio;
    let req_dpbp = if is_kernel {
        base_dpbp + num_dpsw * DPSW_DRAW_DPBP + num_dpseci * DPSECI_DRAW_DPBP
    } else {
        base_dpbp
    };
    let req_dpmcp = if is_kernel {
        base_dpmcp + num_dpsw * DPSW_DRAW_DPMCP + num_dpseci * DPSECI_DRAW_DPMCP
    } else {
        base_dpmcp
    };
    let req_dpcon = if is_kernel {
        dpnis * imin(cpus, num_queues)
    } else {
        dpnis * t
    };

    Sizing {
        name: nm.clone(),
        is_kernel,
        is_root_kernel,
        is_restricted,
        dpnis,
        t,
        cpus,
        num_queues,
        effective_dpio: effective(intent, &nm, Family::Dpio, req_dpio),
        effective_dpbp: effective(intent, &nm, Family::Dpbp, req_dpbp),
        effective_dpmcp: effective(intent, &nm, Family::Dpmcp, req_dpmcp),
        effective_dpcon: effective(intent, &nm, Family::Dpcon, req_dpcon),
        num_dpseci,
        num_dpsw,
    }
}

// ---- effective tenant list (design D6a: the materialised reserved kernel) ----

fn kernel_declared(intent: &Intent) -> bool {
    intent.tenants.iter().any(|c| c.name.is_kernel())
}

fn link_names_kernel(intent: &Intent) -> bool {
    intent
        .links
        .iter()
        .any(|l| l.interface_a.is_kernel() || l.interface_b.is_kernel())
}

/// The tenants the derivation sizes: the declared ones, plus the reserved kernel
/// materialised when a link names it but the intent never declared it (design D6a;
/// `derive.qnt` `effectiveTenants`).
fn effective_tenants(intent: &Intent, inv: &Inventory) -> Vec<Tenant> {
    let mut v = intent.tenants.clone();
    if link_names_kernel(intent) && !kernel_declared(intent) {
        v.push(kernel_tenant(i64::from(inv.cpus)));
    }
    v
}

fn tenant_by_name<'a>(tenants: &'a [Tenant], name: &TenantName) -> Option<&'a Tenant> {
    tenants.iter().find(|t| &t.name == name)
}

// ---- objects (through the witness constructors) ----

fn emit_companions(t: &Tenant, fam: Family, value: i64, objects: &mut BTreeSet<PlannedObject>) {
    if value >= 1 {
        for ord in 1..=value {
            objects.insert(t.companion(fam, u(ord)));
        }
    }
}

fn build_tenant_objects(
    t: &Tenant,
    s: &Sizing,
    intent: &Intent,
    objects: &mut BTreeSet<PlannedObject>,
) {
    // A child DPRC for every tenant that owns one (design D6): an isolated tenant or
    // a public holder; the reserved kernel and a restricted drawer own none.
    if !(s.is_root_kernel || s.is_restricted) {
        objects.insert(t.child_dprc());
    }
    // Emission-order families: dpio before dpmcp (ADR-0012), then dpbp, dpcon.
    emit_companions(t, Family::Dpio, s.effective_dpio.value, objects);
    emit_companions(t, Family::Dpmcp, s.effective_dpmcp.value, objects);
    emit_companions(t, Family::Dpbp, s.effective_dpbp.value, objects);
    emit_companions(t, Family::Dpcon, s.effective_dpcon.value, objects);
    // one dpni per origin, all sharing the tenant's queue count
    if s.dpnis >= 1 {
        for ord in 1..=s.dpnis {
            let (obj, _iface) = t.dpni(u(ord), u(s.num_queues));
            objects.insert(obj);
        }
    }
    // one dpseci per crypto block, sized by that block's own flows
    for (i, k) in crypto_blocks_of(intent, &t.name).iter().enumerate() {
        objects.insert(t.dpseci(u(i64::try_from(i + 1).unwrap_or(0)), u(k.flows)));
    }
    // one dpsw per owned hardware fabric, sized by its interface count
    for (i, f) in hw_owned_fabrics(intent, &t.name).iter().enumerate() {
        let num_ifs = i64::try_from(hw_fabric_attach_points(intent, f).len()).unwrap_or(0);
        objects.insert(f.dpsw(u(i64::try_from(i + 1).unwrap_or(0)), u(num_ifs)));
    }
}

/// dprtc.0, pinned in Root and kernel-owned (`derive.qnt` `dprtcObj`; DPRTC-I1/I2).
/// Built through the kernel's companion witness so it, too, is not a bare literal.
fn dprtc_obj() -> PlannedObject {
    kernel_tenant(0).companion(Family::Dprtc, 1)
}

// ---- emission order (object-model.md §5 step 1) ----

fn push_keys(t: &Tenant, fam: Family, n: i64, order: &mut Vec<ObjectKey>) {
    if n >= 1 {
        for i in 1..=n {
            order.push(ObjectKey::new(t.name.clone(), fam, u(i)));
        }
    }
}

fn tenant_order(t: &Tenant, s: &Sizing, order: &mut Vec<ObjectKey>) {
    if !(s.is_root_kernel || s.is_restricted) {
        order.push(ObjectKey::new(t.name.clone(), Family::Dprc, 1));
    }
    push_keys(t, Family::Dpio, s.effective_dpio.value, order);
    push_keys(t, Family::Dpmcp, s.effective_dpmcp.value, order);
    push_keys(t, Family::Dpbp, s.effective_dpbp.value, order);
    push_keys(t, Family::Dpcon, s.effective_dpcon.value, order);
    push_keys(t, Family::Dpni, s.dpnis, order);
    push_keys(t, Family::Dpseci, s.num_dpseci, order);
    push_keys(t, Family::Dpsw, s.num_dpsw, order);
}

// ---- provenance nodes (design D6: value points at what it consumed) ----

fn provkeys(keys: &[(&str, &str, &str)]) -> BTreeSet<ProvenanceKey> {
    keys.iter()
        .map(|(t, r, c)| ProvenanceKey::new(*t, *r, *c))
        .collect()
}

fn dpio_node(s: &Sizing) -> ProvenanceNode {
    let feeder = if s.is_kernel { "cpus" } else { "T" };
    ProvenanceNode {
        rule: "dpio".into(),
        anchor: "ADR-0012 (companions.qnt companionDraw)".to_owned(),
        mark: Measurement::Measured,
        request: s.effective_dpio.request,
        extra: s.effective_dpio.extra,
        value: s.effective_dpio.value,
        inputs: provkeys(&[(s.name.as_str(), feeder, "")]),
        constructs: BTreeSet::new(),
    }
}

fn dpbp_node(s: &Sizing) -> ProvenanceNode {
    ProvenanceNode {
        rule: "dpbp".into(),
        anchor: if s.is_kernel {
            "ADR-0012 (companions.qnt companionDraw); families/dpsw.qnt + dpseci.qnt draw"
                .to_owned()
        } else {
            "ADR-0012 (companions.qnt companionDraw)".to_owned()
        },
        mark: Measurement::Measured,
        request: s.effective_dpbp.request,
        extra: s.effective_dpbp.extra,
        value: s.effective_dpbp.value,
        inputs: if s.is_kernel {
            provkeys(&[(s.name.as_str(), "dpnis", "")])
        } else {
            BTreeSet::new()
        },
        constructs: BTreeSet::new(),
    }
}

fn dpmcp_node(s: &Sizing) -> ProvenanceNode {
    ProvenanceNode {
        rule: "dpmcp".into(),
        anchor: if s.is_kernel {
            "ADR-0012 (companions.qnt companionDraw); families/dpsw.qnt + dpseci.qnt draw"
                .to_owned()
        } else {
            "companions.qnt: one MC portal per process".to_owned()
        },
        mark: Measurement::Measured,
        request: s.effective_dpmcp.request,
        extra: s.effective_dpmcp.extra,
        value: s.effective_dpmcp.value,
        inputs: if s.is_kernel {
            provkeys(&[
                (s.name.as_str(), "cpus", ""),
                (s.name.as_str(), "dpnis", ""),
            ])
        } else {
            BTreeSet::new()
        },
        constructs: BTreeSet::new(),
    }
}

fn dpni_queues_node(s: &Sizing) -> ProvenanceNode {
    let feeder = if s.is_kernel { "cpus" } else { "T" };
    ProvenanceNode {
        rule: "dpni-queues".into(),
        anchor: if s.is_kernel {
            "dpni.md line 64/226-236: ls-addni injects nproc; DPCON-I1".to_owned()
        } else {
            "ADR-0012: transmit queues >= T".to_owned()
        },
        mark: Measurement::Measured,
        request: s.num_queues,
        extra: None,
        value: s.num_queues,
        inputs: provkeys(&[(s.name.as_str(), feeder, "")]),
        constructs: BTreeSet::new(),
    }
}

fn dpcon_node(s: &Sizing) -> ProvenanceNode {
    ProvenanceNode {
        rule: "dpcon".into(),
        anchor: "dpcon.md DPCON-I1: one dpcon per polled queue".to_owned(),
        mark: Measurement::Measured,
        request: s.effective_dpcon.request,
        extra: s.effective_dpcon.extra,
        value: s.effective_dpcon.value,
        inputs: provkeys(&[
            (s.name.as_str(), "dpnis", ""),
            (s.name.as_str(), "dpni-queues", ""),
        ]),
        constructs: BTreeSet::new(),
    }
}

fn dpsw_node(intent: &Intent, f: &Fabric) -> ProvenanceNode {
    let ni = i64::try_from(hw_fabric_attach_points(intent, f).len()).unwrap_or(0);
    ProvenanceNode {
        rule: "dpsw".into(),
        anchor: "dpsw.md kernel bindability predicate (read-not-verified)".to_owned(),
        mark: Measurement::Measured,
        request: ni,
        extra: None,
        value: ni,
        inputs: BTreeSet::new(),
        constructs: BTreeSet::from([f.name.clone()]),
    }
}

fn dprtc_node() -> ProvenanceNode {
    ProvenanceNode {
        rule: "dprtc".into(),
        anchor: "DPRTC-I1 singleton, DPRTC-I2 kernel-owned".to_owned(),
        mark: Measurement::Measured,
        request: 1,
        extra: None,
        value: 1,
        inputs: BTreeSet::new(),
        constructs: BTreeSet::new(),
    }
}

fn link_edge_node(l: &Link) -> ProvenanceNode {
    ProvenanceNode {
        rule: "link-edge".into(),
        anchor: "object-model.md §2, DPNI-I9 pseudo-wire".to_owned(),
        mark: Measurement::Measured,
        request: 0,
        extra: None,
        value: 0,
        inputs: provkeys(&[
            (l.interface_a.as_str(), "dpnis", ""),
            (l.interface_b.as_str(), "dpnis", ""),
        ]),
        constructs: BTreeSet::from([l.name.clone()]),
    }
}

fn fabric_edge_node(f: &Fabric) -> ProvenanceNode {
    ProvenanceNode {
        rule: "fabric-edge".into(),
        anchor: "DPAA2 User Manual §2.2.2 fig. 6c; dpsw.md".to_owned(),
        mark: Measurement::Measured,
        request: 0,
        extra: None,
        value: 0,
        inputs: [ProvenanceKey::new(&f.forwarded_by, "dpsw", &f.name)]
            .into_iter()
            .collect(),
        constructs: BTreeSet::from([f.name.clone()]),
    }
}

fn fabric_wire_node(intent: &Intent, g: &Fabric) -> ProvenanceNode {
    let mut inputs: BTreeSet<ProvenanceKey> = [ProvenanceKey::new(&g.forwarded_by, "dpnis", "")]
        .into_iter()
        .collect();
    for m in &g.members {
        if let Some(c) = member_tenant(intent, m)
            && c != g.forwarded_by
        {
            inputs.insert(ProvenanceKey::new(&c, "dpnis", ""));
        }
    }
    ProvenanceNode {
        rule: "fabric-wire".into(),
        anchor: "DPAA2 User Manual §2.2.2 fig. 6b; object-model.md §2 DPNI-I9".to_owned(),
        mark: Measurement::Measured,
        request: 0,
        extra: None,
        value: 0,
        inputs,
        constructs: BTreeSet::from([g.name.clone()]),
    }
}

// A flat sequence of provenance-node inserts, one per rule; splitting it would only
// scatter the transcription of `derive.qnt` `addTenantProv` across helpers.
#[allow(clippy::too_many_lines)]
fn add_tenant_prov(intent: &Intent, s: &Sizing, m: &mut BTreeMap<ProvenanceKey, ProvenanceNode>) {
    let nm = &s.name;
    let pn = terminated_port_names(intent, nm);
    let np = i64::try_from(terminated_ports(intent, nm).len()).unwrap_or(0);
    m.insert(
        ProvenanceKey::new(nm, "ports", ""),
        ProvenanceNode {
            rule: "ports".into(),
            anchor: "design D1: declared port constructs".to_owned(),
            mark: Measurement::Measured,
            request: np,
            extra: None,
            value: np,
            inputs: BTreeSet::new(),
            constructs: pn.clone(),
        },
    );
    m.insert(
        ProvenanceKey::new(nm, "dpnis", ""),
        ProvenanceNode {
            rule: "dpnis".into(),
            anchor: "object-model.md §2: one dpni per terminated port, link end, fabric membership"
                .to_owned(),
            mark: Measurement::Measured,
            request: s.dpnis,
            extra: None,
            value: s.dpnis,
            inputs: BTreeSet::new(),
            constructs: dpni_constructs(intent, nm),
        },
    );
    m.insert(ProvenanceKey::new(nm, "dpio", ""), dpio_node(s));
    m.insert(ProvenanceKey::new(nm, "dpbp", ""), dpbp_node(s));
    m.insert(ProvenanceKey::new(nm, "dpmcp", ""), dpmcp_node(s));
    m.insert(
        ProvenanceKey::new(nm, "dpni-queues", ""),
        dpni_queues_node(s),
    );
    m.insert(ProvenanceKey::new(nm, "dpcon", ""), dpcon_node(s));
    if s.is_kernel {
        m.insert(
            ProvenanceKey::new(nm, "cpus", ""),
            ProvenanceNode {
                rule: "cpus".into(),
                anchor: "ADR-0012: one dpio per online CPU".to_owned(),
                mark: Measurement::Measured,
                request: s.cpus,
                extra: None,
                value: s.cpus,
                inputs: BTreeSet::new(),
                constructs: BTreeSet::new(),
            },
        );
    } else {
        m.insert(
            ProvenanceKey::new(nm, "T", ""),
            ProvenanceNode {
                rule: "T".into(),
                anchor: "design D3: T = 1 + Σ workers over terminated ports; workers-per-port table (10G⇒2, 25G⇒5), unmeasured".to_owned(),
                mark: Measurement::Unmeasured,
                request: s.t,
                extra: None,
                value: s.t,
                inputs: provkeys(&[(nm.as_str(), "ports", "")]),
                constructs: pn.clone(),
            },
        );
    }
    if !(s.is_root_kernel || s.is_restricted) {
        m.insert(
            ProvenanceKey::new(nm, "dprc", ""),
            ProvenanceNode {
                rule: "dprc".into(),
                anchor: "dprc.md: restool-default child options".to_owned(),
                mark: Measurement::Measured,
                request: 1,
                extra: None,
                value: 1,
                inputs: BTreeSet::new(),
                constructs: BTreeSet::from([ConstructName::from(nm.as_str())]),
            },
        );
    }
    if s.num_dpseci > 0 {
        m.insert(
            ProvenanceKey::new(nm, "dpseci", ""),
            ProvenanceNode {
                rule: "dpseci".into(),
                anchor: "dpseci.md: num_queues >= each block's flows, DPSECI_OPT_HAS_CG".to_owned(),
                mark: Measurement::Measured,
                request: s.num_dpseci,
                extra: None,
                value: s.num_dpseci,
                inputs: BTreeSet::new(),
                constructs: BTreeSet::from([ConstructName::from(nm.as_str())]),
            },
        );
    }
    for f in hw_owned_fabrics(intent, nm) {
        m.insert(
            ProvenanceKey::new(nm, "dpsw", &f.name),
            dpsw_node(intent, f),
        );
    }
    for p in terminated_ports(intent, nm) {
        m.insert(
            ProvenanceKey::new(nm, "port-edge", &p.name),
            ProvenanceNode {
                rule: "port-edge".into(),
                anchor: "object-model.md §2: dpni<->dpmac edge".to_owned(),
                mark: Measurement::Measured,
                request: 0,
                extra: None,
                value: 0,
                inputs: provkeys(&[(nm.as_str(), "dpnis", "")]),
                constructs: BTreeSet::from([p.name.clone()]),
            },
        );
    }
}

fn full_prov(
    intent: &Intent,
    priced: &[&Tenant],
    sizing: &BTreeMap<TenantName, Sizing>,
) -> BTreeMap<ProvenanceKey, ProvenanceNode> {
    let mut m = BTreeMap::new();
    m.insert(ProvenanceKey::new(KERNEL, "dprtc", ""), dprtc_node());
    for t in priced {
        if let Some(s) = sizing.get(&t.name) {
            add_tenant_prov(intent, s, &mut m);
        }
    }
    for l in &intent.links {
        m.insert(
            ProvenanceKey::new(l.interface_a.as_str(), "link-edge", &l.name),
            link_edge_node(l),
        );
    }
    for f in &intent.fabrics {
        if f.switching == Switching::Hardware {
            m.insert(
                ProvenanceKey::new(&f.forwarded_by, "fabric-edge", &f.name),
                fabric_edge_node(f),
            );
        } else {
            m.insert(
                ProvenanceKey::new(&f.forwarded_by, "fabric-wire", &f.name),
                fabric_wire_node(intent, f),
            );
        }
    }
    m
}

// ---- edges (through the witness constructors) ----

fn build_edges(
    intent: &Intent,
    ets: &[Tenant],
    sizing: &BTreeMap<TenantName, Sizing>,
    edges: &mut BTreeSet<crate::compiled::Edge>,
) {
    // Port (6a): the owner's dpni for the port <-> its dpmac.
    for c in &intent.tenants {
        let (Some(s), Some(ct)) = (sizing.get(&c.name), tenant_by_name(ets, &c.name)) else {
            continue;
        };
        for p in terminated_ports(intent, &c.name) {
            let ord = ordinal_where(&origin_list(intent, &c.name), |o| is_port(o, &p.name));
            let (_dpni, edge) = p.terminate(ct, ord, u(s.num_queues));
            edges.insert(edge);
        }
    }
    // Link (6b): the two link-end dpnis; interface_a is the `a` end.
    for l in &intent.links {
        let (Some(sa), Some(ta)) = (
            sizing.get(&l.interface_a),
            tenant_by_name(ets, &l.interface_a),
        ) else {
            continue;
        };
        let (Some(sb), Some(tb)) = (
            sizing.get(&l.interface_b),
            tenant_by_name(ets, &l.interface_b),
        ) else {
            continue;
        };
        let ord_left = ordinal_where(&origin_list(intent, &l.interface_a), |o| {
            is_link_side(o, &l.name, 0)
        });
        let ord_right = ordinal_where(&origin_list(intent, &l.interface_b), |o| {
            is_link_side(o, &l.name, 1)
        });
        let (_a, ia) = ta.dpni(ord_left, u(sa.num_queues));
        let (_b, ib) = tb.dpni(ord_right, u(sb.num_queues));
        edges.insert(l.wire(ia, ib));
    }
    // Hardware fabric (6c): one edge per dpsw interface, 0-based.
    for f in &intent.fabrics {
        if f.switching != Switching::Hardware {
            continue;
        }
        let dpsw_key = ObjectKey::new(
            f.forwarded_by.clone(),
            Family::Dpsw,
            dpsw_ordinal_of(intent, f),
        );
        for (ifx, endpoint) in hw_fabric_attach_points(intent, f).into_iter().enumerate() {
            edges.insert(f.edge(&dpsw_key, u32::try_from(ifx).unwrap_or(0), endpoint));
        }
    }
    // Software fabric (6b): the owner's wire dpni <-> each member tenant's wire dpni.
    for g in &intent.fabrics {
        if g.switching != Switching::Software {
            continue;
        }
        let (Some(so), Some(owner)) = (
            sizing.get(&g.forwarded_by),
            tenant_by_name(ets, &g.forwarded_by),
        ) else {
            continue;
        };
        for m in &g.members {
            let Some(c) = member_tenant(intent, m) else {
                continue;
            };
            if c == g.forwarded_by {
                continue;
            }
            let (Some(sc), Some(ct)) = (sizing.get(&c), tenant_by_name(ets, &c)) else {
                continue;
            };
            let ord_owner = ordinal_where(&origin_list(intent, &g.forwarded_by), |o| {
                is_wire_owner_of(o, &g.name, &c)
            });
            let ord_member =
                ordinal_where(&origin_list(intent, &c), |o| is_wire_member_of(o, &g.name));
            let (_o, owner_if) = owner.dpni(ord_owner, u(so.num_queues));
            let (_mm, member_if) = ct.dpni(ord_member, u(sc.num_queues));
            edges.insert(g.wire(owner_if, member_if));
        }
    }
}

// ---- the derivation ----

/// The pure derivation (design D3/D4/D6; `derive.qnt` `derive`): intent plus the
/// observed offer become the complete [`CompiledPlan`]. Total by construction — safe
/// to run on any intent, since [`crate::refuse`] runs it for its feasibility count
/// before the refusal set is known.
#[must_use]
pub(crate) fn derive(intent: &Intent, inv: &Inventory) -> CompiledPlan {
    let ets = effective_tenants(intent, inv);
    let priced: Vec<&Tenant> = ets.iter().filter(|t| has_pricing(t)).collect();
    let sizing: BTreeMap<TenantName, Sizing> = priced
        .iter()
        .map(|t| (t.name.clone(), size_tenant(intent, inv, t)))
        .collect();

    let mut objects = BTreeSet::new();
    objects.insert(dprtc_obj());
    for t in &priced {
        if let Some(s) = sizing.get(&t.name) {
            build_tenant_objects(t, s, intent, &mut objects);
        }
    }

    let mut order = vec![ObjectKey::new(KERNEL, Family::Dprtc, 1)];
    for t in &priced {
        if let Some(s) = sizing.get(&t.name) {
            tenant_order(t, s, &mut order);
        }
    }

    let mut edges = BTreeSet::new();
    build_edges(intent, &ets, &sizing, &mut edges);

    let provenance = full_prov(intent, &priced, &sizing);

    CompiledPlan {
        objects,
        edges,
        order,
        provenance,
    }
}
