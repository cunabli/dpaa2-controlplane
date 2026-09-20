//! The dpio-lifecycle ITF-replay CI rung (pool-objects task 4.1): every committed dpio trace
//! replays green through the task-2.3 seat surface, board-free. The model is the oracle — a
//! regenerated trace whose seats the Rust pure functions no longer reproduce fails here
//! loudly; regenerate with `pnpm model:freeze-dpio` and reconcile the transcription.
//!
//! Each trace is a directed run of `models/families/dpio.qnt` `dpio_lifecycle`. The ITF
//! carries the two state vars (`s`/`dpioSeats`) but not the action between them, so the
//! replayer reduces each state to the seat register and infers the transition from the seat
//! delta (mirroring `dpio_lifecycle`'s action set). The dpio→dpmcp probe draw is
//! adapter-procedural with no api-side surface (pool-objects design D4), so `probeDrawTest` /
//! `probeExhaustionDisabledTest` replay state conformance only; the api-side oracles are seat
//! admission (`admit_seat`, DPIO-I2) and the mode-dead priorities surface (DPIO-I3).
//!
//! A `numPriorities` outside the `1..=8` MC range has no [`Priorities`] constructor, so it
//! fails at decode — a model↔core divergence made loud.
//!
//! [`Priorities`]: dpaa2_api::families::dpio::Priorities

use std::collections::BTreeSet;

use dpaa2_api::families::dpio::{
    ChannelMode, SeatCeilingExceeded, SeatRegime, admit_seat, seat_ceiling, seat_count,
};
use dpaa2_verify::intent::dpio_itf::{DpioStep, DpioWorld, parse_dpio_trace};

/// The model's regime ceilings lifted to the Rust seat arithmetic (`dpio.qnt` `NUM_CPUS = 2`,
/// `NUM_THREADS = 1`; DPIO-I2).
const CPUS: i64 = 2;
const THREADS: i64 = 1;

/// Every committed trace under `models/traces/families/dpio/`, with the model face it pins.
const TRACES: &[(&str, &str)] = &[
    (
        "probeDrawTest",
        "DPIO-I1/DPMCP-I1: the probe draws one free dpmcp (state conformance only)",
    ),
    (
        "probeExhaustionDisabledTest",
        "DPIO-I1: an exhausted dpmcp pool disables the probe (state conformance only)",
    ),
    (
        "seatBoundRefusedTest",
        "DPIO-I2: kernel seats top out at NUM_CPUS; the next create is -ERANGE",
    ),
    (
        "noChannelReportsPrioritiesTest",
        "DPIO-I3: a NoChannel seat still reports its priorities (mode is dead)",
    ),
];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/traces/families/dpio/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// A trace/core disagreement — a FINDING, surfaced loudly, never papered over.
fn finding(file: &str, step: usize, msg: &str) -> ! {
    panic!("{file}: trace/core disagreement at step {step}: {msg}");
}

/// The per-state conformance: the DPIO-I2 seat envelope holds for every regime, and every
/// seat's capability surface reads the priority count alone (DPIO-I3).
fn check_state(file: &str, i: usize, w: &DpioWorld) {
    let seats = w.seat_vec();
    for regime in SeatRegime::SEAT_REGIMES {
        if seat_count(&seats, regime) > seat_ceiling(regime, CPUS, THREADS) {
            finding(file, i, "a regime's seats exceed the ceiling (DPIO-I2)");
        }
    }
    for seat in &seats {
        // A validly decoded seat refines out priority 0, so it is always notify-capable, and
        // eight-priority exactly at the count — never a function of the (dead) channel mode.
        assert!(
            seat.notify_capable(),
            "{file} step {i}: a seat is notify-capable"
        );
        assert_eq!(
            seat.eight_priority(),
            seat.cfg().priorities.get() == 8,
            "{file} step {i}: eight-priority reads the count"
        );
    }
}

/// The per-transition conformance: a seat added by `createDpioAt` was admitted by the
/// regime's seat gate over the prior register (DPIO-I2 `-ERANGE` guard).
fn check_transition(file: &str, i: usize, prev: &DpioWorld, w: &DpioWorld) {
    let before = prev.seat_vec();
    for (num, seat) in &w.seats {
        if !prev.seats.contains_key(num)
            && admit_seat(&before, seat.regime(), CPUS, THREADS).is_err()
        {
            finding(file, i, "a seat was created past its disabled regime gate");
        }
    }
}

/// The last decoded world of a run (the state before a `.fail()` sentinel, or the final
/// state).
fn last_world(steps: &[DpioStep]) -> &DpioWorld {
    steps
        .iter()
        .rev()
        .find_map(|s| match s {
            DpioStep::World(w) => Some(w),
            DpioStep::Refused => None,
        })
        .expect("a run has at least one world")
}

