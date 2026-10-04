//! The edge-kind table — pair legality, the port restriction, the severed-witness
//! predicate, and the per-kind reification policy (ADR-0022; cross-dprc-links design D3).
//!
//! Structurally isomorphic to `models/core/connect.qnt`, sum-for-sum: [`legal_pair`]
//! mirrors `legalPair` (seven unordered pairs), [`legal_ports`] mirrors `legalPorts`
//! (dpdmux↔dpmac demands port 0), and [`edge_demands_severed_witness`] mirrors
//! `edgeDemandsSeveredWitness` (dpni↔dpmac only). Identifiers use the quint spelling.
//!
//! The teardown machinery for the one delivered kind is **claimed, not retyped**: the
//! dpni↔dpmac severed-witness law stays on
//! [`SeveredProof`](crate::plan::SeveredProof) / [`Transition::Unbind`](crate::plan::Transition),
//! and [`edge_demands_severed_witness`] is the shared predicate that carrier consumes
//! (ADR-0022 decision 2). [`reification_policy`] is the planner-facing declaration the
//! later cross-dprc-links tasks (3.2–3.5) read; it does not drive the transition executor.

use core::fmt;

use crate::core::error::Error;
use crate::core::family::Family;
use crate::core::model::{DpniId, DprcId};
use crate::families::dprc::Refusal;
use crate::plan::Class;

/// The seven legal edge kinds (`core/connect.qnt` `legalPair`; object-model.md §2).
///
/// Unordered — both orders are read as the same edge.
const LEGAL_PAIRS: [(Family, Family); 7] = [
    (Family::Dpni, Family::Dpmac),
    (Family::Dpni, Family::Dpni),
    (Family::Dpni, Family::Dpsw),
    (Family::Dpni, Family::Dpdmux),
    (Family::Dpsw, Family::Dpmac),
    (Family::Dpdmux, Family::Dpmac),
    (Family::Dpci, Family::Dpci),
];

/// Whether two families may form an edge (`core/connect.qnt` `legalPair`).
///
/// Order-insensitive: an edge is unordered, so both orders of each of the seven
/// `LEGAL_PAIRS` accept.
#[must_use]
pub fn legal_pair(a: Family, b: Family) -> bool {
    LEGAL_PAIRS
        .iter()
        .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
}

/// Whether the ports satisfy the uplink restriction (`core/connect.qnt` `legalPorts`).
///
/// A dpdmux facing a dpmac demands port 0 — only `dpdmux.N.0` may be a dpmac uplink
/// since MC 10.37 (ADR-0009; dpdmux.md). Every other pairing is unrestricted here; pair
/// legality itself is [`legal_pair`].
#[must_use]
pub fn legal_ports(a: Family, a_port: u32, b: Family, b_port: u32) -> bool {
    (a != Family::Dpdmux || b != Family::Dpmac || a_port == 0)
        && (b != Family::Dpdmux || a != Family::Dpmac || b_port == 0)
}

/// Whether tearing this edge down demands a severed witness (`core/connect.qnt`
/// `edgeDemandsSeveredWitness`).
///
/// True for dpni↔dpmac only: unbinding the dpni's kernel face while the edge stands
/// strands the port driverless (ADR-0008 §8). The delivered carrier of this law is
/// [`SeveredProof`](crate::plan::SeveredProof) /
/// [`Transition::Unbind`](crate::plan::Transition) — this predicate is the shared
/// decision it consumes, claimed by the table rather than restated (ADR-0022 decision 2).
/// Every other kind is vacuous: dpni↔dpni has no driver handback, the dpdmux uplink is
/// un-disconnectable on the pinned firmware (ADR-0009), and the rest carry no recorded
/// hazard.
#[must_use]
pub fn edge_demands_severed_witness(a: Family, b: Family) -> bool {
    (a == Family::Dpni && b == Family::Dpmac) || (a == Family::Dpmac && b == Family::Dpni)
}

/// Where a kind's `dprc connect` is issued (ADR-0022 edge-kind table).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnectAuthority {
    /// Connect at a common ancestor (`CONNECT_ANCESTOR`), then the kernel binds the
    /// dpni face (dpni↔dpmac).
    AncestorConnectThenKernelBind,
    /// Connect at a common ancestor; no kernel face to bind.
    AncestorConnect,
}

