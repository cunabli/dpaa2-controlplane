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

pub use compiled::{
    AttachPoint, Attributes, CompiledPlan, Container, Edge, Interface, Measurement, ObjectKey,
    PlannedObject, ProvenanceKey, ProvenanceNode,
};
pub use error::Error;
pub use family::{ALL_FAMILIES, DERIVED_FAMILIES, Family, Permission};
pub use intent::{
    Crypto, Dataplane, Extra, Fabric, Intent, Isolation, KERNEL, Link, Member, Port, Switching,
    Tenant, kernel_tenant,
};
pub use inventory::{Availability, Ceiling, DpmacLinkType, DpmacOffer, EthInterface, Inventory};
pub use matcher::{
    Ambiguity, BoardObject, ConfigFacet, Handle, MatchObject, MatchPair, MatchPlan, MatchVerdict,
    apply as apply_match, converge as converge_match, converge_class as converge_match_class,
    match_board, pair_class,
};
pub use model::{
    DesiredPort, DesiredTopology, DpmacId, DpniId, FacetMismatch, Lifecycle, LinkType, MacAddr,
    MacMode, MacParseError, ObjectKind, ObservedDpmac, ObservedDpni, ObservedTopology, Presence,
};
pub use plan::{AssertMismatch, Class, DriftReport, Plan, Transition};
pub use port::{ConfigSource, KernelControl, McControl};
pub use reconcile::{ReconcileOptions, reconcile, reconcile_with};
pub use refuse::{Compiled, REFUSAL_VARIANTS, Refusal, WARNING_VARIANTS, Warning, compile};
pub use types::{ConstructName, RuleName, TenantName};