#[test]
fn dpio_traces_replay_green() {
    for (file, face) in TRACES {
        let steps = parse_dpio_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            matches!(steps.first(), Some(DpioStep::World(_))),
            "{file} ({face}): a directed run starts at a world"
        );
        let mut prev: Option<&DpioWorld> = None;
        for (i, step) in steps.iter().enumerate() {
            match step {
                DpioStep::Refused => assert_eq!(
                    i,
                    steps.len() - 1,
                    "{file} ({face}): a refused (.fail) step must be terminal"
                ),
                DpioStep::World(w) => {
                    check_state(file, i, w);
                    if let Some(p) = prev {
                        check_transition(file, i, p, w);
                    }
                    prev = Some(w);
                }
            }
        }
    }
}

/// DPIO-I2: kernel seats top out at `NUM_CPUS`; the frozen `.fail()` tail leaves the
/// ceiling-full register as the last world, over which the seat gate refuses the next create
/// with the `-ERANGE` shape.
#[test]
fn seat_create_refused_at_the_regime_ceiling() {
    let steps = parse_dpio_trace(&load("seatBoundRefusedTest")).unwrap();
    let seats = last_world(&steps).seat_vec();
    let err = admit_seat(&seats, SeatRegime::KernelSeat, CPUS, THREADS)
        .expect_err("kernel seats are at the ceiling");
    assert_eq!(
        err,
        SeatCeilingExceeded {
            regime: SeatRegime::KernelSeat,
            count: CPUS,
            ceiling: CPUS,
        }
    );
}

/// DPIO-I3 (Breaking): a `NoChannel` seat at eight priorities still reports capable and
/// eight-priority — the reported count decides, the mode is dead in the kernel.
#[test]
fn no_channel_still_reports_priorities() {
    let steps = parse_dpio_trace(&load("noChannelReportsPrioritiesTest")).unwrap();
    let w = last_world(&steps);
    let seat = w.seats.values().next().expect("the run creates one dpio");
    assert_eq!(seat.cfg().mode, ChannelMode::NoChannel);
    assert!(seat.notify_capable(), "a NoChannel seat is still capable");
    assert!(seat.eight_priority(), "eight priorities are still reported");
}

/// The dpio→dpmcp probe draw is adapter-procedural (pool-objects design D4): the seat surface
/// records only the observation flag, so this run replays state conformance — the seat is
/// probed and the drawn dpmcp is held by that dpio.
#[test]
fn probe_draw_records_the_observation_and_draws_a_dpmcp() {
    let steps = parse_dpio_trace(&load("probeDrawTest")).unwrap();
    let w = last_world(&steps);
    let (dpio_num, seat) = w.seats.iter().next().expect("the run creates one dpio");
    assert!(seat.probed(), "the probed observation flag is set");
    let dpmcp = w
        .cores
        .iter()
        .find(|((fam, _), _)| fam.as_str() == "dpmcp")
        .expect("the run creates one dpmcp");
    assert_eq!(
        dpmcp.1.allocated_by,
        Some((dpaa2_api::core::family::Family::Dpio, *dpio_num)),
        "the probed dpio holds the drawn dpmcp"
    );
}

/// A tampered register must break the diff — the replay is load-bearing, not a rubber stamp.
/// The ceiling-full register refuses; the same register missing one seat admits.
#[test]
fn replay_detects_a_diverging_world() {
    let steps = parse_dpio_trace(&load("seatBoundRefusedTest")).unwrap();
    let mut seats = last_world(&steps).seat_vec();
    let full = admit_seat(&seats, SeatRegime::KernelSeat, CPUS, THREADS).is_ok();
    seats.pop();
    let with_headroom = admit_seat(&seats, SeatRegime::KernelSeat, CPUS, THREADS).is_ok();
    assert_ne!(
        full, with_headroom,
        "dropping a seat must open the gate the full register closed"
    );
}

/// The committed trace set is exactly the `TRACES` table — a dropped or unlisted trace file
/// fails CI rather than silently shrinking coverage.
#[test]
fn every_committed_trace_is_listed() {
    let dir = format!(
        "{}/../../models/traces/families/dpio",
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

/// The decoder rejects a `numPriorities` outside the `1..=8` MC create range — the envelope
/// made loud (a hand-built seat stands in for a regenerated trace the core no longer
/// reproduces).
#[test]
fn decode_rejects_priorities_out_of_range() {
    let json = r##"{"states":[{"s":{"objs":{"#map":[]}},"dpioSeats":{"#map":[
        [{"fam":{"tag":"Dpio","value":{"#tup":[]}},"num":{"#bigint":"0"}},
         {"mode":{"tag":"LocalChannel","value":{"#tup":[]}},"numPriorities":{"#bigint":"9"},
          "probed":false,"regime":{"tag":"KernelSeat","value":{"#tup":[]}}}]]}}]}"##;
    let err = parse_dpio_trace(json).expect_err("numPriorities 9 has no constructor");
    assert!(err.contains("1..=8"), "{err}");
}