/// What a kind's teardown demands (ADR-0022 edge-kind table).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TeardownLaw {
    /// The edge is severed — the dpmac handed back `KernelOwned`→`Offered`, minting
    /// `SeveredProof` — before the dpni's kernel face unbinds
    /// ([`edge_demands_severed_witness`]; ADR-0008 §8).
    SeveredWitness,
    /// Disconnect only: no driver handback exists by construction.
    DisconnectOnly,
    /// Un-disconnectable on the pinned firmware; the model refuses the teardown (ADR-0009).
    Refused,
}

/// A legal kind's reification policy — its connect authority and teardown law beyond the
/// shared connect/disconnect verbs (ADR-0022 edge-kind table; cross-dprc-links design D3).
///
/// Planner-facing: the later cross-dprc-links tasks read this to decide what a plan may
/// emit per kind. It does not drive the [`Transition`](crate::plan::Transition) executor;
/// the dpni↔dpmac machinery is claimed there, not here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReificationPolicy {
    /// A kind whose policy is defined on this surface.
    Defined {
        /// Where connect is issued.
        connect: ConnectAuthority,
        /// What teardown demands.
        teardown: TeardownLaw,
    },
    /// A legal edge whose reification awaits its own change — an explicit state,
    /// distinct from the `None` an illegal pair yields.
    AwaitsOwnChange,
}

/// The reification policy for an edge, or `None` when the pair is not legal
/// (ADR-0022 edge-kind table).
///
/// Order-insensitive. Three kinds carry a defined policy — dpni↔dpmac (connect + kernel
/// bind, severed-witness teardown), dpni↔dpni (ancestor connect, disconnect-only), and
/// the dpdmux↔dpmac uplink (ancestor connect, teardown refused on the pinned firmware,
/// ADR-0009). The remaining legal kinds are [`ReificationPolicy::AwaitsOwnChange`].
#[must_use]
pub fn reification_policy(a: Family, b: Family) -> Option<ReificationPolicy> {
    let unordered = |x: Family, y: Family| (a == x && b == y) || (a == y && b == x);
    if unordered(Family::Dpni, Family::Dpmac) {
        Some(ReificationPolicy::Defined {
            connect: ConnectAuthority::AncestorConnectThenKernelBind,
            teardown: TeardownLaw::SeveredWitness,
        })
    } else if unordered(Family::Dpni, Family::Dpni) {
        Some(ReificationPolicy::Defined {
            connect: ConnectAuthority::AncestorConnect,
            teardown: TeardownLaw::DisconnectOnly,
        })
    } else if unordered(Family::Dpdmux, Family::Dpmac) {
        Some(ReificationPolicy::Defined {
            connect: ConnectAuthority::AncestorConnect,
            teardown: TeardownLaw::Refused,
        })
    } else if legal_pair(a, b) {
        Some(ReificationPolicy::AwaitsOwnChange)
    } else {
        None
    }
}

// ---- dpni↔dpni wire: ends-keyed transitions, ancestor, typed refusal (cross-dprc-links task 3.2) ----

/// One dpni end of a dpni↔dpni wire — its MC identity and the container it resides in
/// (cross-dprc-links design D2). Reuses the delivered handle types ([`DpniId`], [`DprcId`]),
/// no parallel identity scheme (ADR-0015: intent identity is a name, the runtime handle is
/// the MC-assigned index). A dpni faces its single endpoint port 0 (object-model.md §2), so
/// the handle alone keys the end and no port field is carried. The `container` is what makes
/// the end representable in any arrangement — root or child — and what [`connect_ancestor`]
/// resolves the issuing ancestor from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WireEnd {
    /// The dpni at this end (the runtime handle, ADR-0015).
    pub dpni: DpniId,
    /// The container the dpni resides in — root or child, no arrangement privileged.
    pub container: DprcId,
}

/// The container a dpni↔dpni `dprc connect` is issued at — the root today
/// (cross-dprc-links design D2; ADR-0022). The fsl-mc root (`dprc.1`) is a common ancestor of
/// every container, so every arrangement — root↔root, root↔child, child↔child — resolves
/// here. A named constant, never an assumption left implicit at a call site.
pub const CONNECT_ANCESTOR: DprcId = DprcId::ROOT;

/// Resolves the common ancestor a dpni↔dpni connect is issued at, from the two ends'
/// containers (cross-dprc-links design D2; ADR-0022). Constant-rooted today:
/// [`CONNECT_ANCESTOR`], the root, which is a common ancestor of any pair of containers — so
/// the resolution is container-agnostic. The signature takes both containers so a later
/// change can resolve a nearer common ancestor without reshaping call sites.
#[must_use]
pub fn connect_ancestor(_left: DprcId, _right: DprcId) -> DprcId {
    CONNECT_ANCESTOR
}

