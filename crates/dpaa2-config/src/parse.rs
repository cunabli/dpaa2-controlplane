//! TOML parsing, validation, and conversion into the neutral [`Intent`].
//!
//! Parses `topology.toml`, validates it, and converts it into the backend-neutral
//! [`dpaa2_api::Intent`] — the vocabulary an operator states, never a count (design
//! D1/D10). The frontend validates *intent* only; turning intent plus an inventory
//! into the object plan is [`dpaa2_api::compile`]'s, not the frontend's. Ports are
//! keyed by stable DPMAC anchors and a DPNI index is refused, its identity being
//! derived from the DPMAC edge.
//!
//! Validation runs before conversion and each failure is a named, actionable
//! [`Error::Config`]. The reserved `kernel` tenant (design D6a) resolves as a tenant
//! reference — a port owner, a link end, a fabric forwarder — without being declared,
//! and declaring it as a `[[tenant]]` is refused.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use dpaa2_api::{
    ALL_FAMILIES, ConfigSource, ConstructName, Crypto, Dataplane, DpmacId, Error, Extra, Fabric,
    Family, Intent, Isolation, KERNEL, Link, MacAddr, MacMode, Member, Port, Switching, Tenant,
    TenantName,
};

use crate::schema::{
    RawCrypto, RawDataplane, RawExtra, RawFabric, RawIntent, RawIsolation, RawLink, RawMacMode,
    RawPort, RawSwitching, RawTenant,
};

/// The one schema version this build accepts (design D1: the `apiVersion` hook).
const ACCEPTED_SCHEMA: i64 = 1;

/// A [`ConfigSource`] backed by a `topology.toml` file on disk.
pub struct TomlConfig {
    path: PathBuf,
}

