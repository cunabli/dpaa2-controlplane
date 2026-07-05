//! The on-disk TOML schema.
//!
//! These types exist only to deserialize `topology.toml`; they are converted into
//! the neutral [`dpaa2_api::DesiredTopology`] in [`crate`], so no `serde` derive ever
//! leaks into the core (config spec). Ports are keyed by their stable DPMAC anchor
//! and never by a DPNI index.

use serde::Deserialize;

/// The whole topology file: a list of ports.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTopology {
    /// The `[[port]]` array.
    #[serde(default)]
    pub port: Vec<RawPort>,
}

/// How a port's MAC is treated, as written in TOML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RawMacMode {
    /// Verify only (default).
    #[default]
    Assert,
    /// Set the DPNI primary MAC.
    Actuate,
}

/// A single `[[port]]` table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawPort {
    /// The stable DPMAC anchor, e.g. `"dpmac.3"`.
    pub dpmac: String,
    /// The stable interface name to assign.
    pub name: String,
    /// The port's known/declared MAC, e.g. `"02:00:00:00:00:03"`.
    #[serde(default)]
    pub mac: Option<String>,
    /// Whether the MAC is asserted (default) or actuated.
    #[serde(default)]
    pub mac_mode: RawMacMode,
    /// Present only to reject DPNI-index pinning with a targeted error; a value
    /// here is always invalid (topology-config spec).
    #[serde(default)]
    pub dpni: Option<String>,
}
