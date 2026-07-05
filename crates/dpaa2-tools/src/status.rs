//! Status reporting: per-port lifecycle and the desired-vs-actual delta.
//!
//! `status` is a first-class surface (proposal): it prints each managed object's
//! lifecycle state and whether the system has diverged from desired, and the caller
//! exits non-zero on divergence.

use core::fmt;

use dpaa2_api::{DesiredTopology, DpmacId, Lifecycle, ObservedTopology, Plan, reconcile};

/// The status of one managed port.
#[derive(Clone, Debug)]
pub struct PortStatus {
    /// The port's stable DPMAC anchor.
    pub dpmac: DpmacId,
    /// The configured stable name.
    pub name: String,
    /// The lifecycle of the matched DPNI, or `Absent` if none is connected.
    pub lifecycle: Lifecycle,
    /// The current (pre-rename) netdev, if any.
    pub netdev: Option<String>,
}

/// A full status report for the managed subgraph.
#[derive(Clone, Debug)]
pub struct StatusReport {
    /// One entry per configured port.
    pub ports: Vec<PortStatus>,
    /// The delta that reconcile would still act on (empty when converged).
    pub plan: Plan,
}

impl StatusReport {
    /// Computes status by matching desired ports against observed state.
    #[must_use]
    pub fn compute(desired: &DesiredTopology, observed: &ObservedTopology) -> Self {
        let ports = desired
            .ports()
            .iter()
            .map(|p| {
                let matched = observed.dpni_connected_to(p.dpmac);
                PortStatus {
                    dpmac: p.dpmac,
                    name: p.name.clone(),
                    lifecycle: matched
                        .map_or(Lifecycle::Absent, dpaa2_api::ObservedDpni::lifecycle),
                    netdev: matched.and_then(|d| d.netdev.clone()),
                }
            })
            .collect();
        let plan = reconcile(desired, observed);
        Self { ports, plan }
    }

    /// Returns `true` when the system has diverged from desired: either work
    /// remains, or drift / an assert mismatch was reported.
    #[must_use]
    pub fn has_diverged(&self) -> bool {
        !self.plan.is_converged() || self.plan.has_divergence()
    }
}

impl fmt::Display for StatusReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for p in &self.ports {
            let netdev = p.netdev.as_deref().unwrap_or("-");
            writeln!(
                f,
                "{dpmac:>10}  {name:<12}  {lifecycle:<10}  netdev={netdev}",
                dpmac = p.dpmac.to_string(),
                name = p.name,
                lifecycle = format!("{:?}", p.lifecycle),
            )?;
        }
        if self.plan.is_converged() && !self.plan.has_divergence() {
            writeln!(f, "state: converged")?;
        } else {
            writeln!(
                f,
                "state: diverged ({} pending, {} drift, {} assert-mismatch)",
                self.plan.transitions.len(),
                self.plan.drift.len(),
                self.plan.assertions.len(),
            )?;
        }
        Ok(())
    }
}
