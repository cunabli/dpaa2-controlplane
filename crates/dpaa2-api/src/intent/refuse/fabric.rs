//! Per-fabric refusal validators: unresolved members, self-membership, and the
//! forwarding/tenancy/unsupported-edge rules (ADR-0018 "Refusals split by
//! construct").

use std::collections::BTreeSet;

use crate::core::types::ConstructName;
use crate::intent::derive::{
    MAX_FDB_ENTRIES, fabric_by_name, hw_fabric_attach_points, port_by_name,
};
use crate::intent::{Fabric, Intent, Member, Switching};

use super::Refusal;
use super::tenant::tenant_names;

pub(super) fn member_unresolved_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
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

fn self_via_chain(intent: &Intent, f: &Fabric, h: &ConstructName) -> bool {
    matches!(
        fabric_by_name(intent, h),
        Some(g) if f.switching == Switching::Software
            && g.switching == Switching::Software
            && g.forwarded_by == f.forwarded_by
    )
}

pub(super) fn self_member_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
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

pub(super) fn fabric_rules_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for f in &intent.fabrics {
        if f.switching == Switching::Hardware && !f.forwarded_by.is_kernel() {
            out.insert(Refusal::FabricNotKernelForwarded {
                fabric: f.name.clone(),
                forwarded_by: f.forwarded_by.clone(),
            });
        }
        for m in &f.members {
            // The port's owner is a `TenantRef` (2026-09-08-vocabulary-v2 D2); resolve it to the
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

/// Switched-domain MAC-table sizing (`refuse.qnt` `macTableRefusals`): a Hardware
/// fabric's `max_fdb_entries` is `mac_table_size` × its interface count, and the
/// product must fit the 16-bit field or the switch is unbuildable. A Software fabric
/// owns its own forwarding table, so a `mac_table_size` on one is a hardware-switch
/// knob misapplied. `None` declared is neither.
pub(super) fn mac_table_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for f in &intent.fabrics {
        let Some(b) = f.mac_table_size else { continue };
        match f.switching {
            Switching::Software => {
                out.insert(Refusal::MacTableOnSoftwareFabric {
                    fabric: f.name.clone(),
                });
            }
            Switching::Hardware => {
                let ni = u32::try_from(hw_fabric_attach_points(intent, f).len()).unwrap_or(0);
                if u64::from(b) * u64::from(ni) > u64::from(MAX_FDB_ENTRIES) {
                    out.insert(Refusal::MacTableOverflow {
                        fabric: f.name.clone(),
                        num_ifs: ni,
                        mac_table_size: b,
                        bound: MAX_FDB_ENTRIES,
                    });
                }
            }
        }
    }
}

/// A Hardware fabric with no interfaces (`refuse.qnt` `emptyFabricRefusals`): a
/// Hardware fabric whose derived dpsw interface-endpoint list is empty is a 0-port
/// switch, and the MC's `num_ifs` accepts 1–65535 (`dpsw.md`; DPAA2UM T101), so it
/// is unrenderable. This closes the only path emitting `max_fdb_entries = 0` (a
/// declared `mac_table_size` × 0 interfaces). A Software fabric owns its own
/// forwarder, so it is exempt.
pub(super) fn empty_fabric_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for f in &intent.fabrics {
        if f.switching == Switching::Hardware && hw_fabric_attach_points(intent, f).is_empty() {
            out.insert(Refusal::EmptyFabric {
                fabric: f.name.clone(),
            });
        }
    }
}
