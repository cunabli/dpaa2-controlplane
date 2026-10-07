//! The dpni↔dpni link-lifecycle ITF-replay CI rung (cross-dprc-links task 4.1): every committed
//! link trace replays green through the task-3.2–3.5 connection surface
//! ([`dpaa2_api::plan::connect`]) and the container typestate ([`dpaa2_api::families::dprc`]),
//! board-free. The model is the oracle — a regenerated trace whose world the Rust judgments no
//! longer reproduce fails here loudly; regenerate with `pnpm model:freeze-link` and reconcile the
//! transcription.
//!
//! Each trace is a directed run of `models/families/link_lifecycle.qnt` `link_lifecycle`. The ITF
//! carries the `world` state but not the action between two states, so the replayer reduces each
//! state to its [`LinkWorld`] vocabulary and asserts, at every state, that the Rust surface agrees
//! with the frozen fields (cross-dprc-links design D4/D5/D7):
//!
//!   - phase↔edge: `phase == Connected` iff a dpni↔dpni edge stands;
//!   - `LINK_I2` + the cross-dprc-links design D4 partial order: a bound end is kernel-visible exactly when it is neither a
//!     standing `DeferredVisibility` obligation nor a declined-visibility residue — populated ends
//!     stay invisible until bind;
//!   - the obligation/residue mints reproduce: a post-bind create's obligation is
//!     [`Container::create_resident_deferred`], a declined residue is [`discharge`] below
//!     `Disruptive`, and a stale node is [`Container::destroy_resident_stale`].
//!
//! The teardown law also replays through the typed plan surface: `wireLifecycleTest`'s
//! connect→disconnect→destroy maps to [`WireTransition::connect_wire`] →
//! [`WireTransition::disconnect_wire_proving`] → [`WireTransition::destroy_end`], and the
//! order-inverted sequence is REJECTED by construction — the negative face — because
//! [`WireTransition::destroy_end`] cannot be built without the [`WireDisconnected`] proof only a
//! disconnect mints (cross-dprc-links design D5, `LINK_I1`; mirrors `dpmac_replay.rs`). The
//! property twins pin the edge-kind table (legal-pair symmetry/membership), the refusal
//! attribution (`LINK_I4`), and the consent/discharge verdict (cross-dprc-links design D5).

use std::collections::BTreeSet;

use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::{ALL_FAMILIES, Family};
use dpaa2_api::core::model::{DpniId, DprcId};
use dpaa2_api::families::dprc::Refusal;
use dpaa2_api::families::dprc::{Container, Options, Plugged, ResidentId, ResidentStep};
use dpaa2_api::plan::Class;
use dpaa2_api::plan::connect::{
    ReificationPolicy, TeardownLaw, WireDisconnected, WireEnd, WireRefusal, WireResidue, WireSide,
    WireTransition, attribute_wire_refusal, discharge, edge_demands_severed_witness, legal_pair,
    reification_policy,
};
use dpaa2_verify::intent::link_itf::{
    ContainerBind, LinkResidue, LinkStep, LinkWorld, WirePhase, parse_link_trace,
};
use proptest::prelude::*;

/// Every committed trace under `models/traces/families/link_lifecycle/`, with the model face it pins.
const TRACES: &[(&str, &str)] = &[
    (
        "dpniPairLegalTest",
        "dpni↔dpni is legal and demands no severed witness (disconnect-only)",
    ),
    (
        "wireLifecycleTest",
        "ends-exist → connect → disconnect → destroy an end",
    ),
    (
        "connectAlreadyConnectedRefusedTest",
        "cardinality one: a second connect of a held end is refused (.fail)",
    ),
    (
        "reconnectAfterDisconnectTest",
        "disconnect-before-reconnect: the torn edge reconnects",
    ),
    (
        "destroyConnectedEndRemovesEdgeTest",
        "destroying a still-connected end is accepted; the edge dies with the endpoint",
    ),
    (
        "crossContainerWireTest",
        "root↔child: the connect carries no container gate",
    ),
    (
        "childToChildWireTest",
        "child↔child: nothing privileges the root but the issuing ancestor",
    ),
    (
        "freshPartialOrderTest",
        "D4: connect legal on the unplugged container, bind plugs it visible after",
    ),
    (
        "bindBeforeConnectTest",
        "D4: bind-then-connect of the now-visible ends stays legal",
    ),
    (
        "postBindCreateObligatedTest",
        "D5: a post-bind create carries its DeferredVisibility obligation by construction",
    ),
    (
        "consentedRebindDischargesTest",
        "D5: a granted rebind discharges the obligation, no residue stands",
    ),
    (
        "declinedConsentResidueTest",
        "D5: declined consent leaves a DeclinedVisibility residue, no silent rebind",
    ),
    (
        "postBindDestroyStaleNodeTest",
        "D5: a post-bind destroy leaves a lazy StaleNode residue that blocks nothing",
    ),
];

