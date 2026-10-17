//! The dpsw-lifecycle ITF-replay CI rung (dpsw-typestate task 6.1): every committed dpsw
//! trace replays green through the task-1.2/3.2 create-surface and census judgments,
//! board-free. The model is the oracle — a regenerated trace whose world the Rust judgments no
//! longer reproduce fails here loudly; regenerate with `pnpm model:freeze-dpsw` and reconcile
//! the transcription.
//!
//! Each trace is a directed run of `models/families/dpsw.qnt` `dpsw_lifecycle`. The ITF carries
//! the `world` state but not the action between two states, so the replayer reduces each state
//! to its [`DpswWorld`] vocabulary and asserts, at every state, that the Rust judgments agree
//! with the frozen fields (dpsw-typestate design D1/D6):
//!
//!   - a model-created block is one [`DpswCfg::new`] accepts, reading back the same twelve
//!     fields (the accept direction of class-for-class agreement; DPSW-I1);
//!   - each refusal trace maps class-for-class to the Rust [`Refusal`] variant, nothing created;
//!   - the pool arithmetic (probe draws, competitor defers, destroy returns) is internally
//!     consistent step to step and matches the model `PROBE_DRAW` constants (DPSW-I9);
//!   - the reset facet (`mcDefaults`) and the phase vocabulary replay (DPSW-I4; DPSW-I9).
//!
//! The three property twins transcribe the model's pure operators and assert they agree with
//! the production surface over a generated input surface: the `classify` bindability order
//! (`dpsw.qnt` lines ~152-230), the census judge including the poison rule (`dpsw.qnt` lines
//! ~590-660), and the `dpswPortBound` switch-port bound (`models/core/connect.qnt` lines ~38-45).

use std::collections::{BTreeMap, BTreeSet};

use dpaa2_api::core::family::Family;
use dpaa2_api::families::dpsw::{
    BroadcastCfg, ComponentType, DpswCfg, DpswCreate, DpswOpt, FdbAgingTime, FloodingCfg,
    MaxFdbEntries, MaxFdbMcGroups, MaxFdbs, MaxVlans, NumIfs, Refusal, SwitchPhase,
    mc_defaults_after_reset,
};
use dpaa2_api::plan::connect::dpsw_port_bound;
use dpaa2_api::plan::dpsw::{
    CensusDelta, FamilyJudgment, ObservableSig, ObservedFace, judge_census,
};
use dpaa2_verify::intent::dpsw_itf::{DpswState, DpswWorld, RawCfg, SwitchView, parse_dpsw_trace};
use proptest::prelude::*;

/// The probe draw and pool ceiling transcribed from `dpsw.qnt` `dpsw_lifecycle` `PROBE_DRAW`
/// (1 dpmcp + 1 dpbp; `dpswParams.draw`) and `POOL_DPBP_CEIL` (DPSW-I9). dpmcp is never
/// competitor-contended — only the probe bind draws it — so it is a fully pinned conservation
/// law; dpbp also carries the hidden competitor, so its law is a bound, not a conservation.
const PROBE_DRAW_DPMCP: i64 = 1;
const PROBE_DRAW_DPBP: i64 = 1;
const POOL_DPBP_CEIL: i64 = 1;

