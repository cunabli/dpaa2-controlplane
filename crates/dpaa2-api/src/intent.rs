//! The intent vocabulary: what an operator states, never a count (design D1;
//! ADR-0005 §1; ADR-0013 §2).
//!
//! Transcribed from `models/intent/types.qnt`, the accepted vocabulary of the
//! 2026-09-02 gate (ADR-0013). An operator declares network *constructs* anchored
//! in hardware — a [`Tenant`] with a [`Dataplane`] and a core budget, a [`Port`]
//! with the rate it must deliver, a [`Link`] between two tenants, a [`Fabric`] one
//! tenant forwards, a [`Crypto`] block sized by its own flows — and no field for a
//! dpio, dpbp, dpcon, dpmcp, queue or worker count. Every such number is the
//! derivation's (`compile`, task 3.2). These types carry no `serde`: the northbound
//! [`crate::ConfigSource`] parses TOML into them (design D10), and nothing below the
//! compiler depends on them (design D11).

use std::collections::BTreeSet;

use crate::family::Family;
use crate::model::{DpmacId, MacAddr, MacMode};
use crate::types::{ConstructName, TenantName};

/// The reserved kernel tenant (design D1; `types.qnt` `KERNEL`): the kernel's own
/// network driver in dprc.1. A port that names no tenant is the kernel's port, and
/// a link end may name it without declaring it (design D6a).
pub const KERNEL: &str = "kernel";

impl TenantName {
    /// Whether this is the reserved [`KERNEL`] tenant — the type-safe replacement
    /// for the `== KERNEL` string test the derivation and refusals lean on.
    #[must_use]
    pub fn is_kernel(&self) -> bool {
        self.as_str() == KERNEL
    }
}

/// A reference to the tenant that may be the reserved kernel — a port's owning
/// tenant and each link end (vocabulary-v2 D2; `types.qnt` `TenantRef`).
///
/// One shared two-case sum replaces the `""`/`"kernel"` sentinel the port default
/// and the link-end kernel exemption used to lean on: the vocabulary has exactly
/// one encoding of "the kernel or a declared tenant". There is deliberately **no
/// `Default` impl** — a reference is never optional in the API, a programmatic
/// [`Intent`] states every reference explicitly, and "an omitted port tenant means
/// the kernel" is a rule of the TOML boundary the parser applies (via
/// [`TenantRef::from_name`]), never a default of the type (a zero-initialized link
/// end silently becoming a kernel end must stay unrepresentable). Compile and
/// derive match on the case, so [`Refusal::TenantAbsent`](crate::Refusal) can only
/// ever fire on [`TenantRef::Named`] with an undeclared name — the `tenant:"kernel"`
/// wrinkle is structurally gone. Not `Copy`: the [`TenantName`] payload owns a heap
/// string.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TenantRef {
    /// The reserved root [`KERNEL`] tenant — a port owned by it lands in the root
    /// container, a link end names it as a pseudo-wire end, and neither carries a
    /// name a refusal can call absent.
    Kernel,
    /// A declared tenant, by name.
    Named(TenantName),
}

impl TenantRef {
    /// Classifies a resolved tenant name into the reference sum: the reserved
    /// [`KERNEL`] name yields [`TenantRef::Kernel`], any other name
    /// [`TenantRef::Named`]. This is the single normalisation the parser applies at
    /// the TOML boundary (design D2) — the KERNEL sentinel classification lives here,
    /// core-side, so an adapter reports a name and lets the vocabulary judge it.
    ///
    /// Domain split from the model twin `tenantRefOf` (`types.qnt`): the model folds
    /// `""` → [`TenantRef::Kernel`] because it lowers the *raw string domain*, where
    /// `""` IS the omitted-tenant default. This Rust side does **not** fold `""` — the
    /// raw side carries an `Option`, so `""` reaching `from_name` is a (bogus) *name*,
    /// not an omission; folding it to [`TenantRef::Kernel`] would silently bless an
    /// empty string the API should never see. An empty name stays [`TenantRef::Named`]:
    ///
    /// ```
    /// use dpaa2_api::{TenantRef, TenantName, KERNEL};
    /// // "" is a name here, never "the kernel": the raw Option already carried absence.
    /// assert_eq!(TenantRef::from_name(TenantName::from("")), TenantRef::Named("".into()));
    /// assert_eq!(TenantRef::from_name(TenantName::from(KERNEL)), TenantRef::Kernel);
    /// ```
    #[must_use]
    pub fn from_name(name: TenantName) -> Self {
        if name.is_kernel() {
            TenantRef::Kernel
        } else {
            TenantRef::Named(name)
        }
    }

