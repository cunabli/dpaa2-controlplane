//! The pure reconciliation core (design D0).
//!
//! [`reconcile`] is a total, deterministic function that performs no I/O: given the
//! operator's [`DesiredTopology`] and one [`ObservedTopology`] snapshot, it computes
//! an ordered [`Plan`] that moves observed toward desired. Because it is pure it is
//! exhaustively unit-testable against the in-memory fake backend (design D10) with
//! zero hardware.
//!
//! Matching is **edge-based** (design D1): a managed DPNI is identified by its
//! connection to a configured DPMAC, never by index, so a renumbered DPNI still
//! matches. Ownership is implicit (design D7): the function only ever iterates the
//! configured ports, so foreign objects are never enumerated, let alone deleted.

use crate::model::{
    DesiredPort, DesiredTopology, Lifecycle, LinkType, MacMode, ObservedTopology, Presence,
};
use crate::plan::{AssertMismatch, DriftReport, Plan, Transition};

/// Options controlling reconciliation policy.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ReconcileOptions {
    /// When set, ports declared [`Presence::Absent`] are torn down. Default off:
    /// a removed port is left in place (design D7).
    pub prune: bool,
}

/// Computes the plan to converge `observed` toward `desired` with default policy
/// (no pruning).
#[must_use]
pub fn reconcile(desired: &DesiredTopology, observed: &ObservedTopology) -> Plan {
    reconcile_with(desired, observed, ReconcileOptions::default())
}

/// Computes the plan to converge `observed` toward `desired` under `options`.
#[must_use]
pub fn reconcile_with(
    desired: &DesiredTopology,
    observed: &ObservedTopology,
    options: ReconcileOptions,
) -> Plan {
    let mut plan = Plan::new();

    for port in desired.ports() {
        match port.presence {
            Presence::Present => plan_present(port, observed, &mut plan),
            Presence::Absent => plan_absent(port, observed, options, &mut plan),
        }
    }

    plan
}

/// Plans convergence for a port the operator wants present.
fn plan_present(port: &DesiredPort, observed: &ObservedTopology, plan: &mut Plan) {
    let link_type = observed
        .dpmac(port.dpmac)
        .map_or(LinkType::Phy, |m| m.link_type);
    let needs_netdev = link_type == LinkType::Phy;

    let Some(dpni) = observed.dpni_connected_to(port.dpmac) else {
        // Absent -> Create, (optionally set MAC), Connect, and wait-to-bind.
        plan.transitions
            .push(Transition::Create { port: port.dpmac });
        if port.mac_mode == MacMode::Actuate
            && let Some(mac) = port.mac
        {
            plan.transitions.push(Transition::SetMac {
                port: port.dpmac,
                mac,
            });
        }
        plan.transitions
            .push(Transition::Connect { port: port.dpmac });
        if needs_netdev {
            plan.transitions.push(Transition::Bind { port: port.dpmac });
        }
        return;
    };

    // A DPNI is already connected to this DPMAC. Refuse immutable drift before
    // planning any further mutation of the live object (design D8).
    let mut drifted = false;
    for (attr, want) in &port.immutable {
        let got = dpni.attributes.get(attr);
        if got.map(String::as_str) != Some(want.as_str()) {
            plan.drift.push(DriftReport {
                dpni: dpni.id,
                attribute: attr.clone(),
                detail: format!(
                    "desired {attr}={want}, observed {}",
                    got.map_or("<absent>", String::as_str)
                ),
            });
            drifted = true;
        }
    }
    if drifted {
        return;
    }

    // MAC: actuate on mismatch, or assert-and-report (design D9).
    if let Some(mac) = port.mac {
        match port.mac_mode {
            MacMode::Actuate if dpni.mac != Some(mac) => {
                plan.transitions.push(Transition::SetMac {
                    port: port.dpmac,
                    mac,
                });
            }
            MacMode::Assert if dpni.mac.is_some() && dpni.mac != Some(mac) => {
                plan.assertions.push(AssertMismatch {
                    port: port.dpmac,
                    field: "mac".to_owned(),
                    detail: format!(
                        "asserted {mac}, observed {}",
                        dpni.mac.expect("checked is_some")
                    ),
                });
            }
            _ => {}
        }
    }

    // Wait-to-bind: a PHY port that has not yet produced a netdev is not converged.
    if needs_netdev && dpni.lifecycle() != Lifecycle::Bound {
        plan.transitions.push(Transition::Bind { port: port.dpmac });
    }
}

