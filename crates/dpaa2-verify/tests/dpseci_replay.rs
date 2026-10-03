//! The dpseci create-surface ITF-replay CI rung (dpseci-typestate task 2.3): every committed
//! dpseci trace replays green through the task-2.1 create-surface judgments, board-free. The
//! model is the oracle — a regenerated trace whose world the Rust judgments no longer reproduce
//! fails here loudly; regenerate with `pnpm model:freeze-dpseci` and reconcile the transcription.
//!
//! Each trace is a directed run of `models/families/dpseci.qnt` `dpseci_lifecycle`. The ITF
//! carries the `world` state but not the action between two states, so the replayer reduces each
//! state to its [`DpseciWorld`] vocabulary and asserts, at every state, that the Rust judgments
//! agree with the frozen fields (dpseci-typestate design D1):
//!
//!   - a model-created block is one `DpseciCfg::new` accepts, reading back the same queue count,
//!     priorities and options (the accept direction of class-for-class agreement);
//!   - DPSECI-I4: `DpseciCfg::congestion_backstop` equals whether `HAS_CG` is set — the backstop
//!     with-CG, no backstop without-CG;
//!   - a raw escape rides the mask attributed, never merged into the named flags;
//!   - a refusal left nothing created (`DpseciPhase::Absent`).
//!
//! The property twin transcribes the model's `classify` order (queue envelope → count match →
//! per-entry range, the restool parse order, V-DPSECI-1 rev 1) and asserts it agrees with
//! `DpseciCfg::new` over the whole cfg surface — the same accept/refuse verdict AND the same
//! refusal variant — which pins length-coupling and both range judgments with no one-sided
//! refusal in either direction.

use std::collections::BTreeSet;

use dpaa2_api::families::dpseci::{DpseciCfg, DpseciOpt, OptionMask, RawEscape, Refusal};
use dpaa2_verify::intent::dpseci_itf::{DpseciPhase, DpseciWorld, RawCfg, parse_dpseci_trace};
use proptest::prelude::*;

/// Every committed trace under `models/traces/families/dpseci/`, with the model face it pins.
const TRACES: &[(&str, &str)] = &[
    (
        "createAcceptedReadbackTest",
        "production child create + read-back (8q/prio2/HAS_CG+HAS_OPR+OPR_SHARED)",
    ),
    (
        "createRawEscapeReadbackTest",
        "raw escape 0x100 rides the mask attributed, not merged into flags",
    ),
    (
        "priorityZeroRefusedTest",
        "priority 0 refused → PriorityOutOfRange, nothing created",
    ),
    (
        "priorityAboveEightRefusedTest",
        "priority above 8 refused → PriorityOutOfRange, nothing created",
    ),
    (
        "priorityCountMismatchRefusedTest",
        "priorities count ≠ num-queues → PriorityCountMismatch, nothing created",
    ),
    (
        "queueCountOutOfRangeRefusedTest",
        "num-queues 17 > 16 → QueueCountOutOfRange, nothing created",
    ),
    (
        "congestionBirthWithCgTest",
        "DPSECI-I4 with HAS_CG: the kernel block carries the backstop",
    ),
    (
        "congestionBirthWithoutCgTest",
        "DPSECI-I4 without HAS_CG: the production shape carries no backstop",
    ),
];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/traces/families/dpseci/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// A trace/core disagreement — a FINDING, surfaced loudly (mirrors `dpni_replay.rs`).
fn finding(file: &str, step: usize, msg: &str) -> ! {
    panic!("{file}: trace/core disagreement at step {step}: {msg}");
}

/// The per-state conformance: a created block is accepted-and-read-back by `DpseciCfg::new`
/// (DPSECI-I4 backstop and escape attribution with it), and a refusal left nothing created.
fn check_state(file: &str, i: usize, w: &DpseciWorld) {
    match (&w.dpseci, w.last_outcome) {
        (DpseciPhase::Created(raw), None) => check_created(file, i, raw),
        (DpseciPhase::Absent, _) => {}
        (DpseciPhase::Created(_), Some(r)) => finding(
            file,
            i,
            &format!("a created object carries a refusal {r:?} — model↔core contradiction"),
        ),
    }
}

/// A model-created block: `DpseciCfg::new` accepts exactly it, reads it back verbatim, and the
/// congestion backstop (DPSECI-I4) tracks `HAS_CG`.
fn check_created(file: &str, i: usize, raw: &RawCfg) {
    let cfg = DpseciCfg::new(raw.options.clone(), raw.num_queues, raw.priorities.clone())
        .unwrap_or_else(|r| {
            finding(
                file,
                i,
                &format!("model created a cfg DpseciCfg::new refuses as {r:?}"),
            )
        });
    if cfg.num_queues() != raw.num_queues {
        finding(file, i, "num_queues reads back changed");
    }
    if cfg.priorities() != raw.priorities.as_slice() {
        finding(file, i, "priorities read back changed");
    }
    if cfg.options() != &raw.options {
        finding(file, i, "options read back changed (flag or escape drift)");
    }
    if cfg.congestion_backstop() != raw.options.contains(DpseciOpt::HasCg) {
        finding(
            file,
            i,
            "congestion_backstop disagrees with the HAS_CG bit (DPSECI-I4)",
        );
    }
}

