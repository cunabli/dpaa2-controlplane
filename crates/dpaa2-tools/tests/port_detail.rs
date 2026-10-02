//! The read-only port-detail view over the in-memory fake backend (provisioning-cli spec
//! "The CLI exposes a read-only port-detail view"). No board is touched.

use std::time::Duration;

use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::core::model::{DesiredPort, DesiredTopology, DpmacId, DpniId, LinkType, MacAddr};
use dpaa2_api::families::dpmac::{
    Arbitration, CarrierReading, CarrierSource, CounterRead, CounterReadout, MacRelation,
};
use dpaa2_api::plan::Class;
use dpaa2_tools::engine::{self, ConvergeConfig, Outcome};
use dpaa2_tools::{StatusReport, render, status};

const MAC_7: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);
const MAC_9: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x09]);

fn fast_cfg() -> ConvergeConfig {
    ConvergeConfig {
        deadline: Duration::from_secs(5),
        poll_interval: Duration::ZERO,
        prune: false,
        // Provisioning from an empty board is disruptive; allow it so the detail runs over a
        // converged port (ADR-0015 decision 12).
        allow: Class::Disruptive,
    }
}

/// A converged kernel-regime PHY port on dpmac.7, carrier scripted up on its peer dpni.
fn converged_kernel_port() -> (FakeBackend, DesiredTopology) {
    let backend = FakeBackend::new()
        .with_dpmac(DpmacId::new(7), LinkType::Phy, MAC_7)
        // ensure creates dpni.1 as the peer; script its netdev carrier up (DPNI-I4).
        .with_carrier(CarrierSource::PeerDpni(DpniId::new(1)), true);
    let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(7), "lan0")]);
    assert_eq!(
        engine::ensure(&desired, &backend, &backend, fast_cfg()).unwrap(),
        Outcome::Converged
    );
    (backend, desired)
}

#[test]
fn converged_port_renders_its_full_surface() {
    // Spec scenario "A converged port renders its full surface": KernelOwned, Inherited, the
    // carrier reading, and the 28 vocabulary counters, each sourced from read-backs.
    let (backend, desired) = converged_kernel_port();
    let observed = engine::observe(&backend, &backend).unwrap();
    let details = status::port_details(&backend, &backend, &desired, &observed).unwrap();

    assert_eq!(details.len(), 1);
    let d = &details[0];
    assert_eq!(d.arbitration, Arbitration::KernelOwned);
    assert_eq!(d.mac_relation, MacRelation::Inherited);
    assert_eq!(d.carrier, CarrierReading::Up);
    match &d.counters {
        CounterReadout::Vocabulary(v) => assert_eq!(v.len(), 28),
        CounterReadout::VersionSignal { .. } => {
            panic!("expected the vocabulary readout, got a version signal")
        }
    }

    let text = render::render_port_details(&details);
    assert!(text.contains("arbitration=KernelOwned"));
    assert!(text.contains("mac=Inherited"));
    assert!(text.contains("carrier=up"));
    assert!(text.contains("counters (28):"));
}

#[test]
fn displayed_values_never_reach_the_plan() {
    // The repeated-status proof: two status runs with DIFFERENT scripted carrier/counter
    // values both plan zero actions and report converged — the display never reaches the plan.
    let (backend, desired) = converged_kernel_port();

    let observed1 = engine::observe(&backend, &backend).unwrap();
    let report1 = StatusReport::compute(&desired, &observed1);
    assert!(report1.plan.transitions.is_empty());
    assert!(!report1.has_diverged());
    let details1 = status::port_details(&backend, &backend, &desired, &observed1).unwrap();

    // Re-script both displayed surfaces between runs.
    backend.set_carrier(CarrierSource::PeerDpni(DpniId::new(1)), Some(false));
    backend.set_counters(
        DpmacId::new(7),
        CounterReadout::Vocabulary(vec![CounterRead::Known(4242); 28]),
    );

    let observed2 = engine::observe(&backend, &backend).unwrap();
    let report2 = StatusReport::compute(&desired, &observed2);
    assert!(report2.plan.transitions.is_empty());
    assert!(!report2.has_diverged());
    let details2 = status::port_details(&backend, &backend, &desired, &observed2).unwrap();

    // The display changed across runs, but neither run planned anything.
    assert_eq!(details1[0].carrier, CarrierReading::Up);
    assert_eq!(details2[0].carrier, CarrierReading::Down);
    assert_ne!(details1[0].counters, details2[0].counters);
}

#[test]
fn driverless_port_renders_the_no_observable_diagnosis() {
    // A fixed-link port with no connected dpni and no scripted carrier reads Offered with a
    // NoObservable carrier — the driverless-port diagnosis, never down.
    let backend = FakeBackend::new().with_dpmac(DpmacId::new(9), LinkType::Fixed, MAC_9);
    let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(9), "wan0")]);
    let observed = engine::observe(&backend, &backend).unwrap();
    let details = status::port_details(&backend, &backend, &desired, &observed).unwrap();

    let d = &details[0];
    assert_eq!(d.arbitration, Arbitration::Offered);
    assert_eq!(d.carrier, CarrierReading::NoObservable);

    let text = render::render_port_details(&details);
    assert!(text.contains("carrier=no-observable (driverless port)"));
}
