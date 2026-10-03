//! Imperative shell for DPAA2 provisioning: the convergence loop, status reporting,
//! and stable-naming `.link` generation that drive the pure core against a concrete
//! backend.
//!
//! The logic lives in a library so it can be exercised against the in-memory fake
//! backend with no hardware (design D10; restool-baseline); the `dpaa2ctl` binary is a thin CLI over
//! it.

pub mod engine;
pub mod link;
pub mod render;
pub mod status;

use dpaa2_api::intent::{Intent, kernel_tenant};

pub use engine::{
    ContainerOutcome, ConvergeConfig, Outcome, PoolDrift, PoolFamilyDrift, PoolOutcome,
    PopulationOutcome, apply, converge_containers, converge_pools, converge_population, ensure,
    observe, plan_containers, plan_pools, plan_population,
};
pub use status::{PortStatus, StatusReport};

/// Reserved-kernel completion (design D1; restool-baseline): the config parser never creates a kernel
/// [`dpaa2_api::intent::Tenant`] — a port with no tenant defaults to the reserved name — so the
/// frontend injects `kernel_tenant(cpus)` at index 0 when a port terminates the kernel
/// and no kernel tenant is declared. A link naming the kernel is materialised inside
/// `compile`'s `effective_tenants`, so this completes the port case only (the
/// `dpaa2-verify` `intent_pairing` normative note; bead gqf.19).
///
/// Lib-side so the one definition is shared: the frontend `compile_intent` pipeline and the
/// operand-pin suites consume this single site rather than hand-mirroring its body
/// (dpseci-hardening design D3).
pub fn complete_kernel(intent: &mut Intent, cpus: u32) {
    let declared = intent.tenants.iter().any(|t| t.name.is_kernel());
    let port_names_kernel = intent.ports.iter().any(|p| p.tenant.is_kernel());
    if port_names_kernel && !declared {
        intent.tenants.insert(0, kernel_tenant(i64::from(cpus)));
    }
}