#[test]
fn dpseci_traces_replay_green() {
    for (file, face) in TRACES {
        let states = parse_dpseci_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            states.len() >= 2,
            "{file} ({face}): a directed run has at least two states"
        );
        assert_eq!(
            states[0],
            DpseciWorld {
                dpseci: DpseciPhase::Absent,
                last_outcome: None,
            },
            "{file} ({face}): a directed run starts at init (Absent, Accepted)"
        );
        for (i, w) in states.iter().enumerate() {
            check_state(file, i, w);
        }
    }
}

/// The refusal traces leave nothing created and carry the class the model recorded — proving the
/// refuse direction at the state level (the full class-for-class agreement is the property twin).
#[test]
fn refusal_traces_create_nothing_and_name_the_class() {
    let expected: &[(&str, Refusal)] = &[
        ("priorityZeroRefusedTest", Refusal::PriorityOutOfRange),
        ("priorityAboveEightRefusedTest", Refusal::PriorityOutOfRange),
        (
            "priorityCountMismatchRefusedTest",
            Refusal::PriorityCountMismatch,
        ),
        (
            "queueCountOutOfRangeRefusedTest",
            Refusal::QueueCountOutOfRange,
        ),
    ];
    for (file, class) in expected {
        let states = parse_dpseci_trace(&load(file)).unwrap();
        let last = states.last().unwrap();
        assert_eq!(
            last.dpseci,
            DpseciPhase::Absent,
            "{file}: a refusal creates nothing"
        );
        assert_eq!(
            last.last_outcome,
            Some(*class),
            "{file}: the frozen refusal names {class:?}"
        );
    }
}

/// The raw-escape trace keeps the escape attributed: it rides the escapes set with its raw
/// value, and the named flags carry only `HAS_CG` — never a merged escape (`dpseci.qnt`
/// `RAW_ESCAPE_CFG` / `createRawEscapeReadbackTest`).
#[test]
fn raw_escape_rides_attributed_not_merged() {
    let states = parse_dpseci_trace(&load("createRawEscapeReadbackTest")).unwrap();
    let DpseciPhase::Created(raw) = &states[1].dpseci else {
        panic!("state 1 is a created dpseci");
    };
    assert!(
        raw.options.contains_escape(RawEscape::new(0x100)),
        "the escape rides the escapes set"
    );
    assert_eq!(raw.options.escapes().len(), 1);
    assert_eq!(
        raw.options.flags().iter().copied().collect::<Vec<_>>(),
        vec![DpseciOpt::HasCg],
        "the named flags carry only HAS_CG — the escape is not merged in"
    );
}

/// The committed trace set is exactly the `TRACES` table — a dropped or unlisted trace file
/// fails CI rather than silently shrinking coverage (mirrors `dpni_replay.rs`).
#[test]
fn every_committed_trace_is_listed() {
    let dir = format!(
        "{}/../../models/traces/families/dpseci",
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

/// A transcription of the model's `classify` (`dpseci.qnt` `dpseci_lifecycle`): the queue
/// envelope first, then the priority count match, then the per-entry range — the restool parse
/// order (V-DPSECI-1 rev 1). `None` is `Accepted`; `Some(r)` is `Refused(r)`.
fn classify_twin(num_queues: usize, priorities: &[u8]) -> Option<Refusal> {
    if !(1..=DpseciCfg::MAX_QUEUE_NUM).contains(&num_queues) {
        Some(Refusal::QueueCountOutOfRange)
    } else if priorities.len() != num_queues {
        Some(Refusal::PriorityCountMismatch)
    } else if !priorities
        .iter()
        .all(|&p| (DpseciCfg::MIN_PRIORITY..=DpseciCfg::MAX_PRIORITY).contains(&p))
    {
        Some(Refusal::PriorityOutOfRange)
    } else {
        None
    }
}

proptest! {
    /// The `classify` twin agrees with `DpseciCfg::new` over the whole cfg surface: the same
    /// accept/refuse verdict AND, on a refusal, the same variant. The queue count straddles the
    /// 1..=16 envelope, the priority vector's length straddles the count match, and the entries
    /// straddle the 1..=8 range, so each judgment and the length-coupling are exercised both
    /// ways — no one-sided refusal in either direction (V-DPSECI-1 rev 1).
    #[test]
    fn classify_twin_agrees_with_the_constructor(
        num_queues in 0usize..=20,
        priorities in prop::collection::vec(0u8..=12, 0..20),
    ) {
        let verdict = classify_twin(num_queues, &priorities);
        let built = DpseciCfg::new(OptionMask::empty(), num_queues, priorities.clone());
        match (verdict, built) {
            (None, Ok(cfg)) => {
                prop_assert_eq!(cfg.num_queues(), num_queues);
                prop_assert_eq!(cfg.priorities(), priorities.as_slice());
            }
            (Some(r), Err(e)) => prop_assert_eq!(r, e, "refusal class disagrees"),
            (None, Err(e)) => {
                prop_assert!(false, "model accepts but DpseciCfg::new refuses {:?}", e);
            }
            (Some(r), Ok(_)) => {
                prop_assert!(false, "model refuses {:?} but DpseciCfg::new accepts", r);
            }
        }
    }
}