impl TomlConfig {
    /// Points at a TOML file. The file is not read until [`ConfigSource::load`].
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl ConfigSource for TomlConfig {
    fn load(&self) -> Result<Intent, Error> {
        let text = std::fs::read_to_string(&self.path)?;
        parse_str(&text)
    }
}

/// Shorthand for a validation failure.
fn cfg(msg: impl Into<String>) -> Error {
    Error::Config(msg.into())
}

/// Expands to the `(field-name, value)` slice of the six rejected count keys every
/// construct table carries (schema.rs `construct_table!`), for [`reject_counts`]. The
/// key list lives once here rather than being repeated at each construct's call site.
macro_rules! counts_of {
    ($c:expr) => {
        &[
            ("dpio", $c.dpio),
            ("dpbp", $c.dpbp),
            ("dpcon", $c.dpcon),
            ("dpmcp", $c.dpmcp),
            ("queues", $c.queues),
            ("workers", $c.workers),
        ]
    };
}

/// Deserializes the document into the raw schema (design D10: serde stays here).
fn deserialize(text: &str) -> Result<RawIntent, Error> {
    toml::from_str(text).map_err(|e| cfg(e.message().to_owned()))
}

/// Checks the mandatory `[intent]` table and its `schema` key (topology-config spec:
/// missing table, missing key, or an unknown version all name the accepted versions).
fn require_schema(raw: &RawIntent) -> Result<(), Error> {
    let table = raw.intent.as_ref().ok_or_else(|| {
        cfg(format!(
            "the file has no `[intent]` table; this build accepts schema versions: {ACCEPTED_SCHEMA}"
        ))
    })?;
    let schema = table.schema.ok_or_else(|| {
        cfg(format!(
            "the `[intent]` table has no `schema` key; this build accepts schema versions: {ACCEPTED_SCHEMA}"
        ))
    })?;
    if schema != ACCEPTED_SCHEMA {
        return Err(cfg(format!(
            "unknown schema version {schema}; this build accepts schema versions: {ACCEPTED_SCHEMA}"
        )));
    }
    Ok(())
}

/// Parses and validates TOML text into the neutral [`Intent`].
///
/// # Errors
/// Returns [`Error::Config`] on malformed TOML, a missing or unknown `[intent]`
/// schema, DPNI-index pinning, a derived count field, malformed DPMAC or MAC
/// references, duplicate names, an unresolved tenant/member reference, a link naming
/// one tenant twice, a `pool`/`restricted` contradiction, or an unknown extra family.
pub fn parse_str(text: &str) -> Result<Intent, Error> {
    let raw = deserialize(text)?;
    require_schema(&raw)?;
    convert(&raw)
}

/// Deserializes and structurally checks a document without resolving cross-construct
/// references — the layer the doc-example harness runs over every published TOML
/// fence, and the structural gate a future frontend can share (topology-config spec).
///
/// It verifies the document deserializes under the `deny_unknown_fields` schema and
/// carries a known `[intent]` schema version; it does *not* resolve tenant, member,
/// or pool references, which are [`parse_str`]'s.
///
/// # Errors
/// Returns [`Error::Config`] on malformed TOML, an unknown field, or a missing/unknown
/// `[intent]` schema version.
pub fn parse_schema(text: &str) -> Result<(), Error> {
    let raw = deserialize(text)?;
    require_schema(&raw)
}

/// Validates the raw document and converts it into the neutral [`Intent`], preserving
/// declaration order (design D6); the extras fold into the plan's [`std::collections::BTreeSet`].
///
/// Every semantic name crosses into a [`TenantName`] or [`ConstructName`] here, at the
/// deserialization boundary, so no bare `String` name flows through the validation or
/// the built [`Intent`] (types.rs: names cannot be confused among one another).
fn convert(raw: &RawIntent) -> Result<Intent, Error> {
    // Collected declaration namespaces, needed before members and references resolve.
    let mut tenant_names: HashSet<TenantName> = HashSet::new();
    for t in &raw.tenant {
        if t.name.is_kernel() {
            return Err(cfg(format!(
                "`[[tenant]]` names `{KERNEL}`, which is reserved for the root dataplane and \
                 never declared"
            )));
        }
        if !tenant_names.insert(t.name.clone()) {
            return Err(cfg(format!("duplicate tenant name `{}`", t.name)));
        }
    }
    let port_names: HashSet<ConstructName> = raw.port.iter().map(|p| p.name.clone()).collect();
    let fabric_names: HashSet<ConstructName> = raw.fabric.iter().map(|f| f.name.clone()).collect();

    let tenants = raw
        .tenant
        .iter()
        .map(convert_tenant)
        .collect::<Result<Vec<_>, _>>()?;

    // Interface names are unique, and every construct name (port, link, fabric) is
    // unique so a fabric member resolves to exactly one construct.
    let mut ifaces: HashSet<ConstructName> = HashSet::new();
    let mut constructs: HashSet<ConstructName> = HashSet::new();
    let ports = raw
        .port
        .iter()
        .map(|p| {
            if !ifaces.insert(p.name.clone()) {
                return Err(cfg(format!("duplicate interface name `{}`", p.name)));
            }
            constructs.insert(p.name.clone());
            convert_port(p, &tenant_names)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let links = raw
        .link
        .iter()
        .map(|l| {
            if !constructs.insert(l.name.clone()) {
                return Err(cfg(format!("duplicate construct name `{}`", l.name)));
            }
            convert_link(l, &tenant_names)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let fabrics = raw
        .fabric
        .iter()
        .map(|f| {
            if !constructs.insert(f.name.clone()) {
                return Err(cfg(format!("duplicate construct name `{}`", f.name)));
            }
            convert_fabric(f, &tenant_names, &port_names, &fabric_names)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let crypto = raw
        .crypto
        .iter()
        .map(|k| convert_crypto(k, &tenant_names))
        .collect::<Result<Vec<_>, _>>()?;

    let extras = raw
        .extra
        .iter()
        .map(|e| convert_extra(e, &tenant_names))
        .collect::<Result<_, _>>()?;

    Ok(Intent {
        tenants,
        ports,
        links,
        fabrics,
        crypto,
        extras,
    })
}

/// Whether `name` resolves as a tenant reference: a declared tenant, or the reserved
/// `kernel`, which is never declared yet is nameable as a port owner, a link end, or a
/// fabric forwarder (design D6a; `models/intent/scenarios/*.toml`).
fn resolves(name: &TenantName, tenants: &HashSet<TenantName>) -> bool {
    name.is_kernel() || tenants.contains(name)
}

fn convert_tenant(t: &RawTenant) -> Result<Tenant, Error> {
    let name = t.name.clone();
    reject_counts(&format!("tenant `{name}`"), counts_of!(t))?;

    let isolation = match t.isolation {
        RawIsolation::Public => Isolation::Public,
        RawIsolation::Restricted => Isolation::Restricted,
        RawIsolation::Isolated => Isolation::Isolated,
    };
    let pool = t.pool.clone().unwrap_or_else(|| "".into());
    if isolation == Isolation::Restricted && pool.is_empty() {
        return Err(cfg(format!(
            "tenant `{name}` is `restricted` but names no `pool` holder"
        )));
    }
    if isolation != Isolation::Restricted && !pool.is_empty() {
        return Err(cfg(format!(
            "tenant `{name}` names a `pool` (`{pool}`) but is not `restricted`; a pool is legal \
             only on a restricted tenant"
        )));
    }
    let dataplane = match t.dataplane {
        RawDataplane::KernelNetlink => Dataplane::KernelNetlink,
        RawDataplane::UserspacePoll => Dataplane::UserspacePoll,
        RawDataplane::UserspaceEvent => Dataplane::UserspaceEvent,
    };
    Ok(Tenant {
        name,
        dataplane,
        max_cores: t.max_cores,
        isolation,
        pool,
    })
}

/// Rejects any derived count field named on a construct with a targeted message
/// (topology-config spec: "A count field is rejected" — every construct table, not just
/// the tenant). `subject` names the offending construct (e.g. ``tenant `router` ``);
/// each `field` is a literal key name, not a semantic name slot, so it stays a `&str`.
fn reject_counts(subject: &str, counts: &[(&str, Option<i64>)]) -> Result<(), Error> {
    for (field, value) in counts {
        if value.is_some() {
            return Err(cfg(format!(
                "{subject} sets `{field}`, but that count is derived from the intent, not declared"
            )));
        }
    }
    Ok(())
}

fn convert_port(p: &RawPort, tenants: &HashSet<TenantName>) -> Result<Port, Error> {
    if let Some(dpni) = &p.dpni {
        return Err(cfg(format!(
            "port `{}` pins a DPNI index (`dpni = \"{dpni}\"`); DPNI identity is derived from the \
             DPMAC edge and must not be set",
            p.name
        )));
    }
    let name = ConstructName::from(validate_name(p.name.as_str())?);
    reject_counts(&format!("port `{name}`"), counts_of!(p))?;
    let dpmac =
        parse_dpmac(&p.dpmac).ok_or_else(|| cfg(format!("port `{name}` has malformed `dpmac`")))?;
    let mac = match &p.mac {
        Some(s) => Some(
            s.parse::<MacAddr>()
                .map_err(|_| cfg(format!("port `{name}` has malformed MAC `{s}`")))?,
        ),
        None => None,
    };
    let mac_mode = match p.mac_mode {
        RawMacMode::Assert => MacMode::Assert,
        RawMacMode::Actuate => MacMode::Actuate,
    };
    // A port with no tenant belongs to the reserved kernel (topology-config spec).
    let tenant = p.tenant.clone().unwrap_or_else(|| TenantName::from(KERNEL));
    if !resolves(&tenant, tenants) {
        return Err(cfg(format!(
            "port `{name}` names tenant `{tenant}`, which is not declared"
        )));
    }
    Ok(Port {
        name,
        dpmac,
        rate: p.rate,
        tenant,
        mac,
        mac_mode,
    })
}

fn convert_link(l: &RawLink, tenants: &HashSet<TenantName>) -> Result<Link, Error> {
    let name = l.name.clone();
    reject_counts(&format!("link `{name}`"), counts_of!(l))?;
    let interface_a = l.interface_a.clone();
    let interface_b = l.interface_b.clone();
    for end in [&interface_a, &interface_b] {
        if !resolves(end, tenants) {
            return Err(cfg(format!(
                "link `{name}` names tenant `{end}`, which is not declared"
            )));
        }
    }
    if interface_a == interface_b {
        return Err(cfg(format!(
            "link `{name}` names the same tenant `{interface_a}` at both ends; a link joins two \
             distinct tenants"
        )));
    }
    Ok(Link {
        name,
        interface_a,
        interface_b,
    })
}

fn convert_fabric(
    f: &RawFabric,
    tenants: &HashSet<TenantName>,
    ports: &HashSet<ConstructName>,
    fabrics: &HashSet<ConstructName>,
) -> Result<Fabric, Error> {
    let name = f.name.clone();
    reject_counts(&format!("fabric `{name}`"), counts_of!(f))?;
    let forwarded_by = f.forwarded_by.clone();
    if !resolves(&forwarded_by, tenants) {
        return Err(cfg(format!(
            "fabric `{name}` is forwarded by tenant `{forwarded_by}`, which is not declared"
        )));
    }
    let switching = match f.switching {
        RawSwitching::Hardware => Switching::Hardware,
        RawSwitching::Software => Switching::Software,
    };
    let members = f
        .members
        .iter()
        .map(|m| classify_member(&name, m, tenants, ports, fabrics))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Fabric {
        name,
        switching,
        forwarded_by,
        members,
    })
}

/// Resolves a fabric member name to a declared port, tenant, or fabric (design D1;
/// the [`Member`] enum). A name matching none is refused. Ports are checked first, so
/// a member is a port where one exists.
fn classify_member(
    fabric: &ConstructName,
    raw_member: &ConstructName,
    tenants: &HashSet<TenantName>,
    ports: &HashSet<ConstructName>,
    fabrics: &HashSet<ConstructName>,
) -> Result<Member, Error> {
    let as_construct = raw_member.clone();
    let as_tenant = TenantName::from(raw_member.as_str());
    if ports.contains(&as_construct) {
        Ok(Member::Port(as_construct))
    } else if resolves(&as_tenant, tenants) {
        Ok(Member::Tenant(as_tenant))
    } else if fabrics.contains(&as_construct) {
        Ok(Member::Fabric(as_construct))
    } else {
        Err(cfg(format!(
            "fabric `{fabric}` names member `{raw_member}`, which is not a declared port, tenant, \
             or fabric"
        )))
    }
}

fn convert_crypto(k: &RawCrypto, tenants: &HashSet<TenantName>) -> Result<Crypto, Error> {
    let tenant = k.tenant.clone();
    reject_counts("`[[crypto]]`", counts_of!(k))?;
    if !resolves(&tenant, tenants) {
        return Err(cfg(format!(
            "`[[crypto]]` names tenant `{tenant}`, which is not declared"
        )));
    }
    Ok(Crypto {
        tenant,
        flows: k.flows,
    })
}

fn convert_extra(e: &RawExtra, tenants: &HashSet<TenantName>) -> Result<Extra, Error> {
    let tenant = e.tenant.clone();
    if !resolves(&tenant, tenants) {
        return Err(cfg(format!(
            "`[[extra]]` names tenant `{tenant}`, which is not declared"
        )));
    }
    let family = parse_family(&e.family)?;
    Ok(Extra {
        tenant,
        family,
        count: e.count,
    })
}

/// Parses a lowercase restool family name (`"dpio"`, …) to a [`Family`]. `compile`
/// refuses a non-companion family or a count below 1 (`ExtraNotCompanion`,
/// `ExtraNotPositive`); the config only parses the name.
fn parse_family(name: &str) -> Result<Family, Error> {
    ALL_FAMILIES
        .iter()
        .copied()
        .find(|f| f.as_str() == name)
        .ok_or_else(|| cfg(format!("`[[extra]]` names unknown family `{name}`")))
}

/// Validates a raw port `name` against the constraints it must satisfy to become both
/// a filename component and `.link` file content: 1-15 bytes (the Linux `IFNAMSIZ`
/// limit) of ASCII alphanumeric, `-`, or `_` only, and returns it for the caller to
/// wrap in a [`ConstructName`]. Without this, a name containing `/`, `..`, or a
/// newline would flow unvalidated into a path or config file rather than being
/// rejected here; the newtype boundary types the name but does not constrain its
/// content, so this content check stays.
fn validate_name(name: &str) -> Result<&str, Error> {
    if name.is_empty() || name.len() > 15 {
        return Err(cfg(format!(
            "port `{name}` has an invalid `name`: must be 1-15 bytes (IFNAMSIZ limit), got {} bytes",
            name.len()
        )));
    }
    if let Some(c) = name
        .chars()
        .find(|&c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    {
        return Err(cfg(format!(
            "port `{name}` has an invalid `name`: character `{c}` is not allowed (only ASCII alphanumeric, `-`, `_`)"
        )));
    }
    Ok(name)
}

/// Parses a `dpmac.N` reference into a [`DpmacId`].
fn parse_dpmac(s: &str) -> Option<DpmacId> {
    let n = s.strip_prefix("dpmac.")?;
    n.parse::<u32>().ok().map(DpmacId::new)
}

/// Convenience: load and validate a topology file at `path`.
///
/// # Errors
/// See [`parse_str`]; also returns [`Error::Io`] if the file cannot be read.
pub fn load(path: impl AsRef<Path>) -> Result<Intent, Error> {
    TomlConfig::new(path.as_ref().to_path_buf()).load()
}

#[cfg(test)]
mod tests {
    //! Parsing, validation, and conversion tests, one per spec scenario plus the
    //! per-field rejections (topology-config spec).

    use super::{parse_schema, parse_str};
    use dpaa2_api::{Dataplane, DpmacId, Family, Isolation, MacAddr, MacMode, Member, Switching};

    /// The mandatory `[intent]` header, prepended to the construct-only fixtures.
    const HEADER: &str = "[intent]\nschema = 1\n";

    fn parse(body: &str) -> dpaa2_api::Intent {
        parse_str(&format!("{HEADER}{body}")).expect("intent parses")
    }
    fn parse_err(body: &str) -> String {
        parse_str(&format!("{HEADER}{body}"))
            .unwrap_err()
            .to_string()
    }

    // ---- Requirement: keyed by DPMAC anchors ----

    #[test]
    fn scenario_port_defined_by_dpmac() {
        let intent = parse(
            r#"
            [[tenant]]
            name = "router"
            dataplane = "userspace-poll"
            max_cores = 16

            [[port]]
            dpmac = "dpmac.7"
            name = "wan0"
            rate = 10000
            tenant = "router"
            "#,
        );
        assert_eq!(intent.ports.len(), 1);
        assert_eq!(intent.ports[0].dpmac, DpmacId::new(7));
        assert_eq!(intent.ports[0].tenant.as_str(), "router");
    }

    #[test]
    fn scenario_dpni_index_is_rejected() {
        let err = parse_err(
            r#"
            [[port]]
            dpmac = "dpmac.3"
            name = "wan0"
            rate = 10000
            dpni = "dpni.3"
            "#,
        );
        assert!(err.contains("dpni"), "names the offending key: {err}");
        assert!(err.contains("DPMAC edge"), "explains why: {err}");
    }

    #[test]
    fn scenario_count_field_is_rejected() {
        let err = parse_err(
            r#"
            [[tenant]]
            name = "router"
            dataplane = "userspace-poll"
            max_cores = 16
            dpio = 10
            "#,
        );
        assert!(err.contains("dpio"), "names the field: {err}");
        assert!(
            err.contains("derived"),
            "states the count is derived: {err}"
        );

        let workers = parse_err(
            r#"
            [[tenant]]
            name = "router"
            dataplane = "userspace-poll"
            max_cores = 16
            workers = 4
            "#,
        );
        assert!(workers.contains("workers"), "names the field: {workers}");
    }

    #[test]
    fn scenario_count_field_on_a_port_is_rejected() {
        // The rejection is not the tenant's alone: any construct naming a derived count
        // is refused with the same "derived" wording (topology-config spec).
        let err = parse_err(
            r#"
            [[port]]
            dpmac = "dpmac.7"
            name = "wan0"
            rate = 10000
            workers = 4
            "#,
        );
        assert!(err.contains("workers"), "names the field: {err}");
        assert!(
            err.contains("derived"),
            "states the count is derived: {err}"
        );
    }

    #[test]
    fn scenario_port_without_tenant_belongs_to_kernel() {
        let intent = parse(
            r#"
            [[port]]
            dpmac = "dpmac.7"
            name = "mgmt"
            rate = 10000
            "#,
        );
        assert!(intent.ports[0].tenant.is_kernel());
    }

    #[test]
    fn scenario_missing_or_unknown_schema_version() {
        // No [intent] table.
        let no_table = parse_str(
            r#"
            [[port]]
            dpmac = "dpmac.7"
            name = "wan0"
            rate = 10000
            "#,
        )
        .unwrap_err()
        .to_string();
        assert!(no_table.contains("[intent]"), "{no_table}");
        assert!(
            no_table.contains('1'),
            "names accepted versions: {no_table}"
        );

        // Table without a schema key.
        let no_key = parse_str("[intent]\n").unwrap_err().to_string();
        assert!(no_key.contains("schema"), "{no_key}");

        // Unknown version.
        let unknown = parse_str("[intent]\nschema = 2\n").unwrap_err().to_string();
        assert!(unknown.contains('1'), "names accepted versions: {unknown}");
    }

    // ---- Requirement: config parses into the neutral model ----

    #[test]
    fn toml_converts_to_neutral_intent_preserving_order() {
        let intent = parse(
            r#"
            [[tenant]]
            name = "router"
            dataplane = "userspace-poll"
            max_cores = 16

            [[port]]
            dpmac = "dpmac.9"
            name = "wan0"
            rate = 10000
            tenant = "router"

            [[port]]
            dpmac = "dpmac.10"
            name = "wan1"
            rate = 10000
            tenant = "router"

            [[crypto]]
            tenant = "router"
            flows = 4

            [[crypto]]
            tenant = "router"
            flows = 8

            [[extra]]
            tenant = "router"
            family = "dpio"
            count = 2
            "#,
        );
        assert_eq!(intent.tenants[0].dataplane, Dataplane::UserspacePoll);
        assert_eq!(intent.tenants[0].isolation, Isolation::Isolated, "default");
        // Ports keep declaration order.
        assert_eq!(intent.ports[0].name.as_str(), "wan0");
        assert_eq!(intent.ports[1].name.as_str(), "wan1");
        // Crypto blocks keep declaration order (ordinal source).
        assert_eq!(intent.crypto[0].flows, 4);
        assert_eq!(intent.crypto[1].flows, 8);
        // Extras fold into the BTreeSet, parsed to a Family.
        let extra = intent.extras.iter().next().expect("one extra");
        assert_eq!(extra.family, Family::Dpio);
        assert_eq!(extra.count, 2);
    }

    #[test]
    fn mac_and_mode_survive_conversion() {
        let intent = parse(
            r#"
            [[port]]
            dpmac = "dpmac.7"
            name = "wan0"
            rate = 10000
            mac = "02-00-00-00-00-07"
            mac_mode = "actuate"
            "#,
        );
        assert_eq!(
            intent.ports[0].mac,
            Some(MacAddr::new([0x02, 0, 0, 0, 0, 0x07]))
        );
        assert_eq!(intent.ports[0].mac_mode, MacMode::Actuate);
    }

    // ---- link, fabric, isolation/pool ----

    #[test]
    fn link_names_two_tenants_and_the_kernel_end_needs_no_declaration() {
        let intent = parse(
            r#"
            [[tenant]]
            name = "ns1"
            dataplane = "kernel-netlink"
            max_cores = 2

            [[link]]
            name = "uplink"
            interface_a = "ns1"
            interface_b = "kernel"
            "#,
        );
        assert_eq!(intent.links[0].interface_a.as_str(), "ns1");
        assert!(intent.links[0].interface_b.is_kernel());
    }

    #[test]
    fn link_with_identical_ends_is_rejected() {
        let err = parse_err(
            r#"
            [[tenant]]
            name = "ns1"
            dataplane = "kernel-netlink"
            max_cores = 2

            [[link]]
            name = "loop"
            interface_a = "ns1"
            interface_b = "ns1"
            "#,
        );
        assert!(err.contains("both ends"), "{err}");
    }

    #[test]
    fn fabric_members_resolve_as_ports_tenants_or_fabrics() {
        let intent = parse(
            r#"
            [[tenant]]
            name = "router"
            dataplane = "userspace-poll"
            max_cores = 16

            [[port]]
            dpmac = "dpmac.7"
            name = "lan0"
            rate = 10000

            [[fabric]]
            name = "lan"
            switching = "hardware"
            forwarded_by = "kernel"
            members = ["lan0", "router"]
            "#,
        );
        assert_eq!(intent.fabrics[0].switching, Switching::Hardware);
        assert!(intent.fabrics[0].forwarded_by.is_kernel());
        assert_eq!(intent.fabrics[0].members[0], Member::Port("lan0".into()));
        assert_eq!(
            intent.fabrics[0].members[1],
            Member::Tenant("router".into())
        );
    }

    #[test]
    fn unresolved_fabric_member_is_rejected() {
        let err = parse_err(
            r#"
            [[fabric]]
            name = "lan"
            switching = "hardware"
            forwarded_by = "kernel"
            members = ["ghost"]
            "#,
        );
        assert!(err.contains("ghost"), "names the member: {err}");
    }

    #[test]
    fn restricted_tenant_needs_a_pool_and_a_pool_needs_restricted() {
        let no_pool = parse_err(
            r#"
            [[tenant]]
            name = "sec"
            dataplane = "userspace-poll"
            max_cores = 16
            isolation = "restricted"
            "#,
        );
        assert!(no_pool.contains("restricted"), "{no_pool}");

        let stray_pool = parse_err(
            r#"
            [[tenant]]
            name = "sec"
            dataplane = "userspace-poll"
            max_cores = 16
            pool = "prim"
            "#,
        );
        assert!(stray_pool.contains("pool"), "{stray_pool}");
    }

    #[test]
    fn restricted_tenant_with_pool_converts() {
        let intent = parse(
            r#"
            [[tenant]]
            name = "prim"
            dataplane = "userspace-poll"
            max_cores = 16
            isolation = "public"

            [[tenant]]
            name = "sec"
            dataplane = "userspace-poll"
            max_cores = 16
            isolation = "restricted"
            pool = "prim"
            "#,
        );
        assert_eq!(intent.tenants[1].isolation, Isolation::Restricted);
        assert_eq!(intent.tenants[1].pool.as_str(), "prim");
    }

    // ---- Requirement: validated before use ----

    #[test]
    fn scenario_duplicate_interface_name() {
        let err = parse_err(
            r#"
            [[port]]
            dpmac = "dpmac.7"
            name = "wan0"
            rate = 10000

            [[port]]
            dpmac = "dpmac.8"
            name = "wan0"
            rate = 10000
            "#,
        );
        assert!(err.contains("duplicate interface name"), "{err}");
    }

    #[test]
    fn scenario_malformed_mac() {
        let err = parse_err(
            r#"
            [[port]]
            dpmac = "dpmac.7"
            name = "wan0"
            rate = 10000
            mac = "not-a-mac"
            "#,
        );
        assert!(err.contains("wan0"), "identifies the port: {err}");
        assert!(err.contains("malformed MAC"), "{err}");
    }

    #[test]
    fn scenario_unknown_tenant_reference() {
        let err = parse_err(
            r#"
            [[port]]
            dpmac = "dpmac.7"
            name = "wan0"
            rate = 10000
            tenant = "router"
            "#,
        );
        assert!(err.contains("wan0"), "names the port: {err}");
        assert!(err.contains("router"), "names the missing tenant: {err}");
    }

    #[test]
    fn scenario_reserved_name_declared() {
        let err = parse_err(
            r#"
            [[tenant]]
            name = "kernel"
            dataplane = "kernel-netlink"
            max_cores = 16
            "#,
        );
        assert!(err.contains("kernel"), "{err}");
        assert!(err.contains("reserved"), "{err}");
    }

    #[test]
    fn duplicate_tenant_name_is_rejected() {
        let err = parse_err(
            r#"
            [[tenant]]
            name = "router"
            dataplane = "userspace-poll"
            max_cores = 16

            [[tenant]]
            name = "router"
            dataplane = "kernel-netlink"
            max_cores = 16
            "#,
        );
        assert!(err.contains("duplicate tenant name"), "{err}");
    }

    #[test]
    fn unknown_extra_family_is_rejected() {
        let err = parse_err(
            r#"
            [[tenant]]
            name = "router"
            dataplane = "userspace-poll"
            max_cores = 16

            [[extra]]
            tenant = "router"
            family = "dpwidget"
            count = 1
            "#,
        );
        assert!(err.contains("dpwidget"), "names the family: {err}");
    }

    #[test]
    fn unknown_field_is_rejected() {
        let err = parse_err(
            r#"
            [[port]]
            dpmac = "dpmac.7"
            name = "wan0"
            rate = 10000
            speed = "fast"
            "#,
        );
        assert!(err.contains("speed") || err.contains("unknown"), "{err}");
    }

    #[test]
    fn malformed_dpmac_is_rejected() {
        let err = parse_err(
            r#"
            [[port]]
            dpmac = "eth3"
            name = "wan0"
            rate = 10000
            "#,
        );
        assert!(err.contains("malformed `dpmac`"), "{err}");
    }

    // ---- port-only and empty documents ----

    #[test]
    fn kernel_port_only_document_parses() {
        let intent = parse(
            r#"
            [[port]]
            dpmac = "dpmac.7"
            name = "lan0"
            rate = 10000

            [[port]]
            dpmac = "dpmac.8"
            name = "lan1"
            rate = 10000
            "#,
        );
        assert_eq!(intent.ports.len(), 2);
        assert!(intent.tenants.is_empty());
        assert!(intent.ports.iter().all(|p| p.tenant.is_kernel()));
    }

    #[test]
    fn header_only_document_is_valid_and_empty() {
        let intent = parse_str(HEADER).expect("just the header parses");
        assert_eq!(intent, dpaa2_api::Intent::default());
    }

    #[test]
    fn parse_schema_accepts_a_fragment_without_resolving_references() {
        // A lone crypto fragment names an undeclared tenant; the structural gate
        // deserializes it, while full parse_str would reject the dangling reference.
        let fragment = "[[crypto]]\ntenant = \"router\"\nflows = 2\n";
        parse_schema(&format!("{HEADER}{fragment}")).expect("structural parse");
        assert!(parse_str(&format!("{HEADER}{fragment}")).is_err());
    }

    #[test]
    fn load_reads_and_parses_a_file_from_disk() {
        // Every other test drives parse_str; this one exercises the on-disk path
        // `dpaa2_config::load` takes (read the file, then parse). std only — a unique
        // name under the temp dir, removed on the way out.
        use std::io::Write;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dpaa2-config-load-{}-{nanos}.toml",
            std::process::id()
        ));
        let body = format!(
            "{HEADER}\
             [[port]]\n\
             dpmac = \"dpmac.7\"\n\
             name = \"lan0\"\n\
             rate = 10000\n"
        );
        std::fs::File::create(&path)
            .and_then(|mut f| f.write_all(body.as_bytes()))
            .expect("write temp topology");

        let loaded = super::load(&path);
        std::fs::remove_file(&path).expect("remove temp topology");

        let intent = loaded.expect("file loads and parses");
        assert_eq!(intent.ports.len(), 1);
        assert_eq!(intent.ports[0].name.as_str(), "lan0");
        assert!(intent.ports[0].tenant.is_kernel());
    }

    #[test]
    fn shipped_example_topology_is_valid() {
        // The example installed to /etc/dpaa2/topology.toml must always parse.
        let example = include_str!("../../../packaging/dpaa2/topology.toml");
        let intent = parse_str(example).expect("shipped example topology parses");
        assert!(!intent.ports.is_empty());
    }
}
