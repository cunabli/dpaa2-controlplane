//! Backend-neutral domain model and pure reconciliation core for DPAA2 provisioning.
//!
//! This crate is the hexagon's centre (design D0): it defines the neutral topology
//! model, the southbound/northbound trait seams, and the pure
//! [`reconcile`](plan::reconcile::reconcile) engine. It depends on neither a concrete MC
//! backend nor a concrete config format, so the reconciler can be exhaustively
//! tested against the in-memory `fake` backend with no hardware.
//!
//! Dependencies point *inward* to this crate: `dpaa2-mc` (southbound), `dpaa2-config`
//! (northbound), and `dpaa2-tools` (the imperative shell) all depend on it, while it
//! depends on none of them.

#![warn(clippy::wildcard_imports)]

pub mod contract;
pub mod core;
pub mod families;
pub mod intent;
pub mod plan;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

// The child-DPRC lifecycle keeps its own module namespace (`dpaa2_api::families::dprc::*`): its
// containment `Refusal` is a distinct vocabulary from the intent-compile
// [`intent::refuse::Refusal`] (2026-08-22-restool-baseline design D4), so the two are deliberately not flattened into one
// namespace where they would collide.
/// Temporary flat aliases; retire when the yfg.5 importer commits land (ADR-0018).
pub use self::intent::compiled;
pub use self::intent::compiled::{
    AttachPoint, Attributes, CompiledPlan, Container, Edge, Interface, Measurement, ObjectKey,
    PlannedObject, ProvenanceKey, ProvenanceNode,
};
pub use self::intent::refuse;
pub use self::intent::refuse::{
    Compiled, REFUSAL_VARIANTS, Referrer, Refusal, WARNING_VARIANTS, Warning, compile,
};
pub use self::intent::{
    Crypto, Dataplane, Extra, Fabric, Intent, Isolation, KERNEL, Link, Member, Port, Switching,
    Tenant, TenantRef, kernel_tenant,
};
