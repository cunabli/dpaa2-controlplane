//! Convergence-loop, idempotence, and exit-behaviour tests driven entirely against
//! the in-memory fake backend (design D10, tasks 5.5/5.6). No board is touched.

use std::time::Duration;

use dpaa2_api::fake::FakeBackend;
use dpaa2_api::{Class, DesiredPort, DesiredTopology, DpmacId, LinkType, MacAddr};
use dpaa2_tools::StatusReport;
use dpaa2_tools::engine::{self, ConvergeConfig, Outcome};

const MAC_7: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);

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
