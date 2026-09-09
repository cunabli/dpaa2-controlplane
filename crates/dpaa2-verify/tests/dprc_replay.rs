//! The dprc-lifecycle ITF-replay CI rung (dprc-encapsulation task 2.3, design D2): every
//! committed dprc trace replays green through the task-2.1 typestate core, board-free.
//! The model is the oracle — a regenerated trace whose world the Rust core no longer
//! reproduces fails here loudly; regenerate with `pnpm model:freeze-dprc` and reconcile
//! the transcription.
//!
//! Each trace is a directed run of `models/families/dprc.qnt` `dprc_lifecycle`; the ITF
//! carries the `world` states but not the action taken between them, so the replayer
//! infers the transition from the state delta (mirroring the model's action set) and
//! drives the real [`dpaa2_api::dprc`] typestate through it, asserting the Rust world
//! equals the frozen next world at every step. A dynamic [`AnyContainer`] wrapper lets
//! the statically-typed container thread through the trace; every transition is a real
//! typestate call, so the wrapper weakens no compile-time guarantee — it lives here in
//! the replay, not in the core.
//!
//! Refusal steps (world unchanged but for `lastOutcome = Refused(r)`) are consumed via
//! [`dpaa2_api::dprc_plan::attribute_mc`] (the parcel's "attribution where the step is a
//! refusal") plus a context-driven typestate drive; the specific refused verb is not
//! recoverable from an unchanged-world delta, so the conformance is the refusal
//! vocabulary, its MC status, and world-invariance — exactly what the model records.
//!
//! An accepted step that changes nothing observable (a bus event, an accepted
//! `connect`, an accepted `spawn` whose grandchild is elided) is observationally one
//! shape — world unchanged, `Accepted` — so the replay consumes it uniformly by driving
//! [`Container::bus_remove_event`] (the no-op the model's DPRC-I7 `busRemoveEvent`
//! witnesses, F-i7) and asserting the world is unchanged.

use std::collections::BTreeSet;

use dpaa2_api::dprc::{
    Container, ContainerState, Created, Declared, Destroyed, Emptied, Locked, Options, Outcome,
    Parent, Plugged, Populated, Refusal, ResidentId, ResidentKind, ResidentOp, ResidentStep,
    Teardown, Unlocked, VfioBind,
};
use dpaa2_api::dprc_plan::{Attribution, PlanOutcome, attribute_mc, plan_move_out};
use dpaa2_verify::dprc_itf::{WorldView, parse_dprc_trace};

/// Every committed trace under `models/families/traces/`, with the model face it pins
/// (the acceptance-criterion coverage list).
const TRACES: &[(&str, &str)] = &[
    (
        "lifecycleTest",
        "full lifecycle ordering: create→resident→plug→bind→unbind",
    ),
    (
        "spawnRefused0x6Test",
        "refusal 0x6 (SPAWN absent), distinct",
    ),
    (
        "allocRefused0x8Test",
        "refusal 0x8 (ALLOC absent), distinct",
    ),
    (
        "connectNoTopologyRefused0x4Test",
        "refusal 0x4 (TOPOLOGY absent), distinct",
    ),
    (
        "connectWithTopologyTest",
        "accepted connect (topology present)",
    ),
    (
        "pluggedMoveRefused0x4Test",
        "refusal 0x4 (plugged-move, DPRC-I3)",
    ),
    (
        "evictionBothKindsTest",
        "eviction of both resident kinds (non-empty destroy)",
    ),
    (
        "DPRC_I1Test",
        "eviction boundary: created stays, assigned evicts (DPRC-I1)",
    ),
    (
        "teardownReachableTest",
        "teardown liveness: empty→destroy (DPRC-I9)",
    ),
    (
        "labelUnderLockTest",
        "set-label accepted under lock (V-DPRC-3)",
    ),
    (
        "lockStripsCreateTest",
        "lock strips create → 0x4 (DPRC-I11 remainder)",
    ),
    ("lockRefusesPlugTest", "lock refuses plug → 0x4 (V-DPRC-3)"),
    ("unlockRestoresTest", "unlock restores the create class"),
    (
        "DPRC_I7Test",
        "visibility/no-op bus event: MC survives (DPRC-I6/I7)",
    ),
];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/families/traces/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// A dprc container held dynamically across a trace: one variant per lifecycle phase,
/// each wrapping the real typestate [`Container<S>`]. The dispatch that a static
/// `Container<S>` cannot do inside a loop.
enum AnyContainer {
    Declared(Container<Declared>),
    Created(Container<Created>),
    Populated(Container<Populated>),
    Plugged(Container<Plugged>),
    Locked(Container<Locked>),
    Emptied(Container<Emptied>),
    Destroyed(Container<Destroyed>),
}

