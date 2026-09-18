use crate::core::types::{ConstructName, TenantName};

/// Who forwards between a fabric's members (design D1; restool-baseline; `types.qnt` `Switching`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Switching {
    /// A DPSW (figure 6c), which only the kernel can drive (`dpsw.md`).
    Hardware,
    /// The forwarding tenant bridges its own dpnis (kernel bridge, VPP bridge
    /// domain, …), which the MC never sees.
    Software,
}

/// A fabric member: a declared port, tenant, or other fabric (design D1 (restool-baseline);
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

/// A fabric: one switched domain over its members (design D1 (restool-baseline); `types.qnt`
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
    /// [`compile`](crate::intent::refuse::compile) ignores it.
    pub renamed: Option<ConstructName>,
}
