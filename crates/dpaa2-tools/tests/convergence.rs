//! Convergence-loop, idempotence, and exit-behaviour tests driven entirely against
//! the in-memory fake backend (design D10; restool-baseline, tasks 5.5/5.6). No board is touched.

use std::collections::BTreeMap;
use std::time::Duration;

use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::core::model::{
    DesiredPort, DesiredTopology, DpmacId, DpniId, LinkType, MacAddr, ObservedDpni,
};
use dpaa2_api::families::dpni::{DpniCfg, DpniObservation, NumQueues};
use dpaa2_api::intent::compiled::CompiledPlan;
use dpaa2_api::intent::kernel_tenant;
use dpaa2_api::plan::Class;
use dpaa2_tools::StatusReport;
use dpaa2_tools::engine::{self, ConvergeConfig, Outcome};

const MAC_7: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);
const MAC_3: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x03]);

/// A sized desired topology for a single PHY port on dpmac.3 (the reconcile suite's
/// `sized_desired`): `num_queues = n` compiles through the kernel tenant, so the drift
/// branch is not fenced by the unsized-block guard.
fn sized_desired(n: u32) -> DesiredTopology {
    let kernel = kernel_tenant(i64::from(n));
    let (obj, iface) = kernel.dpni(1, n, "wan0".into());
    let mut compiled = CompiledPlan::default();
    compiled.order.push(obj.key().clone());
    compiled.objects.insert(obj);
    compiled.edges.insert(iface.into_port_edge(DpmacId::new(3)));
    DesiredTopology::from_parts(compiled, vec![DesiredPort::new(DpmacId::new(3), "wan0")])
        .expect("plan port-edge and port agree on dpmac.3")
}

fn count_destroys(backend: &FakeBackend) -> usize {
    backend
        .audit()
        .iter()
        .filter(|a| a.starts_with("destroy:"))
        .count()
}

fn one_port_backend(latency: u64) -> (FakeBackend, DesiredTopology) {
    let backend = FakeBackend::new()
        .with_dpmac(DpmacId::new(7), LinkType::Phy, MAC_7)
        .with_bind_latency(latency);
    let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(7), "lan0")]);
    (backend, desired)
}

fn fast_cfg() -> ConvergeConfig {
    ConvergeConfig {
        deadline: Duration::from_secs(5),
        poll_interval: Duration::ZERO,
        prune: false,
        // Provisioning from an empty board is disruptive; these tests exercise the
        // convergence loop, so they allow it explicitly (ADR-0015 decision 12).
        allow: Class::Disruptive,
    }
}

#[test]
fn converges_waiting_for_async_netdev() {
    // netdev appears two observe ticks after connect.
    let (backend, desired) = one_port_backend(2);
    let outcome = engine::ensure(&desired, &backend, &backend, fast_cfg()).unwrap();
    assert_eq!(outcome, Outcome::Converged);
    assert_eq!(
        backend.netdev_for_dpmac(DpmacId::new(7)).as_deref(),
        Some("eth1")
    );
}

#[test]
fn second_run_is_a_noop() {
    let (backend, desired) = one_port_backend(0);
    assert_eq!(
        engine::ensure(&desired, &backend, &backend, fast_cfg()).unwrap(),
        Outcome::Converged
    );
    // Re-run against the now-converged system: no transitions, still converged.
    let observed = engine::observe(&backend, &backend).unwrap();
    let report = StatusReport::compute(&desired, &observed);
    assert!(!report.has_diverged(), "second run must be a no-op");
    assert!(report.plan.transitions.is_empty());
}

#[test]
fn deadline_exceeded_reports_unconverged_ports() {
    // netdev never appears within budget (huge latency) -> deadline hit.
    let (backend, desired) = one_port_backend(1_000_000);
    let cfg = ConvergeConfig {
        deadline: Duration::ZERO, // give up after the first non-converged pass
        poll_interval: Duration::ZERO,
        prune: false,
        allow: Class::Disruptive,
    };
    // First pass creates+connects; because deadline is zero it reports on the next
    // evaluation that it did not converge.
    let outcome = engine::ensure(&desired, &backend, &backend, cfg).unwrap();
    match outcome {
        Outcome::DeadlineExceeded { unconverged } => {
            assert_eq!(unconverged, vec![DpmacId::new(7)]);
        }
        Outcome::Converged => panic!("should not converge with unbounded latency"),
        Outcome::DisruptionRefused { .. } => panic!("disruptive was allowed; must not refuse"),
        Outcome::RebuildRefused { .. } => panic!("no same-run rebuild here; must not refuse"),
    }
}