/// Every committed trace under `models/traces/families/dpsw/`, with the model law it pins.
const TRACES: &[(&str, &str)] = &[
    (
        "createBindableAcceptedTest",
        "DPSW-I1: the 2-port bindable shape is accepted, PhaseCreated, mcDefaults intact",
    ),
    (
        "vendorRecipeRefusedTest",
        "DPSW-I2: the ls-addsw PER_VLAN/PER_OBJECT default refused VendorRecipeShape, nothing created",
    ),
    (
        "ctrlIfDisabledRefusedTest",
        "DPSW-I1: ctrl-if off refused UnbindableCfg, nothing created",
    ),
    (
        "fewFdbsRefusedTest",
        "DPSW-I1: max_fdbs < num_ifs refused UnbindableCfg, nothing created",
    ),
    (
        "probeDrawsAndBindsTest",
        "DPSW-I9/I3: the probe draws 1 dpmcp + 1 dpbp and binds, the pool drains to zero",
    ),
    (
        "resetFacetTotalTest",
        "DPSW-I4: the bind-time total reset dismantles MC-default state (mcDefaults false once bound)",
    ),
    (
        "censusCompetitorDefersTest",
        "DPSW-I9: a competitor drains the dpbp so the probe defers silently, then settles once restored",
    ),
    (
        "destroySoleResizePathTest",
        "destroy is the sole resize path: a bound switch returns its draws to the pool",
    ),
];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/traces/families/dpsw/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// A trace/core disagreement — a FINDING, surfaced loudly (mirrors `dpseci_replay.rs`).
fn finding(file: &str, step: usize, msg: &str) -> ! {
    panic!("{file}: trace/core disagreement at step {step}: {msg}");
}

/// Whether a state carries a kernel-bound switch (`dpsw.qnt` `isBound`).
fn is_bound(state: &DpswState) -> bool {
    matches!(state, DpswState::Switch(sw) if sw.phase == SwitchPhase::Bound)
}

/// The reported block as a [`DpswCreate`] through the refined range newtypes; a value beyond a
/// field's envelope is a loud finding (the range boundary, distinct from the DPSW-I1 judgment
/// under test — every frozen created block is bindable, so this never fires on a clean trace).
fn to_create(file: &str, i: usize, raw: &RawCfg) -> DpswCreate {
    let envelope = |name: &str| format!("{name} out of the create envelope");
    DpswCreate {
        num_ifs: NumIfs::new(raw.num_ifs)
            .unwrap_or_else(|_| finding(file, i, &envelope("num_ifs"))),
        options: raw.options.clone(),
        max_vlans: MaxVlans::new(raw.max_vlans)
            .unwrap_or_else(|_| finding(file, i, &envelope("max_vlans"))),
        max_fdbs: MaxFdbs::new(raw.max_fdbs)
            .unwrap_or_else(|_| finding(file, i, &envelope("max_fdbs"))),
        max_fdb_entries: MaxFdbEntries::new(raw.max_fdb_entries)
            .unwrap_or_else(|_| finding(file, i, &envelope("max_fdb_entries"))),
        fdb_aging_time: FdbAgingTime::new(raw.fdb_aging_time)
            .unwrap_or_else(|_| finding(file, i, &envelope("fdb_aging_time"))),
        max_fdb_mc_groups: MaxFdbMcGroups::new(raw.max_fdb_mc_groups)
            .unwrap_or_else(|_| finding(file, i, &envelope("max_fdb_mc_groups"))),
        mem_size: raw.mem_size,
        component_type: raw.component_type,
        flooding_cfg: raw.flooding_cfg,
        broadcast_cfg: raw.broadcast_cfg,
        max_meters_per_if: raw.max_meters_per_if,
    }
}

/// A model-created block: [`DpswCfg::new`] accepts exactly it and reads its observable fields
/// back verbatim (DPSW-I1 accept direction; `dpsw.qnt` `createBindableAcceptedTest`).
fn check_created(file: &str, i: usize, sw: &SwitchView) {
    let raw = &sw.cfg;
    let cfg = DpswCfg::new(to_create(file, i, raw)).unwrap_or_else(|r| {
        finding(
            file,
            i,
            &format!("model created a cfg DpswCfg::new refuses as {r:?}"),
        )
    });
    if cfg.num_ifs() != raw.num_ifs
        || cfg.max_vlans() != raw.max_vlans
        || cfg.max_fdbs() != raw.max_fdbs
        || cfg.max_fdb_entries() != raw.max_fdb_entries
        || cfg.fdb_aging_time() != raw.fdb_aging_time
        || cfg.max_fdb_mc_groups() != raw.max_fdb_mc_groups
    {
        finding(file, i, "a range field reads back changed");
    }
    if cfg.component_type() != raw.component_type
        || cfg.flooding_cfg() != raw.flooding_cfg
        || cfg.broadcast_cfg() != raw.broadcast_cfg
    {
        finding(file, i, "an enumerated field reads back changed");
    }
    if cfg.options() != &raw.options {
        finding(file, i, "options read back changed");
    }
}

