use crate::core::types::ConstructName;
use crate::intent::TenantRef;

/// A link: point-to-point dpni↔dpni pseudo-wire between two tenants
/// (object-model.md §2, DPNI-I9; `types.qnt` `Link`). Each end names the tenant
/// whose interface terminates the wire — interfaces, not ports, so tunnels have
/// room. The reserved [`KERNEL`](crate::intent::KERNEL) is nameable at an end without being declared
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
    /// [`compile`](crate::intent::refuse::compile) ignores it.
    pub renamed: Option<ConstructName>,
}