/// Which freed end of a disconnected wire a teardown or reconnect names
/// (cross-dprc-links design D5/D7). A [`WireDisconnected`] proof carries both ends the
/// disconnect freed; the side selects one, so a destroy names only an end the wire held.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireSide {
    /// The `a` end of the disconnected wire.
    A,
    /// The `b` end of the disconnected wire.
    B,
}

/// Proof a dpni↔dpni wire was disconnected before one of its ends is destroyed or reconnected
/// (cross-dprc-links design D5/D7; `models/families/link_lifecycle.qnt` `LINK_I1`
/// disconnect-before-destroy, `LINK_I3` cardinality-one / disconnect-before-reconnect). The wire
/// twin of the dpmac [`SeveredProof`](crate::plan::SeveredProof) proof-carrying idiom (ADR-0022):
/// the fields are private, so the only mint is
/// [`WireTransition::disconnect_wire_proving`](WireTransition::disconnect_wire_proving) and neither
/// [`WireTransition::destroy_end`](WireTransition::destroy_end) nor
/// [`WireTransition::reconnect_wire`](WireTransition::reconnect_wire) — the two guarded
/// constructors that demand it — is reachable without a prior disconnect. Unlike `SeveredProof`
/// it is `Clone`: dpni↔dpni is disconnect-only with no driver handback
/// (cross-dprc-links design D3), so there is no once-only kernel-face hazard to fence and
/// both freed ends are legitimately destroyable.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WireDisconnected {
    a: WireEnd,
    b: WireEnd,
}

impl WireDisconnected {
    /// The two ends the disconnect freed (unordered, as the wire is).
    #[must_use]
    pub fn ends(&self) -> (WireEnd, WireEnd) {
        (self.a, self.b)
    }

    /// The freed end `side` names — read back from the proof, so a teardown or reconnect can
    /// only ever name an end the wire actually held (`LINK_I1`/`LINK_I3`).
    #[must_use]
    pub fn end(&self, side: WireSide) -> WireEnd {
        match side {
            WireSide::A => self.a,
            WireSide::B => self.b,
        }
    }
}

/// A dpni↔dpni wire transition keyed by its two ends — the Rust twin of the model's
/// endpoint-pair actions (`models/families/link_lifecycle.qnt` `connectWireAt` /
/// `disconnectWireAt`; cross-dprc-links design D2). Kept in its own vocabulary rather than a
/// [`Transition`](crate::plan::Transition) variant: a wire is keyed by container-resident
/// ends and an issuing ancestor — a different id domain from the port-anchored
/// [`Transition`](crate::plan::Transition) — so this follows the `plan/dprc.rs` "extend where they don't fit" idiom
/// (ADR-0018), leaving the delivered Plan pipeline untouched. dpni↔dpni is disconnect-only
/// teardown with no severed witness (cross-dprc-links design D3; [`edge_demands_severed_witness`]
/// is false for the kind).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WireTransition {
    /// `dprc connect` of two dpni ends, issued at their resolved common ancestor.
    ConnectWire {
        /// One end (unordered — a wire has no direction).
        a: WireEnd,
        /// The other end.
        b: WireEnd,
        /// The container the connect is issued at ([`connect_ancestor`], the root today).
        ancestor: DprcId,
    },
    /// `dprc disconnect` of the wire between two dpni ends — the only teardown verb the kind
    /// carries (disconnect-only, cross-dprc-links design D3).
    DisconnectWire {
        /// One end.
        a: WireEnd,
        /// The other end.
        b: WireEnd,
    },
    /// Destroy one freed end of a disconnected wire — reachable only from a
    /// [`WireDisconnected`] proof, so disconnect-before-destroy (cross-dprc-links design D5,
    /// `LINK_I1`) is a type law, not planner discipline. The variant carries the proof
    /// (unforgeable, no public constructor) and the [`WireSide`] it names, so a struct literal
    /// naming a still-connected end is unrepresentable; build it through
    /// [`destroy_end`](WireTransition::destroy_end).
    DestroyEnd {
        /// Proof the wire was disconnected first — names the destroyed end.
        proof: WireDisconnected,
        /// Which freed end is destroyed.
        side: WireSide,
    },
}

impl WireTransition {
    /// Builds a [`WireTransition::ConnectWire`], resolving the issuing ancestor from the two
    /// ends' containers via [`connect_ancestor`] (cross-dprc-links design D2) rather than
    /// leaving it to a call site. Container-agnostic: root↔root, root↔child and child↔child
    /// all build the same shape, differing only in the ends' `container` fields.
    #[must_use]
    pub fn connect_wire(a: WireEnd, b: WireEnd) -> Self {
        Self::ConnectWire {
            a,
            b,
            ancestor: connect_ancestor(a.container, b.container),
        }
    }

