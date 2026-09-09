//! Imperative shell for DPAA2 provisioning: the convergence loop, status reporting,
//! and stable-naming `.link` generation that drive the pure core against a concrete
//! backend.
//!
//! The logic lives in a library so it can be exercised against the in-memory fake
//! backend with no hardware (design D10); the `dpaa2ctl` binary is a thin CLI over
//! it.

pub mod engine;
pub mod link;
pub mod render;
pub mod status;

pub use engine::{
    ContainerOutcome, ConvergeConfig, Outcome, apply, converge_containers, ensure, observe,
    plan_containers,
};
pub use status::{PortStatus, StatusReport};
