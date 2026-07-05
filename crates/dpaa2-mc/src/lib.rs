//! Southbound `restool`/sysfs backend for DPAA2 provisioning.
//!
//! This crate is the hexagon's southbound adapter: it implements the neutral
//! [`dpaa2_api::McControl`] and [`dpaa2_api::KernelControl`] trait seams over
//! `restool` v2.4 and the fsl-mc sysfs bus, so the pure core can drive real
//! hardware without depending on either. It introduces **no `unsafe` code**.
//!
//! The implementation lives in dedicated modules; this file only wires them up and
//! re-exports the public surface:
//! - [`restool`]: the [`RestoolMc`] MC shim and command recipe.
//! - [`kernel`]: the [`SysfsKernel`] netdev/bind adapter.
//! - [`runner`]: the [`Runner`] seam and its `restool` process implementation.
//! - [`parse`]: pure parsers for `restool` output.

pub mod kernel;
pub mod parse;
pub mod restool;
pub mod runner;

pub use kernel::SysfsKernel;
pub use restool::{DEFAULT_CONTAINER, RestoolMc};
pub use runner::{RestoolRunner, Runner};
