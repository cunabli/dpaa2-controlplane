//! The dpni create-surface ITF-replay CI rung (dpni-typestate task 5.1, design D8): every
//! committed dpni trace replays green through the task-2.1/2.2 typestate core, board-free.
//! The model is the oracle — a regenerated trace whose world the Rust core no longer
//! reproduces fails here loudly; regenerate with `pnpm model:freeze-dpni` and reconcile
//! the transcription.
//!
//! Each trace is a directed run of `models/families/dpni.qnt` `dpni_scenario`; the ITF
//! carries the `world` states but not the action taken between them, so the replayer
//! infers the transition from the state delta (mirroring the model's action set) and
//! drives the real [`dpaa2_api::families::dpni`] core through it. The conformance is the
//! **observation** projection (dpni-typestate design D4): a created dpni is compared as its
//! read-back [`DpniObservation`], which drops write-only `dist_key_size`, so the write-only
//! trace's `dist_key_size = 56` is created (accept parity, checked at decode) yet never
//! rides the comparison. Refusal steps carry the refused class in the frozen `lastOutcome`;
//! the replayer drives the matching core refusal witness (dpni-typestate design D2).
//!
//! A create block whose frozen field falls outside the board-verified envelope has no Rust
//! constructor, so the decode fails and the replay panics — a model↔core divergence made
//! loud, exactly what this rung exists to catch.

use std::collections::BTreeSet;

use dpaa2_api::core::model::MacAddr;
use dpaa2_api::families::dpni::{
    DistKeySize, Dpni, DpniDisposition, DpniObservation, InterfaceConstruct, NumQueues,
    ProfileOutcome, derive_profile, drift_disposition,
};
use dpaa2_api::intent::Dataplane;
use dpaa2_verify::intent::dpni_itf::{
    DpniOutcome, DpniPhase, DpniRefusal, DpniWorld, parse_dpni_trace,
};

/// Every committed trace under `models/traces/families/dpni/`, with the model face it pins
/// (the acceptance-criterion coverage list).
const TRACES: &[(&str, &str)] = &[
    (
        "scenarioPmdCreateReadbackTest",
        "PMD-profile create + read-back (16q/16tc, raw escape)",
    ),
    (
        "scenarioKernelCreateReadbackTest",
        "kernel-profile create + read-back (1q/1tc)",
    ),
    (
        "scenarioEnvelopeRefusedTest",
        "envelope refusal: num_queues 33 → RangeViolation, nothing created",
    ),
    (
        "scenarioDeadOptionRefusedTest",
        "dead-option refusal: --max-senders named, nothing created",
    ),
    (
        "scenarioNumRxTcsRefusedTest",
        "num_rx_tcs refusal: never-settable, shares the parity class",
    ),
    (
        "scenarioWriteOnlyDistKeySizeTest",
        "write-only dist_key_size 56 created, excluded from the read-back",
    ),
    (
        "scenarioPrimaryMacMutationTest",
        "primary-MAC mutation: runtime slot moves, create block untouched",
    ),
];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/traces/families/dpni/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// A trace/core disagreement — a FINDING, surfaced loudly, never papered over.
fn finding(file: &str, step: usize, msg: &str) -> ! {
    panic!("{file}: trace/core disagreement at step {step}: {msg}");
}

/// The comparable projection of a dpni world: a created dpni reduced to its read-back
/// [`DpniObservation`] (write-only `dist_key_size` dropped, dpni-typestate design D4), the
/// runtime MAC, and the recorded outcome. Both the frozen world and the Rust-driven core
/// project to this, and the replay's `==` is the whole conformance check.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Observed {
    dpni: Option<DpniObservation>,
    primary_mac: MacAddr,
    last_outcome: DpniOutcome,
}

/// The frozen world projected to its observable surface.
fn from_frozen(w: &DpniWorld) -> Observed {
    Observed {
        dpni: match &w.dpni {
            DpniPhase::Absent => None,
            DpniPhase::Created(cfg) => Some(DpniObservation::project(cfg)),
        },
        primary_mac: w.primary_mac,
        last_outcome: w.last_outcome,
    }
}

/// The running Rust core projected to the same observable surface.
fn from_core(handle: Option<&Dpni>, mac: MacAddr, last: DpniOutcome) -> Observed {
    Observed {
        dpni: handle.map(|d| DpniObservation::project(d.cfg())),
        primary_mac: mac,
        last_outcome: last,
    }
}

