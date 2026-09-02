//! The observed hardware offer: `compile`'s second input, read never written
//! (design D2; ADR-0013 §3).
//!
//! Transcribed from `models/intent/types.qnt`. The inventory is what the hardware
//! offers — each dpmac with the attributes `dpmac info` reports and an
//! [`Availability`] from the ADR-0003 safety matrix, the DPL-owned objects a plan
//! must never claim (ADR-0001 §4), and one three-valued [`Ceiling`] per derived
//! family (ADR-0011). `ensure` reads it from the board; tests and the model read it
//! from change #2's reference snapshot. It carries no `serde` (design D10).

use std::collections::BTreeMap;

use crate::family::Family;
use crate::model::DpmacId;

/// A dpmac's physical media type, immutable for the object's life (DPMAC-I3,
/// `dpmac.md`; `types.qnt` `EthInterface`). Only the reference board's values are
/// enumerated; a new value enters with a board that has one.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EthInterface {
    /// 10G fibre (XFI).
    Xfi,
    /// 25G+ (CAUI).
    Caui,
    /// RGMII copper.
    Rgmii,
}

/// A dpmac's link type, as `dpmac info` reports it (DPMAC-I3; `types.qnt`
/// `LinkType`).
///
/// Distinct from [`crate::LinkType`], the reconciler's two-valued Phy/Fixed
/// abstraction (design E1): this is the four-valued *inventory attribute* the
/// board reports, kept separate so neither shadows the other.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DpmacLinkType {
    /// No link configured.
    None,
    /// A fixed link (no PHY, no netdev).
    Fixed,
    /// A PHY-backed link.
    Phy,
    /// A backplane link.
    Backplane,
}

/// Whether a dpmac may anchor a port (design D2; `types.qnt` `Availability`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Availability {
    /// Free to anchor a port.
    Free,
    /// Reserved by the ADR-0003 §3 safety matrix (dpmac.3 total-deny, dpmac.17
    /// management plane), with the reason.
    Reserved(String),
    /// Owned by a DPL object (ADR-0001 §4), with its owner label — the fit check
    /// classifies these as foreign rather than drift.
    Foreign(String),
}

/// One dpmac the board offers, by the attributes `dpmac info` reports (design D2;
/// `types.qnt` `DpmacOffer`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DpmacOffer {
    /// The stable dpmac id (DPMAC-I1).
    pub id: DpmacId,
    /// The maximum supported rate in Mbps (`dpmac info` "maximum supported rate").
    pub max_rate: i64,
    /// The media type.
    pub eth_if: EthInterface,
    /// The link type.
    pub link_type: DpmacLinkType,
    /// Whether the dpmac may anchor a port.
    pub avail: Availability,
}

/// ADR-0011's three-valued pool ceiling as a type (design D2; `types.qnt`
/// `Ceiling`).
///
/// Feasibility refuses against [`Ceiling::Counted`] and [`Ceiling::Observed`] and
/// warns on [`Ceiling::Unknown`]; nothing here invents a number.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Ceiling {
    /// The listed pool is the ceiling (dpbp, ADR-0011 decision 1).
    Counted(i64),
    /// An unlisted ceiling the board measured, with its provenance (dpni at 18,
    /// ADR-0011 decision 2).
    Observed {
        /// The measured ceiling.
        n: i64,
        /// The ADR-0011 provenance of the measurement.
        provenance: String,
    },
    /// The cap ended without a refusal, or the family was never measured.
    Unknown,
}

/// The hardware offer as data (design D2; `types.qnt` `Inventory`).
///
/// Observed, never operator-written — an operator-written inventory would be a
/// second source of truth the board contradicts (design D2, alternative rejected).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Inventory {
    /// Online CPUs of the kernel container: the kernel dataplane draws one dpio
    /// per CPU (ADR-0012).
    pub cpus: u32,
    /// Every DPC-born dpmac, by id (DPMAC-I1).
    pub dpmacs: BTreeMap<DpmacId, DpmacOffer>,
    /// DPL-owned objects a plan must never claim, with their owner label
    /// (ADR-0001 §4). Keyed by `(family, number)`.
    pub foreign: BTreeMap<(Family, u32), String>,
    /// One ceiling per derived family ([`crate::DERIVED_FAMILIES`] is the domain).
    pub ceilings: BTreeMap<Family, Ceiling>,
}

impl Inventory {
    /// The availability of an object id against the inventory (design D2;
    /// `types.qnt` `availabilityOf`): a dpmac reads its offer, a DPL-owned object
    /// reads [`Availability::Foreign`], anything else is free to derive.
    #[must_use]
    pub fn availability_of(&self, family: Family, num: u32) -> Availability {
        if family == Family::Dpmac
            && let Some(offer) = self.dpmacs.get(&DpmacId::new(num))
        {
            return offer.avail.clone();
        }
        if let Some(owner) = self.foreign.get(&(family, num)) {
            return Availability::Foreign(owner.clone());
        }
        Availability::Free
    }
}