/// Reads an observable accessor off whichever phase the container is in.
macro_rules! with_any {
    ($any:expr, $c:ident => $body:expr) => {
        match $any {
            AnyContainer::Declared($c) => $body,
            AnyContainer::Created($c) => $body,
            AnyContainer::Populated($c) => $body,
            AnyContainer::Plugged($c) => $body,
            AnyContainer::Locked($c) => $body,
            AnyContainer::Emptied($c) => $body,
            AnyContainer::Destroyed($c) => $body,
        }
    };
}

/// A trace/core disagreement — a FINDING, surfaced loudly, never papered over.
fn finding(file: &str, step: usize, msg: &str) -> ! {
    panic!("{file}: trace/core disagreement at step {step}: {msg}");
}

/// The running Rust world projected into the same [`WorldView`] the trace decodes to.
fn view(any: &AnyContainer, parent: &Parent, last: Outcome) -> WorldView {
    WorldView {
        phase: with_any!(any, c => c.phase()),
        identity: with_any!(any, c => c.identity()),
        options: with_any!(any, c => c.options()),
        label: with_any!(any, c => c.label().clone()),
        residents: with_any!(any, c => c.residents().clone()),
        parent_residents: parent.residents().clone(),
        last_outcome: last,
    }
}

fn op_outcome(file: &str, step: usize, op: ResidentOp) -> Outcome {
    match op {
        ResidentOp::Done => Outcome::Accepted,
        ResidentOp::Refused(r) => Outcome::Refused(r),
        ResidentOp::Inapplicable(id) => {
            finding(file, step, &format!("resident op inapplicable to {id:?}"))
        }
    }
}

/// Places a resident, advancing an unplugged face to [`Populated`] (the accepted arm of
/// create/assign).
fn place(
    file: &str,
    step: usize,
    any: AnyContainer,
    id: ResidentId,
    kind: ResidentKind,
) -> AnyContainer {
    macro_rules! placed {
        ($c:expr) => {
            match kind {
                ResidentKind::CreatedIn => match $c.create_resident(id) {
                    ResidentStep::Placed(p) => AnyContainer::Populated(p),
                    ResidentStep::Refused(_, r) => {
                        finding(file, step, &format!("create refused {r:?}"))
                    }
                    ResidentStep::Duplicate(_, i) => {
                        finding(file, step, &format!("duplicate {i:?}"))
                    }
                },
                ResidentKind::AssignedIn => match $c.assign_in(id) {
                    ResidentStep::Placed(p) => AnyContainer::Populated(p),
                    ResidentStep::Refused(_, r) => {
                        finding(file, step, &format!("assign refused {r:?}"))
                    }
                    ResidentStep::Duplicate(_, i) => {
                        finding(file, step, &format!("duplicate {i:?}"))
                    }
                },
            }
        };
    }
    match any {
        AnyContainer::Created(c) => placed!(c),
        AnyContainer::Populated(c) => placed!(c),
        _ => finding(file, step, "resident placed on a non-unplugged face"),
    }
}