    /// The tenant name this reference resolves to: the reserved [`KERNEL`] for the
    /// kernel case, the declared name otherwise. The derivation keys and compares
    /// objects by it, and a refusal that must name the referenced tenant reads it.
    #[must_use]
    pub fn resolved(&self) -> TenantName {
        match self {
            TenantRef::Kernel => KERNEL.into(),
            TenantRef::Named(n) => n.clone(),
        }
    }

    /// Whether this reference is the reserved [`TenantRef::Kernel`] case — the
    /// variant-arm replacement for the `== KERNEL` string test at link ends.
    #[must_use]
    pub fn is_kernel(&self) -> bool {
        matches!(self, TenantRef::Kernel)
    }
}

/// Where a tenant's dataplane runs and the delivery mechanism that drives its
/// companion sizing (design D1; ADR-0012 pricing; `types.qnt` `Dataplane`).
///
/// The value names the ownership mechanism, not just "kernel/userspace", leaving
/// room for a future kernel dataplane (XDP/BPF) beside [`Dataplane::KernelNetlink`].
/// `#[non_exhaustive]`: a VFIO passthrough value (a guest dataplane the host cannot
/// see) is change #4's, and a priced replacement for `UserspaceEvent` is a later
/// scenario's (design D5).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum Dataplane {
    /// The kernel's own driver, configured over netlink.
    KernelNetlink,
    /// A userspace poll-mode process (VPP, DPDK).
    UserspacePoll,
    /// A userspace event-driven process. ADR-0012 does not price it, so `compile`
    /// refuses it (`UnpricedDataplane`) until a scenario prices its draws.
    UserspaceEvent,
}

/// How a tenant sits in the MC container tree (design D6a; `types.qnt`
/// `Isolation`): the private-VLAN shape the tree already enforces.
///
/// [`Isolation::Isolated`] is the default the TOML applies when the field is
/// absent, so every prior intent keeps its shape. The pool holder a
/// [`Isolation::Restricted`] tenant draws inside rides in the variant payload
/// (vocabulary-v2 D1, PASS5-F4): a pool on a non-restricted tenant, and a
/// restricted tenant with no pool, have no constructor — they are unrepresentable
/// rather than refused, and restrictedness is read off the variant, never a `""`
/// sentinel. Not `Copy`: the [`TenantName`] payload owns a heap string.
///
/// TYPESTATE HAZARDS (deferred to the typestate roadmap change, not this one): two
/// zero-value escape hatches survive the sum encoding and want a typestate to close.
/// (a) [`Default`] on `Isolation` (and on [`Intent`]) admits a zero-value intent that
/// never routes a tenant reference through [`TenantRef::from_name`], so its `""`→name
/// discipline can be skipped by constructing the value directly. (b) An empty
/// [`TenantName`] is constructible (`TenantName::from("")`), so an empty pool holder or
/// tenant name is representable at the type level though no valid intent carries one.
/// Both are recorded here for the future typestate change; no code change lands now.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Isolation {
    /// A holder that accepts legal drawers into its own dprc; the reserved kernel
    /// is implicitly public.
    Public,
    /// Community co-residency: the tenant's objects are created in its `pool`
    /// holder's dprc (a DPDK secondary pooling a userspace-poll primary). The
    /// `pool` names that public holder.
    Restricted {
        /// The public holder this restricted tenant draws inside.
        pool: TenantName,
    },
    /// Its own child dprc, MC-isolated from siblings — the default.
    #[default]
    Isolated,
}

