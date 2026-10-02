//! The dpmac-lifecycle ITF-replay CI rung (dpmac-typestate task 2.3): every committed dpmac
//! trace replays green through the task-2.1 observation judgments and the task-2.2 typed edge
//! law, board-free. The model is the oracle — a regenerated trace whose world the Rust pure
//! functions no longer reproduce fails here loudly; regenerate with `pnpm model:freeze-dpmac`
//! and reconcile the transcription.
//!
//! Each trace is a directed run of `models/families/dpmac.qnt` `dpmac_lifecycle`. The ITF
//! carries the `world` state but not the action between two states, so the replayer reduces
//! each state to its [`DpmacWorld`] vocabulary and asserts, at every state, that the Rust
//! judgments agree with the frozen fields:
//!
//!   - DPMAC-I6: `judge_arbitration(peer) == arbitration` and
//!     `standalone_bound_for(peer) == standaloneBound == macNetdev`;
//!   - DPMAC-I2: the port MAC stays `BOOT_MAC` through every observation;
//!   - DPMAC-I3: the stable attribute projection is constant (anchored to the boot value);
//!   - `DPMAC_SeverOrder` (ADR-0008 §8): `kernelFace == KernelFaceReleased` implies
//!     `severed == Severed`.
//!
//! The edge law (`DPMAC_SeverOrder`) also replays through the typed plan surface: the
//! sever-then-unbind trace's two teardown transitions map to [`Transition::sever`] →
//! [`Transition::unbind`], and the order-inverted trace is REJECTED by construction — the
//! negative face — because [`Transition::unbind`] cannot be built without the [`SeveredProof`]
//! that only `sever` mints (ADR-0008 §8; dpmac-typestate task 2.2). The property twins pin the
//! counter vocabulary (DPMAC-I7) and the MAC-relation partition (dpmac-typestate design D5).

use std::collections::BTreeSet;

use dpaa2_api::core::model::{DpniId, MacAddr};
use dpaa2_api::families::dpmac::{
    Arbitration, BOOT_ATTRS, BOOT_MAC, Counter, CounterRead, FirmwareVersion, KernelFace,
    MacRelation, PeerObservation, SeveredWitness, counter_vocabulary, judge_arbitration,
    judge_mac_relation, read_counter, stable_attributes, standalone_bound_for,
};
use dpaa2_api::plan::Transition;
use dpaa2_verify::intent::dpmac_itf::{DpmacStep, DpmacWorld, parse_dpmac_trace};
use proptest::prelude::*;

/// Every committed trace under `models/traces/families/dpmac/`, with the model face it pins.
const TRACES: &[(&str, &str)] = &[
    (
        "offeredAtBootTest",
        "boot-born offer: Offered, standalone bound, macN present",
    ),
    (
        "kernelOwnedOnConnectTest",
        "same-container kernel dpni binds: KernelOwned, macN gone",
    ),
    (
        "remoteOwnedOnCrossContainerTest",
        "cross-container consumer: RemoteOwned, standalone keeps PHY",
    ),
    (
        "arbitrationHandbackTest",
        "disconnect hands the port back: KernelOwned → Offered",
    ),
    (
        "arbitrationLawTest",
        "DPMAC-I6 holds after each kind of observation",
    ),
    (
        "severThenUnbindTest",
        "ADR-0008 §8: sever, then unbind — the legal teardown order",
    ),
    (
        "hazardUnbindBeforeSeverRefusedTest",
        "ADR-0008 §8: unbind-before-sever is disabled (.fail)",
    ),
    (
        "severOrderLawTest",
        "DPMAC_SeverOrder holds after each teardown step",
    ),
];

/// A representative facing dpni for the typed edge law (the dpmac World carries no object id;
/// the sever/unbind ordering is independent of identity).
const EDGE_DPNI: DpniId = DpniId::new(7);

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/traces/families/dpmac/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// A trace/core disagreement — a FINDING, surfaced loudly (mirrors `pool_replay.rs`).
fn finding(file: &str, step: usize, msg: &str) -> ! {
    panic!("{file}: trace/core disagreement at step {step}: {msg}");
}

