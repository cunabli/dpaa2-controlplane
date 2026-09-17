use crate::core::types::TenantName;

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
/// [`Intent`](crate::intent::Intent) states every reference explicitly, and "an omitted port tenant means
/// the kernel" is a rule of the TOML boundary the parser applies (via
/// [`TenantRef::from_name`]), never a default of the type (a zero-initialized link
/// end silently becoming a kernel end must stay unrepresentable). Compile and
/// derive match on the case, so [`Refusal::TenantAbsent`](crate::intent::refuse::Refusal) can only
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
    /// use dpaa2_api::intent::{KERNEL, TenantRef};
    /// use dpaa2_api::core::types::TenantName;
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
/// (a) [`Default`] on `Isolation` (and on [`Intent`](crate::intent::Intent)) admits a zero-value intent that
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
/// Crypto demand is not a tenant field — each [`Crypto`](crate::intent::Crypto) block carries its own
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
    /// acceptance set, is inert after one converge, and [`compile`](crate::intent::refuse::compile)
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
