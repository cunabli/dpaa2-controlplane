//! Shared test fixtures (behind the `testkit` feature): the reference-board
//! inventory both the crate's unit tests and its `compile` integration tests
//! assert against, single-sourced here (`models/intent/inventory.qnt`
//! `REF_INVENTORY`; ADR-0013 §7) so the two suites cannot drift.
//!
//! The `dpaa2_api::testkit` path is unchanged by the ADR-0018 module-tree move;
//! its final namespace home is deferred to a later step (bead
//! dpaa2-controlplane-yfg.6).
//!
//! It sits outside the five ADR-0018 namespaces (core, intent, plan, contract, families) as cfg-gated test scaffolding, not part of the crate's public domain surface.

use std::collections::BTreeMap;

use crate::core::family::Family;
use crate::core::inventory::{
    Availability, Ceiling, DpmacLinkType, DpmacOffer, EthInterface, Inventory,
};
use crate::core::model::DpmacId;

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
