//! Per-object-family vocabulary: the sums, typestates, verb surfaces, and
//! per-family refusal payloads each MC object family carries, filed one module
//! per family as the tiles land (ADR-0018).
//!
//! Mirrors `models/families/` one-to-one so the correspondence is checkable by
//! path — `families/dprc.rs` twins `models/families/dprc.qnt` (the ADR-0014
//! quint mirror).

pub mod dprc;