    /// Builds a [`WireTransition::DisconnectWire`] for the wire between two dpni ends
    /// (cross-dprc-links design D3). The convenience over
    /// [`disconnect_wire_proving`](Self::disconnect_wire_proving) for callers that only tear the
    /// edge down and need no destroy/reconnect proof — it drops the minted witness.
    #[must_use]
    pub fn disconnect_wire(a: WireEnd, b: WireEnd) -> Self {
        Self::disconnect_wire_proving(a, b).0
    }

    /// Disconnects a dpni↔dpni wire and mints the [`WireDisconnected`] proof the teardown and
    /// reconnect laws demand (cross-dprc-links design D5/D7; `link_lifecycle.qnt`
    /// `disconnectWireAt`). Returns the [`WireTransition::DisconnectWire`] step and the proof
    /// together — the planner pushes the disconnect, then threads the proof into
    /// [`destroy_end`](Self::destroy_end) or [`reconnect_wire`](Self::reconnect_wire), each of
    /// which reads its target end back from it. Because the proof is the only key those two
    /// constructors accept and only this mints it, the disconnect-before-destroy and
    /// disconnect-before-reconnect orders are owned by the types, not by planner discipline.
    #[must_use]
    pub fn disconnect_wire_proving(a: WireEnd, b: WireEnd) -> (Self, WireDisconnected) {
        (Self::DisconnectWire { a, b }, WireDisconnected { a, b })
    }

    /// Destroys the freed end `side` of a disconnected wire, consuming the [`WireDisconnected`]
    /// proof [`disconnect_wire_proving`](Self::disconnect_wire_proving) minted and reading the
    /// end back from it (cross-dprc-links design D5; `link_lifecycle.qnt` `destroyEndAt`, whose
    /// `not(connected)` guard this types). Because the proof has no public constructor, a destroy
    /// without a prior disconnect does not typecheck — the fields a forged literal would name are
    /// private:
    ///
    /// ```compile_fail,E0451
    /// use dpaa2_api::plan::connect::{WireDisconnected, WireEnd, WireSide, WireTransition};
    /// use dpaa2_api::core::model::{DpniId, DprcId};
    /// // `WireDisconnected`'s fields are private and its only mint is `disconnect_wire_proving`,
    /// // so a forged proof naming a never-disconnected end does not construct (LINK_I1).
    /// let forged = WireDisconnected {
    ///     a: WireEnd { dpni: DpniId::new(0), container: DprcId::ROOT },
    ///     b: WireEnd { dpni: DpniId::new(1), container: DprcId::ROOT },
    /// };
    /// let _ = WireTransition::destroy_end(forged, WireSide::A);
    /// ```
    #[must_use]
    pub fn destroy_end(proof: WireDisconnected, side: WireSide) -> Self {
        Self::DestroyEnd { proof, side }
    }

    /// Reconnects the freed end `side` of a disconnected wire to `new_peer`, consuming the
    /// [`WireDisconnected`] proof (cross-dprc-links design D7; DPRC-I5 promoted;
    /// `link_lifecycle.qnt` `reconnectAfterDisconnectTest`). Cardinality-one lives in the type:
    /// only a disconnect mints the proof this demands, so reconnecting an end a standing wire
    /// still holds is unrepresentable — a reconnect requires a prior disconnect
    /// (disconnect-before-reconnect). The fresh-connect path for never-connected ends stays
    /// [`connect_wire`](Self::connect_wire); this builds a [`WireTransition::ConnectWire`] through
    /// it, so the ancestor resolution and the executor are the delivered ones, unreshaped:
    ///
    /// ```compile_fail,E0451
    /// use dpaa2_api::plan::connect::{WireDisconnected, WireEnd, WireSide, WireTransition};
    /// use dpaa2_api::core::model::{DpniId, DprcId};
    /// // No disconnect proof, no reconnect: a held end cannot be rewired without one (DPRC-I5).
    /// let forged = WireDisconnected {
    ///     a: WireEnd { dpni: DpniId::new(0), container: DprcId::ROOT },
    ///     b: WireEnd { dpni: DpniId::new(1), container: DprcId::ROOT },
    /// };
    /// let peer = WireEnd { dpni: DpniId::new(2), container: DprcId::ROOT };
    /// let _ = WireTransition::reconnect_wire(forged, WireSide::A, peer);
    /// ```
    // Proof consumed on purpose — the law's demand, as `Link::wire` consumes its `Interface`.
    #[allow(clippy::needless_pass_by_value)]
    #[must_use]
    pub fn reconnect_wire(proof: WireDisconnected, side: WireSide, new_peer: WireEnd) -> Self {
        Self::connect_wire(proof.end(side), new_peer)
    }
}

