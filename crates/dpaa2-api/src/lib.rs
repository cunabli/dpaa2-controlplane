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

#![warn(clippy::wildcard_imports)]

pub mod compiled;
pub mod contract;
pub mod core;
mod derive;
pub mod dprc;
pub mod dprc_plan;
pub mod intent;
pub mod matcher;
pub mod plan;
pub mod reconcile;
pub mod refuse;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use compiled::{
    AttachPoint, Attributes, CompiledPlan, Container, Edge, Interface, Measurement, ObjectKey,
    PlannedObject, ProvenanceKey, ProvenanceNode,
};
// The child-DPRC lifecycle keeps its own module namespace (`dpaa2_api::dprc::*`): its
// containment `Refusal` is a distinct vocabulary from the intent-compile
// [`refuse::Refusal`] (design D4), so the two are deliberately not flattened into one
// crate-root namespace where they would collide.
pub use intent::{
    Crypto, Dataplane, Extra, Fabric, Intent, Isolation, KERNEL, Link, Member, Port, Switching,
    Tenant, TenantRef, kernel_tenant,
};
pub use matcher::{
    Ambiguity, BoardObject, ConfigFacet, Handle, MatchObject, MatchPair, MatchPlan, MatchVerdict,
    apply as apply_match, converge as converge_match, converge_class as converge_match_class,
    match_board, pair_class,
};
pub use plan::{AssertMismatch, Class, DriftReport, Plan, Transition};
pub use reconcile::{ReconcileOptions, reconcile, reconcile_with};
pub use refuse::{
    Compiled, REFUSAL_VARIANTS, Referrer, Refusal, WARNING_VARIANTS, Warning, compile,
};

// Temporary flat-path aliases: the importer crates migrate to the ADR-0018
// namespaced paths one commit at a time; this block retires at the end of
// the series (bead dpaa2-controlplane-yfg.2).
pub use self::contract as port;
#[cfg(any(test, feature = "testkit"))]
pub use self::contract::fake;
pub use self::contract::{ConfigSource, KernelControl, McControl};
pub use self::core::error::Error;
pub use self::core::family::{ALL_FAMILIES, DERIVED_FAMILIES, Family, Permission};
pub use self::core::inventory::{
    Availability, Ceiling, DpmacLinkType, DpmacOffer, EthInterface, Inventory,
};
pub use self::core::model::{
    DesiredPort, DesiredTopology, DpmacId, DpniId, DprcId, FacetMismatch, Lifecycle, LinkType,
    MacAddr, MacMode, MacParseError, ObjectRef, ObservedDpmac, ObservedDpni, ObservedTopology,
    Presence,
};
pub use self::core::types::{ConstructName, RuleName, TenantName};
pub use self::core::{error, family, inventory, model, types};
