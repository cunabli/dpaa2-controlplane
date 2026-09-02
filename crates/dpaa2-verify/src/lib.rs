//! Dual-mode model-based-testing harness for the DPAA2 control plane.
//!
//! Binds the Quint model corpus under `models/` to the board through one
//! shared adapter (model action ↔ restool command ↔ read-back
//! observation) and provides the batch-suite generator, the online
//! driver, the ITF trace replayer, and the coded port safety envelope.
//! See `openspec/changes/verify-foundation` for the requirements.

pub mod adapter;
pub mod driver;
pub mod generate;
pub mod intent_itf;
/// The fsl-mc ioctl command-id policy as code (task 6.5).
pub mod ioctlpolicy;
pub mod itf;
/// Cross-checks the hand-maintained coverage/baseline/suite/roadmap docs.
pub mod ledger;
/// The MC command status table as code (task 6.4).
pub mod mcstatus;
pub mod replay;
pub mod safety;
pub mod snapshot;
/// Machine-readable verdicts and their per-suite index (task 6.2).
pub mod verdict;