/// A dpni↔dpni link pattern the MC refuses at actuation, surfaced as a typed value
/// (cross-dprc-links design D2; ADR-0013 §link) — never a silent drop, never a stringly
/// backend error. The plan surface pre-forbids no arrangement (the `LINK_I4` posture of
/// `models/families/link_lifecycle.qnt`: refusable moves are admitted and the MC refuses
/// them); a refusable connect is issued and its refusal is classified here, following the
/// `plan/dprc.rs` [`attribute_refusal`](crate::plan::dprc::attribute_refusal) idiom.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireRefusal {
    /// A connect issued below the common ancestor (child-issued) refused No privilege
    /// (`0x4`) because the issuing child lacks `TOPOLOGY_CHANGES_ALLOWED`, while the same
    /// connect at the root ancestor is accepted (V-DPCI-1; docs/baseline/dprc.md connect
    /// rows). Issuing at [`CONNECT_ANCESTOR`] is the resolution that avoids this pattern.
    ChildIssuedConnectUnprivileged,
}

/// Classifies an MC [`Error`] from a dpni↔dpni connect into a typed [`WireRefusal`], or
/// `None` when the error is not a wire-connect refusal (cross-dprc-links design D2; the
/// `plan/dprc.rs` [`attribute_refusal`](crate::plan::dprc::attribute_refusal) idiom). The
/// No-privilege (`0x4`) decode routes through the single core-side sentinel mapping
/// [`Refusal::from_status`](crate::families::dprc::Refusal::from_status) rather than
/// re-spelling the status byte (the classification lives once, core-side). An unrelated
/// error passes through as `None` — never swallowed, never collapsed to a backend string.
#[must_use]
pub fn attribute_wire_refusal(error: &Error) -> Option<WireRefusal> {
    match error {
        Error::McStatus { status }
            if Refusal::from_status(*status) == Some(Refusal::TopologyLockGate) =>
        {
            Some(WireRefusal::ChildIssuedConnectUnprivileged)
        }
        _ => None,
    }
}

// ---- post-bind healing-policy obligations (cross-dprc-links design D5; ADR-0022 healing row) ----

/// The consented `Disruptive` rebind cycle (unbind → bind → re-observe) that discharges a
/// standing [`DeferredVisibility`] — the sole modeled discharge (cross-dprc-links design D5;
/// `models/families/link_lifecycle.qnt` `rebindDischargeAt`). One value stands for the whole
/// cycle; its private field binds it to the endpoint whose visibility it restores, so it is
/// minted only alongside its obligation in the [`PostBindCreate`] bundle, never alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RebindCycle {
    endpoint: WireEnd,
}

impl RebindCycle {
    /// The rebind's disruption class — always [`Class::Disruptive`] (cross-dprc-links design D5;
    /// the ADR-0015 decision 12 classing the consent gate reads). The cycle flaps the kernel
    /// face, so it is never hitless.
    #[must_use]
    pub fn class(&self) -> Class {
        Class::Disruptive
    }

    /// The endpoint whose visibility this cycle restores.
    #[must_use]
    pub fn endpoint(&self) -> WireEnd {
        self.endpoint
    }
}

/// The eager drift obligation a post-bind create carries (cross-dprc-links design D5; ADR-0017
/// healing policy; `models/families/link_lifecycle.qnt` `DeferredVisibility`): the object is
/// MC-accepted but kernel-invisible, so convergence is judged by re-observation after a scan,
/// never by the create's acceptance. The private field has no public constructor — the obligation
/// is minted only inside its [`PostBindCreate`] bundle by
/// [`create_resident_deferred`](crate::families::dprc::Container::create_resident_deferred), so an
/// obligation-less post-bind create is unrepresentable, not merely refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeferredVisibility {
    endpoint: WireEnd,
}

impl DeferredVisibility {
    /// The endpoint the obligation names — the end whose re-observation discharges it.
    #[must_use]
    pub fn endpoint(&self) -> WireEnd {
        self.endpoint
    }
}

