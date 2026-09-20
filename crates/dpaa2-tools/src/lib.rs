//! Imperative shell for DPAA2 provisioning: the convergence loop, status reporting,
//! and stable-naming `.link` generation that drive the pure core against a concrete
//! backend.
//!
//! The logic lives in a library so it can be exercised against the in-memory fake
//! backend with no hardware (design D10; restool-baseline); the `dpaa2ctl` binary is a thin CLI over
//! it.

pub mod engine;
pub mod link;
pub mod render;
pub mod status;

pub use engine::{
    ContainerOutcome, ConvergeConfig, Outcome, PoolDrift, PoolFamilyDrift, PoolOutcome, apply,
    converge_containers, converge_pools, ensure, observe, plan_containers, plan_pools,
};
pub use status::{PortStatus, StatusReport};
