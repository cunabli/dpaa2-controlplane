//! The read-only dpsw switch detail view over the in-memory fake backend (dpsw-typestate task
//! 5.1). No board is touched: the fake scripts the pool row and the per-object detail, so the
//! live-switch face (full attributes, decoded options with a raw escape, a Connected and a
//! Disconnected port) and the honest-unknown face (an Unobservable port plus absent
//! attribute/options lines) both render offline (provisioning-cli spec "The CLI exposes a
//! read-only switch detail view").

use std::collections::BTreeMap;

use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::contract::{DpswDetail, DpswIfDetail, DpswIfFace};
use dpaa2_api::core::family::Family;
use dpaa2_api::core::model::{DprcId, ObjectRef};
use dpaa2_api::families::dpsw::{
    BroadcastCfg, ComponentType, CounterReadout, DpswCounterRead, DpswOpt, FdbAgingTime,
    FloodingCfg, IfLinkState, MaxFdbEntries, MaxFdbMcGroups, MaxFdbs, MaxVlans, NumIfs,
    ObservedOpts, RawEscape, Taildrop,
};
use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};
use dpaa2_tools::{render, status};

const CONTAINER: DprcId = DprcId::new(5);

/// Seeds `container` with one plugged dpsw pool row and scripts `detail` for it, so
/// [`status::dpsw_details`] lists the row (`observe_pool`) then reads its detail (`observe_dpsw`).
fn backend_with(object: ObjectRef, detail: DpswDetail) -> FakeBackend {
    FakeBackend::new()
        .with_pool_object(
            CONTAINER,
            ObservedPoolObject {
                object,
                label: RawLabel::from("fabric0"),
                plugged: true,
                drawn: false,
            },
        )
        .with_dpsw_detail(object, detail)
}

#[test]
fn live_switch_renders_attributes_options_and_the_interface_table() {
    // A live switch: full attributes, decoded options including a raw escape, and a Connected plus
    // a Disconnected port — the Disconnected port is the distinct free token, never unknown
    // (dpsw-typestate design D6).
    let object = ObjectRef::new(Family::Dpsw, 3);
    let endpoint = ObjectRef::new(Family::Dpni, 11);
    let mut ifaces = BTreeMap::new();
    ifaces.insert(
        0,
        DpswIfDetail {
            face: DpswIfFace::Connected(endpoint),
            link: Some(IfLinkState::Up),
            taildrop: Some(Taildrop {
                enabled: true,
                units: "BUFFERS".to_owned(),
                threshold: 100,
            }),
            max_frame_length: Some(1522),
            counters: CounterReadout::new(vec![
                ("tx_frames".to_owned(), DpswCounterRead::Known(42)),
                ("rx_frames".to_owned(), DpswCounterRead::Unread),
            ]),
        },
    );
    ifaces.insert(
        1,
        DpswIfDetail {
            face: DpswIfFace::Disconnected,
            link: Some(IfLinkState::NotApplicable),
            taildrop: None,
            max_frame_length: Some(1522),
            counters: CounterReadout::new(vec![]),
        },
    );
    let detail = DpswDetail {
        num_ifs: Some(NumIfs::new(2).unwrap()),
        max_vlans: Some(MaxVlans::new(16).unwrap()),
        max_fdbs: Some(MaxFdbs::new(8).unwrap()),
        max_fdb_entries: Some(MaxFdbEntries::new(1024).unwrap()),
        fdb_aging_time: Some(FdbAgingTime::new(300).unwrap()),
        max_fdb_mc_groups: Some(MaxFdbMcGroups::new(32).unwrap()),
        component_type: Some(ComponentType::CVlan),
        flooding_cfg: Some(FloodingCfg::PerFdb),
        broadcast_cfg: Some(BroadcastCfg::PerFdb),
        mem_size: Some(4096),
        options: Some(
            ObservedOpts::empty()
                .with_named(DpswOpt::CtrlIfDis)
                .with_escape(RawEscape::new(0x200)),
        ),
        ifaces,
    };
    let backend = backend_with(object, detail);

    let rows = status::dpsw_details(&backend, CONTAINER).expect("detail reads exit zero");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].plugged);

    insta::assert_snapshot!(render::render_dpsw_details(&rows));
}

#[test]
fn honest_unknown_switch_renders_unknown_never_blank_or_zero() {
    // Absent attribute and options lines plus an Unobservable port: every honest-unknown face
    // renders the literal `unknown`, never blank or zero, and the read is never an error — the
    // command exits zero (provisioning-cli spec; DPSW-I7).
    let object = ObjectRef::new(Family::Dpsw, 7);
    let mut ifaces = BTreeMap::new();
    ifaces.insert(
        0,
        DpswIfDetail {
            face: DpswIfFace::Unobservable,
            link: None,
            taildrop: None,
            max_frame_length: None,
            counters: CounterReadout::new(vec![]),
        },
    );
    let detail = DpswDetail {
        num_ifs: None,
        max_vlans: None,
        max_fdbs: None,
        max_fdb_entries: None,
        fdb_aging_time: None,
        max_fdb_mc_groups: None,
        component_type: None,
        flooding_cfg: None,
        broadcast_cfg: None,
        mem_size: None,
        options: None,
        ifaces,
    };
    let backend = backend_with(object, detail);

    let rows =
        status::dpsw_details(&backend, CONTAINER).expect("an unobservable face is not an error");

    let text = render::render_dpsw_details(&rows);
    // The attribute line, options line, and the port face are all the honest-unknown token.
    assert!(text.contains("num_ifs=unknown"));
    assert!(text.contains("options=unknown"));
    assert!(
        text.contains("endpoint=unknown link=unknown taildrop=unknown max_frame_length=unknown")
    );
    // Never a defaulted zero or a blank where a value was absent.
    assert!(!text.contains("num_ifs=0"));
    assert!(!text.contains("options=[]"));

    insta::assert_snapshot!(text);
}