/// Drives the core refusal witness the frozen refusal names (dpni-typestate design D2): the
/// specific refused input is not recoverable from an `Absent` world, so the conformance is
/// the refusal vocabulary and its core disposition, exactly what the model records.
fn witness_refusal(file: &str, step: usize, r: DpniRefusal) {
    match r {
        // The envelope refusal (num_queues 33 > 32): an out-of-envelope count has no
        // refined constructor (dpni.qnt `createOutOfEnvelopeRefusedTest`).
        DpniRefusal::RangeViolation => {
            assert!(
                NumQueues::new(33).is_err(),
                "{file}: an out-of-envelope num_queues must have no constructor"
            );
        }
        // The dead option / num_rx_tcs is named and refused, and folds into the crate's
        // error idiom (dpni.qnt `DeadOptionRefusal`).
        DpniRefusal::DeadOption(u) => {
            let refusal = u.refuse();
            assert_eq!(
                refusal.option(),
                u,
                "{file}: dead-option refusal names {u:?}"
            );
            assert!(
                refusal.to_string().contains(u.name()),
                "{file}: rendered refusal names the item"
            );
        }
        // UserspaceEvent is unpriced and derives no profile (dpni.qnt `deriveProfile`).
        DpniRefusal::UnpricedDataplane => {
            assert_eq!(
                derive_profile(Dataplane::UserspaceEvent, InterfaceConstruct::Loopback),
                ProfileOutcome::Unpriced,
                "{file}: UserspaceEvent derives no profile"
            );
        }
    }
    let _ = step;
}

/// Applies the transition inferred from `prev`→`next` to the running core, driving the real
/// typestate; returns the new handle, MAC, and recorded outcome.
fn apply(
    file: &str,
    step: usize,
    handle: Option<Dpni>,
    mac: MacAddr,
    prev: &DpniWorld,
    next: &DpniWorld,
) -> (Option<Dpni>, MacAddr, DpniOutcome) {
    // A refusal: the world is unchanged but for `lastOutcome`.
    if let DpniOutcome::Refused(r) = next.last_outcome {
        witness_refusal(file, step, r);
        return (handle, mac, DpniOutcome::Refused(r));
    }

    match (&prev.dpni, &next.dpni) {
        // create: Absent → Created(cfg). The decoded block is in-envelope (checked at
        // decode), so `Dpni::create` is infallible and carries it verbatim.
        (DpniPhase::Absent, DpniPhase::Created(cfg)) => {
            let dpni = Dpni::create(cfg.clone());
            assert_eq!(
                dpni.cfg(),
                cfg,
                "{file}: create must carry the block verbatim"
            );
            assert_eq!(
                dpni.primary_mac(),
                MacAddr::ZERO,
                "{file}: create starts the primary MAC at zero"
            );
            // The write-only law made load-bearing: the read-back is invariant under any
            // in-range dist_key_size (dpni-typestate design D4).
            let mut zeroed = cfg.clone();
            zeroed.dist_key_size = DistKeySize::DEFAULT;
            assert_eq!(
                DpniObservation::project(cfg),
                DpniObservation::project(&zeroed),
                "{file}: dist_key_size must not ride the observation"
            );
            (Some(dpni), MacAddr::ZERO, DpniOutcome::Accepted)
        }
        // primary-MAC mutation: Created(cfg) → Created(cfg), MAC moves. The one runtime
        // mutation; the immutable create block is untouched (dpni-typestate design D1).
        (DpniPhase::Created(cfg), DpniPhase::Created(next_cfg))
            if cfg == next_cfg && next.primary_mac != mac =>
        {
            let mut dpni =
                handle.unwrap_or_else(|| finding(file, step, "mac mutation without a live dpni"));
            let new_mac = next.primary_mac;
            dpni.set_primary_mac(new_mac);
            assert_eq!(
                dpni.cfg(),
                cfg,
                "{file}: the create block stays immutable under a MAC set"
            );
            assert_eq!(
                drift_disposition(cfg, new_mac, &DpniObservation::project(cfg), mac),
                DpniDisposition::PrimaryMacMutation,
                "{file}: a MAC-only difference plans the mutation"
            );
            (Some(dpni), new_mac, DpniOutcome::Accepted)
        }
        _ => finding(
            file,
            step,
            &format!(
                "no transition mirrors delta {:?} → {:?}",
                prev.dpni, next.dpni
            ),
        ),
    }
}

