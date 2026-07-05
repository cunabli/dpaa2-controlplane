//! Golden-fixture parse tests and command-construction assertions for the restool
//! shim (design D10, task 4.6). No board is touched: parsing runs over recorded
//! output, and command construction is asserted via a recording runner.

use std::cell::RefCell;
use std::collections::HashMap;

use dpaa2_api::{DpmacId, DpniId, LinkType, MacAddr, McControl};
use dpaa2_mc::RestoolMc;
use dpaa2_mc::parse::{parse_dpmac_info, parse_dpni_info, parse_dpni_object_id, parse_dprc_show};
use dpaa2_mc::runner::Runner;

const DPRC_SHOW: &str = include_str!("fixtures/dprc_show.txt");
const DPNI_CONNECTED: &str = include_str!("fixtures/dpni_info_connected.txt");
const DPNI_UNCONNECTED: &str = include_str!("fixtures/dpni_info_unconnected.txt");
const DPMAC_PHY: &str = include_str!("fixtures/dpmac_info_phy.txt");
const DPMAC_FIXED: &str = include_str!("fixtures/dpmac_info_fixed.txt");
const DPNI_CREATE: &str = include_str!("fixtures/dpni_create_script.txt");

#[test]
fn parses_dprc_show_object_lists() {
    let (dpnis, dpmacs) = parse_dprc_show(DPRC_SHOW);
    assert_eq!(dpnis, vec![DpniId::new(0), DpniId::new(7)]);
    assert_eq!(
        dpmacs,
        vec![DpmacId::new(17), DpmacId::new(3), DpmacId::new(7)]
    );
}

#[test]
fn parses_connected_dpni_endpoint_and_mac() {
    let info = parse_dpni_info(DPNI_CONNECTED);
    assert_eq!(info.endpoint, Some(DpmacId::new(7)));
    assert_eq!(info.mac, Some(MacAddr::new([0, 0, 0, 0, 0, 0x29])));
}

#[test]
fn parses_unconnected_dpni_as_no_endpoint() {
    let info = parse_dpni_info(DPNI_UNCONNECTED);
    assert_eq!(info.endpoint, None);
}

#[test]
fn parses_dpmac_link_types() {
    assert_eq!(parse_dpmac_info(DPMAC_PHY).link_type, LinkType::Phy);
    assert_eq!(parse_dpmac_info(DPMAC_FIXED).link_type, LinkType::Fixed);
}

#[test]
fn parses_created_object_id() {
    assert_eq!(parse_dpni_object_id(DPNI_CREATE), Some(DpniId::new(7)));
}

/// A runner that returns canned output keyed by the first two args and records the
/// exact argument vectors issued, so command construction can be asserted.
struct RecordingRunner {
    calls: RefCell<Vec<Vec<String>>>,
    responses: HashMap<String, String>,
}

