//! Southbound `restool`/sysfs backend for DPAA2 provisioning.
//!
//! This crate is the hexagon's southbound adapter: it implements the neutral
//! [`dpaa2_api::contract::McControl`] and [`dpaa2_api::contract::KernelControl`] trait seams over
//! `restool` v2.4 and the fsl-mc sysfs bus (through the `dpaa2-hal`
//! primitives), so the pure core can drive real hardware without depending on
//! either. It introduces **no `unsafe` code**.
//!
//! The implementation lives in dedicated modules; this file only wires them up and
//! re-exports the public surface:
//! - [`restool`]: the [`RestoolMc`] MC shim and command recipe.
//! - [`kernel`]: the [`SysfsKernel`] netdev/bind adapter.
//! - [`pool`]: the pool-family delta→id dispatch edge (pool-objects design D2).
//! - [`populate`]: child-container population + the VFIO handoff (pool-objects task 3.3).
//! - [`probe`]: the kernel root-bind read-back, judged per-target (pool-objects task 3.2).
//! - [`runner`]: the [`Runner`] seam and its `restool` process implementation.
//! - [`parse`]: pure parsers for `restool` output.

pub mod kernel;
pub mod parse;
pub mod pool;
pub mod populate;
pub mod probe;
pub mod restool;
pub mod runner;

pub use kernel::SysfsKernel;
pub use pool::{PoolDispatch, create_dpio_seat, dispatch_pool_deltas};
pub use populate::{ChildPopulation, populate_child, vfio_handoff};
pub use probe::observe_bind_probe;
pub use restool::{DEFAULT_CONTAINER, RestoolMc};
pub use runner::{RestoolRunner, RunOutcome, Runner};
