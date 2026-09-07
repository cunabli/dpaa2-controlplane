//! The refusal vocabulary: `compile`'s other half (design D5; ADR-0013 §5).
//!
//! Transcribed from `models/intent/refuse.qnt`. `compile` is total: an intent is
//! either a complete plan with its [`Warning`]s or the *complete* set of
//! [`Refusal`]s — every rule runs unconditionally and their union is returned,
//! never the first violation, so the operator fixes a file in one pass. One variant
//! per rule; the payload names the offending construct and the shortfall.
//!
//! Naming: the model spells the two anchor refusals `ReservedAnchor` /
//! `ForeignAnchor` only because Quint constructor names collide with the
//! [`crate::inventory::Availability`] constructors of the same name (refuse.qnt
//! DEVIATION). Rust enum variants are namespaced by their type, so this transcribes
//! them under the accepted ADR-0013 §5 spelling [`Refusal::Reserved`] /
//! [`Refusal::Foreign`].

use std::collections::{BTreeMap, BTreeSet};

use crate::compiled::CompiledPlan;
use crate::derive::{
    derive, fabric_by_name, has_pricing, is_hw_switched_port, port_by_name, seeded_rate_classes,
    terminated_ports, thread_count,
};
use crate::family::{DERIVED_FAMILIES, Family};
use crate::intent::{
    Dataplane, Fabric, Intent, Member, Switching, Tenant, TenantRef, kernel_tenant,
};
use crate::inventory::{Availability, Ceiling, Inventory};
use crate::model::{DesiredPort, DesiredTopology, DpmacId};
use crate::types::{ConstructName, TenantName};

/// The queue-pair ceiling of one dpseci device (`models/families/dpseci.qnt`
/// `DPSECI_MAX_QUEUE_NUM`; verified `.build/src/linux/drivers/crypto/caam/dpseci.h:25`
/// and `.build/src/dpdk/.../fsl_dpseci.h:25`): a single object carries at most 16
/// Tx/Rx queue pairs, so a crypto block demanding more `flows` than this cannot be
/// realized by one device (`CryptoFlowsOverDevice`).
const DPSECI_MAX_QUEUE_NUM: i64 = 16;

/// The site a [`Refusal::TenantAbsent`] refers *from* (vocabulary-v2 D3, PASS3-F13;
/// `refuse.qnt` `Referrer`, whose model constructors carry a `Ref` prefix to dodge
/// Quint's type/constructor namespace clash — the ITF decoder maps `Ref*` ⇒ these).
///
/// This replaces the old `construct: ConstructName` that smuggled the literal tokens
/// `"crypto"`/`"extra"`/`"pool"` through the construct-name space, where a port an
/// operator legally named `pool` could collide in refusal rendering. As a typed sum
/// the tokens leave that space entirely: a port, link end, or fabric is named by its
/// own [`ConstructName`]; crypto and extra — which carry no name of their own — and
/// the restricted drawer are identified by the referencing [`TenantName`]. Rendering
/// derives the human string from the variant.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Referrer {
    /// A port whose owning tenant is absent (the port's name).
    Port(ConstructName),
    /// A link end naming an absent tenant (the link's name).
    LinkEnd(ConstructName),
    /// A fabric whose forwarder is absent (the fabric's name).
    Fabric(ConstructName),
    /// A crypto block whose tenant is absent (crypto carries no name of its own, so
    /// the referencing tenant identifies it).
    Crypto(TenantName),
    /// An extra whose tenant is absent (likewise identified by the referencing tenant).
    Extra(TenantName),
    /// A restricted drawer whose `pool` holder is absent (the drawing tenant's name).
    Pool(TenantName),
}

