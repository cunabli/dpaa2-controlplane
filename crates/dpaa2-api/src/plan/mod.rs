//! The pure planning half of the hexagon at the namespace root (ADR-0018): the
//! reconcile engine ([`reconcile`]), the identity-across-time [`matcher`], and the
//! plan value types live here, with family-specific planners filed as `plan/<f>.rs`
//! ([`dprc`] first, the pattern for the rest).
//!
//! The output of reconciliation: an ordered [`Plan`] of [`Transition`]s plus
//! non-actuating [`DriftReport`]s and [`AssertMismatch`]es.
//!
//! Transitions that *create* an object reference the port's stable [`DpmacId`](crate::core::model::DpmacId)
//! anchor rather than a DPNI index, because the index is not known until the MC
//! assigns it at create time (design D1; restool-baseline). Transitions that *tear down* an existing
//! object reference the observed [`DpniId`](crate::core::model::DpniId).

pub mod dprc;
pub mod matcher;
pub mod reconcile;

mod report;
mod transition;

pub use report::{AssertMismatch, DriftReport, Plan};
pub use transition::{Class, Transition};
