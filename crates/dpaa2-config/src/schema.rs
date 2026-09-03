//! The on-disk TOML schema (the intent construct vocabulary, ADR-0013 §2).
//!
//! These types exist only to deserialize `topology.toml`; [`crate::parse`] validates
//! them and converts them into the neutral [`dpaa2_api::Intent`], so no `serde` derive
//! ever leaks into the core (topology-config spec; design D10). Every table is
//! `deny_unknown_fields`, so a mistyped or retired key is rejected by name rather than
//! silently ignored. Ports are keyed by their stable DPMAC anchor and never by a DPNI
//! index, and no construct carries a dpio/dpbp/dpcon/dpmcp/queue/worker count — those
//! are the derivation's (design D1). Every construct table carries those retired count
//! keys as explicit rejected `Option`s (minted by [`construct_table`]) so the parser
//! names them in the "count is derived" error instead of the generic unknown-field one.
//!
//! Name slots deserialize straight into the [`TenantName`]/[`ConstructName`] newtypes
//! the neutral model uses, so a name cannot be confused among slots even in the raw
//! layer (types.rs). `serde` stays out of `dpaa2-api` (design D10): the [`name`] family
//! of `deserialize_with` helpers convert through the newtypes' infallible `From<String>`
//! rather than a foreign `Deserialize` impl. Slots whose value can be *malformed* —
//! `dpmac`, `mac`, `family` — stay `String` on purpose, so [`crate::parse`] can name the
//! offending construct in the error; a `deserialize_with` type error would lose it.

use dpaa2_api::{ConstructName, TenantName};
use serde::{Deserialize, Deserializer};

/// Deserializes a name slot straight into its dpaa2-api newtype (types.rs), through the
/// newtype's infallible `From<String>` so `serde` need never touch `dpaa2-api` (design D10).
fn name<'de, D, T>(de: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: From<String>,
{
    Ok(T::from(String::deserialize(de)?))
}

/// The `Option<T>` sibling of [`name`], for the optional name slots (`pool`, `tenant`).
fn name_opt<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: From<String>,
{
    Ok(Option::<String>::deserialize(de)?.map(T::from))
}

/// The `Vec<T>` sibling of [`name`], for a fabric's member list.
fn name_vec<'de, D, T>(de: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: From<String>,
{
    Ok(Vec::<String>::deserialize(de)?
        .into_iter()
        .map(T::from)
        .collect())
}

/// Defines a construct table that also carries the six retired count keys
/// (dpio/dpbp/dpcon/dpmcp/queues/workers) as rejected `Option`s. `serde` accepts the
/// key under `deny_unknown_fields` so [`crate::parse`] names it in the "count is
/// derived" error rather than the generic unknown-field one (topology-config spec: "A
/// count field is rejected"; design D1: counts are derived, never declared). The body
/// fields are written per table; the rejected counts are appended identically to each.
macro_rules! construct_table {
    ($(#[$meta:meta])* $name:ident { $($field:tt)* }) => {
        $(#[$meta])*
        #[derive(Debug, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($field)*
            /// Present only to reject a derived count with a targeted error (design D1).
            #[serde(default)]
            pub dpio: Option<i64>,
            /// See [`Self::dpio`].
            #[serde(default)]
            pub dpbp: Option<i64>,
            /// See [`Self::dpio`].
            #[serde(default)]
            pub dpcon: Option<i64>,
            /// See [`Self::dpio`].
            #[serde(default)]
            pub dpmcp: Option<i64>,
            /// See [`Self::dpio`].
            #[serde(default)]
            pub queues: Option<i64>,
            /// See [`Self::dpio`].
            #[serde(default)]
            pub workers: Option<i64>,
        }
    };
}

/// The whole intent document: the mandatory `[intent]` table plus the construct
/// arrays. `intent` is optional here only so [`crate::parse`] can emit the precise
/// "no `[intent]` table" message rather than serde's generic missing-field one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawIntent {
    /// The document-level `[intent]` table (schema version).
    pub intent: Option<RawIntentTable>,
    /// The `[[tenant]]` array.
    #[serde(default)]
    pub tenant: Vec<RawTenant>,
    /// The `[[port]]` array.
    #[serde(default)]
    pub port: Vec<RawPort>,
    /// The `[[link]]` array.
    #[serde(default)]
    pub link: Vec<RawLink>,
    /// The `[[fabric]]` array.
    #[serde(default)]
    pub fabric: Vec<RawFabric>,
    /// The `[[crypto]]` array (ordered — declaration order numbers each dpseci).
    #[serde(default)]
    pub crypto: Vec<RawCrypto>,
    /// The `[[extra]]` array (the additive raise-only override channel).
    #[serde(default)]
    pub extra: Vec<RawExtra>,
}