/// The rule an intent broke, naming the offending construct (design D5; ADR-0013
/// §5). The `refuse.qnt` refusal vocabulary (vocabulary-v2 D1 deleted the two
/// pool-shape contradictions, now unrepresentable in [`crate::intent::Isolation`];
/// D4 added the three parity twins [`Refusal::LinkSelfLoop`],
/// [`Refusal::RenameDoubleClaim`], [`Refusal::KernelDeclared`]).
///
/// `#[non_exhaustive]`: a `PoolShortfall` variant is reserved for `reconcile`
/// (change #6, drift against a live census) and a passthrough value is change #4's,
/// so callers must not assume the set is closed.
///
/// `Ord` is derived so [`compile`] can return the *complete* refusal set as a
/// deterministic [`std::collections::BTreeSet`] — the model's `Set[Refusal]` (design
/// D5); the ordering is incidental (payload-lexicographic), never semantic.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[non_exhaustive]
pub enum Refusal {
    /// A construct (port, link end, fabric owner, crypto, extra, a restricted
    /// tenant's `pool`) names a tenant not declared (DPDCEI-I1 generalised).
    TenantAbsent {
        /// The site that named the missing tenant (vocabulary-v2 D3): a typed
        /// [`Referrer`], never a construct-name string carrying a reserved token.
        referrer: Referrer,
        /// The undeclared tenant name.
        tenant: TenantName,
    },
    /// A fabric member names a port/tenant/fabric not declared.
    MemberUnresolved {
        /// The fabric carrying the member.
        fabric: ConstructName,
        /// The unresolved member.
        member: Member,
    },
    /// A fabric member resolves to the fabric's own owner.
    SelfMember {
        /// The fabric.
        fabric: ConstructName,
        /// The self-referencing member.
        member: Member,
    },
    /// The port's dpmac is not in the inventory.
    Unanchored {
        /// The port.
        port: ConstructName,
        /// The missing dpmac.
        dpmac: DpmacId,
    },
    /// The port's dpmac is Reserved by the ADR-0003 §3 safety matrix (spelled
    /// `ReservedAnchor` in refuse.qnt; see the module note).
    Reserved {
        /// The port.
        port: ConstructName,
        /// The reserved dpmac.
        dpmac: DpmacId,
        /// The reservation reason.
        why: String,
    },
    /// The port's dpmac is Foreign, owned by a DPL object (spelled `ForeignAnchor`
    /// in refuse.qnt; see the module note).
    Foreign {
        /// The port.
        port: ConstructName,
        /// The foreign dpmac.
        dpmac: DpmacId,
        /// The owner label.
        owner: String,
    },
    /// Two ports on one dpmac, or one port in two fabrics.
    DoubleClaimed {
        /// The doubly-claimed dpmac.
        dpmac: DpmacId,
        /// The constructs claiming it.
        constructs: Vec<ConstructName>,
    },
    /// `rate` exceeds the dpmac's `max_rate`.
    OverRate {
        /// The port.
        port: ConstructName,
        /// The requested rate (Mbps).
        rate: i64,
        /// The dpmac's maximum rate (Mbps).
        max_rate: i64,
    },
    /// A hardware fabric whose `forwarded_by` is not the kernel (`dpsw.md`: only
    /// the kernel drives a dpsw).
    FabricNotKernelForwarded {
        /// The fabric.
        fabric: ConstructName,
        /// The non-kernel forwarder.
        forwarded_by: TenantName,
    },
    /// A member port whose tenant differs from the fabric's forwarder.
    PortTenantMismatch {
        /// The fabric.
        fabric: ConstructName,
        /// The member port.
        port: ConstructName,
        /// The port's (mismatched) tenant.
        tenant: TenantName,
    },
    /// A hardware fabric listing a hardware fabric (unsupported until dpsw↔dpsw is
    /// verified).
    UnsupportedEdge {
        /// The outer fabric.
        fabric: ConstructName,
        /// The inner hardware-fabric member.
        member: ConstructName,
    },
    /// A userspace-poll tenant terminates a rate class with no seeded worker row
    /// (design D3).
    UnknownRateClass {
        /// The tenant.
        tenant: TenantName,
        /// The rate classes it terminates.
        rates: Vec<i64>,
    },
    /// The derived thread count exceeds `max_cores` (design D3; ADR-0012 never
    /// rations).
    CoreBudgetExceeded {
        /// The tenant.
        tenant: TenantName,
        /// The derived thread count.
        t: i64,
        /// The declared budget.
        max_cores: i64,
    },
    /// An extra on a family that is not one of the four companions (design D5).
    ExtraNotCompanion {
        /// The tenant.
        tenant: TenantName,
        /// The non-companion family.
        family: Family,
    },
    /// An extra whose count is below 1 (design D5).
    ExtraNotPositive {
        /// The tenant.
        tenant: TenantName,
        /// The family.
        family: Family,
        /// The non-positive count.
        count: i64,
    },
    /// A crypto block whose flows are below 1 (design D1; `dpseci.md`). The
    /// 1-based ordinal keeps two bad blocks of one tenant distinct (task 2.6e).
    CryptoFlowsNotPositive {
        /// The tenant.
        tenant: TenantName,
        /// The block's 1-based declaration ordinal.
        ordinal: u32,
        /// The non-positive flows.
        flows: i64,
    },
    /// A crypto block whose flows exceed one dpseci's `DPSECI_MAX_QUEUE_NUM` queue
    /// pairs — one block is one device, so this is refused, not clamped; the remedy
    /// is splitting across blocks (task 2.6e; `dpseci.h`).
    CryptoFlowsOverDevice {
        /// The tenant.
        tenant: TenantName,
        /// The block's 1-based declaration ordinal.
        ordinal: u32,
        /// The requested flows.
        flows: i64,
        /// One dpseci's queue-pair ceiling.
        max_flows: i64,
    },
    /// The summed derived count for a family exceeds a Counted/Observed ceiling
    /// (ADR-0011; design D2).
    Infeasible {
        /// The family.
        family: Family,
        /// The summed derived count.
        needed: i64,
        /// The ceiling.
        available: i64,
    },
    /// A tenant whose dataplane ADR-0012 does not price (today `UserspaceEvent`;
    /// design D3).
    UnpricedDataplane {
        /// The tenant.
        tenant: TenantName,
        /// The unpriced dataplane.
        dataplane: Dataplane,
    },
    /// A restricted tenant's pool holder is not public (design D6a).
    HolderNotPublic {
        /// The tenant.
        tenant: TenantName,
        /// The non-public holder.
        holder: TenantName,
    },
    /// A restricted tenant's holder itself has a pool — no chains (design D6a).
    PoolChain {
        /// The tenant.
        tenant: TenantName,
        /// The chaining holder.
        holder: TenantName,
    },
    /// A restricted drawer's dataplane differs from its holder's — the reserved
    /// kernel counting as kernel-netlink (design D6a).
    PoolDataplaneMismatch {
        /// The tenant.
        tenant: TenantName,
        /// The drawer's dataplane.
        drawer: Dataplane,
        /// The holder's dataplane.
        holder: Dataplane,
    },
    /// A link whose two ends resolve to the same tenant (vocabulary-v2 D4). The
    /// compile-side twin of the parse self-loop check
    /// (`crates/dpaa2-config/src/parse.rs` `convert_link`); with [`TenantRef`], two
    /// [`TenantRef::Kernel`] ends are the same tenant too. Deliberate duplication
    /// across the config→api seam (design D11): the raw model twin is
    /// `intent_raw.qnt` `RawLinkSelfLoop` (`crates/dpaa2-verify/src/raw_itf.rs`).
    LinkSelfLoop {
        /// The link naming one tenant at both ends.
        link: ConstructName,
    },
    /// A rename `from` naming a construct currently declared and not itself renamed
    /// away (vocabulary-v2 D4): the target would be claimed twice. The compile-side
    /// twin of the parse check (`crates/dpaa2-config/src/parse.rs` `check_renames`),
    /// covering both the tenant and the port/link/fabric namespaces. Deliberate
    /// duplication across the config→api seam (design D11).
    RenameDoubleClaim {
        /// The construct declaring the rename.
        construct: ConstructName,
        /// The contested name it claims.
        from: ConstructName,
    },
    /// The intent declares a tenant named `kernel` that is not the reserved kernel
    /// (vocabulary-v2 D4). The compile-side twin of the parse reserved-name check
    /// (`crates/dpaa2-config/src/parse.rs` `convert`); nullary because the name is
    /// the fact. The reserved [`kernel_tenant`] is materialised into the tenant list
    /// by the frontend and derive, so it is exempt — only a kernel-named tenant of a
    /// non-reserved shape is the programmatic declaration the TOML boundary refuses.
    KernelDeclared,
}