#[test]
fn dpni_traces_replay_green() {
    for (file, face) in TRACES {
        let states = parse_dpni_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            states.len() >= 2,
            "{file}: a directed run has at least two states"
        );

        // The Rust world starts at `init`: no dpni, a zero primary MAC.
        let mut handle: Option<Dpni> = None;
        let mut mac = MacAddr::ZERO;
        let mut last = DpniOutcome::Accepted;
        assert_eq!(
            from_core(handle.as_ref(), mac, last),
            from_frozen(&states[0]),
            "{file} ({face}): initial world diverges"
        );

        for (i, next) in states.iter().enumerate().skip(1) {
            let prev = &states[i - 1];
            let (new_handle, new_mac, outcome) = apply(file, i, handle, mac, prev, next);
            handle = new_handle;
            mac = new_mac;
            last = outcome;
            assert_eq!(
                from_core(handle.as_ref(), mac, last),
                from_frozen(next),
                "{file} ({face}): world diverges after step {i}"
            );
        }
    }
}

/// A tampered expectation must break the diff — the replay is load-bearing, not a rubber
/// stamp (mirrors `dprc_replay.rs`'s divergence guard).
#[test]
fn replay_detects_a_diverging_world() {
    let states = parse_dpni_trace(&load("scenarioPmdCreateReadbackTest")).unwrap();
    // Drive the real create, then compare against a tampered expectation whose num_queues
    // was flipped: the projection must differ.
    let (handle, mac, last) = apply(
        "scenarioPmdCreateReadbackTest",
        1,
        None,
        MacAddr::ZERO,
        &states[0],
        &states[1],
    );
    let DpniPhase::Created(cfg) = &states[1].dpni else {
        panic!("state 1 is a created dpni");
    };
    let mut tampered_cfg = cfg.clone();
    tampered_cfg.num_queues = NumQueues::new(1).unwrap();
    let tampered = Observed {
        dpni: Some(DpniObservation::project(&tampered_cfg)),
        primary_mac: MacAddr::ZERO,
        last_outcome: DpniOutcome::Accepted,
    };
    assert_ne!(
        from_core(handle.as_ref(), mac, last),
        tampered,
        "a flipped num_queues must diverge from the driven world"
    );
}

/// The frozen trace set covers every model face the freeze contract names, so a dropped
/// trace file fails CI rather than silently shrinking coverage.
#[test]
fn every_committed_trace_is_listed() {
    let dir = format!(
        "{}/../../models/traces/families/dpni",
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

/// The decoder rejects a create block whose frozen field falls outside the board-verified
/// envelope — the loud model↔core divergence the rung exists to catch (a hand-built trace
/// stands in for a regenerated one the core no longer reproduces).
#[test]
fn decode_rejects_an_out_of_envelope_create() {
    // A minimal ITF world whose Created block carries num_queues 33 (> 32).
    let json = r##"{"states":[{"world":{
        "dpni":{"tag":"Created","value":{
            "options":{"flags":{"#set":[]},"escapes":{"#set":[]}},
            "numQueues":{"#bigint":"33"},"numTcs":{"#bigint":"0"},
            "macFilterEntries":{"#bigint":"0"},"vlanFilterEntries":{"#bigint":"0"},
            "qosEntries":{"#bigint":"0"},"fsEntries":{"#bigint":"0"},
            "numCgs":{"#bigint":"0"},"distKeySize":{"#bigint":"0"},
            "numCeetmCh":{"#bigint":"0"},"numOpr":{"#bigint":"0"},
            "rootContainer":false}},
        "primaryMac":[{"#bigint":"0"},{"#bigint":"0"},{"#bigint":"0"},{"#bigint":"0"},{"#bigint":"0"},{"#bigint":"0"}],
        "lastOutcome":{"tag":"Accepted","value":{"#tup":[]}}}}]}"##;
    let err = parse_dpni_trace(json).expect_err("num_queues 33 must have no constructor");
    assert!(err.contains("numQueues"), "{err}");
}