/// The per-state conformance: the arbitration equation (DPMAC-I6), MAC immutability
/// (DPMAC-I2), attribute constancy (DPMAC-I3), and the sever-order law (`DPMAC_SeverOrder`).
fn check_state(file: &str, i: usize, w: &DpmacWorld) {
    if judge_arbitration(w.peer) != w.arbitration {
        finding(
            file,
            i,
            "judge_arbitration(peer) disagrees with the frozen arbitration",
        );
    }
    let bound = standalone_bound_for(w.peer);
    if bound != w.standalone_bound || bound != w.mac_netdev {
        finding(
            file,
            i,
            "standalone_bound_for(peer) disagrees with standaloneBound/macNetdev",
        );
    }
    if w.mac_addr != BOOT_MAC {
        finding(file, i, "the port MAC drifted from BOOT_MAC (DPMAC-I2)");
    }
    if stable_attributes(&w.attributes) != stable_attributes(&BOOT_ATTRS) {
        finding(
            file,
            i,
            "the stable attribute projection drifted from boot (DPMAC-I3)",
        );
    }
    if w.kernel_face == KernelFace::KernelFaceReleased && w.severed != SeveredWitness::Severed {
        finding(
            file,
            i,
            "kernel-face released without a severed witness (DPMAC_SeverOrder, ADR-0008 §8)",
        );
    }
}

/// One edge-teardown action derived from a dpni↔dpmac World delta (ADR-0008 §8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EdgeAction {
    /// The edge was severed: `severed` advanced `NotSevered → Severed`.
    Sever,
    /// The kernel-face unbound: `kernelFace` advanced `KernelFaceBound → KernelFaceReleased`.
    Unbind,
}

/// The ordered edge-teardown actions a decoded run performs, inferred from the World deltas
/// (the ITF names no action; the sever/unbind steps are read from the two edge fields).
fn edge_actions(steps: &[DpmacStep]) -> Vec<EdgeAction> {
    let worlds: Vec<&DpmacWorld> = steps
        .iter()
        .filter_map(|s| match s {
            DpmacStep::World(w) => Some(w),
            DpmacStep::Refused => None,
        })
        .collect();
    let mut actions = Vec::new();
    for pair in worlds.windows(2) {
        let (prev, w) = (pair[0], pair[1]);
        if prev.severed == SeveredWitness::NotSevered && w.severed == SeveredWitness::Severed {
            actions.push(EdgeAction::Sever);
        }
        if prev.kernel_face == KernelFace::KernelFaceBound
            && w.kernel_face == KernelFace::KernelFaceReleased
        {
            actions.push(EdgeAction::Unbind);
        }
    }
    actions
}

/// Replays an edge-action sequence against the typed plan surface (dpmac-typestate task 2.2).
/// An unbind demands the [`SeveredProof`] a prior sever minted; with no proof in hand the
/// transition cannot be built, so the unbind-before-sever order is refused HERE — the law is
/// enforced by the type, not by this replayer's discipline (ADR-0008 §8).
fn replay_edge_law(actions: &[EdgeAction]) -> Result<Vec<Transition>, &'static str> {
    let mut proof = None;
    let mut out = Vec::new();
    for action in actions {
        match action {
            EdgeAction::Sever => {
                let (disconnect, p) = Transition::sever(EDGE_DPNI);
                proof = Some(p);
                out.push(disconnect);
            }
            EdgeAction::Unbind => {
                let Some(p) = proof.take() else {
                    return Err("unbind before sever: no severed proof to consume (ADR-0008 §8)");
                };
                out.push(Transition::unbind(EDGE_DPNI, p));
            }
        }
    }
    Ok(out)
}

fn last_world(steps: &[DpmacStep]) -> &DpmacWorld {
    steps
        .iter()
        .rev()
        .find_map(|s| match s {
            DpmacStep::World(w) => Some(w),
            DpmacStep::Refused => None,
        })
        .expect("a run has at least one world")
}