/// The `Refusal` variant names, in declaration order — the Rust copy of the
/// `refuse.qnt` refusal vocabulary as a `&str` list the model lint can read
/// (ADR-0014: an enumeration that restates the model is a linted copy, tied back
/// to it by `intent_lint` R14; `Reserved`/`Foreign` carry the accepted ADR-0013
/// §5 spelling, aliased to the model's anchor names in the lint). `Refusal` is
/// payload-carrying, so it cannot be iterated like [`crate::ALL_FAMILIES`]; this
/// list stands in, kept honest by the exhaustive `match` in [`Refusal::name`].
pub const REFUSAL_VARIANTS: [&str; 25] = [
    "TenantAbsent",
    "MemberUnresolved",
    "SelfMember",
    "Unanchored",
    "Reserved",
    "Foreign",
    "DoubleClaimed",
    "OverRate",
    "FabricNotKernelForwarded",
    "PortTenantMismatch",
    "UnsupportedEdge",
    "UnknownRateClass",
    "CoreBudgetExceeded",
    "ExtraNotCompanion",
    "ExtraNotPositive",
    "CryptoFlowsNotPositive",
    "CryptoFlowsOverDevice",
    "Infeasible",
    "UnpricedDataplane",
    "HolderNotPublic",
    "PoolChain",
    "PoolDataplaneMismatch",
    "LinkSelfLoop",
    "RenameDoubleClaim",
    "KernelDeclared",
];

impl Refusal {
    /// This variant's name, the same token [`REFUSAL_VARIANTS`] lists. The
    /// exhaustive `match` is what ties that list to the enum (ADR-0014): a
    /// variant added, removed, or renamed forces this arm — and so the adjacent
    /// list — to change, and each arm returns a name the list must also carry.
    /// `#[non_exhaustive]` does not bite here, inside the defining crate.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::TenantAbsent { .. } => "TenantAbsent",
            Self::MemberUnresolved { .. } => "MemberUnresolved",
            Self::SelfMember { .. } => "SelfMember",
            Self::Unanchored { .. } => "Unanchored",
            Self::Reserved { .. } => "Reserved",
            Self::Foreign { .. } => "Foreign",
            Self::DoubleClaimed { .. } => "DoubleClaimed",
            Self::OverRate { .. } => "OverRate",
            Self::FabricNotKernelForwarded { .. } => "FabricNotKernelForwarded",
            Self::PortTenantMismatch { .. } => "PortTenantMismatch",
            Self::UnsupportedEdge { .. } => "UnsupportedEdge",
            Self::UnknownRateClass { .. } => "UnknownRateClass",
            Self::CoreBudgetExceeded { .. } => "CoreBudgetExceeded",
            Self::ExtraNotCompanion { .. } => "ExtraNotCompanion",
            Self::ExtraNotPositive { .. } => "ExtraNotPositive",
            Self::CryptoFlowsNotPositive { .. } => "CryptoFlowsNotPositive",
            Self::CryptoFlowsOverDevice { .. } => "CryptoFlowsOverDevice",
            Self::Infeasible { .. } => "Infeasible",
            Self::UnpricedDataplane { .. } => "UnpricedDataplane",
            Self::HolderNotPublic { .. } => "HolderNotPublic",
            Self::PoolChain { .. } => "PoolChain",
            Self::PoolDataplaneMismatch { .. } => "PoolDataplaneMismatch",
            Self::LinkSelfLoop { .. } => "LinkSelfLoop",
            Self::RenameDoubleClaim { .. } => "RenameDoubleClaim",
            Self::KernelDeclared => "KernelDeclared",
        }
    }
}

/// A non-fatal note attached to an accepted compile (design D2/D3; ADR-0013 §5).
///
/// The review's escape-hatch-warns rule: the compiler flags what it prices on
/// unmeasured evidence, never silently.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Warning {
    /// A derived family's ceiling is [`crate::inventory::Ceiling::Unknown`], so
    /// feasibility could not check it — accepted, never invented (ADR-0011).
    UnknownCeiling {
        /// The family.
        family: Family,
        /// The count feasibility could not check.
        needed: i64,
    },
    /// A userspace-poll tenant terminates more than one seeded rate class, so the
    /// worker formula prices its T over an unmeasured cross-class mix (design D3).
    UnmeasuredCombination {
        /// The tenant.
        tenant: TenantName,
        /// The rate classes mixed.
        rates: Vec<i64>,
    },
}

/// The 2 `Warning` variant names, in declaration order — the Rust copy of the
/// `refuse.qnt` warning vocabulary as a `&str` list the model lint can read
/// (ADR-0014: an enumeration that restates the model is a linted copy, tied
/// back to it by `intent_lint` R14). `Warning` is payload-carrying, so this
/// list stands in, kept honest by the exhaustive `match` in [`Warning::name`].
pub const WARNING_VARIANTS: [&str; 2] = ["UnknownCeiling", "UnmeasuredCombination"];

