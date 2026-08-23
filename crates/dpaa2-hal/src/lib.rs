//! Typed, policy-free primitives for the kernel interfaces DPAA2 hardware is
//! reached through (ADR-0001 §6).
//!
//! This crate is the hexagon's lowest layer: each module speaks one kernel
//! interface (today the fsl-mc sysfs bus; the VFIO binding, netlink link
//! operations, and the MC-portal ioctl transport arrive with the changes that
//! consume them) and exposes it as plain typed operations with `std::io`
//! errors. Policy — retry, tolerance, error mapping, trait seams — belongs to
//! the adapters above (`dpaa2-mc`), never here.

pub mod sysfs;

pub use sysfs::FslMcSysfs;
