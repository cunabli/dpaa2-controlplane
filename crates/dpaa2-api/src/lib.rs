//! Backend-neutral domain model and pure reconciliation core for DPAA2 provisioning.
//!
//! This crate is the hexagon's centre (design D0): it defines the neutral topology
//! model, the southbound/northbound trait seams, and the pure
//! [`reconcile`](reconcile::reconcile) engine. It depends on neither a concrete MC
//! backend nor a concrete config format, so the reconciler can be exhaustively
//! tested against the in-memory `fake` backend with no hardware.
//!
//! Dependencies point *inward* to this crate: `dpaa2-mc` (southbound), `dpaa2-config`
//! (northbound), and `dpaa2-tools` (the imperative shell) all depend on it, while it
//! depends on none of them.

pub mod compiled;
mod derive;
pub mod dprc;
pub mod dprc_plan;
pub mod error;
pub mod family;
pub mod intent;
pub mod inventory;
pub mod matcher;
pub mod model;
pub mod plan;
pub mod port;
pub mod reconcile;
pub mod refuse;
pub mod types;

#[cfg(any(test, feature = "testkit"))]
pub mod fake;

/// Shared test fixtures (behind the `testkit` feature): the reference-board
/// inventory both the crate's unit tests and its `compile` integration tests
/// assert against, single-sourced here (`models/intent/inventory.qnt`
/// `REF_INVENTORY`; ADR-0013 §7) so the two suites cannot drift.
#[cfg(any(test, feature = "testkit"))]
pub mod testkit {
    use std::collections::BTreeMap;

    use crate::family::Family;
    use crate::inventory::{
        Availability, Ceiling, DpmacLinkType, DpmacOffer, EthInterface, Inventory,
    };
    use crate::model::DpmacId;

    /// The ADR-0003 §3 total-deny reservation reason carried by dpmac.3.
    pub const RESERVED_3: &str =
        "ADR-0003 §3: wired to a peer that must never see traffic (total-deny)";

    /// One `(DpmacId, DpmacOffer)` entry of the reference inventory's dpmac map.
    #[must_use]
    pub fn offer(id: u32, rate: i64, avail: Availability) -> (DpmacId, DpmacOffer) {
        let d = DpmacId::new(id);
        (
            d,
            DpmacOffer {
                id: d,
                max_rate: rate,
                eth_if: EthInterface::Xfi,
                link_type: DpmacLinkType::Phy,
                avail,
            },
        )
    }

    /// The reference board inventory (`models/intent/inventory.qnt` `REF_INVENTORY`)
    /// with a variable online-CPU count — the one axis the alphabet's `REF_INVENTORY`
    /// fixes, varied by callers to exercise the kernel per-CPU draw.
    #[must_use]
    pub fn ref_inventory(cpus: u32) -> Inventory {
        let dpmacs = BTreeMap::from([
            offer(3, 25_000, Availability::Reserved(RESERVED_3.to_owned())),
            offer(4, 25_000, Availability::Free),
            offer(5, 25_000, Availability::Free),
            offer(6, 25_000, Availability::Free),
            offer(7, 10_000, Availability::Free),
            offer(8, 10_000, Availability::Free),
            offer(9, 10_000, Availability::Free),
            offer(10, 10_000, Availability::Free),
            offer(
                17,
                1_000,
                Availability::Reserved("ADR-0003 §3: management plane (dpni.0)".to_owned()),
            ),
        ]);
        let ceilings = BTreeMap::from([
            (Family::Dprc, Ceiling::Unknown),
            (
                Family::Dpni,
                Ceiling::Observed {
                    n: 18,
                    provenance: "ADR-0011 decision 2".to_owned(),
                },
            ),
            (Family::Dpbp, Ceiling::Counted(63)),
            (Family::Dpio, Ceiling::Unknown),
            (Family::Dpcon, Ceiling::Unknown),
            (
                Family::Dpmcp,
                Ceiling::Observed {
                    n: 203,
                    provenance: "ADR-0011 decision 3".to_owned(),
                },
            ),
            (Family::Dpseci, Ceiling::Unknown),
            (Family::Dpsw, Ceiling::Unknown),
        ]);
        Inventory {
            cpus,
            dpmacs,
            labels: BTreeMap::from([((Family::Dpni, 0), String::new())]),
            ceilings,
        }
    }
}

pub use compiled::{
    AttachPoint, Attributes, CompiledPlan, Container, Edge, Interface, Measurement, ObjectKey,
    PlannedObject, ProvenanceKey, ProvenanceNode,
};
// The child-DPRC lifecycle keeps its own module namespace (`dpaa2_api::dprc::*`): its
// containment `Refusal` is a distinct vocabulary from the intent-compile
// [`refuse::Refusal`] (design D4), so the two are deliberately not flattened into one
// crate-root namespace where they would collide.
pub use error::Error;
pub use family::{ALL_FAMILIES, DERIVED_FAMILIES, Family, Permission};
pub use intent::{
    Crypto, Dataplane, Extra, Fabric, Intent, Isolation, KERNEL, Link, Member, Port, Switching,
    Tenant, TenantRef, kernel_tenant,
};
pub use inventory::{Availability, Ceiling, DpmacLinkType, DpmacOffer, EthInterface, Inventory};
pub use matcher::{
    Ambiguity, BoardObject, ConfigFacet, Handle, MatchObject, MatchPair, MatchPlan, MatchVerdict,
    apply as apply_match, converge as converge_match, converge_class as converge_match_class,
    match_board, pair_class,
};
pub use model::{
    DesiredPort, DesiredTopology, DpmacId, DpniId, FacetMismatch, Lifecycle, LinkType, MacAddr,
    MacMode, MacParseError, ObservedDpmac, ObservedDpni, ObservedTopology, Presence,
};
pub use plan::{AssertMismatch, Class, DriftReport, Plan, Transition};
pub use port::{ConfigSource, KernelControl, McControl};
pub use reconcile::{ReconcileOptions, reconcile, reconcile_with};
pub use refuse::{
    Compiled, REFUSAL_VARIANTS, Referrer, Refusal, WARNING_VARIANTS, Warning, compile,
};
pub use types::{ConstructName, RuleName, TenantName};
