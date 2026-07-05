//! Northbound TOML config frontend for DPAA2 provisioning.
//!
//! This crate is the hexagon's northbound adapter: it turns `topology.toml` into the
//! neutral [`dpaa2_api::DesiredTopology`] the reconciler consumes, and depends on the
//! core but not on any backend. The parsing/validation logic lives in the private
//! `parse` module; the private `schema` module holds the raw `serde` shapes.

mod parse;
mod schema;

pub use parse::{TomlConfig, load, parse_str};