/// The per-state conformance: a created switch is accepted-and-read-back by `DpswCfg::new`, and
/// a refusal left nothing created.
fn check_state(file: &str, i: usize, w: &DpswWorld) {
    match (&w.dpsw, w.last_outcome) {
        (DpswState::Switch(sw), None) => check_created(file, i, sw),
        (DpswState::Absent, _) => {}
        (DpswState::Switch(_), Some(r)) => finding(
            file,
            i,
            &format!("a created switch carries a refusal {r:?} — model↔core contradiction"),
        ),
    }
}

#[test]
fn dpsw_traces_replay_green() {
    for (file, face) in TRACES {
        let states = parse_dpsw_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            states.len() >= 2,
            "{file} ({face}): a directed run has at least two states"
        );
        assert_eq!(
            states[0].dpsw,
            DpswState::Absent,
            "{file} ({face}): a directed run starts at init (Absent)"
        );
        assert_eq!(
            states[0].last_outcome, None,
            "{file} ({face}): init is Accepted"
        );
        assert_eq!(
            (states[0].pool.dpmcp, states[0].pool.dpbp),
            (PROBE_DRAW_DPMCP, PROBE_DRAW_DPBP),
            "{file} ({face}): init pool is exactly the census (one each)"
        );
        for (i, w) in states.iter().enumerate() {
            check_state(file, i, w);
        }
    }
}

/// The refusal traces leave nothing created and carry the class the model recorded — the
/// refuse direction at the state level (full class-for-class agreement is the property twin).
#[test]
fn refusal_traces_create_nothing_and_name_the_class() {
    let expected: &[(&str, Refusal)] = &[
        ("vendorRecipeRefusedTest", Refusal::VendorRecipeShape),
        ("ctrlIfDisabledRefusedTest", Refusal::UnbindableCfg),
        ("fewFdbsRefusedTest", Refusal::UnbindableCfg),
    ];
    for (file, class) in expected {
        let states = parse_dpsw_trace(&load(file)).unwrap();
        let last = states.last().unwrap();
        assert_eq!(
            last.dpsw,
            DpswState::Absent,
            "{file}: a refusal creates nothing"
        );
        assert_eq!(
            last.last_outcome,
            Some(*class),
            "{file}: the frozen refusal names {class:?}"
        );
    }
}

