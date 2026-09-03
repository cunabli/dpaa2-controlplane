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
/// absent, so every prior intent keeps its shape.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Isolation {
    /// A holder that accepts legal drawers into its own dprc; the reserved kernel
    /// is implicitly public.
    Public,
    /// Community co-residency: the tenant's objects are created in its `pool`
    /// holder's dprc (a DPDK secondary pooling a userspace-poll primary).
    Restricted,
    /// Its own child dprc, MC-isolated from siblings — the default.
    #[default]
    Isolated,
}

/// A tenant of hardware capacity (design D1; `types.qnt` `Tenant`).
///
/// `max_cores` is the budget the derived thread count must fit under (design D3).
/// Crypto demand is not a tenant field — each [`Crypto`] block carries its own
/// flows. `isolation` places the tenant in the container tree (default
/// [`Isolation::Isolated`]); `pool` names the public holder a restricted tenant
/// draws inside (`""` when absent).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Tenant {
    /// The tenant's name; the key namespace of every object it draws.
    pub name: TenantName,
    /// Where its dataplane runs.
    pub dataplane: Dataplane,
    /// The core budget the derived thread count must fit under (design D3).
    pub max_cores: i64,
    /// Its place in the container tree (default [`Isolation::Isolated`]).
    pub isolation: Isolation,
    /// The public holder a restricted tenant draws inside (empty when absent).
    pub pool: TenantName,
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
        pool: TenantName::from(""),
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
    /// The tenant that terminates the port.
    pub tenant: TenantName,
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
    /// The tenant whose interface terminates one end.
    pub interface_a: TenantName,
    /// The tenant whose interface terminates the other end.
    pub interface_b: TenantName,
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
/// not a shape. Members are ordered: declaration order numbers the dpsw interfaces
/// and dpni ordinals.
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
/// Lists keep declaration order — it is the ordinal source (design D6): a
/// `[[port]]`/`[[crypto]]` array is ordered, so a tenant's Nth block numbers its
/// Nth dpseci. Only `extras` is a set — unordered, matched by `(tenant, family)`,
/// additive, never by position.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Intent {
    /// The declared tenants, in order.
    pub tenants: Vec<Tenant>,
    /// The declared ports, in order.
    pub ports: Vec<Port>,
    /// The declared links, in order.
    pub links: Vec<Link>,
    /// The declared fabrics, in order.
    pub fabrics: Vec<Fabric>,
    /// The declared crypto blocks, in order.
    pub crypto: Vec<Crypto>,
    /// The additive extras, unordered.
    pub extras: BTreeSet<Extra>,
}
