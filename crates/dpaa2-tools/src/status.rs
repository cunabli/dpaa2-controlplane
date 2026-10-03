//! Status reporting: per-port lifecycle and the desired-vs-actual delta.
//!
//! `status` is a first-class surface (proposal): it prints each managed object's
//! lifecycle state and whether the system has diverged from desired, and the caller
//! exits non-zero on divergence.

use core::fmt;

use dpaa2_api::contract::{KernelControl, McControl};
use dpaa2_api::core::error::Error;
use dpaa2_api::core::model::{DesiredTopology, DpmacId, Lifecycle, ObservedTopology};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpmac::{
    Arbitration, CarrierReading, CounterReadout, MacRelation, carrier_source, judge_arbitration,
    judge_mac_relation_observed, peer_observation_from_root,
};
use dpaa2_api::plan::Plan;
use dpaa2_api::plan::reconcile::reconcile;

/// The status of one managed port.
#[derive(Clone, Debug)]
pub struct PortStatus {
    /// The port's stable DPMAC anchor.
    pub dpmac: DpmacId,
    /// The configured stable name (the dpaa2-api newtype; rendered to a string only at
    /// the print site).
    pub name: ConstructName,
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
                    lifecycle: matched.map_or(
                        Lifecycle::Absent,
                        dpaa2_api::core::model::ObservedDpni::lifecycle,
                    ),
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

/// One port's read-only detail surface (provisioning-cli spec "The CLI exposes a read-only
/// port-detail view"): the four observed facets an operator inspects. Display-only by
/// construct — [`port_details`] computes it after `reconcile`, and no field here is ever an
/// input to a plan, drift, or assertion (link readings lag PHY reality per V-LINK-2;
/// counters are traffic-dependent).
#[derive(Clone, Debug)]
pub struct PortDetail {
    /// The port's stable DPMAC anchor.
    pub dpmac: DpmacId,
    /// The configured stable name (rendered to a string only at the print site).
    pub name: ConstructName,
    /// Who owns the port, judged from the observed peer ([`judge_arbitration`]).
    pub arbitration: Arbitration,
    /// How the connected dpni's primary MAC relates to the port ([`judge_mac_relation_observed`]).
    pub mac_relation: MacRelation,
    /// The link carrier, judged display-only from the read-back ([`CarrierReading`]).
    pub carrier: CarrierReading,
    /// The vocabulary-checked counter read-back (DPMAC-I7; dpmac-typestate design D4).
    pub counters: CounterReadout,
}

/// Computes the read-only per-port detail surface from board read-backs (provisioning-cli
/// spec). For each declared port it reads the dpmac observation
/// ([`McControl::observe_dpmac`]), judges arbitration from the observed peer dpni
/// ([`judge_arbitration`]), judges the MAC relation ([`judge_mac_relation_observed`]) from the
/// peer's primary MAC against the port's burned-in MAC and any intent-declared value, and reads the
/// carrier of the device the arbitration mapping picks ([`carrier_source`] +
/// [`KernelControl::carrier`]), judging it into a [`CarrierReading`]. Every facet is sourced
/// from a read-back; none re-derives a judgment.
///
/// Display-only by construct: this runs after `reconcile` and feeds only the detail render —
/// it is never an input to the plan the exit code gates on.
///
/// Cross-container peers are not derivable from the root topology, so a dpmac with no root
/// peer reads as `Offered` here; a same-container kernel peer reads `KernelOwned`. That is
/// the kernel-regime port the spec scenario covers; `RemoteOwned` needs a cross-container
/// observation this root-scoped read does not carry.
///
/// # Errors
/// Propagates a backend read failure ([`McControl::observe_dpmac`] or
/// [`KernelControl::carrier`]).
pub fn port_details<M: McControl, K: KernelControl>(
    mc: &M,
    kernel: &K,
    desired: &DesiredTopology,
    observed: &ObservedTopology,
) -> Result<Vec<PortDetail>, Error> {
    desired
        .ports()
        .iter()
        .map(|p| {
            let obs = mc.observe_dpmac(p.dpmac)?;
            let peer = observed.dpni_connected_to(p.dpmac);
            let arbitration = judge_arbitration(peer_observation_from_root(peer));
            let mac_relation =
                judge_mac_relation_observed(peer.and_then(|d| d.mac), obs.mac, p.mac);
            let source = carrier_source(arbitration, p.dpmac, peer.map(|d| d.id));
            let carrier = CarrierReading::from(kernel.carrier(source)?);
            Ok(PortDetail {
                dpmac: p.dpmac,
                name: p.name.clone(),
                arbitration,
                mac_relation,
                carrier,
                counters: obs.counters,
            })
        })
        .collect()
}

impl fmt::Display for StatusReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for p in &self.ports {
            let netdev = p.netdev.as_deref().unwrap_or("-");
            writeln!(
                f,
                "{dpmac:>10}  {name:<12}  {lifecycle:<10}  netdev={netdev}",
                dpmac = p.dpmac.to_string(),
                name = p.name.as_str(),
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