impl Warning {
    /// This variant's name, the same token [`WARNING_VARIANTS`] lists and the
    /// ITF trace tag carries. The exhaustive `match` ties the list to the enum
    /// (ADR-0014): a variant added, removed, or renamed forces this arm — and
    /// so the adjacent list — to change.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::UnknownCeiling { .. } => "UnknownCeiling",
            Self::UnmeasuredCombination { .. } => "UnmeasuredCombination",
        }
    }
}

/// A successful compile: the object plan and its non-fatal [`Warning`]s (design D5;
/// `refuse.qnt` `Compiled::Ok`). The failing half is the complete refusal set
/// [`compile`] returns as its `Err`, so the model's `Compiled` sum maps onto Rust's
/// [`Result`]: `Ok(Compiled)` ⇔ `Ok({plan, warnings})`, `Err(refusals)` ⇔
/// `Refused(set)`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Compiled {
    /// The compiled object plan.
    pub plan: CompiledPlan,
    /// The non-fatal warnings attached to the accepted compile.
    pub warnings: BTreeSet<Warning>,
}

impl Compiled {
    /// Pairs the compiled plan with its port-family actuation projection into the
    /// [`DesiredTopology`] `reconcile` drives (design D10/D11).
    ///
    /// The projection is the intent's terminated ports — the dpnis the plan carries a
    /// dpni↔dpmac port-edge for; a hardware-switched port yields a dpsw interface, not
    /// a port-edge, so it is excluded and the two facets agree by construction.
    ///
    /// # Panics
    ///
    /// Panics on a [`crate::FacetMismatch`] — a compiled plan whose port-edges disagree
    /// with its own terminated ports is a compiler bug, never operator input (design
    /// D11: a compile-produced pairing failing the facet check cannot happen).
    #[must_use]
    pub fn desired_topology(&self, intent: &Intent) -> DesiredTopology {
        let ports: Vec<DesiredPort> = intent
            .ports
            .iter()
            .filter(|p| !is_hw_switched_port(intent, &p.name))
            .map(|p| DesiredPort {
                mac: p.mac,
                mac_mode: p.mac_mode,
                ..DesiredPort::new(p.dpmac, p.name.as_str())
            })
            .collect();
        DesiredTopology::from_parts(self.plan.clone(), ports)
            .expect("a compiled plan pairs coherently with its terminated-port projection")
    }
}

// ---- shared lookups ----

fn tenant_names(intent: &Intent) -> BTreeSet<TenantName> {
    intent.tenants.iter().map(|c| c.name.clone()).collect()
}

fn tenant_by_name<'a>(intent: &'a Intent, n: &TenantName) -> Option<&'a Tenant> {
    intent.tenants.iter().find(|c| &c.name == n)
}

// ---- rule 1: a construct names an undeclared tenant (design D5) ----

fn tenant_absent_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    let names = tenant_names(intent);
    // A port's tenant and each link end is a `TenantRef` (vocabulary-v2 D2): the
    // `Kernel` case is the reserved tenant, always available (a port belongs to it,
    // a link end names it as a pseudo-wire end), so `TenantAbsent` fires only on the
    // `Named` case with an undeclared name — no site can ever name `kernel` as absent.
    for p in &intent.ports {
        if let TenantRef::Named(n) = &p.tenant
            && !names.contains(n)
        {
            out.insert(Refusal::TenantAbsent {
                referrer: Referrer::Port(p.name.clone()),
                tenant: n.clone(),
            });
        }
    }
    for l in &intent.links {
        for end in [&l.interface_a, &l.interface_b] {
            if let TenantRef::Named(n) = end
                && !names.contains(n)
            {
                out.insert(Refusal::TenantAbsent {
                    referrer: Referrer::LinkEnd(l.name.clone()),
                    tenant: n.clone(),
                });
            }
        }
    }
    for f in &intent.fabrics {
        if !names.contains(&f.forwarded_by) {
            out.insert(Refusal::TenantAbsent {
                referrer: Referrer::Fabric(f.name.clone()),
                tenant: f.forwarded_by.clone(),
            });
        }
    }
    for k in &intent.crypto {
        if !names.contains(&k.tenant) {
            out.insert(Refusal::TenantAbsent {
                referrer: Referrer::Crypto(k.tenant.clone()),
                tenant: k.tenant.clone(),
            });
        }
    }
    for e in &intent.extras {
        if !names.contains(&e.tenant) {
            out.insert(Refusal::TenantAbsent {
                referrer: Referrer::Extra(e.tenant.clone()),
                tenant: e.tenant.clone(),
            });
        }
    }
}

// ---- rule 2: a fabric member names something not declared ----

fn member_unresolved_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    let names = tenant_names(intent);
    for f in &intent.fabrics {
        for m in &f.members {
            let unresolved = match m {
                Member::Port(p) => port_by_name(intent, p).is_none(),
                Member::Tenant(c) => !names.contains(c),
                Member::Fabric(h) => fabric_by_name(intent, h).is_none(),
            };
            if unresolved {
                out.insert(Refusal::MemberUnresolved {
                    fabric: f.name.clone(),
                    member: m.clone(),
                });
            }
        }
    }
}

// ---- rule 3: a fabric member resolves to the fabric's own owner ----

fn self_via_chain(intent: &Intent, f: &Fabric, h: &ConstructName) -> bool {
    matches!(
        fabric_by_name(intent, h),
        Some(g) if f.switching == Switching::Software
            && g.switching == Switching::Software
            && g.forwarded_by == f.forwarded_by
    )
}

