//! The read-only dpseci detail view over the in-memory fake backend (dpseci-typestate task
//! 4.1). No board is touched: the fake scripts the pool row and the per-object detail, so the
//! privileged Observed face and the honest-unknown portal face both render offline.

use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::contract::{DpseciDetail, DpseciPortalReadout};
use dpaa2_api::core::family::Family;
use dpaa2_api::core::model::{DprcId, ObjectRef};
use dpaa2_api::families::dpseci::{DpseciOpt, OptionMask, RawEscape};
use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};
use dpaa2_tools::{render, status};

const CONTAINER: DprcId = DprcId::new(5);

/// Seeds `container` with one plugged dpseci pool row and scripts `detail` for it, so
/// [`status::dpseci_details`] lists the row (`observe_pool`) then reads its detail (`observe_dpseci`).
fn backend_with(object: ObjectRef, plugged: bool, detail: DpseciDetail) -> FakeBackend {
    FakeBackend::new()
        .with_pool_object(
            CONTAINER,
            ObservedPoolObject {
                object,
                label: RawLabel::from("crypto0"),
                plugged,
                drawn: false,
            },
        )
        .with_dpseci_detail(object, detail)
}

#[test]
fn privileged_run_renders_options_and_version() {
    // The Observed portal face: restool queues/priorities plus GET_ATTR options and GET_API_VERSION
    // from a read-back; an unnamed bit renders by value (dpseci-typestate design D5; dpseci-hardening design D1).
    let object = ObjectRef::new(Family::Dpseci, 4);
    let detail = DpseciDetail {
        num_tx_queues: Some(3),
        num_rx_queues: Some(3),
        tx_priorities: vec![2, 2, 2],
        portal: DpseciPortalReadout::Observed {
            options: OptionMask::empty()
                .with_flag(DpseciOpt::HasCg)
                .with_escape(RawEscape::new(0x100)),
            api_major: 5,
            api_minor: 4,
        },
    };
    let backend = backend_with(object, true, detail);

    let rows = status::dpseci_details(&backend, CONTAINER).expect("detail reads exit zero");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].plugged);

    let text = render::render_dpseci_details(&rows);
    assert!(
        text.contains("dpseci.4 queues tx=3/rx=3 tx-priorities=[2,2,2] plugged=true drawn=false")
    );
    assert!(text.contains("options=[HasCg,raw:0x100] version=5.4"));
}

#[test]
fn unprivileged_run_renders_the_honest_unknown_and_exits_zero() {
    // The portal was unavailable this run: the queues still render from restool `info`, the
    // portal line carries its reason as honest-unknown, and the read is never an error — the
    // command exits zero (dpseci-typestate design D5).
    let object = ObjectRef::new(Family::Dpseci, 7);
    let detail = DpseciDetail {
        num_tx_queues: Some(2),
        num_rx_queues: Some(2),
        tx_priorities: vec![1, 1],
        portal: DpseciPortalReadout::Unobservable {
            reason: "permission denied opening /dev/dprc.1".to_owned(),
        },
    };
    let backend = backend_with(object, true, detail);

    let rows = status::dpseci_details(&backend, CONTAINER)
        .expect("an unobservable portal is not an error");

    let text = render::render_dpseci_details(&rows);
    assert!(
        text.contains("dpseci.7 queues tx=2/rx=2 tx-priorities=[1,1] plugged=true drawn=false")
    );
    assert!(text.contains("portal=no-observable (permission denied opening /dev/dprc.1)"));
    assert!(!text.contains("options=["));
}
