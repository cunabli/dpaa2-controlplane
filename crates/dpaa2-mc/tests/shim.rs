//! Golden-fixture parse tests and command-construction assertions for the restool
//! shim (design D10; restool-baseline, task 4.6). No board is touched: parsing runs over recorded
//! output, and command construction is asserted via a recording runner.

use std::cell::RefCell;
use std::collections::HashMap;

use dpaa2_api::contract::McControl;
use dpaa2_api::core::error::Error;
use dpaa2_api::core::model::{DpmacId, DpniId, DprcId, LinkType, MacAddr};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpmac::{self, CounterRead, CounterReadout};
use dpaa2_api::families::dpni::{DpniCfg, NumQueues, Profile};
use dpaa2_mc::parse::{
    parse_dpmac_info, parse_dpni_info, parse_dpni_object_id, parse_dprc_show, parse_dpseci_info,
};
use dpaa2_mc::runner::{RunOutcome, Runner};
use dpaa2_mc::{DpseciReadout, RestoolMc};

/// The unsized port-only projection's create block (`num_queues` 0 ⇒ host fallback), the
/// argument the reconciler's `from_ports` path carries into `create_dpni`.
fn unsized_cfg() -> DpniCfg {
    DpniCfg::defaults()
}

/// A create block sized at `n` transmit queues, everything else at its MC default —
/// what a sized compiled plan carries when only the queue count is pinned.
fn sized_cfg(n: u16) -> DpniCfg {
    DpniCfg {
        num_queues: NumQueues::new(n).expect("in envelope"),
        ..DpniCfg::defaults()
    }
}

const DPRC_SHOW: &str = include_str!("fixtures/dprc_show.txt");
const DPNI_CONNECTED: &str = include_str!("fixtures/dpni_info_connected.txt");
const DPNI_UNCONNECTED: &str = include_str!("fixtures/dpni_info_unconnected.txt");
const DPMAC_PHY: &str = include_str!("fixtures/dpmac_info_phy.txt");
const DPMAC_FIXED: &str = include_str!("fixtures/dpmac_info_fixed.txt");
const DPNI_CREATE: &str = include_str!("fixtures/dpni_create_script.txt");
// Fixture provenance: derived from `dpseci_commands.c` `print_dpseci_attr` (lines ~222-253 —
// the field order and spellings restool prints) filled with the board-verified production
// VPP child profile (8-queue/priority-2 dpseci, `docs/baseline/dpseci.md` "Command surface"
// and the reference environment). There is no options line: restool discards the mask at
// print (DPSECI-I3, dpseci-typestate design D5).
const DPSECI_INFO: &str = include_str!("fixtures/dpseci_info.txt");

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

#[test]
fn parses_dpseci_info_counts_plugged_and_priorities() {
    // The production child profile reads back: id, plugged, 8/8 queues, all-2 priorities.
    // The type has no options field (DPSECI-I3, dpseci-typestate design D5).
    let info = parse_dpseci_info(DPSECI_INFO);
    assert_eq!(info.id, Some(0));
    assert_eq!(info.plugged, Some(true));
    assert_eq!(info.num_tx_queues, Some(8));
    assert_eq!(info.num_rx_queues, Some(8));
    assert_eq!(info.tx_priorities, vec![2, 2, 2, 2, 2, 2, 2, 2]);
}

