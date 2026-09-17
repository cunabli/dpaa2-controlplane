//! The board-suite half of the harness: the restool adapter and driver, the
//! read-back snapshot and coded safety envelope, the batch-suite generator, the
//! machine-readable verdicts, and the four-ledger cross-check (R1–R10).

pub mod adapter;
pub mod driver;
/// The read-only fit-check emitter: renders a board sitting from a probe
/// plan (2026-08-30-verify-foundation task 4.1, 2026-08-22-restool-baseline design D12).
pub mod fitcheck;
pub mod generate;
/// The fsl-mc ioctl command-id policy as code (2026-08-30-verify-foundation task 6.5).
pub mod ioctlpolicy;
/// Cross-checks the hand-maintained coverage/baseline/suite/roadmap docs.
pub mod ledger;
/// The MC command status table as code (2026-08-30-verify-foundation task 6.4).
pub mod mcstatus;
pub mod replay;
pub mod safety;
pub mod snapshot;
/// Machine-readable verdicts and their per-suite index (2026-08-30-verify-foundation task 6.2).
pub mod verdict;