fn self_member_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for f in &intent.fabrics {
        for m in &f.members {
            let is_self = match m {
                Member::Tenant(c) => f.switching == Switching::Software && *c == f.forwarded_by,
                Member::Fabric(h) => *h == f.name || self_via_chain(intent, f, h),
                Member::Port(_) => false,
            };
            if is_self {
                out.insert(Refusal::SelfMember {
                    fabric: f.name.clone(),
                    member: m.clone(),
                });
            }
        }
    }
}

// ---- rule 4: the port's dpmac anchor (design D2; ADR-0003 §3; ADR-0001 §4) ----

fn anchor_refusals(intent: &Intent, inv: &Inventory, out: &mut BTreeSet<Refusal>) {
    // Ownership is judged against the declared-name recognition set (ADR-0010 §4 as
    // refined by ADR-0015): a dpmac held by an object wearing one of these names is
    // ours, not foreign.
    let declared = intent.declared_names();
    for p in &intent.ports {
        if !inv.dpmacs.contains_key(&p.dpmac) {
            out.insert(Refusal::Unanchored {
                port: p.name.clone(),
                dpmac: p.dpmac,
            });
            continue;
        }
        match inv.availability_of(Family::Dpmac, p.dpmac.into_inner(), &declared) {
            Availability::Reserved(why) => {
                out.insert(Refusal::Reserved {
                    port: p.name.clone(),
                    dpmac: p.dpmac,
                    why,
                });
            }
            Availability::Foreign(owner) => {
                out.insert(Refusal::Foreign {
                    port: p.name.clone(),
                    dpmac: p.dpmac,
                    owner,
                });
            }
            Availability::Free => {
                let max_rate = inv.dpmacs[&p.dpmac].max_rate;
                if p.rate > max_rate {
                    out.insert(Refusal::OverRate {
                        port: p.name.clone(),
                        rate: p.rate,
                        max_rate,
                    });
                }
            }
        }
    }
}

// ---- rule 5: a dpmac or a port claimed twice (design D5) ----

fn ports_on_dpmac(intent: &Intent, d: DpmacId) -> BTreeSet<ConstructName> {
    intent
        .ports
        .iter()
        .filter(|p| p.dpmac == d)
        .map(|p| p.name.clone())
        .collect()
}

fn fabrics_naming_port(intent: &Intent, pn: &ConstructName) -> BTreeSet<ConstructName> {
    intent
        .fabrics
        .iter()
        .filter(|f| {
            f.members
                .iter()
                .any(|m| matches!(m, Member::Port(p) if p == pn))
        })
        .map(|f| f.name.clone())
        .collect()
}

fn member_port_names(intent: &Intent) -> BTreeSet<ConstructName> {
    let mut s = BTreeSet::new();
    for f in &intent.fabrics {
        for m in &f.members {
            if let Member::Port(p) = m {
                s.insert(p.clone());
            }
        }
    }
    s
}

fn double_claimed_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    let claimed: BTreeSet<DpmacId> = intent.ports.iter().map(|p| p.dpmac).collect();
    for d in claimed {
        let ports = ports_on_dpmac(intent, d);
        if ports.len() >= 2 {
            out.insert(Refusal::DoubleClaimed {
                dpmac: d,
                constructs: ports.into_iter().collect(),
            });
        }
    }
    for pn in member_port_names(intent) {
        let fabrics = fabrics_naming_port(intent, &pn);
        if fabrics.len() >= 2 {
            let dpmac = port_by_name(intent, &pn).map_or(DpmacId::new(0), |p| p.dpmac);
            out.insert(Refusal::DoubleClaimed {
                dpmac,
                constructs: fabrics.into_iter().collect(),
            });
        }
    }
}

// ---- rule 6: fabric forwarding, port tenancy, unsupported edges (design D4/D5) ----

fn fabric_rules_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for f in &intent.fabrics {
        if f.switching == Switching::Hardware && !f.forwarded_by.is_kernel() {
            out.insert(Refusal::FabricNotKernelForwarded {
                fabric: f.name.clone(),
                forwarded_by: f.forwarded_by.clone(),
            });
        }
        for m in &f.members {
            // The port's owner is a `TenantRef` (vocabulary-v2 D2); resolve it to the
            // name the fabric forwarder is spelled as before comparing.
            if let Member::Port(pn) = m
                && let Some(port) = port_by_name(intent, pn)
                && port.tenant.resolved() != f.forwarded_by
            {
                out.insert(Refusal::PortTenantMismatch {
                    fabric: f.name.clone(),
                    port: pn.clone(),
                    tenant: port.tenant.resolved(),
                });
            }
        }
        if f.switching == Switching::Hardware {
            for m in &f.members {
                if let Member::Fabric(h) = m
                    && matches!(fabric_by_name(intent, h), Some(g) if g.switching == Switching::Hardware)
                {
                    out.insert(Refusal::UnsupportedEdge {
                        fabric: f.name.clone(),
                        member: h.clone(),
                    });
                }
            }
        }
    }
}

// ---- rule 7: userspace-poll thread-count sizing (design D3; ADR-0012) ----

fn sizing_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for c in &intent.tenants {
        if c.dataplane != Dataplane::UserspacePoll {
            continue;
        }
        let ports = terminated_ports(intent, &c.name);
        match thread_count(&ports) {
            None => {
                out.insert(Refusal::UnknownRateClass {
                    tenant: c.name.clone(),
                    rates: ports.iter().map(|p| p.rate).collect(),
                });
            }
            Some(t) => {
                if t > c.max_cores {
                    out.insert(Refusal::CoreBudgetExceeded {
                        tenant: c.name.clone(),
                        t,
                        max_cores: c.max_cores,
                    });
                }
            }
        }
    }
}

// ---- rule 8: additive extras (design D5) ----