/// A post-bind create as the healing policy reifies it (cross-dprc-links design D5; ADR-0022
/// healing row): the eager [`DeferredVisibility`] obligation and its planned [`RebindCycle`]
/// discharge, minted together. The bundle IS the mechanism — both halves are built only here, by
/// [`create_resident_deferred`](crate::families::dprc::Container::create_resident_deferred), so
/// neither the obligation nor its discharge exists alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PostBindCreate {
    /// The eager obligation the create carries.
    pub obligation: DeferredVisibility,
    /// The planned discharge — the only way the obligation clears.
    pub discharge: RebindCycle,
}

impl PostBindCreate {
    /// Mints the obligation and its discharge together for `endpoint`, both bound to it — the
    /// crate-internal mechanism the Plugged create face drives
    /// ([`create_resident_deferred`](crate::families::dprc::Container::create_resident_deferred)).
    #[must_use]
    pub(crate) fn new(endpoint: WireEnd) -> Self {
        Self {
            obligation: DeferredVisibility { endpoint },
            discharge: RebindCycle { endpoint },
        }
    }
}

/// The typed standing residues this surface may carry (cross-dprc-links design D5; the
/// pool-objects honest-residue idiom). One enum mirrors the model sum sum-for-sum
/// (`models/families/link_lifecycle.qnt` `WireResidue`): a [`StaleNode`](Self::StaleNode) left by
/// a lazy post-bind destroy, and a [`DeclinedVisibility`](Self::DeclinedVisibility) left when a
/// rebind's consent is declined. Neither blocks a convergence verdict; both stand indefinitely.
/// No reboot residue lives here — reboot residue stays exclusive to ADR-0020 pool shrink.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireResidue {
    /// The destroy side: a kernel node lingering after an MC destroy in a bound container
    /// (lazy mirror, cross-dprc-links design D5). Blocks nothing, demands no discharge.
    StaleNode(WireEnd),
    /// The create side: a [`DeferredVisibility`] whose rebind consent was declined
    /// (cross-dprc-links design D5). The node stays kernel-invisible; no silent rebind fires.
    DeclinedVisibility(WireEnd),
}

impl fmt::Display for WireResidue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleNode(end) => write!(
                f,
                "stale node {} in {}: a kernel node lingering after destroy — blocks nothing, demands no discharge",
                end.dpni, end.container
            ),
            Self::DeclinedVisibility(end) => write!(
                f,
                "declined visibility {} in {}: the rebind consent was declined — the node stays kernel-invisible, no silent rebind",
                end.dpni, end.container
            ),
        }
    }
}