/// A tenant of hardware capacity (design D1; `types.qnt` `Tenant`).
///
/// `max_cores` is the budget the derived thread count must fit under (design D3).
/// Crypto demand is not a tenant field — each [`Crypto`] block carries its own
/// flows. `isolation` places the tenant in the container tree (default
/// [`Isolation::Isolated`]) and, for a restricted tenant, names the public holder
/// it draws inside as the [`Isolation::Restricted`] payload — there is no separate
/// `pool` field (vocabulary-v2 D1).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Tenant {
    /// The tenant's name; the key namespace of every object it draws.
    pub name: TenantName,
    /// Where its dataplane runs.
    pub dataplane: Dataplane,
    /// The core budget the derived thread count must fit under (design D3).
    pub max_cores: i64,
    /// Its place in the container tree (default [`Isolation::Isolated`]); a
    /// restricted tenant carries its pool holder in the [`Isolation::Restricted`]
    /// payload.
    pub isolation: Isolation,
    /// An accepted `renamed = { from }` clause — the tenant's prior name, or `None`
    /// when absent (ADR-0015 decision 10 / task 6.5). It widens the rename matcher's
    /// acceptance set, is inert after one converge, and [`compile`](crate::compile)
    /// ignores it (it derives objects by name, not by rename).
    pub renamed: Option<TenantName>,
}

/// The reserved kernel as a tenant value (design D6a; `types.qnt` `kernelTenant`):
/// kernel-netlink and implicitly public, so a restricted tenant may pool it and it
/// never itself draws inside another holder.
#[must_use]
pub fn kernel_tenant(max_cores: i64) -> Tenant {
    Tenant {
        name: KERNEL.into(),
        dataplane: Dataplane::KernelNetlink,
        max_cores,
        isolation: Isolation::Public,
        renamed: None,
    }
}

/// A port: the dpmac anchor, the rate it must deliver in Mbps (the unit `dpmac
/// info` reports), and the tenant that terminates it (design D1; `types.qnt`
/// `Port`). A port named by a fabric is terminated by the fabric's forwarder — a
/// differing tenant is refused (`PortTenantMismatch`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Port {
    /// The port's name (its construct identity in refusals and provenance).
    pub name: ConstructName,
    /// The stable dpmac anchor.
    pub dpmac: DpmacId,
    /// The rate the port must deliver, in Mbps.
    pub rate: i64,
    /// The tenant reference that terminates the port — the reserved kernel or a
    /// declared tenant ([`TenantRef`], vocabulary-v2 D2). A port that named no
    /// tenant in the TOML is the [`TenantRef::Kernel`] case (the parser's default);
    /// there is no `""` sentinel.
    pub tenant: TenantRef,
    /// The port's known/declared MAC, if any — an actuation-only fact the
    /// derivation never reads (design D9). It rides on the port so
    /// [`compile`](crate::compile)'s
    /// [`DesiredPort`](crate::model::DesiredPort) projection keeps the operator's
    /// MAC intent, but the sizing rules ignore it; the Quint model omits it
    /// deliberately, which is why the model-copy lint does not bind it (ADR-0013 §11).
    pub mac: Option<MacAddr>,
    /// Whether [`mac`](Self::mac) is asserted (verified) or actuated (written) —
    /// likewise an actuation-only fact the derivation never reads (design D9),
    /// carried for the projection and omitted from the model.
    pub mac_mode: MacMode,
    /// An accepted `renamed = { from }` clause — the port's prior name, or `None`
    /// when absent (ADR-0015 decision 10 / task 6.5). For the rename matcher only;
    /// [`compile`](crate::compile) ignores it.
    pub renamed: Option<ConstructName>,
}

