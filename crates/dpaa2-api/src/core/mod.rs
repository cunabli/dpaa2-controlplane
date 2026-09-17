//! The pure domain: entities, value types, shared sums, errors, and the macros
//! shared across MC object families (ADR-0018).
//!
//! Nothing here names IO or an adapter. This is the one shared home for the
//! neutral vocabulary — there is no sibling `model` namespace to file against, so
//! "core or model?" is never a question. Mirrors `models/core/` on the quint side
//! (ADR-0014).

pub mod error;
pub mod family;
pub mod inventory;
pub mod model;
pub mod types;
