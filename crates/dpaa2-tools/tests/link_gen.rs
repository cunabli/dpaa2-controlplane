//! Tests for `systemd.link` generation (design D3/D4, tasks 6.3/6.4). Files are
//! written into a scratch dir, not `/run`, so no privileges are needed.

use dpaa2_api::{
    DesiredPort, DesiredTopology, DpmacId, DpniId, LinkType, MacAddr, ObservedDpmac, ObservedDpni,
    ObservedTopology,
};
use dpaa2_tools::link;

const MAC_7: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);

fn scratch(name: &str) -> std::path::PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!("dpaa2ctl-linktest-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn renders_match_and_name_stanzas() {
    let rendered = link::render_link(MAC_7, "lan0");
    assert!(rendered.contains("MACAddress=02:00:00:00:00:07"));
    assert!(rendered.contains("Name=lan0"));
    assert!(rendered.contains("[Match]") && rendered.contains("[Link]"));
}

#[test]
fn phy_port_gets_a_link_file_named_by_precedence() {
    let dir = scratch("phy");
    let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(7), "lan0")]);
    let observed = ObservedTopology {
        dpnis: vec![],
        dpmacs: vec![ObservedDpmac {
            id: DpmacId::new(7),
            link_type: LinkType::Phy,
            mac: Some(MAC_7),
        }],
    };
    let written = link::generate(&desired, &observed, &dir).unwrap();
    assert_eq!(written.len(), 1);
    assert_eq!(
        written[0],
        link::link_path(&dir, "lan0"),
        "10- prefix sorts before stock 99-default.link"
    );
    let body = std::fs::read_to_string(&written[0]).unwrap();
    assert!(body.contains("MACAddress=02:00:00:00:00:07"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mac_is_sourced_from_the_connected_dpni_when_the_dpmac_reports_none() {
    // The real-board case: assert mode (no declared MAC) and `dpmac info` carries no
    // usable MAC, but the provisioned DPNI inherited one. It must still get a file.
    let dir = scratch("dpni-mac");
    let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(3), "wan0")]);
    let observed = ObservedTopology {
        dpnis: vec![ObservedDpni {
            id: DpniId::new(1),
            connected_to: Some(DpmacId::new(3)),
            mac: Some(MAC_7),
            netdev: Some("eth1".to_owned()),
            attributes: std::collections::BTreeMap::new(),
        }],
        dpmacs: vec![ObservedDpmac {
            id: DpmacId::new(3),
            link_type: LinkType::Phy,
            mac: None,
        }],
    };
    let written = link::generate(&desired, &observed, &dir).unwrap();
    assert_eq!(written.len(), 1, "DPNI MAC must back the .link file");
    let body = std::fs::read_to_string(&written[0]).unwrap();
    assert!(body.contains("MACAddress=02:00:00:00:00:07"));
    assert!(body.contains("Name=wan0"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fixed_link_port_gets_no_file() {
    let dir = scratch("fixed");
    let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(5), "qsfp0")]);
    let observed = ObservedTopology {
        dpnis: vec![],
        dpmacs: vec![ObservedDpmac {
            id: DpmacId::new(5),
            link_type: LinkType::Fixed,
            mac: Some(MacAddr::new([0x02, 0, 0, 0, 0, 0x05])),
        }],
    };
    let written = link::generate(&desired, &observed, &dir).unwrap();
    assert!(written.is_empty(), "fixed-link ports need no rename");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn zero_mac_placeholder_yields_no_file() {
    let dir = scratch("zero");
    let desired = DesiredTopology::from_ports([DesiredPort::new(DpmacId::new(7), "lan0")]);
    // No declared MAC and the observed DPMAC carries the all-zero placeholder.
    let observed = ObservedTopology {
        dpnis: vec![],
        dpmacs: vec![ObservedDpmac {
            id: DpmacId::new(7),
            link_type: LinkType::Phy,
            mac: Some(MacAddr::ZERO),
        }],
    };
    let written = link::generate(&desired, &observed, &dir).unwrap();
    assert!(written.is_empty(), "zero MAC must not produce a match");
    let _ = std::fs::remove_dir_all(&dir);
}