/// Judges consent for a post-bind create's [`RebindCycle`] discharge (cross-dprc-links task 3.4;
/// ADR-0015 decision 12): granted when `allowed` covers the cycle's [`Class::Disruptive`] through
/// the derived [`Ord`] gate, yielding the cycle; otherwise the obligation stands as a typed
/// [`WireResidue::DeclinedVisibility`] residue and nothing rebinds.
///
/// The model's `Consent = Granted | Declined` (`models/families/link_lifecycle.qnt`) folds into
/// the [`Class`] allow here deliberately — the consent judgment IS the ADR-0015 class gate, so no
/// parallel Rust `Consent` enum exists for a model-twin lint to expect.
///
/// # Errors
/// Returns [`WireResidue::DeclinedVisibility`] when `allowed` does not cover
/// [`Class::Disruptive`] — consent is declined, so the obligation stands and nothing rebinds.
pub fn discharge(plan: PostBindCreate, allowed: Class) -> Result<RebindCycle, WireResidue> {
    if allowed >= plan.discharge.class() {
        Ok(plan.discharge)
    } else {
        Err(WireResidue::DeclinedVisibility(plan.obligation.endpoint()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_legal_pair_accepts_in_both_orders() {
        for &(a, b) in &LEGAL_PAIRS {
            assert!(legal_pair(a, b), "{a:?}-{b:?}");
            assert!(legal_pair(b, a), "{b:?}-{a:?}");
        }
    }

    #[test]
    fn an_illegal_pair_refuses() {
        assert!(!legal_pair(Family::Dpbp, Family::Dpni));
        assert!(!legal_pair(Family::Dpni, Family::Dpbp));
    }

    #[test]
    fn dpdmux_facing_a_dpmac_demands_port_zero_in_both_orders() {
        assert!(legal_ports(Family::Dpdmux, 0, Family::Dpmac, 3));
        assert!(!legal_ports(Family::Dpdmux, 1, Family::Dpmac, 3));
        assert!(legal_ports(Family::Dpmac, 3, Family::Dpdmux, 0));
        assert!(!legal_ports(Family::Dpmac, 3, Family::Dpdmux, 1));
        // An unrestricted pair never fails on its ports.
        assert!(legal_ports(Family::Dpni, 2, Family::Dpmac, 5));
    }

    #[test]
    fn severed_witness_is_demanded_only_by_dpni_dpmac() {
        assert!(edge_demands_severed_witness(Family::Dpni, Family::Dpmac));
        assert!(edge_demands_severed_witness(Family::Dpmac, Family::Dpni));
        assert!(!edge_demands_severed_witness(Family::Dpni, Family::Dpni));
        assert!(!edge_demands_severed_witness(Family::Dpdmux, Family::Dpmac));
    }

    #[test]
    fn policy_rows_match_the_adr_0022_table() {
        assert_eq!(
            reification_policy(Family::Dpni, Family::Dpmac),
            Some(ReificationPolicy::Defined {
                connect: ConnectAuthority::AncestorConnectThenKernelBind,
                teardown: TeardownLaw::SeveredWitness,
            })
        );
        // dpni↔dpni never demands a severed witness: disconnect-only teardown.
        assert_eq!(
            reification_policy(Family::Dpni, Family::Dpni),
            Some(ReificationPolicy::Defined {
                connect: ConnectAuthority::AncestorConnect,
                teardown: TeardownLaw::DisconnectOnly,
            })
        );
        assert_eq!(
            reification_policy(Family::Dpdmux, Family::Dpmac),
            Some(ReificationPolicy::Defined {
                connect: ConnectAuthority::AncestorConnect,
                teardown: TeardownLaw::Refused,
            })
        );
        // A legal kind without delivered machinery is an explicit variant, not `None`.
        assert_eq!(
            reification_policy(Family::Dpci, Family::Dpci),
            Some(ReificationPolicy::AwaitsOwnChange)
        );
        assert_eq!(reification_policy(Family::Dpbp, Family::Dpni), None);
    }

    #[test]
    fn the_policy_teardown_agrees_with_the_severed_predicate() {
        for &(a, b) in &LEGAL_PAIRS {
            let severed = matches!(
                reification_policy(a, b),
                Some(ReificationPolicy::Defined {
                    teardown: TeardownLaw::SeveredWitness,
                    ..
                })
            );
            assert_eq!(severed, edge_demands_severed_witness(a, b), "{a:?}-{b:?}");
        }
    }

    // ---- dpni↔dpni wire representability, ancestor, refusal (cross-dprc-links task 3.2) ----

    fn end(dpni: u32, container: DprcId) -> WireEnd {
        WireEnd {
            dpni: DpniId::new(dpni),
            container,
        }
    }

    #[test]
    fn every_arrangement_connects_at_the_root_ancestor() {
        // root↔root, root↔child and child↔child all build the same ConnectWire shape, the
        // ancestor resolved to the root (cross-dprc-links design D2).
        let root = DprcId::ROOT;
        let child_a = DprcId::new(2);
        let child_b = DprcId::new(3);
        for (left, right) in [(root, root), (root, child_a), (child_a, child_b)] {
            assert_eq!(
                WireTransition::connect_wire(end(0, left), end(1, right)),
                WireTransition::ConnectWire {
                    a: end(0, left),
                    b: end(1, right),
                    ancestor: CONNECT_ANCESTOR,
                }
            );
        }
        assert_eq!(CONNECT_ANCESTOR, DprcId::ROOT);
    }

    #[test]
    fn two_root_ends_key_identically_to_cross_container_ends() {
        // Container-agnostic representability: two root-resident ends build a ConnectWire no
        // different in shape from a cross-container one — same variant, same resolved
        // ancestor, differing only in an end's container field (cross-dprc-links design D2).
        let root_root = WireTransition::connect_wire(end(0, DprcId::ROOT), end(1, DprcId::ROOT));
        let cross = WireTransition::connect_wire(end(0, DprcId::ROOT), end(1, DprcId::new(2)));
        assert!(matches!(
            root_root,
            WireTransition::ConnectWire { ancestor, .. } if ancestor == DprcId::ROOT
        ));
        assert!(matches!(
            cross,
            WireTransition::ConnectWire { ancestor, .. } if ancestor == DprcId::ROOT
        ));
    }

    #[test]
    fn disconnect_carries_the_ends() {
        assert_eq!(
            WireTransition::disconnect_wire(end(0, DprcId::ROOT), end(1, DprcId::new(2))),
            WireTransition::DisconnectWire {
                a: end(0, DprcId::ROOT),
                b: end(1, DprcId::new(2)),
            }
        );
    }

    // ---- plan laws: disconnect-before-destroy, disconnect-before-reconnect (cross-dprc-links task 3.5) ----
    // Negative face (no destroy/reconnect without a proof): the two `compile_fail` doctests.

    #[test]
    fn disconnect_proving_mints_a_proof_naming_both_ends() {
        let a = end(0, DprcId::ROOT);
        let b = end(1, DprcId::new(2));
        let (step, proof) = WireTransition::disconnect_wire_proving(a, b);
        // The plain disconnect is the same step with the witness dropped.
        assert_eq!(step, WireTransition::disconnect_wire(a, b));
        assert_eq!(proof.ends(), (a, b));
        assert_eq!(proof.end(WireSide::A), a);
        assert_eq!(proof.end(WireSide::B), b);
    }

    #[test]
    fn destroy_with_proof_builds_and_names_the_freed_end() {
        let a = end(0, DprcId::ROOT);
        let b = end(1, DprcId::new(2));
        let (_step, proof) = WireTransition::disconnect_wire_proving(a, b);
        let destroy = WireTransition::destroy_end(proof, WireSide::B);
        assert!(matches!(
            &destroy,
            WireTransition::DestroyEnd { proof, side: WireSide::B } if proof.end(WireSide::B) == b
        ));
    }

    #[test]
    fn reconnect_with_proof_builds_a_connect_for_the_freed_end() {
        // DPRC-I5: reconnect rewires the freed end through the fresh-connect builder (cross-dprc-links design D7).
        let a = end(0, DprcId::ROOT);
        let b = end(1, DprcId::new(2));
        let peer = end(5, DprcId::new(3));
        let (_step, proof) = WireTransition::disconnect_wire_proving(a, b);
        let reconnect = WireTransition::reconnect_wire(proof, WireSide::A, peer);
        assert_eq!(reconnect, WireTransition::connect_wire(a, peer));
        assert!(matches!(
            reconnect,
            WireTransition::ConnectWire { ancestor, .. } if ancestor == CONNECT_ANCESTOR
        ));
    }

    #[test]
    fn connect_ancestor_is_the_root_for_any_pair() {
        assert_eq!(connect_ancestor(DprcId::ROOT, DprcId::ROOT), DprcId::ROOT);
        assert_eq!(
            connect_ancestor(DprcId::new(2), DprcId::new(3)),
            DprcId::ROOT
        );
        assert_eq!(
            connect_ancestor(DprcId::ROOT, DprcId::new(9)),
            CONNECT_ANCESTOR
        );
    }

    #[test]
    fn child_issued_connect_no_privilege_is_a_typed_refusal() {
        // V-DPCI-1: child-issued connect refused No privilege (0x4) without
        // TOPOLOGY_CHANGES_ALLOWED surfaces as the typed pattern (cross-dprc-links design D2).
        assert_eq!(
            attribute_wire_refusal(&Error::McStatus { status: 0x4 }),
            Some(WireRefusal::ChildIssuedConnectUnprivileged)
        );
    }

    #[test]
    fn an_unrelated_error_passes_through_as_none() {
        // The plan/dprc.rs idiom: an error that is not a wire-connect refusal is not
        // classified — it passes through as None, never collapsed to a backend string.
        assert_eq!(
            attribute_wire_refusal(&Error::McStatus { status: 0x8 }),
            None
        );
        assert_eq!(attribute_wire_refusal(&Error::Backend("boom".into())), None);
    }

    // ---- post-bind create discharge (cross-dprc-links task 3.4; reconciler reqs 4-5) ----

    #[test]
    fn consent_covering_disruptive_yields_the_rebind_cycle() {
        // Granted == an allow covering Disruptive via Class's derived Ord (cross-dprc-links design D5).
        let there = end(2, DprcId::ROOT);
        let cycle = discharge(PostBindCreate::new(there), Class::Disruptive)
            .expect("a Disruptive allow discharges");
        assert_eq!(cycle.class(), Class::Disruptive);
        assert_eq!(cycle.endpoint(), there);
    }

    #[test]
    fn consent_below_disruptive_declines_to_typed_residue() {
        // A sub-Disruptive allow declines to a typed residue, never a silent rebind (cross-dprc-links design D5).
        let there = end(2, DprcId::new(2));
        for allowed in [Class::Hitless, Class::Boundary] {
            let residue = discharge(PostBindCreate::new(there), allowed)
                .expect_err("a sub-Disruptive allow declines");
            assert_eq!(residue, WireResidue::DeclinedVisibility(there));
        }
    }
}
