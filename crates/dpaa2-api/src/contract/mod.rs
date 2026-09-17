//! The hexagonal contracts: trait seams the pure core depends on
//! (ADR-0018; 2026-08-22-restool-baseline design D0, D6).
//!
//! These traits are the crate's quint-able contracts — adapters implement or
//! consume them, their obligations are what `models/core/` constrains, and
//! dpaa2-verify replays frozen traces through their implementations (ADR-0018).
//! The namespace is `contract`, not `ports` (a port is a domain noun in DPAA2 —
//! network ports live in [`crate::core`]), not `io` (a sans-io crate exports no
//! `::io`, and DPIO is an object family), and not `adapters` (the concrete
//! implementations live in the adapter crates, outside this hexagon).
//!
//! The reconciler references only these traits, never a concrete `restool` or ioctl
//! type (mc-backend spec: "Core depends only on traits"). Two southbound seams split
//! MC-portal work ([`McControl`]) from kernel-side binding and netdev observation
//! ([`KernelControl`]), because binding is often a state we *wait to observe* rather
//! than an action we execute. One northbound seam ([`ConfigSource`]) yields the
//! neutral [`Intent`](crate::intent::Intent).

/// The in-crate reference backend (2026-08-22-restool-baseline design D10): a hardware-free double that
/// implements the southbound seams over an in-memory model for tests.
#[cfg(any(test, feature = "testkit"))]
pub mod fake;

mod config;
mod kernel;
mod mc;

pub use config::ConfigSource;
pub use kernel::KernelControl;
pub use mc::McControl;
