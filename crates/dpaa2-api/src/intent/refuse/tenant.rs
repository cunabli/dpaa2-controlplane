//! Per-tenant refusal validators: undeclared-tenant resolution, userspace-poll
//! sizing, dataplane pricing, isolation/pooling, and the reserved-kernel
//! declaration parity twin (ADR-0018 "Refusals split by construct").

use std::collections::BTreeSet;

use crate::core::types::TenantName;
use crate::intent::derive::{has_pricing, terminated_ports, thread_count};
use crate::intent::{Dataplane, Intent, Tenant, TenantRef, kernel_tenant};

use super::{Referrer, Refusal};

pub(super) fn tenant_names(intent: &Intent) -> BTreeSet<TenantName> {
    intent.tenants.iter().map(|c| c.name.clone()).collect()
}

fn tenant_by_name<'a>(intent: &'a Intent, n: &TenantName) -> Option<&'a Tenant> {
    intent.tenants.iter().find(|c| &c.name == n)
}

pub(super) fn tenant_absent_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    let names = tenant_names(intent);
    // No site can ever name the reserved `kernel` as absent, by two mechanisms. A
    // port's tenant and each link end is a `TenantRef` (2026-09-08-vocabulary-v2 D2) whose sole
    // constructor [`TenantRef::from_name`] folds the reserved name to the `Kernel`
    // case, which carries no name — so `TenantAbsent` fires there only on a `Named`
    // with an undeclared name. The fabric forwarder, crypto tenant, and extra tenant
    // are `TenantName`s: the guard exempts the reserved kernel, which resolves without
    // being declared, exactly as parse's `resolves` (design D1, 2026-09-06-intent-layer; dpaa2-config). Whether
    // the kernel object then materialises for such a reference is the business of the
    // split materialisation, not of resolution.
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
        if !f.forwarded_by.is_kernel() && !names.contains(&f.forwarded_by) {
            out.insert(Refusal::TenantAbsent {
                referrer: Referrer::Fabric(f.name.clone()),
                tenant: f.forwarded_by.clone(),
            });
        }
    }
    for k in &intent.crypto {
        if !k.tenant.is_kernel() && !names.contains(&k.tenant) {
            out.insert(Refusal::TenantAbsent {
                referrer: Referrer::Crypto(k.tenant.clone()),
                tenant: k.tenant.clone(),
            });
        }
    }
    for e in &intent.extras {
        if !e.tenant.is_kernel() && !names.contains(&e.tenant) {
            out.insert(Refusal::TenantAbsent {
                referrer: Referrer::Extra(e.tenant.clone()),
                tenant: e.tenant.clone(),
            });
        }
    }
}

pub(super) fn sizing_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
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

pub(super) fn unpriced_dataplane_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for c in &intent.tenants {
        if !has_pricing(c) {
            out.insert(Refusal::UnpricedDataplane {
                tenant: c.name.clone(),
                dataplane: c.dataplane,
            });
        }
    }
}

// Restrictedness is read off the `Restricted { pool }` payload, not a `""`
// sentinel: the two contradiction shapes (a pool on a non-restricted tenant, a
// restricted tenant with no pool) are unrepresentable in [`Isolation`], so only
// the holder-relationship refusals — which need cross-tenant lookups a type
// cannot carry — remain here. The TOML surface still names both contradictions
// (`crates/dpaa2-config/src/parse.rs` `convert_tenant`).
pub(super) fn pool_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
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

/// The intent declares a tenant named `kernel` that is not the reserved kernel
/// (`refuse.qnt` `kernelDeclaredRefusals`). The compile-side twin of the parse
/// reserved-name check (`crates/dpaa2-config/src/parse.rs` `convert`). The reserved
/// [`kernel_tenant`] is materialised into the tenant list by the frontend and derive
/// and must compile, so only a kernel-named tenant of a non-reserved shape is the
/// programmatic declaration the TOML boundary refuses. Deliberate config→api
/// duplication (design D11, 2026-09-06-intent-layer).
pub(super) fn kernel_declared_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    if intent
        .tenants
        .iter()
        .any(|t| t.name.is_kernel() && *t != kernel_tenant(t.max_cores))
    {
        out.insert(Refusal::KernelDeclared);
    }
}
