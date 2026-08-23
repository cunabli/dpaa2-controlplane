//! Dual-mode model-based-testing harness for the DPAA2 control plane.
//!
//! Binds the Quint model corpus under `models/` to the board through one
//! shared adapter (model action ↔ restool command ↔ read-back
//! observation) and provides the batch-suite generator, the online
//! driver, the ITF trace replayer, and the coded port safety envelope.
//! See `openspec/changes/verify-foundation` for the requirements.

pub mod adapter;
pub mod itf;
pub mod replay;