/// Plans teardown for a port the operator wants absent (prune only).
fn plan_absent(
    port: &DesiredPort,
    observed: &ObservedTopology,
    options: ReconcileOptions,
    plan: &mut Plan,
) {
    let Some(dpni) = observed.dpni_connected_to(port.dpmac) else {
        return;
    };
    if !options.prune {
        return;
    }
    if dpni.netdev.is_some() {
        plan.transitions.push(Transition::Unbind { dpni: dpni.id });
    }
    plan.transitions
        .push(Transition::Disconnect { dpni: dpni.id });
    plan.transitions.push(Transition::Destroy { dpni: dpni.id });
}

#[cfg(test)]
mod tests {
    //! Engine unit tests, run against the neutral model and the in-memory fake (D10).

    use std::collections::BTreeMap;

    use crate::fake::FakeBackend;
    use crate::model::{
        DesiredPort, DesiredTopology, DpmacId, DpniId, Lifecycle, LinkType, MacAddr, MacMode,
        ObservedDpmac, ObservedDpni, ObservedTopology, Presence,
    };
    use crate::plan::Transition;
    use crate::port::McControl;
    use crate::reconcile::{ReconcileOptions, reconcile, reconcile_with};

    const MAC_3: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x03]);

    fn phy(id: u32, mac: MacAddr) -> ObservedDpmac {
        ObservedDpmac {
            id: DpmacId::new(id),
            link_type: LinkType::Phy,
            mac: Some(mac),
        }
    }

    fn dpni(
        id: u32,
        connected: Option<u32>,
        mac: Option<MacAddr>,
        netdev: Option<&str>,
    ) -> ObservedDpni {
        ObservedDpni {
            id: DpniId::new(id),
            connected_to: connected.map(DpmacId::new),
            mac,
            netdev: netdev.map(str::to_owned),
            attributes: BTreeMap::new(),
        }
    }

    #[test]
    fn lifecycle_is_connected_when_bound_but_no_netdev() {
        let d = dpni(7, Some(3), Some(MAC_3), None);
        assert_eq!(d.lifecycle(), Lifecycle::Connected);
        let bound = dpni(7, Some(3), Some(MAC_3), Some("eth7"));
        assert_eq!(bound.lifecycle(), Lifecycle::Bound);
    }

    #[test]
    fn absent_port_yields_create_then_connect_then_bind() {
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&desired, &observed);
        assert_eq!(
            plan.transitions,
            vec![
                Transition::Create {
                    port: DpmacId::new(3)
                },
                Transition::Connect {
                    port: DpmacId::new(3)
                },
                Transition::Bind {
                    port: DpmacId::new(3)
                },
            ]
        );
    }

    #[test]
    fn absent_port_with_actuate_mac_yields_create_then_set_mac_then_connect_then_bind() {
        let mut port = DesiredPort::new(DpmacId::new(3), "wan0");
        port.mac = Some(MAC_3);
        port.mac_mode = MacMode::Actuate;
        let desired = DesiredTopology::from_ports([port]);
        let observed = ObservedTopology {
            dpnis: vec![],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&desired, &observed);
        assert_eq!(
            plan.transitions,
            vec![
                Transition::Create {
                    port: DpmacId::new(3)
                },
                Transition::SetMac {
                    port: DpmacId::new(3),
                    mac: MAC_3
                },
                Transition::Connect {
                    port: DpmacId::new(3)
                },
                Transition::Bind {
                    port: DpmacId::new(3)
                },
            ]
        );
    }

    #[test]
    fn converged_state_is_idempotent() {
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![dpni(7, Some(3), Some(MAC_3), Some("eth7"))],
            dpmacs: vec![phy(3, MAC_3)],
        };
        assert!(reconcile(&desired, &observed).is_converged());
    }

    #[test]
    fn renumbered_dpni_still_matches_by_edge() {
        // Same DPMAC edge, different DPNI index -> no change planned.
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![dpni(42, Some(3), Some(MAC_3), Some("eth42"))],
            dpmacs: vec![phy(3, MAC_3)],
        };
        assert!(reconcile(&desired, &observed).is_converged());
    }

    #[test]
    fn foreign_object_is_preserved() {
        // A DPNI connected to a DPMAC we do not manage is never touched.
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![
                dpni(7, Some(3), Some(MAC_3), Some("eth7")),
                dpni(9, Some(17), None, Some("eth9")), // dpmac.17 mgmt, not in desired
            ],
            dpmacs: vec![phy(3, MAC_3), phy(17, MacAddr::new([0, 0, 0, 0, 0, 17]))],
        };
        let plan = reconcile(&desired, &observed);
        assert!(plan.is_converged(), "foreign dpni.9 must not be touched");
    }

    #[test]
    fn immutable_drift_is_reported_and_refused() {
        let mut port = DesiredPort::new(DpmacId::new(3), "wan0");
        port.immutable.insert("num_tcs".to_owned(), "8".to_owned());
        let desired = DesiredTopology::from_ports([port]);

        let mut attrs = BTreeMap::new();
        attrs.insert("num_tcs".to_owned(), "1".to_owned()); // create-time mismatch
        let observed = ObservedTopology {
            dpnis: vec![ObservedDpni {
                attributes: attrs,
                ..dpni(7, Some(3), Some(MAC_3), Some("eth7"))
            }],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&desired, &observed);
        assert!(plan.transitions.is_empty(), "no destructive change");
        assert_eq!(plan.drift.len(), 1);
        assert_eq!(plan.drift[0].attribute, "num_tcs");
    }

    #[test]
    fn assert_only_mac_mismatch_is_reported_not_actuated() {
        let mut port = DesiredPort::new(DpmacId::new(3), "wan0");
        port.mac = Some(MAC_3);
        port.mac_mode = MacMode::Assert;
        let desired = DesiredTopology::from_ports([port]);
        let observed = ObservedTopology {
            dpnis: vec![dpni(
                7,
                Some(3),
                Some(MacAddr::new([9, 9, 9, 9, 9, 9])),
                Some("eth7"),
            )],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&desired, &observed);
        assert!(plan.transitions.is_empty());
        assert_eq!(plan.assertions.len(), 1);
        assert_eq!(plan.assertions[0].field, "mac");
    }

    #[test]
    fn actuate_mac_mismatch_plans_a_set_mac() {
        let mut port = DesiredPort::new(DpmacId::new(3), "wan0");
        port.mac = Some(MAC_3);
        port.mac_mode = MacMode::Actuate;
        let desired = DesiredTopology::from_ports([port]);
        let observed = ObservedTopology {
            dpnis: vec![dpni(
                7,
                Some(3),
                Some(MacAddr::new([9, 9, 9, 9, 9, 9])),
                Some("eth7"),
            )],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&desired, &observed);
        assert_eq!(
            plan.transitions,
            vec![Transition::SetMac {
                port: DpmacId::new(3),
                mac: MAC_3
            }]
        );
        assert!(plan.assertions.is_empty());
    }

    #[test]
    fn teardown_is_opt_in() {
        let mut port = DesiredPort::new(DpmacId::new(3), "wan0");
        port.presence = Presence::Absent;
        let desired = DesiredTopology::from_ports([port]);
        let observed = ObservedTopology {
            dpnis: vec![dpni(7, Some(3), Some(MAC_3), Some("eth7"))],
            dpmacs: vec![phy(3, MAC_3)],
        };

        // Default: no prune -> nothing destroyed.
        assert!(reconcile(&desired, &observed).is_converged());

        // With prune -> unbind, disconnect, destroy in order.
        let plan = reconcile_with(&desired, &observed, ReconcileOptions { prune: true });
        assert_eq!(
            plan.transitions,
            vec![
                Transition::Unbind {
                    dpni: DpniId::new(7)
                },
                Transition::Disconnect {
                    dpni: DpniId::new(7)
                },
                Transition::Destroy {
                    dpni: DpniId::new(7)
                },
            ]
        );
    }

    #[test]
    fn fixed_link_port_needs_no_bind() {
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![dpni(7, Some(3), Some(MAC_3), None)],
            dpmacs: vec![ObservedDpmac {
                id: DpmacId::new(3),
                link_type: LinkType::Fixed,
                mac: Some(MAC_3),
            }],
        };
        // Fixed link: connected == provisioned, no netdev, no Bind.
        assert!(reconcile(&desired, &observed).is_converged());
    }

    #[test]
    fn full_loop_converges_against_fake_backend() {
        // observe -> reconcile -> act -> re-observe with the fake MC (latency 1 tick).
        let backend = FakeBackend::new()
            .with_dpmac(DpmacId::new(3), LinkType::Phy, MAC_3)
            .with_bind_latency(1);
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);

        // Drive a bounded loop applying transitions until converged.
        let mut converged = false;
        for _ in 0..8 {
            let observed = backend.observe().unwrap();
            let plan = reconcile(&desired, &observed);
            if plan.is_converged() {
                converged = true;
                break;
            }
            for t in &plan.transitions {
                match t {
                    Transition::Create { .. } => {
                        let id = backend.create_dpni().unwrap();
                        backend.connect(id, DpmacId::new(3)).unwrap();
                    }
                    Transition::Connect { .. } | Transition::Bind { .. } => {}
                    other => panic!("unexpected transition {other:?}"),
                }
            }
        }
        assert!(converged, "loop must converge once netdev appears");
        assert_eq!(
            backend.netdev_for_dpmac(DpmacId::new(3)).as_deref(),
            Some("eth1")
        );
    }
}