/// A resident id present in `next` but not `prev` (an add), if exactly one.
fn single_added(prev: &WorldView, next: &WorldView) -> Option<ResidentId> {
    let added: Vec<_> = next
        .residents
        .keys()
        .filter(|id| !prev.residents.contains_key(id))
        .copied()
        .collect();
    (added.len() == 1).then(|| added[0])
}

/// A resident id present in `prev` but not `next` (a removal), if exactly one.
fn single_removed(prev: &WorldView, next: &WorldView) -> Option<ResidentId> {
    let removed: Vec<_> = prev
        .residents
        .keys()
        .filter(|id| !next.residents.contains_key(id))
        .copied()
        .collect();
    (removed.len() == 1).then(|| removed[0])
}

/// A resident whose `plugged` flag flipped between `prev` and `next`, if exactly one.
fn single_plug_flip(prev: &WorldView, next: &WorldView) -> Option<(ResidentId, bool)> {
    let flips: Vec<_> = next
        .residents
        .iter()
        .filter_map(|(id, r)| {
            prev.residents
                .get(id)
                .filter(|p| p.plugged != r.plugged)
                .map(|_| (*id, r.plugged))
        })
        .collect();
    (flips.len() == 1).then(|| flips[0])
}

/// Consumes a refusal step: world unchanged, `lastOutcome = Refused(r)`. Drives
/// `dprc_plan::attribute_mc` and a context-selected typestate refusing method, asserting
/// both agree with the frozen refusal. Returns the outcome; the container is unchanged.
fn check_refusal(file: &str, step: usize, any: &AnyContainer, prev: &WorldView, r: Refusal) {
    // Attribution is well-defined and its MC status matches the refusal's (design D4).
    let attr = attribute_mc(r, prev.options);
    let attr_status = match attr {
        Attribution::PermissionGap { .. }
        | Attribution::PoolExhaustion
        | Attribution::LockGate
        | Attribution::PluggedMove
        | Attribution::FaceNotAssignable
        | Attribution::RestoolClientGuard { .. } => r.mc_status(),
    };
    assert_eq!(
        attr_status,
        r.mc_status(),
        "{file}: attribution status drift"
    );
    assert!(
        matches!(r.mc_status(), 4 | 6 | 8),
        "{file}: refusal status out of band"
    );

    // Context-driven typestate drive: pick the verb the phase + refusal identify and
    // assert the real core refuses with the same status.
    let plugged = prev.residents.iter().find(|(_, res)| res.plugged);
    match (prev.phase, r) {
        (ContainerState::Locked, Refusal::TopologyLockGate) => {
            if let AnyContainer::Locked(c) = any {
                assert_eq!(
                    c.connect_endpoints(),
                    Outcome::Refused(Refusal::TopologyLockGate)
                );
                assert_eq!(
                    c.create_resident(ResidentId::new(9)),
                    Outcome::Refused(Refusal::TopologyLockGate)
                );
            }
        }
        (_, Refusal::TopologyLockGate) if plugged.is_some() => {
            // Plugged-move (DPRC-I3): drive move_out on the plugged resident (a clone) and
            // the plan-level attribution.
            let (&pid, res) = plugged.expect("just matched");
            assert_eq!(
                plan_move_out(prev.phase, res, pid),
                PlanOutcome::Refused(Attribution::PluggedMove)
            );
            match any {
                AnyContainer::Populated(c) => {
                    assert_eq!(
                        c.clone().move_out(pid, &mut Parent::new()),
                        ResidentOp::Refused(Refusal::TopologyLockGate)
                    );
                }
                AnyContainer::Plugged(c) => {
                    assert_eq!(
                        c.clone().move_out(pid, &mut Parent::new()),
                        ResidentOp::Refused(Refusal::TopologyLockGate)
                    );
                }
                _ => finding(file, step, "plugged-move refusal on a non-holder face"),
            }
        }
        (ContainerState::Created | ContainerState::Populated, Refusal::TopologyLockGate) => {
            // connect refused for want of TOPOLOGY_CHANGES.
            match any {
                AnyContainer::Created(c) => {
                    assert_eq!(c.connect_endpoints(), Outcome::Refused(r));
                }
                AnyContainer::Populated(c) => {
                    assert_eq!(c.connect_endpoints(), Outcome::Refused(r));
                }
                _ => {}
            }
        }
        (ContainerState::Created | ContainerState::Populated, Refusal::SpawnViolation) => match any
        {
            AnyContainer::Created(c) => assert_eq!(c.spawn_grandchild(), Outcome::Refused(r)),
            AnyContainer::Populated(c) => assert_eq!(c.spawn_grandchild(), Outcome::Refused(r)),
            _ => {}
        },
        (ContainerState::Created | ContainerState::Populated, Refusal::AllocViolation) => {
            let fresh = ResidentId::new(9);
            match any {
                AnyContainer::Created(c) => assert!(matches!(
                    c.clone().create_resident(fresh),
                    ResidentStep::Refused(_, Refusal::AllocViolation)
                )),
                AnyContainer::Populated(c) => assert!(matches!(
                    c.clone().create_resident(fresh),
                    ResidentStep::Refused(_, Refusal::AllocViolation)
                )),
                _ => {}
            }
        }
        _ => finding(
            file,
            step,
            &format!("unhandled refusal context {:?}/{r:?}", prev.phase),
        ),
    }
}

