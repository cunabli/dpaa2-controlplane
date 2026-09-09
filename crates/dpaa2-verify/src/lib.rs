//! Dual-mode model-based-testing harness for the DPAA2 control plane.
//!
//! Binds the Quint model corpus under `models/` to the board through one
//! shared adapter (model action ↔ restool command ↔ read-back
//! observation) and provides the batch-suite generator, the online
//! driver, the ITF trace replayer, and the coded port safety envelope.
//! See `openspec/changes/verify-foundation` for the requirements.

pub mod adapter;
/// Reader for frozen dprc-lifecycle ITF traces (dprc-encapsulation task 2.3); the
/// stepped-machine twin of [`intent_itf`], replayed by `tests/dprc_replay.rs`.
pub mod dprc_itf;
pub mod driver;
pub mod edits_itf;
/// The read-only fit-check emitter: renders a board sitting from a probe
/// plan (verify-foundation task 4.1, design D12).
pub mod fitcheck;
pub mod generate;
pub mod intent_itf;
/// The fsl-mc ioctl command-id policy as code (verify-foundation task 6.5).
pub mod ioctlpolicy;
pub mod itf;
/// Cross-checks the hand-maintained coverage/baseline/suite/roadmap docs.
pub mod ledger;
/// The MC command status table as code (verify-foundation task 6.4).
pub mod mcstatus;
/// Reader, TOML emitter, and verdict matcher for frozen raw-conformance ITF
/// traces (intent-layer task 3.3e).
pub mod raw_itf;
pub mod replay;
pub mod safety;
pub mod snapshot;
/// Machine-readable verdicts and their per-suite index (verify-foundation task 6.2).
pub mod verdict;
