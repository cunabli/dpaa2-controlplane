//! The pure reconciliation core (design D0; add-dpaa2-provisioning).
//!
//! [`reconcile`] is a total, deterministic function that performs no I/O: given the
//! operator's [`DesiredTopology`] and one [`ObservedTopology`] snapshot, it computes
//! an ordered [`Plan`] that moves observed toward desired. Because it is pure it is
//! exhaustively unit-testable against the in-memory fake backend (design D10; restool-baseline) with
//! zero hardware.
//!
//! Matching is **edge-based** (design D1; restool-baseline): a managed DPNI is identified by its
//! connection to a configured DPMAC, never by index, so a renumbered DPNI still
//! matches. Ownership is implicit (design D7; ADR-0005): the function only ever iterates the
//! configured ports, so foreign objects are never enumerated, let alone deleted.

use std::collections::BTreeSet;

use crate::core::model::{
    DesiredPort, DesiredTopology, DpniId, Lifecycle, LinkType, MacAddr, MacMode, ObservedTopology,
    Presence,
};
use crate::families::dpni::{DpniCfg, DpniDisposition, DpniObservation, drift_disposition};
use crate::plan::{AssertMismatch, Plan, RebuildRefusal, Transition};

/// Options controlling reconciliation policy.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ReconcileOptions {
    /// When set, ports declared [`Presence::Absent`] are torn down. Default off:
    /// a removed port is left in place (design D7; ADR-0005).
    pub prune: bool,
}

/// Computes the plan to converge `observed` toward `desired` with default policy
/// (no pruning).
#[must_use]
pub fn reconcile(desired: &DesiredTopology, observed: &ObservedTopology) -> Plan {
    reconcile_with(
        desired,
        observed,
        ReconcileOptions::default(),
        &BTreeSet::new(),
    )
}

/// Computes the plan to converge `observed` toward `desired` under `options`.
///
/// `run_created` names the dpnis this convergence run created: a create-immutable drift
/// on one of them is a same-run rebuild, which the converge loop refuses instead of
/// churning hardware (pool-objects design D12; ADR-0008 §9). An empty set is the standing
/// behavior — every drift disposes normally.
#[must_use]
pub fn reconcile_with(
    desired: &DesiredTopology,
    observed: &ObservedTopology,
    options: ReconcileOptions,
    run_created: &BTreeSet<DpniId>,
) -> Plan {
    let mut plan = Plan::new();

    // Report every derived object the port facet has no executor for, by family
    // (design D10; restool-baseline): reconcile actuates the dpni↔dpmac port subset below and reports
    // the rest as plan-only, never as drift or error.
    plan.plan_only = desired.plan().plan_only_by_family();

    for port in desired.ports() {
        match port.presence {
            Presence::Present => {
                // The compiled block, carried verbatim into Create (dpni-typestate design D3);
                // no block ⇒ `DpniCfg::defaults()` (num_queues 0, prior `unwrap_or(0)`).
                let cfg = desired
                    .plan()
                    .port_dpni_cfg(port.dpmac)
                    .unwrap_or_else(DpniCfg::defaults);
                plan_present(port, observed, cfg, run_created, &mut plan);
            }
            Presence::Absent => plan_absent(port, observed, options, &mut plan),
        }
    }

    plan
}