fn extra_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    let companions = [Family::Dpio, Family::Dpbp, Family::Dpmcp, Family::Dpcon];
    for e in &intent.extras {
        if !companions.contains(&e.family) {
            out.insert(Refusal::ExtraNotCompanion {
                tenant: e.tenant.clone(),
                family: e.family,
            });
        }
        if e.count < 1 {
            out.insert(Refusal::ExtraNotPositive {
                tenant: e.tenant.clone(),
                family: e.family,
                count: e.count,
            });
        }
    }
}

// ---- rule 9: a crypto block's flows out of the one-device range (design D1) ----

fn crypto_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for (i, k) in intent.crypto.iter().enumerate() {
        let ordinal = u32::try_from(i + 1).unwrap_or(0);
        if k.flows < 1 {
            out.insert(Refusal::CryptoFlowsNotPositive {
                tenant: k.tenant.clone(),
                ordinal,
                flows: k.flows,
            });
        } else if k.flows > DPSECI_MAX_QUEUE_NUM {
            out.insert(Refusal::CryptoFlowsOverDevice {
                tenant: k.tenant.clone(),
                ordinal,
                flows: k.flows,
                max_flows: DPSECI_MAX_QUEUE_NUM,
            });
        }
    }
}

// ---- rule 10: cross-plan feasibility against the ceilings (ADR-0011; design D2) ----

fn feasibility_refusals(
    inv: &Inventory,
    counts: &BTreeMap<Family, i64>,
    out: &mut BTreeSet<Refusal>,
) {
    for fam in DERIVED_FAMILIES {
        let needed = counts[&fam];
        let available = match inv.ceilings.get(&fam) {
            Some(Ceiling::Counted(n) | Ceiling::Observed { n, .. }) => Some(*n),
            _ => None,
        };
        if let Some(n) = available
            && needed > n
        {
            out.insert(Refusal::Infeasible {
                family: fam,
                needed,
                available: n,
            });
        }
    }
}

// ---- rule 11: a tenant whose dataplane has no companion pricing (design D3) ----

fn unpriced_dataplane_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for c in &intent.tenants {
        if !has_pricing(c) {
            out.insert(Refusal::UnpricedDataplane {
                tenant: c.name.clone(),
                dataplane: c.dataplane,
            });
        }
    }
}

// ---- rule 12: tenant isolation and pooling (design D6a; vocabulary-v2 D1) ----
//
// Restrictedness is read off the `Restricted { pool }` payload, not a `""`
// sentinel: the two contradiction shapes (a pool on a non-restricted tenant, a
// restricted tenant with no pool) are unrepresentable in [`Isolation`], so only
// the holder-relationship refusals — which need cross-tenant lookups a type
// cannot carry — remain here. The TOML surface still names both contradictions
// (`crates/dpaa2-config/src/parse.rs` `convert_tenant`).

fn pool_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    use crate::intent::Isolation;
    for c in &intent.tenants {
        let Isolation::Restricted { pool } = &c.isolation else {
            continue;
        };
        if pool.is_kernel() {
            if c.dataplane != Dataplane::KernelNetlink {
                out.insert(Refusal::PoolDataplaneMismatch {
                    tenant: c.name.clone(),
                    drawer: c.dataplane,
                    holder: Dataplane::KernelNetlink,
                });
            }
            continue;
        }
        match tenant_by_name(intent, pool) {
            None => {
                out.insert(Refusal::TenantAbsent {
                    referrer: Referrer::Pool(c.name.clone()),
                    tenant: pool.clone(),
                });
            }
            Some(h) => {
                if h.isolation != Isolation::Public {
                    out.insert(Refusal::HolderNotPublic {
                        tenant: c.name.clone(),
                        holder: pool.clone(),
                    });
                }
                if matches!(h.isolation, Isolation::Restricted { .. }) {
                    out.insert(Refusal::PoolChain {
                        tenant: c.name.clone(),
                        holder: pool.clone(),
                    });
                }
                if c.dataplane != h.dataplane {
                    out.insert(Refusal::PoolDataplaneMismatch {
                        tenant: c.name.clone(),
                        drawer: c.dataplane,
                        holder: h.dataplane,
                    });
                }
            }
        }
    }
}

/// The per-derived-family object count of a plan, saturating at [`i64::MAX`]
/// (`refuse.qnt` `feasibilityRefusals`/`warnings`; ADR-0011). Single-sources the
/// per-family tally the feasibility rule and the ceiling warnings both fold, so a
/// single [`derive`] feeds every family-count consumer.
fn family_counts(plan: &CompiledPlan) -> BTreeMap<Family, i64> {
    DERIVED_FAMILIES
        .into_iter()
        .map(|fam| {
            let n = i64::try_from(
                plan.objects
                    .iter()
                    .filter(|o| o.key().family == fam)
                    .count(),
            )
            .unwrap_or(i64::MAX);
            (fam, n)
        })
        .collect()
}

// ---- rules 13-15: the three parity refusals (vocabulary-v2 D4) ----

/// A link whose two ends resolve to the same tenant (`refuse.qnt`
/// `linkSelfLoopRefusals`). The compile-side twin of the parse self-loop check
/// (`crates/dpaa2-config/src/parse.rs` `convert_link`, "a link joins two distinct
/// tenants"); with [`TenantRef`] two [`TenantRef::Kernel`] ends compare equal too.
/// A refused link never reaches derivation — `compile` returns the refusal set and
/// never hands its plan out. Deliberate config→api duplication (design D11).
fn link_self_loop_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for l in &intent.links {
        if l.interface_a == l.interface_b {
            out.insert(Refusal::LinkSelfLoop {
                link: l.name.clone(),
            });
        }
    }
}