impl RecordingRunner {
    fn new() -> Self {
        let mut responses = HashMap::new();
        responses.insert("dprc show".to_owned(), DPRC_SHOW.to_owned());
        responses.insert("dpni info".to_owned(), DPNI_CONNECTED.to_owned());
        responses.insert("dpmac info".to_owned(), DPMAC_PHY.to_owned());
        responses.insert("--script dpni".to_owned(), DPNI_CREATE.to_owned());
        // The `--script <type> create` calls echo the new object reference.
        responses.insert("--script dpio".to_owned(), "dpio.0\n".to_owned());
        responses.insert("--script dpbp".to_owned(), "dpbp.0\n".to_owned());
        responses.insert("--script dpmcp".to_owned(), "dpmcp.0\n".to_owned());
        responses.insert("--script dpcon".to_owned(), "dpcon.0\n".to_owned());
        Self {
            calls: RefCell::new(Vec::new()),
            responses,
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }
}

impl Runner for RecordingRunner {
    fn run(&self, args: &[&str]) -> Result<String, dpaa2_api::Error> {
        self.calls
            .borrow_mut()
            .push(args.iter().map(|s| (*s).to_owned()).collect());
        let key = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
        Ok(self.responses.get(&key).cloned().unwrap_or_default())
    }
}

#[test]
fn observe_composes_show_info_calls_into_topology() {
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1");
    let topo = mc.observe().expect("observe");
    // Two DPNIs and three DPMACs enumerated from the fixtures.
    assert_eq!(topo.dpnis.len(), 2);
    assert_eq!(topo.dpmacs.len(), 3);
    assert_eq!(topo.dpnis[0].connected_to, Some(DpmacId::new(7)));
}

#[test]
fn create_provisions_private_deps_then_creates_dpni_unplugged() {
    // Pin cores=queues=1 so the sequence is bounded and deterministic.
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1").with_cores(1);
    let id = mc.create_dpni().expect("create");
    assert_eq!(id, DpniId::new(7));

    let calls = mc.runner_calls();
    let is_create = |c: &Vec<String>, kind: &str| {
        c.first().map(String::as_str) == Some("--script")
            && c.get(1).map(String::as_str) == Some(kind)
    };
    let pos = |kind: &str| calls.iter().position(|c| is_create(c, kind));

    // The private dependencies exist before the DPNI is created.
    let dpni_at = pos("dpni").expect("dpni created");
    assert!(
        pos("dpbp").expect("dpbp created") < dpni_at,
        "dpbp before dpni"
    );
    assert!(
        pos("dpmcp").expect("dpmcp created") < dpni_at,
        "dpmcp before dpni"
    );
    assert!(
        pos("dpcon").expect("dpcon created") < dpni_at,
        "dpcon before dpni"
    );
    assert!(
        pos("dpio").expect("dpio created") < dpni_at,
        "dpio before dpni"
    );

    // The DPNI create is issued with an explicit queue count...
    assert!(calls[dpni_at].iter().any(|a| a == "--num-queues=1"));
    // ...and the sequence ends there: create_dpni() no longer plugs or syncs.
    // Plugging (and the actuate-mode SetMac that must precede it) now happens in
    // connect(), matching the design recipe's ordering.
    assert_eq!(dpni_at, calls.len() - 1, "dpni create is the last call");
}

#[test]
fn create_rolls_back_deps_when_dpni_create_fails() {
    // Pin cores=queues=1 so exactly one dpbp/dpmcp/dpcon are created.
    let mc =
        RestoolMc::with_runner(FailingRunner::new(("--script", "dpni")), "dprc.1").with_cores(1);
    let err = mc.create_dpni().expect_err("dpni create fails");
    assert!(matches!(err, dpaa2_api::Error::Backend(_)));

    let calls = mc.runner_calls();
    // The failed attempt's private deps are torn down, in reverse creation order,
    // rather than left orphaned and plugged in the container.
    let destroys: Vec<&str> = calls
        .iter()
        .filter(|c| c.get(1).map(String::as_str) == Some("destroy"))
        .map(|c| c[0].as_str())
        .collect();
    assert_eq!(destroys, vec!["dpcon", "dpmcp", "dpbp"]);
}

#[test]
fn create_tops_up_dpio_pool_idempotently() {
    // DPRC_SHOW has no dpio; with cores=2 the shim creates two DPIOs (+companion mcp).
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1").with_cores(2);
    mc.create_dpni().expect("create");
    let calls = mc.runner_calls();
    let dpio_creates = calls
        .iter()
        .filter(|c| {
            c.first().map(String::as_str) == Some("--script")
                && c.get(1).map(String::as_str) == Some("dpio")
        })
        .count();
    assert_eq!(dpio_creates, 2, "topped up to the core count");
}

#[test]
fn set_mac_uses_dpni_update() {
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1");
    mc.set_mac(DpniId::new(7), MacAddr::new([2, 0, 0, 0, 0, 7]))
        .expect("set mac");
    let calls = mc.runner_calls();
    assert_eq!(
        calls[0],
        vec!["dpni", "update", "dpni.7", "--mac-addr=02:00:00:00:00:07"]
    );
}

#[test]
fn connect_plugs_then_issues_edge_then_sync() {
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1");
    mc.connect(DpniId::new(7), DpmacId::new(3))
        .expect("connect");
    let calls = mc.runner_calls();
    assert_eq!(
        calls[0],
        vec!["dprc", "assign", "dprc.1", "--object=dpni.7", "--plugged=1"]
    );
    assert_eq!(
        calls[1],
        vec![
            "dprc",
            "connect",
            "dprc.1",
            "--endpoint1=dpni.7",
            "--endpoint2=dpmac.3"
        ]
    );
    assert_eq!(calls[2], vec!["dprc", "sync"]);
}

#[test]
fn destroy_issues_destroy_then_sync() {
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1");
    mc.destroy(DpniId::new(7)).expect("destroy");
    let calls = mc.runner_calls();
    assert_eq!(calls[0], vec!["dpni", "destroy", "dpni.7"]);
    assert_eq!(calls[1], vec!["dprc", "sync"]);
}

/// Wraps a [`RecordingRunner`] but fails one call (matched by its first two args,
/// e.g. `("--script", "dpni")`) after recording it, so rollback behaviour can be
/// asserted without a board.
struct FailingRunner {
    inner: RecordingRunner,
    fail_on: (&'static str, &'static str),
}

impl FailingRunner {
    fn new(fail_on: (&'static str, &'static str)) -> Self {
        Self {
            inner: RecordingRunner::new(),
            fail_on,
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.inner.calls()
    }
}

impl Runner for FailingRunner {
    fn run(&self, args: &[&str]) -> Result<String, dpaa2_api::Error> {
        let out = self.inner.run(args)?;
        if args.first().copied() == Some(self.fail_on.0)
            && args.get(1).copied() == Some(self.fail_on.1)
        {
            return Err(dpaa2_api::Error::Backend("injected failure".to_owned()));
        }
        Ok(out)
    }
}

// Small accessor helper used by the construction tests above.
trait RunnerCalls {
    fn runner_calls(&self) -> Vec<Vec<String>>;
}
impl RunnerCalls for RestoolMc<RecordingRunner> {
    fn runner_calls(&self) -> Vec<Vec<String>> {
        self.runner().calls()
    }
}
impl RunnerCalls for RestoolMc<FailingRunner> {
    fn runner_calls(&self) -> Vec<Vec<String>> {
        self.runner().calls()
    }
}