/// The pool arithmetic replays internally consistent step to step and matches the model
/// `PROBE_DRAW` constants (DPSW-I9): dpmcp is conserved against the bound draw (never
/// competitor-contended, so a deferred switch holds no dpmcp — the `DeferredIsSilentFace`
/// replay), both counts stay within their ceilings, a step moves at most one draw, a bind
/// drops both counts by the draw, and a bound destroy returns both.
#[test]
fn pool_arithmetic_replays_consistently() {
    for (file, _) in TRACES {
        let states = parse_dpsw_trace(&load(file)).unwrap();
        for (i, w) in states.iter().enumerate() {
            let held = if is_bound(&w.dpsw) {
                PROBE_DRAW_DPMCP
            } else {
                0
            };
            assert_eq!(
                w.pool.dpmcp + held,
                PROBE_DRAW_DPMCP,
                "{file} step {i}: dpmcp is conserved against the bound draw (DPSW-I9)"
            );
            assert!(
                (0..=PROBE_DRAW_DPMCP).contains(&w.pool.dpmcp),
                "{file} step {i}: free dpmcp within its ceiling"
            );
            assert!(
                (0..=POOL_DPBP_CEIL).contains(&w.pool.dpbp),
                "{file} step {i}: free dpbp within POOL_DPBP_CEIL"
            );
        }
        for i in 1..states.len() {
            let (prev, cur) = (&states[i - 1], &states[i]);
            assert!(
                (cur.pool.dpmcp - prev.pool.dpmcp).abs() <= PROBE_DRAW_DPMCP,
                "{file} step {i}: a step moves at most one dpmcp draw"
            );
            assert!(
                (cur.pool.dpbp - prev.pool.dpbp).abs() <= PROBE_DRAW_DPBP,
                "{file} step {i}: a step moves at most one dpbp draw"
            );
            if !is_bound(&prev.dpsw) && is_bound(&cur.dpsw) {
                assert_eq!(
                    (
                        prev.pool.dpmcp - cur.pool.dpmcp,
                        prev.pool.dpbp - cur.pool.dpbp
                    ),
                    (PROBE_DRAW_DPMCP, PROBE_DRAW_DPBP),
                    "{file} step {i}: the bind draws exactly 1 dpmcp + 1 dpbp"
                );
            }
            if is_bound(&prev.dpsw) && cur.dpsw == DpswState::Absent {
                assert_eq!(
                    (
                        cur.pool.dpmcp - prev.pool.dpmcp,
                        cur.pool.dpbp - prev.pool.dpbp
                    ),
                    (PROBE_DRAW_DPMCP, PROBE_DRAW_DPBP),
                    "{file} step {i}: a bound destroy returns 1 dpmcp + 1 dpbp"
                );
            }
        }
    }
}

/// The reset facet and phase vocabulary replay (DPSW-I4; DPSW-I9): a bound switch has its
/// MC-default state reset per `mc_defaults_after_reset` (total scope ⇒ false), while a created
/// or deferred switch left its MC-default state untouched (no reset ran — defer-not-fail).
#[test]
fn reset_facet_and_phase_replay_by_vocabulary() {
    for (file, _) in TRACES {
        let states = parse_dpsw_trace(&load(file)).unwrap();
        for (i, w) in states.iter().enumerate() {
            if let DpswState::Switch(sw) = &w.dpsw {
                match sw.phase {
                    SwitchPhase::Bound => assert_eq!(
                        sw.mc_defaults,
                        mc_defaults_after_reset(true),
                        "{file} step {i}: a bound switch applies the reset facet (DPSW-I4)"
                    ),
                    SwitchPhase::Created | SwitchPhase::Deferred => assert!(
                        sw.mc_defaults,
                        "{file} step {i}: an unbound switch left MC-default state intact (DPSW-I9)"
                    ),
                }
            }
        }
    }
}

/// A transcription of the model's `classify` (`dpsw.qnt` `dpsw_lifecycle` lines ~208-211, on
/// `isVendorRecipeShape`/`kernelBindable`/`ctrlIfEnabled`): the ls-addsw vendor shape first (so
/// a vendor recipe is named by its own refusal), then any other DPSW-I1 unbindable clause.
/// `None` is `Accepted`; `Some(r)` is `Refused(r)`.
fn classify_twin(
    num_ifs: u16,
    max_fdbs: u16,
    flooding: FloodingCfg,
    broadcast: BroadcastCfg,
    ctrl_if_dis: bool,
) -> Option<Refusal> {
    let ctrl_if_enabled = !ctrl_if_dis;
    let vendor = flooding == FloodingCfg::PerVlan && broadcast == BroadcastCfg::PerObject;
    let bindable = ctrl_if_enabled
        && flooding == FloodingCfg::PerFdb
        && broadcast == BroadcastCfg::PerFdb
        && max_fdbs >= num_ifs;
    if vendor {
        Some(Refusal::VendorRecipeShape)
    } else if !bindable {
        Some(Refusal::UnbindableCfg)
    } else {
        None
    }
}