/// Applies the transition inferred from `prev`→`next` to the running container, driving
/// the real typestate core; returns the new container and the recorded outcome.
// One dispatch mirroring `dprc.qnt`'s whole action set — kept in a single match so the
// inference reads against the model in one place rather than scattered across helpers.
#[allow(clippy::too_many_lines)]
fn apply(
    file: &str,
    step: usize,
    any: AnyContainer,
    parent: &mut Parent,
    prev: &WorldView,
    next: &WorldView,
) -> (AnyContainer, Outcome) {
    let child_same =
        prev.phase == next.phase && prev.residents == next.residents && prev.label == next.label;
    let parent_same = prev.parent_residents == next.parent_residents;

    // A step with no observable child/parent change: an accepted no-op (bus event /
    // accepted connect / accepted spawn) or a refusal.
    if child_same && parent_same {
        match next.last_outcome {
            Outcome::Accepted => {
                let o = match &any {
                    AnyContainer::Created(c) => c.bus_remove_event(),
                    AnyContainer::Populated(c) => c.bus_remove_event(),
                    AnyContainer::Plugged(c) => c.bus_remove_event(),
                    AnyContainer::Locked(c) => c.bus_remove_event(),
                    AnyContainer::Emptied(c) => c.bus_remove_event(),
                    _ => finding(file, step, "accepted no-op on an inactive face"),
                };
                assert_eq!(o, Outcome::Accepted);
                return (any, Outcome::Accepted);
            }
            Outcome::Refused(r) => {
                check_refusal(file, step, &any, prev, r);
                return (any, Outcome::Refused(r));
            }
        }
    }

    // set-label: only the label moved.
    if prev.phase == next.phase && prev.residents == next.residents && parent_same {
        let lbl = next.label.clone();
        macro_rules! relabel {
            ($variant:ident, $c:ident) => {{
                let mut c = $c;
                let r = c.set_label(lbl.clone());
                (AnyContainer::$variant(c), r)
            }};
        }
        return match any {
            AnyContainer::Created(c) => relabel!(Created, c),
            AnyContainer::Populated(c) => relabel!(Populated, c),
            AnyContainer::Plugged(c) => relabel!(Plugged, c),
            AnyContainer::Locked(c) => relabel!(Locked, c),
            AnyContainer::Emptied(c) => relabel!(Emptied, c),
            _ => finding(file, step, "set-label on an inactive face"),
        };
    }

    // create: Declared → Created (identity/options pool-assigned).
    if prev.phase == ContainerState::Declared && next.phase == ContainerState::Created {
        return match any {
            AnyContainer::Declared(c) => (
                AnyContainer::Created(c.create(next.options)),
                Outcome::Accepted,
            ),
            _ => finding(file, step, "create on a non-Declared face"),
        };
    }

    // plug container: Populated → Plugged(Unbound).
    if prev.phase == ContainerState::Populated
        && next.phase == ContainerState::Plugged(VfioBind::Unbound)
    {
        return match any {
            AnyContainer::Populated(c) => (AnyContainer::Plugged(c.plug()), Outcome::Accepted),
            _ => finding(file, step, "plug on a non-Populated face"),
        };
    }
    // bind / unbind vfio: Plugged(_) → Plugged(_).
    if prev.phase == ContainerState::Plugged(VfioBind::Unbound)
        && next.phase == ContainerState::Plugged(VfioBind::BoundVfioFslMc)
    {
        return match any {
            AnyContainer::Plugged(mut c) => {
                let r = c.bind_vfio();
                (AnyContainer::Plugged(c), r)
            }
            _ => finding(file, step, "bind on a non-Plugged face"),
        };
    }
    if prev.phase == ContainerState::Plugged(VfioBind::BoundVfioFslMc)
        && next.phase == ContainerState::Plugged(VfioBind::Unbound)
    {
        return match any {
            AnyContainer::Plugged(mut c) => {
                let r = c.unbind_vfio();
                (AnyContainer::Plugged(c), r)
            }
            _ => finding(file, step, "unbind on a non-Plugged face"),
        };
    }

    // lock / unlock.
    if next.phase == ContainerState::Locked && prev.phase != ContainerState::Locked {
        return match any {
            AnyContainer::Created(c) => (AnyContainer::Locked(c.lock()), Outcome::Accepted),
            AnyContainer::Populated(c) => (AnyContainer::Locked(c.lock()), Outcome::Accepted),
            _ => finding(file, step, "lock on a non-unplugged face"),
        };
    }
    if prev.phase == ContainerState::Locked && next.phase != ContainerState::Locked {
        return match any {
            AnyContainer::Locked(c) => {
                let restored = match c.unlock() {
                    Unlocked::Empty(cc) => AnyContainer::Created(cc),
                    Unlocked::Occupied(cc) => AnyContainer::Populated(cc),
                };
                (restored, Outcome::Accepted)
            }
            _ => finding(file, step, "unlock on a non-Locked face"),
        };
    }

    // teardown: → Emptied / → Destroyed.
    if next.phase == ContainerState::Emptied {
        macro_rules! do_empty {
            ($c:expr) => {
                match $c.empty(parent) {
                    Teardown::Done(e) => AnyContainer::Emptied(e),
                    Teardown::ResidentPlugged(_) => {
                        finding(file, step, "empty blocked by a plugged resident")
                    }
                }
            };
        }
        return match any {
            AnyContainer::Created(c) => (do_empty!(c), Outcome::Accepted),
            AnyContainer::Populated(c) => (do_empty!(c), Outcome::Accepted),
            AnyContainer::Plugged(c) => (do_empty!(c), Outcome::Accepted),
            _ => finding(file, step, "empty on a non-live face"),
        };
    }
    if next.phase == ContainerState::Destroyed {
        if prev.phase == ContainerState::Emptied {
            return match any {
                AnyContainer::Emptied(c) => {
                    (AnyContainer::Destroyed(c.destroy()), Outcome::Accepted)
                }
                _ => finding(file, step, "empty-destroy on a non-Emptied face"),
            };
        }
        macro_rules! do_destroy {
            ($c:expr) => {
                match $c.destroy(parent) {
                    Teardown::Done(d) => AnyContainer::Destroyed(d),
                    Teardown::ResidentPlugged(_) => {
                        finding(file, step, "destroy blocked by a plugged resident")
                    }
                }
            };
        }
        return match any {
            AnyContainer::Created(c) => (do_destroy!(c), Outcome::Accepted),
            AnyContainer::Populated(c) => (do_destroy!(c), Outcome::Accepted),
            AnyContainer::Plugged(c) => (do_destroy!(c), Outcome::Accepted),
            _ => finding(file, step, "destroy on a non-live face"),
        };
    }

    // resident-scoped: move-out, plug/unplug a resident, or create/assign a resident.
    if let Some(id) = single_removed(prev, next) {
        return match any {
            AnyContainer::Populated(mut c) => {
                let o = c.move_out(id, parent);
                (AnyContainer::Populated(c), op_outcome(file, step, o))
            }
            AnyContainer::Plugged(mut c) => {
                let o = c.move_out(id, parent);
                (AnyContainer::Plugged(c), op_outcome(file, step, o))
            }
            _ => finding(file, step, "move-out on a non-holder face"),
        };
    }
    if let Some((id, now_plugged)) = single_plug_flip(prev, next) {
        macro_rules! flip {
            ($c:expr) => {{
                let o = if now_plugged {
                    $c.plug_resident(id)
                } else {
                    $c.unplug_resident(id)
                };
                op_outcome(file, step, o)
            }};
        }
        return match any {
            AnyContainer::Populated(mut c) => {
                let r = flip!(c);
                (AnyContainer::Populated(c), r)
            }
            AnyContainer::Plugged(mut c) => {
                let r = flip!(c);
                (AnyContainer::Plugged(c), r)
            }
            _ => finding(file, step, "plug/unplug resident on a non-holder face"),
        };
    }
    if let Some(id) = single_added(prev, next) {
        let kind = next.residents[&id].kind;
        return (place(file, step, any, id, kind), Outcome::Accepted);
    }

    finding(
        file,
        step,
        &format!(
            "no transition mirrors delta {:?} → {:?}",
            prev.phase, next.phase
        ),
    );
}