fn load(file: &str) -> String {
    let path = format!(
        "{}/../../models/traces/families/link_lifecycle/{file}.itf.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// A trace/core disagreement — a FINDING, surfaced loudly (mirrors `dpmac_replay.rs`).
fn finding(file: &str, step: usize, msg: &str) -> ! {
    panic!("{file}: trace/core disagreement at step {step}: {msg}");
}

/// A `Container<Plugged>` — the bound face that mints the post-bind-create obligation and the
/// stale-node residue through the public API ([`Container::create_resident_deferred`] /
/// [`Container::destroy_resident_stale`]).
fn plugged() -> Container<Plugged> {
    let created = Container::declare().create(Options::DEFAULT);
    match created.create_resident(ResidentId::new(1)) {
        ResidentStep::Placed(c) => c.plug(),
        _ => unreachable!("an ALLOC-allowed Created container places its first resident"),
    }
}

fn end(dpni: u32, container: u32) -> WireEnd {
    WireEnd {
        dpni: DpniId::new(dpni),
        container: DprcId::new(container),
    }
}

fn world_end(w: &LinkWorld, dpni: u32) -> WireEnd {
    let container = w
        .endpoints
        .get(&dpni)
        .expect("a referenced end is present")
        .container;
    end(dpni, container)
}

/// The per-state conformance: phase↔edge, the `LINK_I2` / cross-dprc-links design D4 visibility equation, and the
/// obligation/residue mint reproductions.
fn check_state(file: &str, i: usize, w: &LinkWorld) {
    if (w.phase == WirePhase::Connected) == w.conns.is_empty() {
        finding(
            file,
            i,
            "phase disagrees with whether a dpni↔dpni edge stands",
        );
    }
    for (&n, e) in &w.endpoints {
        let obligated = w.obligations.contains(&n);
        let declined = w.residues.contains(&LinkResidue::DeclinedVisibility(n));
        let expect_visible = w.container_bind == ContainerBind::Bound && !obligated && !declined;
        if e.bus_visible != expect_visible {
            finding(
                file,
                i,
                "bus visibility disagrees with the bind/obligation/residue state (LINK_I2, cross-dprc-links design D4)",
            );
        }
    }
    for &n in &w.obligations {
        if w.container_bind != ContainerBind::Bound {
            finding(
                file,
                i,
                "a DeferredVisibility obligation stands while unplugged",
            );
        }
        let we = world_end(w, n);
        let pc = plugged().create_resident_deferred(we);
        if pc.obligation.endpoint() != we || pc.discharge.endpoint() != we {
            finding(
                file,
                i,
                "the post-bind-create mint does not name the obligated end",
            );
        }
    }
    for r in &w.residues {
        match *r {
            LinkResidue::DeclinedVisibility(n) => {
                let we = world_end(w, n);
                let pc = plugged().create_resident_deferred(we);
                if discharge(pc, Class::Hitless) != Err(WireResidue::DeclinedVisibility(we)) {
                    finding(
                        file,
                        i,
                        "a declined discharge does not reproduce the residue",
                    );
                }
            }
            LinkResidue::StaleNode(n) => {
                if w.endpoints.contains_key(&n) {
                    finding(file, i, "a stale-node end is still present (not destroyed)");
                }
                // The model residue carries no container; judge the mint's variant and identity
                // on the destroyed end's dpni (the StaleNode mint is container-agnostic).
                let we = end(n, DprcId::ROOT.into_inner());
                if plugged().destroy_resident_stale(we) != WireResidue::StaleNode(we) {
                    finding(
                        file,
                        i,
                        "the stale-node mint does not reproduce the residue",
                    );
                }
            }
        }
    }
}

/// One inferred wire transition, read from the World field deltas (the ITF names no action).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WireAction {
    Connect(WireEnd, WireEnd),
    Disconnect(WireEnd, WireEnd),
    Destroy(u32),
}

fn same_edge(x: (u32, u32), y: (u32, u32)) -> bool {
    x == y || x == (y.1, y.0)
}

/// The ordered wire transitions a decoded run performs, inferred from the World deltas: a new edge
/// is a connect, a dropped edge a disconnect, a vanished endpoint a destroy.
fn wire_actions(steps: &[LinkStep]) -> Vec<WireAction> {
    let worlds: Vec<&LinkWorld> = steps
        .iter()
        .filter_map(|s| match s {
            LinkStep::World(w) => Some(w),
            LinkStep::Refused => None,
        })
        .collect();
    let mut actions = Vec::new();
    for pair in worlds.windows(2) {
        let (prev, w) = (pair[0], pair[1]);
        for &e in &w.conns {
            if !prev.conns.iter().any(|&p| same_edge(p, e)) {
                actions.push(WireAction::Connect(world_end(w, e.0), world_end(w, e.1)));
            }
        }
        for &e in &prev.conns {
            if !w.conns.iter().any(|&q| same_edge(q, e)) {
                actions.push(WireAction::Disconnect(
                    world_end(prev, e.0),
                    world_end(prev, e.1),
                ));
            }
        }
        for &n in prev.endpoints.keys() {
            if !w.endpoints.contains_key(&n) {
                actions.push(WireAction::Destroy(n));
            }
        }
    }
    actions
}

/// Replays a wire-action sequence against the typed plan surface (cross-dprc-links task 3.5). A
/// destroy demands the [`WireDisconnected`] proof a prior disconnect minted; with no proof in hand
/// the [`WireTransition::destroy_end`] cannot be built, so the destroy-before-disconnect order is
/// refused HERE — the law is enforced by the type, not by this replayer's discipline (`LINK_I1`).
fn replay_wire_law(actions: &[WireAction]) -> Result<Vec<WireTransition>, &'static str> {
    let mut proof: Option<WireDisconnected> = None;
    let mut out = Vec::new();
    for action in actions {
        match *action {
            WireAction::Connect(a, b) => out.push(WireTransition::connect_wire(a, b)),
            WireAction::Disconnect(a, b) => {
                let (step, p) = WireTransition::disconnect_wire_proving(a, b);
                proof = Some(p);
                out.push(step);
            }
            WireAction::Destroy(n) => {
                let Some(p) = proof.take() else {
                    return Err("destroy before disconnect: no WireDisconnected proof (LINK_I1)");
                };
                let side = if p.end(WireSide::A).dpni == DpniId::new(n) {
                    WireSide::A
                } else if p.end(WireSide::B).dpni == DpniId::new(n) {
                    WireSide::B
                } else {
                    return Err("destroy names an end the disconnect never freed");
                };
                out.push(WireTransition::destroy_end(p, side));
            }
        }
    }
    Ok(out)
}