/// A link: point-to-point dpni↔dpni pseudo-wire between two tenants
/// (object-model.md §2, DPNI-I9; `types.qnt` `Link`). Each end names the tenant
/// whose interface terminates the wire — interfaces, not ports, so tunnels have
/// room. The reserved [`KERNEL`] is nameable at an end without being declared
/// (design D6a).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Link {
    /// The link's name (its construct identity).
    pub name: ConstructName,
    /// The tenant reference whose interface terminates one end ([`TenantRef`],
    /// vocabulary-v2 D2); the reserved kernel is the [`TenantRef::Kernel`] case.
    pub interface_a: TenantRef,
    /// The tenant reference whose interface terminates the other end.
    pub interface_b: TenantRef,
    /// An accepted `renamed = { from }` clause — the link's prior name, or `None`
    /// when absent (ADR-0015 decision 10 / task 6.5). For the rename matcher only;
    /// [`compile`](crate::compile) ignores it.
    pub renamed: Option<ConstructName>,
}

/// Who forwards between a fabric's members (design D1; `types.qnt` `Switching`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Switching {
    /// A DPSW (figure 6c), which only the kernel can drive (`dpsw.md`).
    Hardware,
    /// The forwarding tenant bridges its own dpnis (kernel bridge, VPP bridge
    /// domain, …), which the MC never sees.
    Software,
}

/// A fabric member: a declared port, tenant, or other fabric (design D1;
/// `types.qnt` `Member`), so a software switch can bridge a hardware-switched
/// domain and a physical port (a chain of switches), stated, not implied.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Member {
    /// A declared port by name.
    Port(ConstructName),
    /// A declared tenant by name.
    Tenant(TenantName),
    /// Another declared fabric by name.
    Fabric(ConstructName),
}

/// A fabric: one switched domain over its members (design D1; `types.qnt`
/// `Fabric`). `forwarded_by` names the tenant that runs its forwarding plane (a
/// dpsw for [`Switching::Hardware`], its own bridging for [`Switching::Software`]).
/// That a hardware fabric is kernel-forwarded is a rule (`FabricNotKernelForwarded`),
/// not a shape. Members are ordered: member order numbers the dpsw interfaces (a
/// structural within-object index, kept as-is like crypto, task 3.3d). It does NOT
/// number dpni ordinals — those come from the fabric's NAME through the attach/wire
/// origins (the `derive` module; `derive.qnt`), so a fabric block reorder never
/// renumbers a dpni (ADR-0015 decision 5).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Fabric {
    /// The fabric's name (its construct identity, and the dpsw provenance key).
    pub name: ConstructName,
    /// Hardware (a dpsw) or software (own bridging).
    pub switching: Switching,
    /// The tenant that runs the forwarding plane.
    pub forwarded_by: TenantName,
    /// The members, in declaration order.
    pub members: Vec<Member>,
    /// An accepted `renamed = { from }` clause — the fabric's prior name, or `None`
    /// when absent (ADR-0015 decision 10 / task 6.5). For the rename matcher only;
    /// [`compile`](crate::compile) ignores it.
    pub renamed: Option<ConstructName>,
}

/// An accelerator for one tenant (design D1; `dpseci.md`; `types.qnt` `Crypto`).
///
/// Its dpseci `num_queues` derives from this block's own `flows` — a
/// tenant-visible demand, never an object count. A tenant may declare several
/// blocks; declaration order numbers its dpseci ordinals (task 2.6e), and no
/// ceiling folds a tenant's blocks together.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Crypto {
    /// The owning tenant.
    pub tenant: TenantName,
    /// The flow demand this block sizes its own dpseci to.
    pub flows: i64,
}

/// An additive extra (design D5; `types.qnt` `Extra`): every derived count is a
/// request, a per-`(tenant, family)` extra adds its `count` on top, so the
/// effective count is `request + count` — raise-only by construction. Only the four
/// companion families dpio/dpbp/dpmcp/dpcon accept an extra; any other family is
/// refused (`ExtraNotCompanion`), and `count` must be ≥ 1 (`ExtraNotPositive`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Extra {
    /// The tenant the extra raises a count for.
    pub tenant: TenantName,
    /// The companion family raised.
    pub family: Family,
    /// The count added on top of the request (≥ 1).
    pub count: i64,
}