#[test]
fn dprc_traces_replay_green() {
    for (file, face) in TRACES {
        let states = parse_dprc_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            states.len() >= 2,
            "{file}: a directed run has at least two states"
        );

        // The Rust world starts at `init`: a declared container and an empty parent.
        let mut any = AnyContainer::Declared(Container::declare());
        let mut parent = Parent::new();
        let mut last = Outcome::Accepted;
        assert_eq!(
            view(&any, &parent, last),
            states[0],
            "{file} ({face}): initial world diverges"
        );

        for (i, next) in states.iter().enumerate().skip(1) {
            let prev = &states[i - 1];
            let (new_any, outcome) = apply(file, i, any, &mut parent, prev, next);
            any = new_any;
            last = outcome;
            assert_eq!(
                view(&any, &parent, last),
                *next,
                "{file} ({face}): world diverges after step {i}"
            );
        }
    }
}

/// A tampered expectation must break the diff — the replay is load-bearing, not a
/// rubber stamp (mirrors `intent_replay.rs`'s divergence guard).
#[test]
fn replay_detects_a_diverging_world() {
    let states = parse_dprc_trace(&load("lifecycleTest")).unwrap();
    // Drive the real first transition (create) but compare against a tampered next
    // world whose options were flipped: the projection must differ.
    let any = AnyContainer::Declared(Container::declare());
    let mut parent = Parent::new();
    let tampered = WorldView {
        options: Options {
            spawn: false,
            ..states[1].options
        },
        ..states[1].clone()
    };
    let (created, _) = apply("lifecycleTest", 1, any, &mut parent, &states[0], &states[1]);
    assert_ne!(
        view(&created, &parent, Outcome::Accepted),
        tampered,
        "a flipped option mask must diverge from the driven world"
    );
}

/// The frozen trace set covers every model face the freeze contract names, so a dropped
/// trace file fails CI rather than silently shrinking coverage.
#[test]
fn every_committed_trace_is_listed() {
    let dir = format!(
        "{}/../../models/families/traces",
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