#[test]
fn link_traces_replay_green() {
    for (file, face) in TRACES {
        let steps = parse_link_trace(&load(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            matches!(steps.first(), Some(LinkStep::World(w)) if w.phase == WirePhase::EndsExist),
            "{file} ({face}): a directed run starts at the ends-exist world"
        );
        for (i, step) in steps.iter().enumerate() {
            match step {
                LinkStep::Refused => assert_eq!(
                    i,
                    steps.len() - 1,
                    "{file} ({face}): a refused (.fail) step must be terminal"
                ),
                LinkStep::World(w) => check_state(file, i, w),
            }
        }
    }
}

/// The committed trace set is exactly the `TRACES` table — a dropped or unlisted trace file fails
/// CI rather than silently shrinking coverage (mirrors `dpmac_replay.rs`).
#[test]
fn every_committed_trace_is_listed() {
    let dir = format!(
        "{}/../../models/traces/families/link_lifecycle",
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

/// `dpniPairLegalTest`: the kind the whole module consumes is legal and disconnect-only — it
/// demands no severed witness, so no severed-witness machinery lives here (cross-dprc-links design D3).
#[test]
fn dpni_pair_is_legal_and_disconnect_only() {
    assert!(legal_pair(Family::Dpni, Family::Dpni));
    // disconnect-only: no severed-witness machinery lives here (cross-dprc-links design D3).
    assert!(!edge_demands_severed_witness(Family::Dpni, Family::Dpni));
    assert!(matches!(
        reification_policy(Family::Dpni, Family::Dpni),
        Some(ReificationPolicy::Defined {
            teardown: TeardownLaw::DisconnectOnly,
            ..
        })
    ));
}

/// The teardown trace replays through the typed plan path (cross-dprc-links task 3.5):
/// `wireLifecycleTest`'s connect/disconnect/destroy maps to `connect_wire` →
/// `disconnect_wire_proving` → `destroy_end`, lowering to that three-step shape.
#[test]
fn teardown_replays_through_the_typed_surface() {
    let steps = parse_link_trace(&load("wireLifecycleTest")).unwrap();
    let actions = wire_actions(&steps);
    assert!(
        matches!(
            actions.as_slice(),
            [
                WireAction::Connect(..),
                WireAction::Disconnect(..),
                WireAction::Destroy(_)
            ]
        ),
        "the frozen teardown is connect, disconnect, destroy: {actions:?}"
    );
    let plan = replay_wire_law(&actions).expect("the legal order threads the proof");
    assert!(
        matches!(
            plan.as_slice(),
            [
                WireTransition::ConnectWire { .. },
                WireTransition::DisconnectWire { .. },
                WireTransition::DestroyEnd { .. }
            ]
        ),
        "the typed path lowers to connect, disconnect, destroy-end: {plan:?}"
    );
}

/// The negative face (bead acceptance): the order-inverted teardown is REJECTED by the typed
/// surface. The mutation reverses the decoded actions to put the destroy before its disconnect,
/// and the replayer refuses it — [`WireTransition::destroy_end`] has no [`WireDisconnected`] proof
/// to consume before a disconnect mints one (`LINK_I1`).
#[test]
fn inverted_teardown_is_rejected() {
    let steps = parse_link_trace(&load("wireLifecycleTest")).unwrap();
    let mut inverted = wire_actions(&steps);
    inverted.reverse();
    assert!(
        matches!(inverted.first(), Some(WireAction::Destroy(_))),
        "the mutation puts the destroy first"
    );
    assert!(
        replay_wire_law(&inverted).is_err(),
        "destroy-before-disconnect cannot be expressed against the typed surface"
    );
}

/// `destroyConnectedEndRemovesEdgeTest`: the frozen run ACCEPTS destroying a still-connected end,
/// the edge dying atomically with the endpoint (V-LINK-6 rev 1, 2026-10-05 — the board's answer).
/// The model surface shows the edge gone and the survivor edge-less; the engine's typed surface
/// still refuses the bare `[Connect, Destroy]` move — [`WireTransition::destroy_end`] holds no
/// [`WireDisconnected`] proof — because disconnect-before-destroy stays a deliberate typestate
/// POLICY, stricter than hardware (`LINK_I1`).
#[test]
fn accepted_destroy_removes_the_edge() {
    let steps = parse_link_trace(&load("destroyConnectedEndRemovesEdgeTest")).unwrap();
    let Some(LinkStep::World(last)) = steps.last() else {
        panic!("the accepted destroy ends at a world, not a refusal sentinel")
    };
    assert_eq!(
        last.phase,
        WirePhase::EndDestroyLegal,
        "the destroy completed"
    );
    assert!(
        last.conns.is_empty(),
        "the edge died with the endpoint (LINK_I1)"
    );
    assert_eq!(last.endpoints.len(), 1, "the survivor remains, edge-less");
    // The engine keeps disconnect-before-destroy as POLICY: the bare connect→destroy the board
    // accepts still cannot be built here — the destroy holds no disconnect proof to consume.
    let connected = match &steps[1] {
        LinkStep::World(w) => w,
        LinkStep::Refused => panic!("state 1 is the connected world"),
    };
    let (a, b) = (connected.conns[0].0, connected.conns[0].1);
    let move_refused = [
        WireAction::Connect(world_end(connected, a), world_end(connected, b)),
        WireAction::Destroy(a),
    ];
    assert!(
        replay_wire_law(&move_refused).is_err(),
        "the engine refuses connect→destroy with no disconnect proof (POLICY, LINK_I1)"
    );
}

/// `reconnectAfterDisconnectTest`: once the edge is torn, the freed end reconnects. Disconnect-
/// before-reconnect lives in the type — [`WireTransition::reconnect_wire`] consumes the
/// [`WireDisconnected`] proof the disconnect minted and rebuilds a fresh connect for the freed end
/// (DPRC-I5, cross-dprc-links design D7).
#[test]
fn reconnect_threads_the_disconnect_proof() {
    let steps = parse_link_trace(&load("reconnectAfterDisconnectTest")).unwrap();
    let actions = wire_actions(&steps);
    assert!(
        matches!(
            actions.as_slice(),
            [
                WireAction::Connect(..),
                WireAction::Disconnect(..),
                WireAction::Connect(..)
            ]
        ),
        "the frozen run is connect, disconnect, reconnect: {actions:?}"
    );
    let WireAction::Disconnect(a, b) = actions[1] else {
        unreachable!("action 1 is the disconnect");
    };
    let (_step, proof) = WireTransition::disconnect_wire_proving(a, b);
    // The freed `a` end reconnects to its former peer `b`, via the proof — the frozen reconnect.
    let reconnect = WireTransition::reconnect_wire(proof, WireSide::A, b);
    assert_eq!(reconnect, WireTransition::connect_wire(a, b));
}

/// The refusal-attribution law is not a wire-connect refusal here (none of the frozen runs are
/// child-issued); it passes through as `None` — never collapsed to a backend string.
#[test]
fn an_unrelated_error_is_not_a_wire_refusal() {
    assert_eq!(attribute_wire_refusal(&Error::Backend("boom".into())), None);
}

fn a_family() -> impl Strategy<Value = Family> {
    prop::sample::select(&ALL_FAMILIES[..])
}

fn a_class() -> impl Strategy<Value = Class> {
    prop_oneof![
        Just(Class::Hitless),
        Just(Class::Boundary),
        Just(Class::Disruptive),
    ]
}

/// The seven legal edge kinds, transcribed from `models/core/connect.qnt` `legalPair` — the model
/// law the property twin compares [`legal_pair`] against.
const MODEL_LEGAL_PAIRS: [(Family, Family); 7] = [
    (Family::Dpni, Family::Dpmac),
    (Family::Dpni, Family::Dpni),
    (Family::Dpni, Family::Dpsw),
    (Family::Dpni, Family::Dpdmux),
    (Family::Dpsw, Family::Dpmac),
    (Family::Dpdmux, Family::Dpmac),
    (Family::Dpci, Family::Dpci),
];

fn legal_pair_twin(a: Family, b: Family) -> bool {
    MODEL_LEGAL_PAIRS
        .iter()
        .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
}

proptest! {
    /// `legalPair` (`core/connect.qnt`): the edge table is symmetric and exactly the seven
    /// unordered kinds — [`legal_pair`] agrees with the transcribed twin in both orders over the
    /// whole 16×16 family space, so no kind is one-sidedly accepted or missing.
    #[test]
    fn legal_pair_agrees_with_the_model_table(a in a_family(), b in a_family()) {
        prop_assert_eq!(legal_pair(a, b), legal_pair_twin(a, b));
        prop_assert_eq!(legal_pair(a, b), legal_pair(b, a));
    }

    /// `LINK_I4` (`link_lifecycle.qnt`): a refusable connect surfaces as a typed refusal through
    /// the single core-side sentinel — [`attribute_wire_refusal`] attributes the No-privilege
    /// (`0x4`) child-issued connect and passes every other status through as `None`, total over
    /// the status byte.
    #[test]
    fn wire_refusal_attribution_is_total(status in any::<u8>()) {
        let got = attribute_wire_refusal(&Error::McStatus { status });
        let expect = if Refusal::from_status(status) == Some(Refusal::TopologyLockGate) {
            Some(WireRefusal::ChildIssuedConnectUnprivileged)
        } else {
            None
        };
        prop_assert_eq!(got, expect);
    }

    /// `rebindDischargeAt` (`link_lifecycle.qnt`; cross-dprc-links design D5): the model's
    /// `Consent = Granted | Declined` folds into the ADR-0015 class gate — [`discharge`] yields
    /// the rebind cycle exactly when the allow covers `Disruptive`, otherwise the typed
    /// declined-visibility residue, and either way names the obligated end.
    #[test]
    fn discharge_verdict_follows_the_class_gate(
        dpni in 0u32..8,
        container in 1u32..4,
        allowed in a_class(),
    ) {
        let we = end(dpni, container);
        let pc = plugged().create_resident_deferred(we);
        match discharge(pc, allowed) {
            Ok(cycle) => {
                prop_assert!(allowed >= Class::Disruptive);
                prop_assert_eq!(cycle.endpoint(), we);
                prop_assert_eq!(cycle.class(), Class::Disruptive);
            }
            Err(residue) => {
                prop_assert!(allowed < Class::Disruptive);
                prop_assert_eq!(residue, WireResidue::DeclinedVisibility(we));
            }
        }
    }
}