/// The complete intent an operator states (design D1; `types.qnt` `Intent`).
///
/// Names are identities (ADR-0015 decision 1): design D6 keys every derived object by
/// `(tenant, family, ordinal)`, and as of task 3.3d the ordinal is minted by NAME
/// order, never by a construct's position in the document (ADR-0015 decision 5, the
/// position-independence law — reordering cosmetic blocks never rewires hardware).
/// tenants/ports/links/fabrics stay `Vec`s for a minimal shape, but the derivation
/// sorts them by name (the `derive` module; `derive.qnt`) for ordinal minting and
/// emission order alike, and no longer consumes their document position. `crypto`
/// is the sole exception (ADR-0015 decision 4 / task 2.6e): a `[[crypto]]` block is
/// genuinely anonymous, so declaration order IS the dpseci ordinal and crypto is never
/// sorted. Only `extras` is a set — unordered, matched by `(tenant, family)`,
/// additive, never by position.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Intent {
    /// The declared tenants, in name order — order-free by construction (dprc identity is
    /// keyed by name, not position; the config keys them as `[tenant.<name>]` tables).
    pub tenants: Vec<Tenant>,
    /// The declared ports, in order.
    pub ports: Vec<Port>,
    /// The declared links, in order.
    pub links: Vec<Link>,
    /// The declared fabrics, in name order — order-free by construction (the config keys
    /// them as `[fabric.<name>]` tables).
    pub fabrics: Vec<Fabric>,
    /// The declared crypto blocks, in order.
    pub crypto: Vec<Crypto>,
    /// The additive extras, unordered.
    pub extras: BTreeSet<Extra>,
}

impl Intent {
    /// The ownership-recognition set of ADR-0010 §4 (`edits.qnt` `intentNames`): every
    /// name the control plane recognizes as its own. It is every declared construct
    /// name — tenants, ports, links, fabrics — plus every active `renamed = { from }`
    /// value on those constructs. The `from` inclusion is decision 10's rename
    /// widening: mid-rename a board object still wears the old name, and it must not be
    /// judged foreign while the rename converges. A label absent from this set (and
    /// non-empty) is somebody else's (ADR-0010 §4, refined by ADR-0015). Crypto blocks
    /// are anonymous (ADR-0015 decision 4) and contribute nothing.
    #[must_use]
    pub fn declared_names(&self) -> BTreeSet<ConstructName> {
        let mut names = BTreeSet::new();
        for t in &self.tenants {
            names.insert(ConstructName::from(&t.name));
            if let Some(from) = &t.renamed {
                names.insert(ConstructName::from(from));
            }
        }
        for p in &self.ports {
            names.insert(p.name.clone());
            if let Some(from) = &p.renamed {
                names.insert(from.clone());
            }
        }
        for l in &self.links {
            names.insert(l.name.clone());
            if let Some(from) = &l.renamed {
                names.insert(from.clone());
            }
        }
        for f in &self.fabrics {
            names.insert(f.name.clone());
            if let Some(from) = &f.renamed {
                names.insert(from.clone());
            }
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_names_include_current_names_and_active_renames() {
        // A small intent with one renamed port: the recognition set carries every
        // declared name and the port's old name (decision 10's widening), so a board
        // object still wearing the old name is not judged foreign mid-rename.
        let intent = Intent {
            tenants: vec![kernel_tenant(4)],
            ports: vec![Port {
                name: "wan1".into(),
                dpmac: DpmacId::new(7),
                rate: 10_000,
                tenant: TenantRef::Kernel,
                mac: None,
                mac_mode: MacMode::default(),
                renamed: Some("wan0".into()),
            }],
            links: vec![Link {
                name: "l0".into(),
                interface_a: TenantRef::Kernel,
                interface_b: TenantRef::Kernel,
                renamed: None,
            }],
            ..Intent::default()
        };
        let names = intent.declared_names();
        assert!(names.contains(&ConstructName::from("kernel")));
        assert!(names.contains(&ConstructName::from("wan1")));
        assert!(names.contains(&ConstructName::from("wan0")));
        assert!(names.contains(&ConstructName::from("l0")));
        assert_eq!(names.len(), 4);
    }
}