/// Plans convergence for a port the operator wants present.
fn plan_present(
    port: &DesiredPort,
    observed: &ObservedTopology,
    cfg: DpniCfg,
    run_created: &BTreeSet<DpniId>,
    plan: &mut Plan,
) {
    let link_type = observed
        .dpmac(port.dpmac)
        .map_or(LinkType::Phy, |m| m.link_type);
    let needs_netdev = link_type == LinkType::Phy;

    let Some(dpni) = observed.dpni_connected_to(port.dpmac) else {
        plan_create(port, cfg, needs_netdev, plan);
        return;
    };

    // Typed cfg drift on the read-back projection (ADR-0001 §4; not the legacy attribute
    // map; MAC held equal to isolate cfg). An unsized block (`num_queues` 0) has no sizing
    // intent, so it is fenced by construct — its 0 sentinel never reads back (7fv.2, row 2).
    if cfg.num_queues.get() != 0
        && let Some(observed_cfg) = dpni.cfg_observation.as_ref()
    {
        let mac = dpni.mac.unwrap_or(MacAddr::ZERO);
        if drift_disposition(&cfg, mac, observed_cfg, mac) == DpniDisposition::DestroyThenCreate {
            // A rebuild of a dpni this run created would churn live hardware into the
            // phylink crash; refuse it, naming the diverging fields (pool-objects design
            // D12; ADR-0008 §9). The refusal actuates nothing for this port.
            if run_created.contains(&dpni.id) {
                plan.refusals.push(RebuildRefusal {
                    port: port.dpmac,
                    dpni: dpni.id,
                    diff: DpniObservation::project(&cfg).diff(observed_cfg),
                });
                return;
            }
            // Standing object: sever the edge while still bound, then unbind, then destroy
            // (ADR-0008 §8 — unbind-first strands the dpmac).
            plan.transitions
                .push(Transition::Disconnect { dpni: dpni.id });
            if dpni.netdev.is_some() {
                plan.transitions.push(Transition::Unbind { dpni: dpni.id });
            }
            plan.transitions.push(Transition::Destroy { dpni: dpni.id });
            plan_create(port, cfg, needs_netdev, plan);
            return;
        }
    }

    // Label repair (ADR-0015 decision 9): a port dpni whose observed label differs from
    // its construct name is relabelled with `set-label`, decision 9's repair verb, never
    // a destroy/create. A freshly created dpni (the Absent branch above) already carries
    // the name — the create stamps it (ADR-0010 §4 ABA guard) — so this emission is the
    // drift/rename repair for a standing object whose label was lost or changed.
    if dpni.label.as_ref() != Some(&port.name) {
        plan.transitions.push(Transition::SetLabel {
            dpni: dpni.id,
            label: port.name.clone(),
        });
    }

    // MAC: actuate on mismatch, or assert-and-report (design D9; ADR-0006).
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

/// Emits the create sequence, shared by the absent branch and the cfg-drift rebuild (ADR-0001 §4).
fn plan_create(port: &DesiredPort, cfg: DpniCfg, needs_netdev: bool, plan: &mut Plan) {
    plan.transitions.push(Transition::Create {
        port: port.dpmac,
        label: port.name.clone(),
        cfg,
    });
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
    // Sever while bound, then unbind, then destroy (ADR-0008 §8): the disconnect
    // re-attaches the standalone MAC driver before the unbind, so the port keeps a driver.
    plan.transitions
        .push(Transition::Disconnect { dpni: dpni.id });
    if dpni.netdev.is_some() {
        plan.transitions.push(Transition::Unbind { dpni: dpni.id });
    }
    plan.transitions.push(Transition::Destroy { dpni: dpni.id });
}

#[cfg(test)]
mod tests {
    //! Engine unit tests, run against the neutral model and the in-memory fake (D10; restool-baseline).

    use std::collections::{BTreeMap, BTreeSet};

    use crate::contract::McControl;
    use crate::contract::fake::FakeBackend;
    use crate::core::model::{
        DesiredPort, DesiredTopology, DpmacId, DpniId, Lifecycle, LinkType, MacAddr, MacMode,
        ObservedDpmac, ObservedDpni, ObservedTopology, Presence,
    };
    use crate::families::dpni::{DpniCfg, DpniObservation};
    use crate::plan::Transition;
    use crate::plan::reconcile::{ReconcileOptions, reconcile, reconcile_with};

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
        labelled(id, connected, mac, netdev, None)
    }

    /// A dpni carrying its construct-name label, so a converged fixture does not draw a
    /// spurious relabel (ADR-0015 decision 9): the label a level-triggered board shows
    /// once its dpnis have been stamped.
    fn labelled(
        id: u32,
        connected: Option<u32>,
        mac: Option<MacAddr>,
        netdev: Option<&str>,
        label: Option<&str>,
    ) -> ObservedDpni {
        ObservedDpni {
            id: DpniId::new(id),
            label: label.map(Into::into),
            connected_to: connected.map(DpmacId::new),
            mac,
            netdev: netdev.map(str::to_owned),
            attributes: BTreeMap::new(),
            cfg_observation: None,
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
                // Port-only projection ⇒ a bare MC-default block (ls-addni parity).
                Transition::Create {
                    port: DpmacId::new(3),
                    label: "wan0".into(),
                    cfg: DpniCfg::defaults(),
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
                    port: DpmacId::new(3),
                    label: "wan0".into(),
                    cfg: DpniCfg::defaults(),
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
    fn create_carries_the_compiled_cfg() {
        // A sized compiled plan carries its whole block into the Create (dpni-typestate task 4.1).
        use crate::intent::compiled::CompiledPlan;
        use crate::intent::kernel_tenant;

        let kernel = kernel_tenant(5);
        let (obj, iface) = kernel.dpni(1, 5, "wan0".into());
        let mut compiled = CompiledPlan::default();
        compiled.order.push(obj.key().clone());
        compiled.objects.insert(obj);
        compiled.edges.insert(iface.into_port_edge(DpmacId::new(3)));

        let desired =
            DesiredTopology::from_parts(compiled, vec![DesiredPort::new(DpmacId::new(3), "wan0")])
                .expect("plan port-edge and port agree on dpmac.3");
        let want_cfg = desired
            .plan()
            .port_dpni_cfg(DpmacId::new(3))
            .expect("compiled port dpni cfg");
        assert_eq!(want_cfg.num_queues.get(), 5, "sized at 5 by the compiler");
        let observed = ObservedTopology {
            dpnis: vec![],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&desired, &observed);
        assert_eq!(
            plan.transitions[0],
            Transition::Create {
                port: DpmacId::new(3),
                label: "wan0".into(),
                cfg: want_cfg,
            }
        );
    }

    #[test]
    fn converged_state_is_idempotent() {
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![labelled(
                7,
                Some(3),
                Some(MAC_3),
                Some("eth7"),
                Some("wan0"),
            )],
            dpmacs: vec![phy(3, MAC_3)],
        };
        assert!(reconcile(&desired, &observed).is_converged());
    }

    #[test]
    fn renumbered_dpni_still_matches_by_edge() {
        // Same DPMAC edge, different DPNI index -> no change planned.
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![labelled(
                42,
                Some(3),
                Some(MAC_3),
                Some("eth42"),
                Some("wan0"),
            )],
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
                labelled(7, Some(3), Some(MAC_3), Some("eth7"), Some("wan0")),
                dpni(9, Some(17), None, Some("eth9")), // dpmac.17 mgmt, not in desired
            ],
            dpmacs: vec![phy(3, MAC_3), phy(17, MacAddr::new([0, 0, 0, 0, 0, 17]))],
        };
        let plan = reconcile(&desired, &observed);
        assert!(plan.is_converged(), "foreign dpni.9 must not be touched");
    }

    /// An `ObservedDpni` carrying the read-back projection of `cfg` (the typed `cfg_observation`).
    fn observed_with_cfg(
        id: u32,
        connected: Option<u32>,
        mac: Option<MacAddr>,
        netdev: Option<&str>,
        label: Option<&str>,
        cfg: &DpniCfg,
    ) -> ObservedDpni {
        ObservedDpni {
            cfg_observation: Some(crate::families::dpni::DpniObservation::project(cfg)),
            ..labelled(id, connected, mac, netdev, label)
        }
    }

    /// A sized create block differing from another only in `num_queues`.
    fn sized_cfg(num_queues: u16) -> DpniCfg {
        DpniCfg {
            num_queues: crate::families::dpni::NumQueues::new(num_queues).unwrap(),
            ..DpniCfg::defaults()
        }
    }

    /// A sized (`num_queues` = `n`) desired topology anchored at dpmac.3 via the compiled path.
    fn sized_desired(n: u32) -> DesiredTopology {
        use crate::intent::compiled::CompiledPlan;
        use crate::intent::kernel_tenant;

        let kernel = kernel_tenant(i64::from(n));
        let (obj, iface) = kernel.dpni(1, n, "wan0".into());
        let mut compiled = CompiledPlan::default();
        compiled.order.push(obj.key().clone());
        compiled.objects.insert(obj);
        compiled.edges.insert(iface.into_port_edge(DpmacId::new(3)));
        DesiredTopology::from_parts(compiled, vec![DesiredPort::new(DpmacId::new(3), "wan0")])
            .expect("plan port-edge and port agree on dpmac.3")
    }

    #[test]
    fn cfg_drift_plans_destroy_then_create() {
        // Spec "Cfg drift plans destroy-and-create": a differing read-back is destroy + create.
        let backend = FakeBackend::new().with_dpmac(DpmacId::new(3), LinkType::Phy, MAC_3);
        let id = backend
            .create_dpni(&"wan0".into(), &sized_cfg(4))
            .expect("create observed-side dpni");
        backend.connect(id, DpmacId::new(3)).expect("connect");
        let observed = backend.observe().expect("observe");

        let plan = reconcile(&sized_desired(8), &observed);
        assert!(
            plan.transitions
                .iter()
                .any(|t| matches!(t, Transition::Destroy { .. })),
            "cfg drift destroys the drifted object: {:?}",
            plan.transitions
        );
        assert!(
            plan.transitions
                .iter()
                .any(|t| matches!(t, Transition::Create { .. })),
            "cfg drift recreates from the desired block: {:?}",
            plan.transitions
        );
        assert!(plan.drift.is_empty(), "drift is planned, not reported");
    }

    #[test]
    fn cfg_drift_suppresses_the_relabel() {
        // The cfg-drift rebuild wins over a stale-label repair: destroy + create, no SetLabel.
        let observed = ObservedTopology {
            dpnis: vec![observed_with_cfg(
                7,
                Some(3),
                Some(MAC_3),
                Some("eth7"),
                Some("stale"),
                &sized_cfg(4),
            )],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&sized_desired(8), &observed);
        assert!(
            !plan
                .transitions
                .iter()
                .any(|t| matches!(t, Transition::SetLabel { .. })),
            "cfg drift suppresses the relabel: {:?}",
            plan.transitions
        );
        assert!(
            plan.transitions
                .iter()
                .any(|t| matches!(t, Transition::Destroy { .. })),
        );
    }

    #[test]
    fn unsized_port_only_dpni_does_not_false_drift() {
        // Spec "An unsized port-only dpni does not false-drift": re-observe converges, no loop.
        let backend = FakeBackend::new().with_dpmac(DpmacId::new(3), LinkType::Phy, MAC_3);
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let id = backend
            .create_dpni(&"wan0".into(), &DpniCfg::defaults())
            .expect("create port-only dpni");
        backend.connect(id, DpmacId::new(3)).expect("connect");
        let observed = backend.observe().expect("observe");

        let plan = reconcile(&desired, &observed);
        assert!(
            !plan
                .transitions
                .iter()
                .any(|t| matches!(t, Transition::Destroy { .. } | Transition::Create { .. })),
            "unsized create must not re-destroy/recreate: {:?}",
            plan.transitions
        );
        assert!(plan.is_converged(), "unsized create converges");
    }

    #[test]
    fn assert_only_mac_mismatch_is_reported_not_actuated() {
        let mut port = DesiredPort::new(DpmacId::new(3), "wan0");
        port.mac = Some(MAC_3);
        port.mac_mode = MacMode::Assert;
        let desired = DesiredTopology::from_ports([port]);
        let observed = ObservedTopology {
            dpnis: vec![labelled(
                7,
                Some(3),
                Some(MacAddr::new([9, 9, 9, 9, 9, 9])),
                Some("eth7"),
                Some("wan0"),
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
            dpnis: vec![labelled(
                7,
                Some(3),
                Some(MacAddr::new([9, 9, 9, 9, 9, 9])),
                Some("eth7"),
                Some("wan0"),
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

        // With prune -> disconnect, unbind, destroy in order (ADR-0008 §8: sever first).
        let plan = reconcile_with(
            &desired,
            &observed,
            ReconcileOptions { prune: true },
            &BTreeSet::new(),
        );
        assert_eq!(
            plan.transitions,
            vec![
                Transition::Disconnect {
                    dpni: DpniId::new(7)
                },
                Transition::Unbind {
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
            dpnis: vec![labelled(7, Some(3), Some(MAC_3), None, Some("wan0"))],
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
    fn stale_label_yields_one_set_label() {
        // A connected port dpni whose label differs from the port name draws exactly one
        // SetLabel with the port name (ADR-0015 decision 9), no destroy/create.
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![labelled(
                7,
                Some(3),
                Some(MAC_3),
                Some("eth7"),
                Some("stale"),
            )],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&desired, &observed);
        assert_eq!(
            plan.transitions,
            vec![Transition::SetLabel {
                dpni: DpniId::new(7),
                label: "wan0".into(),
            }]
        );
    }

    #[test]
    fn absent_label_yields_one_set_label() {
        // An empty label column reads the same as drift: one SetLabel repairs it.
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![labelled(7, Some(3), Some(MAC_3), Some("eth7"), None)],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&desired, &observed);
        assert_eq!(
            plan.transitions,
            vec![Transition::SetLabel {
                dpni: DpniId::new(7),
                label: "wan0".into(),
            }]
        );
    }

    #[test]
    fn matching_label_yields_no_set_label() {
        // A label already equal to the port name is idempotent — no relabel.
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![labelled(
                7,
                Some(3),
                Some(MAC_3),
                Some("eth7"),
                Some("wan0"),
            )],
            dpmacs: vec![phy(3, MAC_3)],
        };
        assert!(reconcile(&desired, &observed).is_converged());
    }

    #[test]
    fn set_label_only_plan_headlines_boundary() {
        // A relabel-only plan headlines boundary — the conservative transition-level
        // class (ADR-0015 decision 12; plan.rs classing rationale).
        let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
        let observed = ObservedTopology {
            dpnis: vec![labelled(
                7,
                Some(3),
                Some(MAC_3),
                Some("eth7"),
                Some("stale"),
            )],
            dpmacs: vec![phy(3, MAC_3)],
        };
        let plan = reconcile(&desired, &observed);
        assert_eq!(plan.headline(), crate::plan::Class::Boundary);
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
                    Transition::Create { label, cfg, .. } => {
                        let id = backend.create_dpni(label, cfg).unwrap();
                        backend.connect(id, DpmacId::new(3)).unwrap();
                    }
                    Transition::Connect { .. } | Transition::Bind { .. } => {}
                    // The create stamps the label (ADR-0010 §4 ABA guard), so a
                    // converged board never emits this; apply it if drift ever does.
                    Transition::SetLabel { dpni, label } => {
                        backend.set_label(*dpni, label).unwrap();
                    }
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

    /// A drifted, run-created port dpni whose observation projects `sized_cfg(4)` against
    /// a `sized_desired(8)` intent — the same-run rebuild the loop-breaker refuses.
    fn drifted_run_created() -> (DesiredTopology, ObservedTopology, BTreeSet<DpniId>) {
        let observed = ObservedTopology {
            dpnis: vec![observed_with_cfg(
                7,
                Some(3),
                Some(MAC_3),
                Some("eth7"),
                Some("wan0"),
                &sized_cfg(4),
            )],
            dpmacs: vec![phy(3, MAC_3)],
        };
        (sized_desired(8), observed, BTreeSet::from([DpniId::new(7)]))
    }

    #[test]
    fn run_created_drift_is_refusal_not_churn() {
        // pool-objects design D12: a same-run rebuild becomes a typed refusal naming the
        // field, and emits no destroy/create for the port.
        let (desired, observed, run_created) = drifted_run_created();
        let plan = reconcile_with(
            &desired,
            &observed,
            ReconcileOptions::default(),
            &run_created,
        );
        assert!(
            !plan
                .transitions
                .iter()
                .any(|t| matches!(t, Transition::Destroy { .. } | Transition::Create { .. })),
            "no hardware churn: {:?}",
            plan.transitions
        );
        assert_eq!(plan.refusals.len(), 1);
        assert_eq!(plan.refusals[0].dpni, DpniId::new(7));
        assert_eq!(plan.refusals[0].port, DpmacId::new(3));
        assert!(
            plan.refusals[0]
                .diff
                .iter()
                .any(|d| d.field == "num_queues"),
            "diff names num_queues: {:?}",
            plan.refusals[0].diff
        );
    }

    #[test]
    fn refusal_is_not_converged() {
        let (desired, observed, run_created) = drifted_run_created();
        let plan = reconcile_with(
            &desired,
            &observed,
            ReconcileOptions::default(),
            &run_created,
        );
        assert!(!plan.is_converged(), "a refusal leaves the board diverged");
    }

    #[test]
    fn standing_drift_tears_down_sever_first_then_creates() {
        // The identical drift on a standing (not run-created) dpni still rebuilds, in the
        // ADR-0008 §8 order: disconnect while bound, then unbind, then destroy, then create.
        let (desired, observed, _) = drifted_run_created();
        let plan = reconcile_with(
            &desired,
            &observed,
            ReconcileOptions::default(),
            &BTreeSet::new(),
        );
        assert_eq!(
            &plan.transitions[..3],
            &[
                Transition::Disconnect {
                    dpni: DpniId::new(7)
                },
                Transition::Unbind {
                    dpni: DpniId::new(7)
                },
                Transition::Destroy {
                    dpni: DpniId::new(7)
                },
            ]
        );
        assert!(
            matches!(plan.transitions[3], Transition::Create { .. }),
            "rebuild recreates after the teardown: {:?}",
            plan.transitions
        );
        assert!(plan.refusals.is_empty(), "standing drift is not a refusal");
    }

    #[test]
    fn observation_diff_names_the_mutated_field_with_both_values() {
        let desired = DpniObservation::project(&sized_cfg(8));
        let observed = DpniObservation::project(&sized_cfg(4));
        let diff = desired.diff(&observed);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].field, "num_queues");
        assert!(
            diff[0].desired.contains('8'),
            "desired: {}",
            diff[0].desired
        );
        assert!(
            diff[0].observed.contains('4'),
            "observed: {}",
            diff[0].observed
        );
    }
}
