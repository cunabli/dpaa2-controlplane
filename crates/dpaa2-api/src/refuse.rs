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

use crate::family::Family;
use crate::intent::{Dataplane, Member};
use crate::model::DpmacId;
use crate::names::{ConstructName, TenantName};

/// The rule an intent broke, naming the offending construct (design D5; ADR-0013
/// §5). All 24 variants of `refuse.qnt`.
///
/// `#[non_exhaustive]`: a `PoolShortfall` variant is reserved for `reconcile`
/// (change #6, drift against a live census) and a passthrough value is change #4's,
/// so callers must not assume the set is closed.
#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Refusal {
    /// A construct (port, link end, fabric owner, crypto, extra, a restricted
    /// tenant's `pool`) names a tenant not declared (DPDCEI-I1 generalised).
    TenantAbsent {
        /// The construct that named the missing tenant.
        construct: ConstructName,
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
    /// A `pool` named on a non-restricted tenant — a contradiction (design D6a).
    PoolWithoutRestricted {
        /// The tenant.
        tenant: TenantName,
        /// The pool it named.
        pool: TenantName,
    },
    /// A restricted tenant that names no pool holder (design D6a).
    RestrictedWithoutPool {
        /// The tenant.
        tenant: TenantName,
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
}

/// The 24 `Refusal` variant names, in declaration order — the Rust copy of the
/// `refuse.qnt` refusal vocabulary as a `&str` list the model lint can read
/// (ADR-0014: an enumeration that restates the model is a linted copy, tied back
/// to it by `intent_lint` R14; `Reserved`/`Foreign` carry the accepted ADR-0013
/// §5 spelling, aliased to the model's anchor names in the lint). `Refusal` is
/// payload-carrying, so it cannot be iterated like [`crate::ALL_FAMILIES`]; this
/// list stands in, kept honest by the exhaustive `match` in [`Refusal::name`].
pub const REFUSAL_VARIANTS: [&str; 24] = [
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
    "PoolWithoutRestricted",
    "RestrictedWithoutPool",
    "HolderNotPublic",
    "PoolChain",
    "PoolDataplaneMismatch",
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
            Self::PoolWithoutRestricted { .. } => "PoolWithoutRestricted",
            Self::RestrictedWithoutPool { .. } => "RestrictedWithoutPool",
            Self::HolderNotPublic { .. } => "HolderNotPublic",
            Self::PoolChain { .. } => "PoolChain",
            Self::PoolDataplaneMismatch { .. } => "PoolDataplaneMismatch",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The list and the enum name a variant the same way, and the list is a
    /// duplicate-free 24 — the runtime half of the tie the exhaustive
    /// [`Refusal::name`] match makes at compile time (ADR-0014).
    #[test]
    fn refusal_variants_match_the_enum() {
        let sample = Refusal::TenantAbsent {
            construct: "port `p`".into(),
            tenant: "t".into(),
        };
        assert!(REFUSAL_VARIANTS.contains(&sample.name()));

        let mut seen = REFUSAL_VARIANTS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), REFUSAL_VARIANTS.len(), "duplicate variant name");
    }
}
