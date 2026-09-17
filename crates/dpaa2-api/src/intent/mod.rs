//! The intent vocabulary: what an operator states, never a count (design D1;
//! ADR-0005 §1; ADR-0013 §2).
//!
//! Transcribed from `models/intent/types.qnt`, the accepted vocabulary of the
//! 2026-09-02 gate (ADR-0013). An operator declares network *constructs* anchored
//! in hardware — a [`Tenant`] with a [`Dataplane`] and a core budget, a [`Port`]
//! with the rate it must deliver, a [`Link`] between two tenants, a [`Fabric`] one
//! tenant forwards, a [`Crypto`] block sized by its own flows — and no field for a
//! dpio, dpbp, dpcon, dpmcp, queue or worker count. Every such number is the
//! derivation's (`compile`, task 3.2). These types carry no `serde`: the northbound
//! [`crate::contract::ConfigSource`] parses TOML into them (2026-08-22-restool-baseline design D10), and nothing below the
//! compiler depends on them (design D11).
//!
//! This module is also the root of the intent-compile pipeline namespace (ADR-0018):
//! the vocabulary here, the pure derivation ([`derive`], private), the object plan it
//! yields ([`compiled`]), and the refusal machinery that gates it ([`refuse`]) file
//! under `dpaa2_api::intent::*` so the compile half lives in one namespace, as the
//! plan half does under `dpaa2_api::plan::*`.

pub mod compiled;
mod derive;
pub mod refuse;

mod crypto;
mod declared;
mod extra;
mod fabric;
mod link;
mod port;
mod tenant;

pub use crypto::Crypto;
pub use declared::Intent;
pub use extra::Extra;
pub use fabric::{Fabric, Member, Switching};
pub use link::Link;
pub use port::Port;
pub use tenant::{Dataplane, Isolation, KERNEL, Tenant, TenantRef, kernel_tenant};
