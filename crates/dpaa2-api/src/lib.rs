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

pub mod error;
pub mod model;
pub mod plan;
pub mod port;
pub mod reconcile;

#[cfg(any(test, feature = "testkit"))]
pub mod fake;

pub use error::Error;
pub use model::{
    DesiredPort, DesiredTopology, DpmacId, DpniId, Lifecycle, LinkType, MacAddr, MacMode,
    MacParseError, ObjectKind, ObservedDpmac, ObservedDpni, ObservedTopology, Presence,
};
pub use plan::{AssertMismatch, DriftReport, Plan, Transition};
pub use port::{ConfigSource, KernelControl, McControl};
pub use reconcile::{ReconcileOptions, reconcile, reconcile_with};