#[test]
fn read_dpseci_attributes_is_unobservable_without_portal_access() {
    // The GET_ATTR read opens /dev/dprc.N; no such node here ⇒ the typed unobservable
    // outcome, never an error (dpseci-typestate design D5). No board, no /dev access.
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1");
    let out = mc
        .read_dpseci_attributes(DprcId::new(60_000), 0)
        .expect("a missing portal node is unobservable, never an error");
    assert!(matches!(out, DpseciReadout::Unobservable { .. }), "{out:?}");
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
        // The `--script <type> create` calls echo the new object reference.
        responses.insert("--script dpni".to_owned(), DPNI_CREATE.to_owned());
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
    fn run(&self, args: &[&str]) -> Result<String, dpaa2_api::core::error::Error> {
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
fn create_renders_create_then_stamp_only() {
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1").with_cores(1);
    // 0 = the unsized port-only projection, so the shim falls back to its host-derived
    // default (here `queues == cores == 1`), keeping this determinism assertion unchanged.
    let id = mc
        .create_dpni(&ConstructName::from("wan0"), &unsized_cfg())
        .expect("create");
    assert_eq!(id, DpniId::new(7));

    assert_eq!(
        mc.runner_calls(),
        vec![
            vec!["--script", "dpni", "create", "--num-queues=1"],
            vec!["dprc", "set-label", "dpni.7", "--label=wan0"],
        ]
    );
}

#[test]
fn create_propagates_dpni_create_failure_with_no_teardown() {
    let mc =
        RestoolMc::with_runner(FailingRunner::new(("--script", "dpni")), "dprc.1").with_cores(1);
    let err = mc
        .create_dpni(&ConstructName::from("wan0"), &unsized_cfg())
        .expect_err("dpni create fails");
    assert!(matches!(err, dpaa2_api::core::error::Error::Backend(_)));

    let calls = mc.runner_calls();
    assert_eq!(
        calls,
        vec![vec!["--script", "dpni", "create", "--num-queues=1"]],
        "the create was the only call: no destroy, no set-label"
    );
}

#[test]
fn create_honors_compiled_num_queues_over_host_derivation() {
    // cores=16 would derive 16 queues; the compiled num_queues=5 is honored, not re-derived.
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1").with_cores(16);
    mc.create_dpni(&ConstructName::from("wan0"), &sized_cfg(5))
        .expect("create");
    let calls = mc.runner_calls();

    let dpni_create = calls
        .iter()
        .find(|c| {
            c.first().map(String::as_str) == Some("--script")
                && c.get(1).map(String::as_str) == Some("dpni")
        })
        .expect("dpni created");
    assert!(
        dpni_create.iter().any(|a| a == "--num-queues=5"),
        "compiled num_queues honored exactly, not host-derived"
    );
}

#[test]
fn create_pmd_profile_emits_the_computed_mask_and_every_nonzero_size() {
    // The full PMD profile: the runner sees exactly one raw options mask (0x800003d0),
    // every nonzero sizing flag, and no option-name token (dpni-typestate task 4.1).
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1").with_cores(16);
    mc.create_dpni(&ConstructName::from("wan0"), &Profile::Pmd.cfg())
        .expect("create");
    let calls = mc.runner_calls();
    let dpni_create = calls
        .iter()
        .find(|c| {
            c.first().map(String::as_str) == Some("--script")
                && c.get(1).map(String::as_str) == Some("dpni")
        })
        .expect("dpni created");

    let opts: Vec<&String> = dpni_create
        .iter()
        .filter(|a| a.starts_with("--options="))
        .collect();
    assert_eq!(opts.len(), 1, "one options arg");
    assert_eq!(opts[0], "--options=0x800003d0");
    for token in [
        "SingleSender",
        "CustomCg",
        "HasKeyMasking",
        "SINGLE_SENDER",
        "DPNI_OPT",
    ] {
        assert!(
            !dpni_create.iter().any(|a| a.contains(token)),
            "no option-name token {token}"
        );
    }

    for present in [
        "--num-queues=16",
        "--num-tcs=16",
        "--vlan-filter-entries=16",
        "--qos-entries=64",
        "--fs-entries=1",
        "--num-cgs=24",
        "--num-channels=1",
    ] {
        assert!(
            dpni_create.iter().any(|a| a == present),
            "expected {present}"
        );
    }
    for omitted in ["--mac-filter-entries", "--dist-key-size", "--num-opr"] {
        assert!(
            !dpni_create.iter().any(|a| a.starts_with(omitted)),
            "omitted flag {omitted} must not appear (0 ⇒ MC default)"
        );
    }
}

#[test]
fn create_defaults_emits_only_num_queues() {
    // A bare create block: only `--num-queues` (host fallback) rides (DPNI-I7).
    let mc = RestoolMc::with_runner(RecordingRunner::new(), "dprc.1").with_cores(1);
    mc.create_dpni(&ConstructName::from("wan0"), &unsized_cfg())
        .expect("create");
    let dpni_create = mc
        .runner_calls()
        .into_iter()
        .find(|c| {
            c.first().map(String::as_str) == Some("--script")
                && c.get(1).map(String::as_str) == Some("dpni")
        })
        .expect("dpni created");
    assert_eq!(
        dpni_create,
        vec!["--script", "dpni", "create", "--num-queues=1"]
    );
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
    fn run(&self, args: &[&str]) -> Result<String, dpaa2_api::core::error::Error> {
        let out = self.inner.run(args)?;
        if args.first().copied() == Some(self.fail_on.0)
            && args.get(1).copied() == Some(self.fail_on.1)
        {
            return Err(dpaa2_api::core::error::Error::Backend(
                "injected failure".to_owned(),
            ));
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

// ---- dpmac observation surface (dpmac-typestate task 3.1, bead dpaa2-controlplane-0xu.7) ----
//
// Fixture provenance: the `Counters:` blocks appended to the two dpmac fixtures carry a
// source-derived 28-row 10.39 vocabulary (restool 2.4 `dpaa2_mac_counters[]` ∩ the first
// 28 `fsl_dpmac.h` enum ids), its structure cross-checked against an operator-supplied
// board capture (2026-10-02, concrete values withheld — the row values here are
// synthesized, varied, with zeros to exercise `Known(0)` vs absence). Board re-validation
// rides dpmac-typestate task 5.3. The restool header is `Counters: ` with a trailing
// space on the board; the parser trims it, so the fixtures carry `Counters:` verbatim-less
// that cosmetic space.

/// A runner that replays one canned [`RunOutcome`] for every call and records the commands
/// issued — the dpmac-observation transcript double (dpmac-typestate task 3.1).
struct OneShot {
    outcome: RunOutcome,
    calls: RefCell<Vec<Vec<String>>>,
}

impl OneShot {
    fn ok(stdout: &str) -> Self {
        Self {
            outcome: RunOutcome {
                stdout: stdout.to_owned(),
                stderr: String::new(),
                code: Some(0),
            },
            calls: RefCell::new(Vec::new()),
        }
    }

    /// restool's `assert(false)` on an out-of-enum value raises SIGABRT: the process is
    /// killed by a signal, so there is no exit code (`docs/baseline/dpmac.md`
    /// "Silent-failure notes"; dpmac-typestate design D4).
    fn dead() -> Self {
        Self {
            outcome: RunOutcome {
                stdout: String::new(),
                stderr: "dpmac.7: Assertion `0' failed.".to_owned(),
                code: None,
            },
            calls: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }
}

impl Runner for OneShot {
    fn run(&self, args: &[&str]) -> Result<String, Error> {
        let out = self.run_capture(args)?;
        if out.code == Some(0) {
            Ok(out.stdout)
        } else {
            Err(Error::Backend(out.stderr))
        }
    }

    fn run_capture(&self, args: &[&str]) -> Result<RunOutcome, Error> {
        self.calls
            .borrow_mut()
            .push(args.iter().map(|s| (*s).to_owned()).collect());
        Ok(self.outcome.clone())
    }
}

fn observe(stdout: &str, id: u32) -> Result<dpaa2_api::families::dpmac::DpmacObservation, Error> {
    RestoolMc::with_runner(OneShot::ok(stdout), "dprc.1").observe_dpmac(DpmacId::new(id))
}

#[test]
fn observe_dpmac_round_trips_the_phy_fixture_counters_known() {
    let obs = observe(DPMAC_PHY, 7).expect("observe");
    assert_eq!(obs.eth_if, dpmac::EthIf::OtherSerdesProtocol);
    assert_eq!(obs.link_type, dpmac::LinkType::PhyManaged);
    assert_eq!(obs.mac, MacAddr::new([0, 0, 0, 0, 0, 0x29]));
    assert_eq!(obs.max_rate, 10_000);
    let CounterReadout::Vocabulary(vals) = &obs.counters else {
        panic!("expected a vocabulary readout, got {:?}", obs.counters);
    };
    assert_eq!(vals.len(), 28);
    assert_eq!(
        vals[0],
        ("rx all frames".to_owned(), CounterRead::Known(142_537))
    );
    // `rx frame errors: 0` — a present zero is Known(0), never absence (DPMAC-I7).
    assert_eq!(vals[2].1, CounterRead::Known(0));
    assert_eq!(vals[27].1, CounterRead::Known(18_995_221));
}

#[test]
fn observe_dpmac_round_trips_the_fixed_fixture_caui_maps_other_serdes() {
    // CAUI is the second non-USXGMII token mapping to OtherSerdesProtocol (coordinator calibration).
    let obs = observe(DPMAC_FIXED, 5).expect("observe");
    assert_eq!(obs.eth_if, dpmac::EthIf::OtherSerdesProtocol);
    assert_eq!(obs.link_type, dpmac::LinkType::Fixed);
    assert_eq!(obs.max_rate, 25_000);
    assert!(matches!(obs.counters, CounterReadout::Vocabulary(ref v) if v.len() == 28));
}

#[test]
fn observe_dpmac_usxgmii_maps_to_usxgmii() {
    let block = DPMAC_PHY.replace("DPMAC_ETH_IF_XFI", "DPMAC_ETH_IF_USXGMII");
    assert_eq!(
        observe(&block, 7).expect("observe").eth_if,
        dpmac::EthIf::Usxgmii
    );
}

#[test]
fn observe_dpmac_short_count_is_a_version_signal() {
    // Drop the last counter row in memory: 27 rows ⇒ firmware-version signal (DPMAC-I7).
    let short = DPMAC_PHY.trim_end().rsplit_once('\n').expect("rows").0;
    assert_eq!(
        observe(short, 7).expect("observe").counters,
        CounterReadout::VersionSignal {
            expected: 28,
            got: 27
        }
    );
}

#[test]
fn observe_dpmac_over_count_is_a_version_signal() {
    // A 29th row (a 10.40-only counter restool would print on newer firmware) ⇒ signal.
    let over = format!("{DPMAC_PHY}tx control: 7\n");
    assert_eq!(
        observe(&over, 7).expect("observe").counters,
        CounterReadout::VersionSignal {
            expected: 28,
            got: 29
        }
    );
}

#[test]
fn observe_dpmac_unknown_name_at_right_count_is_a_version_signal() {
    // 28 rows but one name outside the vocabulary: sequence inequality ⇒ signal, not Known.
    let bogus = DPMAC_PHY.replace("rx pause:", "rx bogus:");
    assert!(matches!(
        observe(&bogus, 7).expect("observe").counters,
        CounterReadout::VersionSignal { got: 28, .. }
    ));
}

#[test]
fn observe_dpmac_tolerates_the_verbose_interrupt_trailer() {
    // A `--verbose` run prints an interrupt/region trailer after the counters; it must not
    // be misread as deviating rows (coordinator board capture 2026-10-02; dpmac-typestate task 5.3).
    let verbose = format!(
        "{DPMAC_PHY}number of mappable regions: 4\nnumber of interrupts: 1\n\
         interrupt[0] mask: 0x1\ninterrupt[0] status: \n"
    );
    assert!(matches!(
        observe(&verbose, 7).expect("observe").counters,
        CounterReadout::Vocabulary(ref v) if v.len() == 28
    ));
}

#[test]
fn observe_dpmac_ignores_the_api_version_line_for_vocabulary() {
    // The board reports `dpmac version: 4.10` yet still prints only 28 rows: the row set is
    // the vocabulary signal, never the version line (DPMAC-I7; coordinator calibration).
    let v410 = DPMAC_PHY.replace("dpmac version: 4.4", "dpmac version: 4.10");
    assert!(matches!(
        observe(&v410, 7).expect("observe").counters,
        CounterReadout::Vocabulary(ref v) if v.len() == 28
    ));
}

#[test]
fn observe_dpmac_dead_spawn_is_a_typed_failure_with_no_state() {
    // restool's assert(false) abort ⇒ Error::Backend (signal death), never partial state.
    let mc = RestoolMc::with_runner(OneShot::dead(), "dprc.1");
    let err = mc.observe_dpmac(DpmacId::new(7)).expect_err("dead spawn");
    assert!(matches!(err, Error::Backend(_)), "got {err:?}");
}

#[test]
fn observe_dpmac_issues_exactly_one_info_spawn() {
    // The whitelist fence: one `dpmac info` and no other dpmac verb (dpmac-typestate design Non-Goals).
    let mc = RestoolMc::with_runner(OneShot::ok(DPMAC_PHY), "dprc.1");
    mc.observe_dpmac(DpmacId::new(7)).expect("observe");
    assert_eq!(mc.runner().calls(), vec![vec!["dpmac", "info", "dpmac.7"]]);
}