/// The intent declares a tenant named `kernel` that is not the reserved kernel
/// (`refuse.qnt` `kernelDeclaredRefusals`). The compile-side twin of the parse
/// reserved-name check (`crates/dpaa2-config/src/parse.rs` `convert`). The reserved
/// [`kernel_tenant`] is materialised into the tenant list by the frontend and derive
/// and must compile, so only a kernel-named tenant of a non-reserved shape is the
/// programmatic declaration the TOML boundary refuses. Deliberate config→api
/// duplication (design D11).
fn kernel_declared_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    if intent
        .tenants
        .iter()
        .any(|t| t.name.is_kernel() && *t != kernel_tenant(t.max_cores))
    {
        out.insert(Refusal::KernelDeclared);
    }
}

/// A rename `from` naming a construct currently declared and not itself renamed away
/// (`refuse.qnt` `renameDoubleClaimRefusals`): the target would be claimed twice. The
/// compile-side twin of the parse check (`crates/dpaa2-config/src/parse.rs`
/// `check_renames`), over the tenant namespace and the shared port/link/fabric
/// namespace, matching the two-namespace split parse makes. Deliberate config→api
/// duplication (design D11).
fn rename_double_claim_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    let tenant_declared_unrenamed = |n: &TenantName| {
        intent
            .tenants
            .iter()
            .any(|t| &t.name == n && t.renamed.is_none())
    };
    for t in &intent.tenants {
        if let Some(from) = &t.renamed
            && tenant_declared_unrenamed(from)
        {
            out.insert(Refusal::RenameDoubleClaim {
                construct: ConstructName::from(&t.name),
                from: ConstructName::from(from),
            });
        }
    }
    let construct_declared_unrenamed = |n: &ConstructName| {
        intent
            .ports
            .iter()
            .any(|p| &p.name == n && p.renamed.is_none())
            || intent
                .links
                .iter()
                .any(|l| &l.name == n && l.renamed.is_none())
            || intent
                .fabrics
                .iter()
                .any(|f| &f.name == n && f.renamed.is_none())
    };
    let constructs = intent
        .ports
        .iter()
        .map(|p| (&p.name, &p.renamed))
        .chain(intent.links.iter().map(|l| (&l.name, &l.renamed)))
        .chain(intent.fabrics.iter().map(|f| (&f.name, &f.renamed)));
    for (name, renamed) in constructs {
        if let Some(from) = renamed
            && construct_declared_unrenamed(from)
        {
            out.insert(Refusal::RenameDoubleClaim {
                construct: name.clone(),
                from: from.clone(),
            });
        }
    }
}

/// Every rule runs unconditionally; the refusal set is their union — the compiler
/// idiom, never first-failure-only (design D5; `refuse.qnt` `refusals`). `counts`
/// is the family tally of the shared, once-derived plan.
#[must_use]
fn refusals(intent: &Intent, inv: &Inventory, counts: &BTreeMap<Family, i64>) -> BTreeSet<Refusal> {
    let mut out = BTreeSet::new();
    tenant_absent_refusals(intent, &mut out);
    member_unresolved_refusals(intent, &mut out);
    self_member_refusals(intent, &mut out);
    anchor_refusals(intent, inv, &mut out);
    double_claimed_refusals(intent, &mut out);
    fabric_rules_refusals(intent, &mut out);
    sizing_refusals(intent, &mut out);
    extra_refusals(intent, &mut out);
    crypto_refusals(intent, &mut out);
    feasibility_refusals(inv, counts, &mut out);
    unpriced_dataplane_refusals(intent, &mut out);
    pool_refusals(intent, &mut out);
    link_self_loop_refusals(intent, &mut out);
    kernel_declared_refusals(intent, &mut out);
    rename_double_claim_refusals(intent, &mut out);
    out
}

/// A warning per derived family whose ceiling is Unknown and whose count is non-zero
/// (ADR-0011), plus one per userspace-poll tenant mixing seeded rate classes (design
/// D3; `refuse.qnt` `warnings`).
#[must_use]
fn warnings(intent: &Intent, inv: &Inventory, counts: &BTreeMap<Family, i64>) -> BTreeSet<Warning> {
    let mut out = BTreeSet::new();
    for fam in DERIVED_FAMILIES {
        let needed = counts[&fam];
        if matches!(inv.ceilings.get(&fam), Some(Ceiling::Unknown)) && needed > 0 {
            out.insert(Warning::UnknownCeiling {
                family: fam,
                needed,
            });
        }
    }
    for c in &intent.tenants {
        if c.dataplane == Dataplane::UserspacePoll {
            let classes = seeded_rate_classes(intent, &c.name);
            if classes.len() > 1 {
                out.insert(Warning::UnmeasuredCombination {
                    tenant: c.name.clone(),
                    rates: classes.into_iter().collect(),
                });
            }
        }
    }
    out
}