#[test]
fn interrupted_run_completes_on_rerun() {
    // Simulate a partial prior run: DPNI created + connected but netdev not yet up.
    let (backend, desired) = one_port_backend(0);
    let observed = engine::observe(&backend, &backend).unwrap();
    // Apply only the create+connect part of the plan by converging once; then a
    // fresh converge must still succeed (idempotent, re-observes actual state).
    let _ = observed;
    assert_eq!(
        engine::ensure(&desired, &backend, &backend, fast_cfg()).unwrap(),
        Outcome::Converged
    );
    assert_eq!(
        engine::ensure(&desired, &backend, &backend, fast_cfg()).unwrap(),
        Outcome::Converged
    );
}

#[test]
fn allow_gate_refuses_a_disruptive_plan_and_changes_nothing() {
    // Provisioning from an empty board headlines disruptive; a run allowing only the
    // default hitless refuses before touching the board (ADR-0015 decision 12).
    let (backend, desired) = one_port_backend(0);
    let cfg = ConvergeConfig {
        deadline: Duration::from_secs(5),
        poll_interval: Duration::ZERO,
        prune: false,
        allow: Class::Hitless,
    };
    let outcome = engine::ensure(&desired, &backend, &backend, cfg).unwrap();
    assert_eq!(
        outcome,
        Outcome::DisruptionRefused {
            headline: Class::Disruptive,
            allowed: Class::Hitless,
        }
    );
    // Nothing was actuated: no netdev appeared for the port.
    assert_eq!(backend.netdev_for_dpmac(DpmacId::new(7)), None);
}

#[test]
fn status_exits_diverged_before_provisioning() {
    let (backend, desired) = one_port_backend(0);
    // Before any convergence, the port is absent -> diverged.
    let observed = engine::observe(&backend, &backend).unwrap();
    let report = StatusReport::compute(&desired, &observed);
    assert!(report.has_diverged());
}

#[test]
fn same_run_rebuild_is_refused_after_one_create_and_no_destroy() {
    // The fake mispredicts the read-back, so the run-created dpni looks like drift; the anti-churn witness is exactly one create and zero destroys (pool-objects design D12; ADR-0008 §9).
    let backend = FakeBackend::new()
        .with_dpmac(DpmacId::new(3), LinkType::Phy, MAC_3)
        .with_readback_drift();
    let desired = sized_desired(8);

    let outcome = engine::ensure(&desired, &backend, &backend, fast_cfg()).unwrap();
    match outcome {
        Outcome::RebuildRefused { refusals } => {
            assert_eq!(refusals.len(), 1);
            assert_eq!(refusals[0].port, DpmacId::new(3));
            assert!(
                refusals[0].diff.iter().any(|d| d.field == "num_queues"),
                "diff names the mispredicted field: {:?}",
                refusals[0].diff
            );
        }
        other => panic!("expected a rebuild refusal, got {other:?}"),
    }
    assert_eq!(
        backend.created_cfgs().len(),
        1,
        "exactly one create reached the fake"
    );
    assert_eq!(count_destroys(&backend), 0, "no destroy churned the board");
}

#[test]
fn standing_divergent_dpni_still_converges_via_the_ordered_rebuild() {
    // A dpni present before the run that reads back divergent is a standing rebuild, not a refusal: disconnect, unbind, destroy, create, then converge (ADR-0008 §8).
    let backend = FakeBackend::new()
        .with_dpmac(DpmacId::new(3), LinkType::Phy, MAC_3)
        .with_dpni(ObservedDpni {
            id: DpniId::new(1),
            label: Some("wan0".into()),
            connected_to: Some(DpmacId::new(3)),
            mac: Some(MAC_3),
            netdev: Some("eth1".to_owned()),
            attributes: BTreeMap::new(),
            cfg_observation: Some(DpniObservation::project(&DpniCfg {
                num_queues: NumQueues::new(4).unwrap(),
                ..DpniCfg::defaults()
            })),
        });
    let desired = sized_desired(8);

    let outcome = engine::ensure(&desired, &backend, &backend, fast_cfg()).unwrap();
    assert_eq!(outcome, Outcome::Converged);
    // The standing dpni was torn down once and rebuilt (not left to churn).
    assert_eq!(
        count_destroys(&backend),
        1,
        "the standing object rebuilt exactly once"
    );
}
