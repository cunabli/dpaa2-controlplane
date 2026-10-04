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

use crate::core::family::Family;

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
}