/// The multiset count map transcribed from `dpsw.qnt` `sigCensus` (lines ~615-619).
fn census_twin(sigs: &[ObservableSig]) -> BTreeMap<ObservableSig, i64> {
    let mut m = BTreeMap::new();
    for s in sigs {
        *m.entry(s.clone()).or_insert(0) += 1;
    }
    m
}

/// The multiset delta transcribed from `dpsw.qnt` `censusDelta` (lines ~626-633): a planned
/// surplus is a create count, an observed surplus a destroy count.
fn delta_twin(
    planned: &BTreeMap<ObservableSig, i64>,
    observed: &BTreeMap<ObservableSig, i64>,
) -> CensusDelta {
    let count =
        |m: &BTreeMap<ObservableSig, i64>, s: &ObservableSig| m.get(s).copied().unwrap_or(0);
    let mut delta = CensusDelta::default();
    for sig in planned.keys().chain(observed.keys()) {
        let (want, have) = (count(planned, sig), count(observed, sig));
        if want > have {
            delta.creates.insert(sig.clone(), want - have);
        } else if have > want {
            delta.destroys.insert(sig.clone(), have - want);
        }
    }
    delta
}

/// The census judge transcribed from `dpsw.qnt` `judgeCensus`/`observedSigsOf` (lines ~644-659):
/// any unobservable member poisons the whole face (judges nothing); only an all-observed census
/// yields a delta (DPSW-I7; ADR-0018).
fn judge_twin(planned: &[ObservableSig], faces: &[ObservedFace]) -> FamilyJudgment {
    if faces
        .iter()
        .any(|f| matches!(f, ObservedFace::SigUnobservable))
    {
        return FamilyJudgment::JudgesNothing;
    }
    let observed: Vec<ObservableSig> = faces
        .iter()
        .filter_map(|f| match f {
            ObservedFace::SigObserved(sig) => Some(sig.clone()),
            ObservedFace::SigUnobservable => None,
        })
        .collect();
    FamilyJudgment::Judges(delta_twin(&census_twin(planned), &census_twin(&observed)))
}

/// A distinct observable signature per key, so generated multisets collide and differ by
/// multiplicity. The fields are plain — a census compares signature identity, not bindability.
fn sig_k(k: u8) -> ObservableSig {
    ObservableSig {
        num_ifs: u16::from(k) + 1,
        options: BTreeSet::new(),
        max_vlans: 16,
        max_fdbs: u16::from(k) + 1,
        max_fdb_entries: 1024,
        fdb_aging_time: 300,
        max_fdb_mc_groups: 32,
        component_type: ComponentType::CVlan,
        flooding_cfg: FloodingCfg::PerFdb,
        broadcast_cfg: BroadcastCfg::PerFdb,
    }
}