/// The total function (design D5; `refuse.qnt` `compile`): an empty refusal set
/// yields the plan and its warnings, else the *complete* refusal set. Pure and
/// deterministic — the [`BTreeSet`] iteration order makes the output byte-stable.
///
/// # Errors
///
/// Returns the non-empty [`BTreeSet`] of every [`Refusal`] the intent broke — never
/// the first violation, so the operator fixes a file in one pass.
pub fn compile(intent: &Intent, inv: &Inventory) -> Result<Compiled, BTreeSet<Refusal>> {
    // Derive once: both the feasibility rule and the ceiling warnings read the
    // plan only through its per-family tally, so a single plan feeds all three
    // consumers (the refusals, the warnings, and the accepted plan itself).
    let plan = derive(intent, inv);
    let counts = family_counts(&plan);
    let rs = refusals(intent, inv, &counts);
    if rs.is_empty() {
        let warnings = warnings(intent, inv, &counts);
        Ok(Compiled { plan, warnings })
    } else {
        Err(rs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list and the enum name a variant the same way, and the list is a
    /// duplicate-free 22 — the runtime half of the tie the exhaustive
    /// [`Refusal::name`] match makes at compile time (ADR-0014).
    #[test]
    fn refusal_variants_match_the_enum() {
        let sample = Refusal::TenantAbsent {
            referrer: Referrer::Port("p".into()),
            tenant: "t".into(),
        };
        assert!(REFUSAL_VARIANTS.contains(&sample.name()));

        let mut seen = REFUSAL_VARIANTS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), REFUSAL_VARIANTS.len(), "duplicate variant name");
    }

    /// The list and the enum name a warning variant the same way, and the list
    /// is a duplicate-free 2 — the runtime half of the tie the exhaustive
    /// [`Warning::name`] match makes at compile time (ADR-0014).
    #[test]
    fn warning_variants_match_the_enum() {
        let samples = [
            Warning::UnknownCeiling {
                family: Family::Dpni,
                needed: 1,
            },
            Warning::UnmeasuredCombination {
                tenant: "t".into(),
                rates: vec![10_000, 25_000],
            },
        ];
        for s in &samples {
            assert!(WARNING_VARIANTS.contains(&s.name()), "{}", s.name());
        }

        let mut seen = WARNING_VARIANTS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), WARNING_VARIANTS.len(), "duplicate variant name");
        assert_eq!(WARNING_VARIANTS.len(), 2);
    }
}

#[cfg(test)]
mod compile_tests {
    //! `compile` against the smallest intent that triggers each refusal variant
    //! (one per variant, asserting the *complete* set — the model's scenario-twin
    //! style), and one positive test per companion rule (design D3/D4/D5/D6a). The
    //! reference-board intent is the oracle for the poll-mode draws (ADR-0013 §7,
    //! `models/intent/scenarios/reference.qnt`).

    use std::collections::BTreeSet;

    use super::{Compiled, Referrer, Refusal, Warning, compile};
    use crate::compiled::{Attributes, Container, ProvenanceNode};
    use crate::family::Family;
    use crate::intent::{
        Crypto, Dataplane, Extra, Fabric, Intent, Isolation, Link, Member, Port, Switching, Tenant,
        TenantRef, kernel_tenant,
    };
    use crate::inventory::{Availability, Ceiling, Inventory};
    use crate::model::DpmacId;
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
    /// carries the holder name (vocabulary-v2 D1).
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
            mac_mode: crate::model::MacMode::Assert,
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

    // The `PoolWithoutRestricted` / `RestrictedWithoutPool` shapes are now
    // unrepresentable (vocabulary-v2 D1): `Isolation::Restricted` carries the pool
    // in its payload, so a pool on a non-restricted tenant and a restricted tenant
    // with no pool have no constructor. There is nothing to refuse and no test to
    // write — the parse surface still names both (`raw_conformance`).

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
    /// (vocabulary-v2 D3, PASS3-F13): an operator legally declares a port named
    /// `pool` and a restricted tenant whose holder is undeclared. The missing-holder
    /// refusal identifies the drawing tenant through [`Referrer::Pool`] — a typed
    /// site, not a `ConstructName` carrying the literal `"pool"` — so it is
    /// distinguishable from anything referring to the port. Before D3 both rendered a
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

    // ---- vocabulary-v2 D4: the three programmatic-parity refusals ----

    /// Spec scenario "A programmatic self-loop link is refused" (vocabulary-v2 D4): an
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
        assert!(err(&intent, &ref_inv()).contains(&Refusal::LinkSelfLoop {
            link: "wire".into()
        }));
    }

    /// Spec scenario "A programmatic kernel declaration is refused" (vocabulary-v2 D4):
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
        assert!(err(&intent, &ref_inv()).contains(&Refusal::KernelDeclared));
    }

    /// Spec scenario "A programmatic rename double-claim is refused" (vocabulary-v2 D4):
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
            mac_mode: crate::model::MacMode::Assert,
            renamed: from.map(Into::into),
        };
        let intent = Intent {
            tenants: vec![poll("router")],
            ports: vec![mk("e0", 7, Some("wan0")), mk("wan0", 8, None)],
            ..Intent::default()
        };
        assert!(
            err(&intent, &ref_inv()).contains(&Refusal::RenameDoubleClaim {
                construct: "e0".into(),
                from: "wan0".into(),
            })
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
    // one positive test per companion rule (design D3/D4/D6a)
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
        assert_eq!(t.mark, crate::compiled::Measurement::Unmeasured);
        assert_eq!(count_fam(&c, "router", Family::Dpni), 2);
        assert_eq!(
            attributes_of(&c, "router", Family::Dpni, 1),
            Attributes::Dpni { num_queues: 5 }
        );
        assert_eq!(count_fam(&c, "router", Family::Dpio), 10); // 2·T
        assert_eq!(count_fam(&c, "router", Family::Dpbp), 2);
        assert_eq!(count_fam(&c, "router", Family::Dpmcp), 1);
        assert_eq!(count_fam(&c, "router", Family::Dpcon), 10); // dpnis·T
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
            Attributes::Dpni { num_queues: 16 } // cpus transmit queues
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

    // ---- the DesiredTopology seam (design D10/D11) ----

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
        // reads, but the projection must carry them (design D9).
        let mac = crate::model::MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);
        let intent = Intent {
            tenants: vec![kernel_tenant(16)],
            ports: vec![Port {
                mac: Some(mac),
                mac_mode: crate::model::MacMode::Actuate,
                ..port("wan0", 7, 10_000, "kernel")
            }],
            ..Intent::default()
        };
        let c = ok(&intent, &ref_inv());
        let topology = c.desired_topology(&intent);
        let projected = &topology.ports()[0];
        assert_eq!(projected.mac, Some(mac));
        assert_eq!(projected.mac_mode, crate::model::MacMode::Actuate);
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
        // the mix but flags it unmeasured (design D3).
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
}