/// The `[intent]` table: the document-level properties anchor (design D1).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawIntentTable {
    /// The mandatory schema version, the `apiVersion` idiom. `Option` only so the
    /// parser can name the accepted versions when it is absent.
    pub schema: Option<i64>,
}

/// Where a tenant's dataplane runs, as written in TOML (design D1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RawDataplane {
    /// The kernel's own driver, over netlink.
    KernelNetlink,
    /// A userspace poll-mode process (VPP, DPDK).
    UserspacePoll,
    /// A userspace event-driven process (declared, refused until priced).
    UserspaceEvent,
}

/// How a tenant sits in the container tree, as written in TOML (design D6a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RawIsolation {
    /// A holder that accepts legal drawers into its own dprc.
    Public,
    /// Community co-residency inside a `pool` holder's dprc.
    Restricted,
    /// Its own child dprc, MC-isolated from siblings — the default.
    #[default]
    Isolated,
}

construct_table! {
    /// A `[[tenant]]` table (design D1).
    RawTenant {
        /// The tenant's name (the reserved `kernel` is refused here — design D1).
        #[serde(deserialize_with = "name")]
        pub name: TenantName,
        /// Where its dataplane runs, and the sizing regime it selects.
        pub dataplane: RawDataplane,
        /// The core budget the derived thread count must fit under.
        pub max_cores: i64,
        /// Its place in the container tree (default [`RawIsolation::Isolated`]).
        #[serde(default)]
        pub isolation: RawIsolation,
        /// A restricted tenant's public holder; absent otherwise.
        #[serde(default, deserialize_with = "name_opt")]
        pub pool: Option<TenantName>,
    }
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

construct_table! {
    /// A `[[port]]` table (design D1).
    RawPort {
        /// The stable DPMAC anchor, e.g. `"dpmac.7"`. Fallible, so it stays `String`:
        /// [`crate::parse`] must name the offending port when it is malformed.
        pub dpmac: String,
        /// The stable interface name to assign.
        #[serde(deserialize_with = "name")]
        pub name: ConstructName,
        /// The rate the port must deliver, in Mbps.
        pub rate: i64,
        /// The owning tenant; absent ⇒ the reserved `kernel` terminates the port.
        #[serde(default, deserialize_with = "name_opt")]
        pub tenant: Option<TenantName>,
        /// The port's known/declared MAC, e.g. `"02:00:00:00:00:07"`. Fallible, so it
        /// stays `String`: [`crate::parse`] must name the offending port when it is
        /// malformed.
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
}

construct_table! {
    /// A `[[link]]` table: a dpni↔dpni pseudo-wire between two tenant ends (design D1).
    RawLink {
        /// The link's name.
        #[serde(deserialize_with = "name")]
        pub name: ConstructName,
        /// The tenant whose interface terminates one end.
        #[serde(deserialize_with = "name")]
        pub interface_a: TenantName,
        /// The tenant whose interface terminates the other end.
        #[serde(deserialize_with = "name")]
        pub interface_b: TenantName,
    }
}

/// Who forwards between a fabric's members, as written in TOML (design D1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RawSwitching {
    /// A dpsw, which only the kernel can drive.
    Hardware,
    /// The forwarding tenant bridges its own dpnis; the MC never sees it.
    Software,
}

construct_table! {
    /// A `[[fabric]]` table: one switched domain over its members (design D1).
    RawFabric {
        /// The fabric's name (and the dpsw provenance key).
        #[serde(deserialize_with = "name")]
        pub name: ConstructName,
        /// Hardware (a dpsw) or software (own bridging).
        pub switching: RawSwitching,
        /// The tenant that runs the forwarding plane.
        #[serde(deserialize_with = "name")]
        pub forwarded_by: TenantName,
        /// The members, in declaration order — each names a port, tenant, or fabric.
        #[serde(default, deserialize_with = "name_vec")]
        pub members: Vec<ConstructName>,
    }
}

construct_table! {
    /// A `[[crypto]]` table: one accelerator for one tenant, sized by its own flows.
    RawCrypto {
        /// The owning tenant.
        #[serde(deserialize_with = "name")]
        pub tenant: TenantName,
        /// The flow demand this block sizes its own dpseci to.
        pub flows: i64,
    }
}

/// An `[[extra]]` table: the additive raise-only override channel (design D5).
///
/// Not a [`construct_table`]: this *is* the sanctioned count channel, so it carries a
/// legitimate `count` rather than the rejected derived-count keys the others do.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExtra {
    /// The tenant the extra raises a count for.
    #[serde(deserialize_with = "name")]
    pub tenant: TenantName,
    /// The companion family raised, lowercase (e.g. `"dpio"`). Fallible, so it stays
    /// `String`: [`crate::parse`] must name the offending family when it is unknown.
    pub family: String,
    /// The count added on top of the derived request.
    pub count: i64,
}
