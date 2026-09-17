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
//! [`crate::core::inventory::Availability`] constructors of the same name (refuse.qnt
//! DEVIATION). Rust enum variants are namespaced by their type, so this transcribes
//! them under the accepted ADR-0013 §5 spelling [`Refusal::Reserved`] /
//! [`Refusal::Foreign`].

use std::collections::{BTreeMap, BTreeSet};

use crate::core::family::{DERIVED_FAMILIES, Family};
use crate::core::inventory::{Ceiling, Inventory};
use crate::core::model::{DesiredPort, DesiredTopology, DpmacId};
use crate::core::types::{ConstructName, TenantName};
use crate::intent::compiled::CompiledPlan;
use crate::intent::derive::{derive, is_hw_switched_port, seeded_rate_classes};
use crate::intent::{Dataplane, Intent, Member};

// Per-construct validators land in construct-named submodules (ADR-0018 "Refusals
// split by construct"); the enums, their linted variant lists, and the compile
// pipeline stay here.
mod crypto;
mod extra;
mod fabric;
mod link;
mod port;
mod tenant;

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
///
/// Landing convention (ADR-0018, "Refusals split by construct, land by family"):
/// the per-construct validators that fill this set live in construct-named
/// submodules of this module, while this enum and its linted variant list stay
/// put. Future object families add their refusal payload types in `families/<f>.rs`,
/// referenced by new top-level `Refusal` variants here — so no family grows this
/// file again.
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
    /// (`crates/dpaa2-config/src/parse.rs` `convert_link`); with [`crate::intent::TenantRef`], two
    /// [`crate::intent::TenantRef::Kernel`] ends are the same tenant too. Deliberate duplication
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
    /// duplication across the config→api seam (design D11): the raw model twin is
    /// `intent_raw.qnt` `RenamedFromDeclared` (`crates/dpaa2-verify/src/raw_itf.rs`).
    RenameDoubleClaim {
        /// The construct declaring the rename.
        construct: ConstructName,
        /// The contested name it claims.
        from: ConstructName,
    },
    /// The intent declares a tenant named `kernel` that is not the reserved kernel
    /// (vocabulary-v2 D4). The compile-side twin of the parse reserved-name check
    /// (`crates/dpaa2-config/src/parse.rs` `convert`); nullary because the name is
    /// the fact. The reserved [`crate::intent::kernel_tenant`] is materialised into the tenant list
    /// by the frontend and derive, so it is exempt — only a kernel-named tenant of a
    /// non-reserved shape is the programmatic declaration the TOML boundary refuses.
    /// Deliberate duplication across the config→api seam (design D11): the raw model
    /// twin is `intent_raw.qnt` `ReservedKernel` (`crates/dpaa2-verify/src/raw_itf.rs`).
    KernelDeclared,
}

/// The `Refusal` variant names, in declaration order — the Rust copy of the
/// `refuse.qnt` refusal vocabulary as a `&str` list the model lint can read
/// (ADR-0014: an enumeration that restates the model is a linted copy, tied back
/// to it by `intent_lint` R14; `Reserved`/`Foreign` carry the accepted ADR-0013
/// §5 spelling, aliased to the model's anchor names in the lint). `Refusal` is
/// payload-carrying, so it cannot be iterated like [`crate::core::family::ALL_FAMILIES`]; this
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
    /// A derived family's ceiling is [`crate::core::inventory::Ceiling::Unknown`], so
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
    /// Panics on a [`crate::core::model::FacetMismatch`] — a compiled plan whose port-edges disagree
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
    tenant::tenant_absent_refusals(intent, &mut out);
    fabric::member_unresolved_refusals(intent, &mut out);
    fabric::self_member_refusals(intent, &mut out);
    port::anchor_refusals(intent, inv, &mut out);
    port::double_claimed_refusals(intent, &mut out);
    fabric::fabric_rules_refusals(intent, &mut out);
    tenant::sizing_refusals(intent, &mut out);
    extra::extra_refusals(intent, &mut out);
    crypto::crypto_refusals(intent, &mut out);
    feasibility_refusals(inv, counts, &mut out);
    tenant::unpriced_dataplane_refusals(intent, &mut out);
    tenant::pool_refusals(intent, &mut out);
    link::link_self_loop_refusals(intent, &mut out);
    tenant::kernel_declared_refusals(intent, &mut out);
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
mod compile_tests;