#[test]
fn dpmac_traces_replay_green() {
    for (file, face) in TRACES {
        let steps = parse_dpmac_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            matches!(steps.first(), Some(DpmacStep::World(_))),
            "{file} ({face}): a directed run starts at a world"
        );
        for (i, step) in steps.iter().enumerate() {
            match step {
                DpmacStep::Refused => assert_eq!(
                    i,
                    steps.len() - 1,
                    "{file} ({face}): a refused (.fail) step must be terminal"
                ),
                DpmacStep::World(w) => check_state(file, i, w),
            }
        }
    }
}

/// The committed trace set is exactly the `TRACES` table — a dropped or unlisted trace file
/// fails CI rather than silently shrinking coverage (mirrors `pool_replay.rs`).
#[test]
fn every_committed_trace_is_listed() {
    let dir = format!(
        "{}/../../models/traces/families/dpmac",
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

/// The legal teardown trace replays through the typed plan path (ADR-0008 §8; the typed edge
/// law from dpmac-typestate task 2.2): its sever/unbind step pair maps to [`Transition::sever`]
/// → [`Transition::unbind`], yielding the disconnect-then-unbind sequence.
#[test]
fn edge_law_replays_through_the_typed_surface() {
    let steps = parse_dpmac_trace(&load("severThenUnbindTest")).unwrap();
    let actions = edge_actions(&steps);
    assert_eq!(
        actions,
        vec![EdgeAction::Sever, EdgeAction::Unbind],
        "the frozen teardown is sever, then unbind"
    );
    let plan = replay_edge_law(&actions).expect("the legal order threads the proof");
    assert!(
        matches!(
            plan.as_slice(),
            [Transition::Disconnect { .. }, Transition::Unbind { .. }]
        ),
        "the typed path lowers to disconnect, then unbind: {plan:?}"
    );
}

/// The negative face (bead acceptance): the order-inverted teardown is REJECTED by the typed
/// surface. The mutation is built in-memory from the decoded trace's actions — inverting their
/// order to unbind-before-sever — and the replayer refuses it, because [`Transition::unbind`]
/// has no [`SeveredProof`] to consume before a sever mints one (ADR-0008 §8).
#[test]
fn inverted_edge_trace_is_rejected() {
    let steps = parse_dpmac_trace(&load("severThenUnbindTest")).unwrap();
    let mut inverted = edge_actions(&steps);
    inverted.reverse();
    assert_eq!(
        inverted,
        vec![EdgeAction::Unbind, EdgeAction::Sever],
        "the mutation inverts the step order"
    );
    assert!(
        replay_edge_law(&inverted).is_err(),
        "unbind-before-sever cannot be expressed against the typed surface"
    );
}

/// The §8 hazard refusal (`hazardUnbindBeforeSeverRefusedTest`): the frozen `.fail()` tail left
/// the port still kernel-bound and unsevered — the unbind did not fire, so no edge action is
/// derived and the kernel-face never reaches `KernelFaceReleased` (ADR-0008 §8).
#[test]
fn hazard_unbind_before_sever_leaves_state_unchanged() {
    let steps = parse_dpmac_trace(&load("hazardUnbindBeforeSeverRefusedTest")).unwrap();
    assert!(
        matches!(steps.last(), Some(DpmacStep::Refused)),
        "the hazard run ends at the disabled-guard sentinel"
    );
    let w = last_world(&steps);
    assert_eq!(
        w.kernel_face,
        KernelFace::KernelFaceBound,
        "the kernel-face is still bound"
    );
    assert_eq!(w.severed, SeveredWitness::NotSevered, "no edge was severed");
    assert!(
        edge_actions(&steps).is_empty(),
        "the refused unbind performs no edge action"
    );
}

/// A small MAC pool so the four MAC-relation classes and the vocabulary cases occur
/// non-vacuously: the zero address, the boot/burned-in port MAC, and two distinct others.
const MAC_POOL: [MacAddr; 4] = [
    MacAddr::ZERO,
    BOOT_MAC,
    MacAddr::new([2, 0, 0, 0, 0, 0x42]),
    MacAddr::new([2, 0, 0, 0, 0, 0x99]),
];

fn a_mac() -> impl Strategy<Value = MacAddr> {
    prop::sample::select(&MAC_POOL[..])
}

fn a_counter() -> impl Strategy<Value = Counter> {
    prop::sample::select(&Counter::ALL_COUNTERS[..])
}

fn a_firmware() -> impl Strategy<Value = FirmwareVersion> {
    prop_oneof![Just(FirmwareVersion::Mc1039), Just(FirmwareVersion::Mc1040)]
}

proptest! {
    /// DPMAC-I7 (`dpmac.qnt` `readCounter`): a read in the firmware's vocabulary is
    /// `Known(raw)`, one outside is `NotInVocabulary` — and never `Known(_)`, so absence ≠
    /// zero holds for every raw value. The returned constructor's defining predicate is
    /// asserted, which partitions the reads (`readCounter` is total).
    #[test]
    fn counter_vocabulary_partitions_reads(
        fw in a_firmware(),
        counter in a_counter(),
        raw in any::<u64>(),
    ) {
        let in_vocab = counter_vocabulary(fw).contains(&counter);
        match read_counter(fw, counter, raw) {
            CounterRead::Known(v) => {
                prop_assert!(in_vocab, "a Known read is in the vocabulary");
                prop_assert_eq!(v, raw, "a Known read carries the raw value");
            }
            CounterRead::NotInVocabulary => {
                prop_assert!(!in_vocab, "a NotInVocabulary read is outside the vocabulary");
            }
        }
        if !in_vocab {
            prop_assert_ne!(
                read_counter(fw, counter, raw),
                CounterRead::Known(0),
                "an absent counter is never Known(0) — absence ≠ zero (DPMAC-I7)"
            );
        }
    }

    /// dpmac-typestate design D5 (`judge_mac_relation`): the four classes partition the space.
    /// Pending is checked first (all-zeros, never drift); Overridden wins the tie with the
    /// burned-in address when intent declares the observed value; then Inherited; else
    /// Mismatched carrying whether intent declared a value. Asserting each returned class's
    /// defining predicate proves the partition, since the classes are mutually exclusive.
    #[test]
    fn mac_relation_partitions_the_space(
        observed in a_mac(),
        burned_in in a_mac(),
        intent in prop::option::of(a_mac()),
    ) {
        match judge_mac_relation(observed, burned_in, intent) {
            MacRelation::Pending => {
                prop_assert!(observed.is_zero());
            }
            MacRelation::Overridden => {
                prop_assert!(!observed.is_zero());
                prop_assert_eq!(intent, Some(observed), "Overridden means intent declared the observed value");
            }
            MacRelation::Inherited => {
                prop_assert!(!observed.is_zero());
                prop_assert_ne!(intent, Some(observed));
                prop_assert_eq!(observed, burned_in, "Inherited means the observed equals the burned-in MAC");
            }
            MacRelation::Mismatched { intent_declared } => {
                prop_assert!(!observed.is_zero());
                prop_assert_ne!(intent, Some(observed));
                prop_assert_ne!(observed, burned_in);
                prop_assert_eq!(intent_declared, intent.is_some(), "Mismatched carries whether intent declared");
            }
        }
    }
}

/// A sanity cross-check that the arbitration equation and the edge-law replay are not vacuous:
/// the three peer observations judge the three phases, and the sever-then-unbind path threads
/// exactly one proof (mirrors the per-state check against a known table).
#[test]
fn arbitration_table_is_covered_by_the_traces() {
    let mut seen = BTreeSet::new();
    for (file, _) in TRACES {
        if let Ok(steps) = parse_dpmac_trace(&load(file)) {
            for step in &steps {
                if let DpmacStep::World(w) = step {
                    seen.insert(w.arbitration);
                }
            }
        }
    }
    for phase in [
        Arbitration::Offered,
        Arbitration::KernelOwned,
        Arbitration::RemoteOwned,
    ] {
        assert!(seen.contains(&phase), "no trace exercises {phase:?}");
    }
    assert_eq!(
        judge_arbitration(PeerObservation::CrossContainerPeer),
        Arbitration::RemoteOwned
    );
}
