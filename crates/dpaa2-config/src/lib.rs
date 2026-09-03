//! Northbound TOML config frontend for DPAA2 provisioning.
//!
//! This crate is the hexagon's northbound adapter: it turns `topology.toml` into the
//! neutral [`dpaa2_api::Intent`] the compiler consumes, and depends on the core but
//! not on any backend. It parses and validates *intent* only — turning intent plus an
//! inventory into the object plan is [`dpaa2_api::compile`]'s (design D10). The
//! validation/conversion logic lives in the private `parse` module; the private
//! `schema` module holds the raw `serde` shapes, so no serde derive reaches the core.
mod parse;
mod schema;

pub use parse::{TomlConfig, load, parse_schema, parse_str};