proptest! {
    /// Twin a — the `classify` twin agrees with `DpswCfg::new` over the whole bindability
    /// surface: the same accept/refuse verdict AND, on a refusal, the same variant. num_ifs vs
    /// max_fdbs straddles the `max_fdbs >= num_ifs` clause, the two domains straddle PER_FDB,
    /// and the ctrl-if flag straddles the control-interface clause — no one-sided refusal in
    /// either direction (`dpsw.qnt` `classify`).
    #[test]
    fn classify_twin_agrees_with_the_constructor(
        num_ifs in 1u16..=10,
        max_fdbs in 0u16..=10,
        flooding in prop_oneof![Just(FloodingCfg::PerVlan), Just(FloodingCfg::PerFdb)],
        broadcast in prop_oneof![Just(BroadcastCfg::PerObject), Just(BroadcastCfg::PerFdb)],
        ctrl_if_dis in any::<bool>(),
    ) {
        let verdict = classify_twin(num_ifs, max_fdbs, flooding, broadcast, ctrl_if_dis);
        let mut options = BTreeSet::new();
        if ctrl_if_dis {
            options.insert(DpswOpt::CtrlIfDis);
        }
        let create = DpswCreate {
            num_ifs: NumIfs::new(num_ifs).unwrap(),
            options,
            max_vlans: MaxVlans::new(16).unwrap(),
            max_fdbs: MaxFdbs::new(max_fdbs).unwrap(),
            max_fdb_entries: MaxFdbEntries::new(1024).unwrap(),
            fdb_aging_time: FdbAgingTime::new(300).unwrap(),
            max_fdb_mc_groups: MaxFdbMcGroups::new(32).unwrap(),
            mem_size: 0,
            component_type: ComponentType::CVlan,
            flooding_cfg: flooding,
            broadcast_cfg: broadcast,
            max_meters_per_if: 0,
        };
        match (verdict, DpswCfg::new(create)) {
            (None, Ok(cfg)) => {
                prop_assert_eq!(cfg.num_ifs(), num_ifs);
                prop_assert_eq!(cfg.max_fdbs(), max_fdbs);
                prop_assert_eq!(cfg.flooding_cfg(), flooding);
                prop_assert_eq!(cfg.broadcast_cfg(), broadcast);
            }
            (Some(r), Err(e)) => prop_assert_eq!(r, e, "refusal class disagrees"),
            (None, Err(e)) => prop_assert!(false, "model accepts but DpswCfg::new refuses {:?}", e),
            (Some(r), Ok(_)) => prop_assert!(false, "model refuses {:?} but DpswCfg::new accepts", r),
        }
    }

    /// Twin b — the census judge twin agrees with `plan::dpsw::judge_census` over generated
    /// planned-sig lists and observed faces, including the poison rule: any unobservable member
    /// judges nothing, never a spurious create/destroy (`dpsw.qnt` `judgeCensus`; DPSW-I7).
    #[test]
    fn census_twin_agrees_with_judge_census(
        planned_keys in prop::collection::vec(0u8..3, 0..6),
        face_keys in prop::collection::vec(prop::option::of(0u8..3), 0..6),
    ) {
        let planned: Vec<ObservableSig> = planned_keys.iter().map(|&k| sig_k(k)).collect();
        let faces: Vec<ObservedFace> = face_keys
            .iter()
            .map(|k| match k {
                Some(k) => ObservedFace::SigObserved(sig_k(*k)),
                None => ObservedFace::SigUnobservable,
            })
            .collect();
        prop_assert_eq!(judge_twin(&planned, &faces), judge_census(&planned, &faces));
    }

    /// Twin c — the `dpswPortBound` twin agrees with the Rust port-bound judgment over
    /// generated (num_ifs, port) pairs: a dpsw endpoint at port M is lawful iff M < num_ifs,
    /// and a non-dpsw endpoint is vacuously lawful (`models/core/connect.qnt` `dpswPortBound`;
    /// DPSW-I10).
    #[test]
    fn port_bound_twin_agrees_with_the_judgment(
        num_ifs in 0u32..=8,
        port in 0u32..=8,
    ) {
        prop_assert_eq!(dpsw_port_bound(Family::Dpsw, port, num_ifs), port < num_ifs);
        prop_assert!(dpsw_port_bound(Family::Dpni, port, num_ifs), "non-dpsw is vacuously lawful");
    }
}

/// The committed trace set is exactly the `TRACES` table — a dropped or unlisted trace file
/// fails CI rather than silently shrinking coverage (mirrors `dpseci_replay.rs`).
#[test]
fn every_committed_trace_is_listed() {
    let dir = format!(
        "{}/../../models/traces/families/dpsw",
        env!("CARGO_MANIFEST_DIR")
    );
    let on_disk: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {dir}: {e}"))
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".itf.json").map(str::to_owned))
        .collect();
    for (file, _) in TRACES {
        assert!(
            on_disk.contains(*file),
            "listed trace {file} is missing on disk"
        );
    }
    assert_eq!(
        on_disk.len(),
        TRACES.len(),
        "an unlisted trace file is present"
    );
}
