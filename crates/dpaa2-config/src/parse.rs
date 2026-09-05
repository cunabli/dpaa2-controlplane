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
//! and declaring it as a `[tenant.kernel]` table is refused.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use dpaa2_api::{
    ALL_FAMILIES, ConfigSource, ConstructName, Crypto, Dataplane, DpmacId, Error, Extra, Fabric,
    Family, Intent, Isolation, KERNEL, Link, MacAddr, MacMode, Member, Port, Switching, Tenant,
    TenantName,
};

use crate::schema::{
    RawCrypto, RawDataplane, RawFabric, RawIntent, RawIsolation, RawLink, RawMacMode, RawPort,
    RawSwitching, RawTenant,
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
/// schema, DPNI-index pinning, a derived count field, a declared name that is not a
/// valid interface name (ADR-0015 decision 13), malformed DPMAC or MAC references,
/// duplicate names, an unresolved tenant/member reference, a link naming one tenant
/// twice, a `pool`/`restricted` contradiction, an unknown extra family, or a
/// `renamed = { from }` whose source is an invalid interface name or names a
/// currently-declared construct that is not itself renamed away (ADR-0015 decision 10).
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

/// Validates the raw document and converts it into the neutral [`Intent`]. Tenants,
/// ports, links, and fabrics take name (key) order — since task 3.3d keys ports and
/// links too (ADR-0015 decision 1), their order is canonical and cosmetic: the
/// derivation mints the name-ordered dpni ordinal, not the document position (decision
/// 5, the position-independence law). `crypto` keeps declaration order (its Nth block
/// numbers the Nth dpseci, decision 4); the extras fold into the plan's
/// [`std::collections::BTreeSet`].
///
/// Every semantic name crosses into a [`TenantName`] or [`ConstructName`] here, at the
/// deserialization boundary, so no bare `String` name flows through the validation or
/// the built [`Intent`] (types.rs: names cannot be confused among one another).
fn convert(raw: &RawIntent) -> Result<Intent, Error> {
    // ADR-0015 decision 13: a declared construct name serves verbatim as the netdev
    // name and its restool label, so it must be a valid Linux interface name and must
    // never match the reserved `family.N` handle pattern. Validate every DECLARED name
    // (the table keys) up front; references (a port's tenant, a link's ends, a fabric's
    // members) resolve against these declarations and are refused separately. The
    // reserved-`kernel` refusal below keeps precedence — `kernel` is a lexically valid
    // name, so the two checks never contend for the same document.
    let declared = raw
        .tenant
        .keys()
        .map(|n| ("tenant", n.as_str(), n.validate()))
        .chain(raw.port.keys().map(|n| ("port", n.as_str(), n.validate())))
        .chain(raw.link.keys().map(|n| ("link", n.as_str(), n.validate())))
        .chain(
            raw.fabric
                .keys()
                .map(|n| ("fabric", n.as_str(), n.validate())),
        );
    for (kind, name, result) in declared {
        result.map_err(|e| {
            cfg(format!(
                "`[{kind}.{name}]` is not a valid interface name: {e}"
            ))
        })?;
    }

    // Collected declaration namespaces, needed before members and references resolve.
    // The map keys are unique by construction (a duplicate name is a TOML key-redefinition
    // parse error), so only the reserved `kernel` refusal remains.
    let mut tenant_names: HashSet<TenantName> = HashSet::new();
    for name in raw.tenant.keys() {
        if name.is_kernel() {
            return Err(cfg(format!(
                "`[tenant.kernel]` declares the reserved tenant name `{KERNEL}`, which is \
                 reserved for the root dataplane and never declared"
            )));
        }
        tenant_names.insert(name.clone());
    }
    let port_names: HashSet<ConstructName> = raw.port.keys().cloned().collect();
    let fabric_names: HashSet<ConstructName> = raw.fabric.keys().cloned().collect();

    // ADR-0015 decision 10 rule (i), as refined (task 6.3): a `renamed = { from }`
    // clause declares the construct's prior name — a temporary widening of the
    // matcher's acceptance set. Validate every `from` value as an interface name
    // (the task-6.2 rule), then refuse the genuine contradiction: `from` naming a
    // construct that is currently declared AND not itself renamed away, so no object
    // is claimed twice. A `from`-target that itself carries a `renamed` clause is
    // admitted (the swap `wan0`↔`eth0` and the chain `a→b→c` declare every target
    // renamed away), a self-rename is inert, and the reserved `kernel` is never
    // declared so `from = "kernel"` passes. Namespaces are separate: a tenant's
    // `from` resolves against declared tenants, a port/link/fabric's against the
    // shared construct namespace. Validation stays up front here; each converter
    // then carries the accepted clause into the neutral `Intent` as `renamed`
    // (task 6.5), where the rename matcher consumes it.
    check_renames(raw)?;

    let tenants = raw
        .tenant
        .iter()
        .map(|(name, t)| convert_tenant(name, t))
        .collect::<Result<Vec<_>, _>>()?;

    // Ports, links, and fabrics share one construct namespace so a fabric member
    // resolves to exactly one construct. Each family is keyed now, so a name is unique
    // within its family (a duplicate is a TOML key redefinition); the only collision
    // this set still catches is CROSS-family — a port and a link, or a fabric, of one
    // name (task 3.3d, ADR-0015 decision 1) — which TOML's separate tables cannot dedupe.
    let mut constructs: HashSet<ConstructName> = HashSet::new();
    let ports = raw
        .port
        .iter()
        .map(|(name, p)| {
            // Ports are processed first; a port name colliding with a later link or
            // fabric is refused when that family inserts the shared name below.
            constructs.insert(name.clone());
            convert_port(name, p, &tenant_names)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let links = raw
        .link
        .iter()
        .map(|(name, l)| {
            if !constructs.insert(name.clone()) {
                return Err(cfg(format!("duplicate construct name `{name}`")));
            }
            convert_link(name, l, &tenant_names)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let fabrics = raw
        .fabric
        .iter()
        .map(|(name, f)| {
            // Fabric-vs-fabric duplicates are now format-enforced (the map keys are unique),
            // but `constructs` spans ports, links, and fabrics, so this still catches the
            // cross-type collision — a fabric named like a port or link — that TOML does not.
            if !constructs.insert(name.clone()) {
                return Err(cfg(format!("duplicate construct name `{name}`")));
            }
            convert_fabric(name, f, &tenant_names, &port_names, &fabric_names)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let crypto = raw
        .crypto
        .iter()
        .map(|k| convert_crypto(k, &tenant_names))
        .collect::<Result<Vec<_>, _>>()?;

    let names = &tenant_names;
    let extras = raw
        .extra
        .iter()
        .flat_map(|(tenant, families)| {
            families
                .iter()
                .map(move |(family, count)| convert_extra(tenant, family, *count, names))
        })
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

/// Whether `target` is a currently-declared construct (port, link, or fabric) that
/// carries no `renamed` clause of its own — the shape ADR-0015 decision 10 rule (i)
/// refuses (task 6.3). A target that is itself renamed away is admitted (the swap
/// and the chain), and a name in no construct family is not declared, so inert.
fn construct_is_declared_unrenamed(raw: &RawIntent, target: &ConstructName) -> bool {
    if let Some(p) = raw.port.get(target) {
        return p.renamed.is_none();
    }
    if let Some(l) = raw.link.get(target) {
        return l.renamed.is_none();
    }
    if let Some(f) = raw.fabric.get(target) {
        return f.renamed.is_none();
    }
    false
}

/// Validates every `renamed = { from }` clause and applies ADR-0015 decision 10 rule
/// (i) as refined (task 6.3). The `from` value SHALL be a valid interface name (the
/// task-6.2 rule); a `from` naming a currently-declared construct that is not itself
/// renamed away is refused, since the target would be claimed twice. Tenants resolve
/// against declared tenants, and ports/links/fabrics against their shared namespace;
/// a `from` that itself carries a `renamed` clause (the swap, the chain) or names an
/// undeclared construct (the plain rename) is admitted. This gate only validates and
/// refuses; the accepted clause is carried into the neutral `Intent` by each converter
/// (task 6.5), where the rename matcher consumes it.
fn check_renames(raw: &RawIntent) -> Result<(), Error> {
    for (name, t) in &raw.tenant {
        let Some(r) = &t.renamed else { continue };
        r.from.validate().map_err(|e| {
            cfg(format!(
                "`[tenant.{name}]` declares an invalid rename source: {e}"
            ))
        })?;
        // Tenant namespace: a `from` naming another declared tenant that carries no
        // clause of its own is the genuine contradiction (the target stays itself).
        if raw
            .tenant
            .get(&r.from)
            .is_some_and(|target| target.renamed.is_none())
        {
            return Err(cfg(format!(
                "`[tenant.{name}]` declares `renamed = {{ from = \"{from}\" }}` but `{from}` is \
                 currently declared and not itself renamed; a construct cannot be claimed twice",
                from = r.from
            )));
        }
    }

    let constructs = raw
        .port
        .iter()
        .map(|(n, p)| ("port", n, &p.renamed))
        .chain(raw.link.iter().map(|(n, l)| ("link", n, &l.renamed)))
        .chain(raw.fabric.iter().map(|(n, f)| ("fabric", n, &f.renamed)));
    for (kind, name, renamed) in constructs {
        let Some(r) = renamed else { continue };
        r.from.validate().map_err(|e| {
            cfg(format!(
                "`[{kind}.{name}]` declares an invalid rename source: {e}"
            ))
        })?;
        if construct_is_declared_unrenamed(raw, &r.from) {
            return Err(cfg(format!(
                "`[{kind}.{name}]` declares `renamed = {{ from = \"{from}\" }}` but `{from}` is \
                 currently declared and not itself renamed; a construct cannot be claimed twice",
                from = r.from
            )));
        }
    }
    Ok(())
}

fn convert_tenant(name: &TenantName, t: &RawTenant) -> Result<Tenant, Error> {
    let name = name.clone();
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
        renamed: t.renamed.as_ref().map(|r| r.from.clone()),
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

fn convert_port(
    name: &ConstructName,
    p: &RawPort,
    tenants: &HashSet<TenantName>,
) -> Result<Port, Error> {
    if let Some(dpni) = &p.dpni {
        return Err(cfg(format!(
            "port `{name}` pins a DPNI index (`dpni = \"{dpni}\"`); DPNI identity is derived from \
             the DPMAC edge and must not be set"
        )));
    }
    // The name's interface-name validity is checked once, up front, over every declared
    // family (see `convert`; ADR-0015 decision 13), so the port path only clones it here.
    let name = name.clone();
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
        renamed: p.renamed.as_ref().map(|r| r.from.clone()),
    })
}

fn convert_link(
    name: &ConstructName,
    l: &RawLink,
    tenants: &HashSet<TenantName>,
) -> Result<Link, Error> {
    let name = name.clone();
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
        renamed: l.renamed.as_ref().map(|r| r.from.clone()),
    })
}

fn convert_fabric(
    name: &ConstructName,
    f: &RawFabric,
    tenants: &HashSet<TenantName>,
    ports: &HashSet<ConstructName>,
    fabrics: &HashSet<ConstructName>,
) -> Result<Fabric, Error> {
    let name = name.clone();
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
        renamed: f.renamed.as_ref().map(|r| r.from.clone()),
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

/// Converts one `family = count` pair under an `[extra.<tenant>]` table into a neutral
/// [`Extra`]. The tenant must resolve (declared, or the reserved `kernel`) and the
/// family must be known; a duplicate (tenant, family) never reaches here, the TOML
/// format having refused it as a key redefinition (schema.rs).
fn convert_extra(
    tenant: &TenantName,
    family: &str,
    count: i64,
    tenants: &HashSet<TenantName>,
) -> Result<Extra, Error> {
    if !resolves(tenant, tenants) {
        return Err(cfg(format!(
            "`[extra.{tenant}]` names tenant `{tenant}`, which is not declared"
        )));
    }
    let family = parse_family(tenant, family)?;
    Ok(Extra {
        tenant: tenant.clone(),
        family,
        count,
    })
}

/// Parses a lowercase restool family name (`"dpio"`, …) to a [`Family`]. `compile`
/// refuses a non-companion family or a count below 1 (`ExtraNotCompanion`,
/// `ExtraNotPositive`); the config only parses the name. `tenant` names the owning
/// `[extra.<tenant>]` table in the error subject.
fn parse_family(tenant: &TenantName, name: &str) -> Result<Family, Error> {
    ALL_FAMILIES
        .iter()
        .copied()
        .find(|f| f.as_str() == name)
        .ok_or_else(|| cfg(format!("`[extra.{tenant}]` names unknown family `{name}`")))
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
    use dpaa2_api::{
        Dataplane, DpmacId, Extra, Family, Isolation, MacAddr, MacMode, Member, Switching,
    };

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
            [tenant.router]
            dataplane = "userspace-poll"
            max_cores = 16

            [port.wan0]
            dpmac = "dpmac.7"
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
            [port.wan0]
            dpmac = "dpmac.3"
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
            [tenant.router]
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
            [tenant.router]
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
            [port.wan0]
            dpmac = "dpmac.7"
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
            [port.mgmt]
            dpmac = "dpmac.7"
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
            [port.wan0]
            dpmac = "dpmac.7"
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
            [tenant.router]
            dataplane = "userspace-poll"
            max_cores = 16

            [port.wan0]
            dpmac = "dpmac.9"
            rate = 10000
            tenant = "router"

            [port.wan1]
            dpmac = "dpmac.10"
            rate = 10000
            tenant = "router"

            [[crypto]]
            tenant = "router"
            flows = 4

            [[crypto]]
            tenant = "router"
            flows = 8

            [extra.router]
            dpio = 2
            "#,
        );
        assert_eq!(intent.tenants[0].dataplane, Dataplane::UserspacePoll);
        assert_eq!(intent.tenants[0].isolation, Isolation::Isolated, "default");
        // Ports take canonical name order (here wan0 < wan1, so also declaration order).
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
    fn keyed_ports_take_canonical_name_order_regardless_of_declaration() {
        // The port name lives in the table key (ADR-0015 decision 1), so the built Intent
        // carries ports in canonical NAME order, never document order — declaring wan0
        // before up0 still yields up0, wan0. The order is cosmetic: the derivation mints
        // the name-ordered dpni ordinal (decision 5, the position-independence law), so
        // whichever order the constructs appear in is inert. (The compiled plan is
        // order-independent; the `dpaa2-verify` pairing/conformance rungs compare up to
        // this name order.)
        let intent = parse(
            r#"
            [tenant.router]
            dataplane = "userspace-poll"
            max_cores = 16

            [port.wan0]
            dpmac = "dpmac.9"
            rate = 10000
            tenant = "router"

            [port.up0]
            dpmac = "dpmac.4"
            rate = 25000
            tenant = "router"
            "#,
        );
        let names: Vec<&str> = intent.ports.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["up0", "wan0"], "ports take canonical name order");
    }

    #[test]
    fn mac_and_mode_survive_conversion() {
        let intent = parse(
            r#"
            [port.wan0]
            dpmac = "dpmac.7"
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
            [tenant.ns1]
            dataplane = "kernel-netlink"
            max_cores = 2

            [link.uplink]
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
            [tenant.ns1]
            dataplane = "kernel-netlink"
            max_cores = 2

            [link.loop]
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
            [tenant.router]
            dataplane = "userspace-poll"
            max_cores = 16

            [port.lan0]
            dpmac = "dpmac.7"
            rate = 10000

            [fabric.lan]
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
            [fabric.lan]
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
            [tenant.sec]
            dataplane = "userspace-poll"
            max_cores = 16
            isolation = "restricted"
            "#,
        );
        assert!(no_pool.contains("restricted"), "{no_pool}");

        let stray_pool = parse_err(
            r#"
            [tenant.sec]
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
            [tenant.prim]
            dataplane = "userspace-poll"
            max_cores = 16
            isolation = "public"

            [tenant.sec]
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
    fn scenario_duplicate_interface_name_is_a_parse_error() {
        // Identity is structural (ADR-0015 decision 1): a second `[port.wan0]` table is a
        // TOML key redefinition, so a duplicate interface name is unrepresentable, not
        // validated — this is the law that replaces the old hand-written check.
        let err = parse_err(
            r#"
            [port.wan0]
            dpmac = "dpmac.7"
            rate = 10000

            [port.wan0]
            dpmac = "dpmac.8"
            rate = 10000
            "#,
        );
        assert!(
            err.contains("duplicate key"),
            "the format refuses the redefinition: {err}"
        );
    }

    #[test]
    fn port_named_like_a_link_is_a_cross_family_collision() {
        // Intra-family duplicates are TOML key redefinitions now, but a port and a link of
        // one name is a cross-family collision the shared construct namespace still
        // refuses (task 3.3d) — the only DuplicateName path left.
        let err = parse_err(
            r#"
            [tenant.ns1]
            dataplane = "kernel-netlink"
            max_cores = 2

            [port.x]
            dpmac = "dpmac.7"
            rate = 10000

            [link.x]
            interface_a = "ns1"
            interface_b = "kernel"
            "#,
        );
        assert!(err.contains("duplicate construct name"), "{err}");
        assert!(err.contains('x'), "names the colliding name: {err}");
    }

    #[test]
    fn scenario_malformed_mac() {
        let err = parse_err(
            r#"
            [port.wan0]
            dpmac = "dpmac.7"
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
            [port.wan0]
            dpmac = "dpmac.7"
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
            [tenant.kernel]
            dataplane = "kernel-netlink"
            max_cores = 16
            "#,
        );
        assert!(err.contains("kernel"), "{err}");
        assert!(err.contains("reserved"), "{err}");
    }

    // ADR-0015 decision 13: a DECLARED construct name must be a valid interface name and
    // never match the reserved `family.N` pattern. Four families, four distinct reasons.

    #[test]
    fn tenant_named_like_a_reserved_handle_is_rejected() {
        let err = parse_err(
            r#"
            [tenant."dpni.4"]
            dataplane = "userspace-poll"
            max_cores = 16
            "#,
        );
        assert!(err.contains("tenant.dpni.4"), "names the construct: {err}");
        assert!(err.contains("reserved"), "states the rule: {err}");
    }

    #[test]
    fn port_name_over_ifnamsiz_is_rejected() {
        let err = parse_err(
            r#"
            [port.porttoolongname0]
            dpmac = "dpmac.7"
            rate = 10000
            "#,
        );
        assert!(
            err.contains("port.porttoolongname0"),
            "names the construct: {err}"
        );
        assert!(err.contains("15-character limit"), "states the rule: {err}");
    }

    #[test]
    fn link_name_with_a_slash_is_rejected() {
        let err = parse_err(
            r#"
            [tenant.ns1]
            dataplane = "kernel-netlink"
            max_cores = 2

            [link."a/b"]
            interface_a = "ns1"
            interface_b = "kernel"
            "#,
        );
        assert!(err.contains("link.a/b"), "names the construct: {err}");
        assert!(err.contains("not allowed"), "states the rule: {err}");
    }

    #[test]
    fn fabric_name_with_whitespace_is_rejected() {
        let err = parse_err(
            r#"
            [fabric."a b"]
            switching = "hardware"
            forwarded_by = "kernel"
            members = []
            "#,
        );
        assert!(err.contains("fabric.a b"), "names the construct: {err}");
        assert!(err.contains("not allowed"), "states the rule: {err}");
    }

    #[test]
    fn duplicate_tenant_name_is_a_parse_error() {
        // Identity is structural: a second `[tenant.router]` table is a TOML key
        // redefinition, so a duplicate tenant name is unrepresentable, not validated —
        // this is the law that replaces the old hand-written check.
        let err = parse_err(
            r#"
            [tenant.router]
            dataplane = "userspace-poll"
            max_cores = 16

            [tenant.router]
            dataplane = "kernel-netlink"
            max_cores = 16
            "#,
        );
        assert!(
            err.contains("duplicate key"),
            "the format refuses the redefinition: {err}"
        );
    }

    #[test]
    fn fabric_named_like_a_port_is_rejected() {
        // Fabric-vs-fabric duplicates are format-enforced now, but a fabric named like a
        // port is a cross-type collision TOML does not prevent — the `constructs`
        // namespace still refuses it.
        let err = parse_err(
            r#"
            [port.lan0]
            dpmac = "dpmac.7"
            rate = 10000

            [fabric.lan0]
            switching = "hardware"
            forwarded_by = "kernel"
            members = ["lan0"]
            "#,
        );
        assert!(err.contains("duplicate construct name"), "{err}");
    }

    #[test]
    fn unknown_extra_family_is_rejected() {
        let err = parse_err(
            r#"
            [tenant.router]
            dataplane = "userspace-poll"
            max_cores = 16

            [extra.router]
            dpwidget = 1
            "#,
        );
        assert!(err.contains("dpwidget"), "names the family: {err}");
    }

    #[test]
    fn duplicate_family_under_one_tenant_is_a_parse_error() {
        // Identity is structural: a second `dpio` under one `[extra.<tenant>]` table is a
        // TOML key redefinition, so a duplicate (tenant, family) is unrepresentable, not
        // validated — this is the law that replaces the old hand-written check.
        let err = parse_err(
            r#"
            [tenant.router]
            dataplane = "userspace-poll"
            max_cores = 16

            [extra.router]
            dpio = 1
            dpio = 2
            "#,
        );
        assert!(
            err.contains("duplicate key"),
            "the format refuses the redefinition: {err}"
        );
    }

    #[test]
    fn extras_across_families_and_tenants_all_convert() {
        // Two families under one tenant table and a second tenant table: every
        // (tenant, family) pair lands in the set.
        let intent = parse(
            r#"
            [tenant.router]
            dataplane = "userspace-poll"
            max_cores = 16

            [extra.router]
            dpio = 2
            dpbp = 3

            [extra.kernel]
            dpmcp = 1
            "#,
        );
        assert_eq!(intent.extras.len(), 3);
        assert!(intent.extras.contains(&Extra {
            tenant: "router".into(),
            family: Family::Dpio,
            count: 2,
        }));
        assert!(intent.extras.contains(&Extra {
            tenant: "router".into(),
            family: Family::Dpbp,
            count: 3,
        }));
        assert!(intent.extras.contains(&Extra {
            tenant: "kernel".into(),
            family: Family::Dpmcp,
            count: 1,
        }));
    }

    #[test]
    fn unknown_field_is_rejected() {
        let err = parse_err(
            r#"
            [port.wan0]
            dpmac = "dpmac.7"
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
            [port.wan0]
            dpmac = "eth3"
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
            [port.lan0]
            dpmac = "dpmac.7"
            rate = 10000

            [port.lan1]
            dpmac = "dpmac.8"
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
             [port.lan0]\n\
             dpmac = \"dpmac.7\"\n\
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

    // ---- Requirement: `renamed = { from }` (ADR-0015 decision 10; task 6.3) ----

    #[test]
    fn rename_from_a_declared_construct_not_itself_renamed_is_rejected() {
        // The genuine contradiction: `eth0` claims to have been `wan0`, but `wan0` is
        // still declared and stays itself — the object would be claimed twice.
        let err = parse_err(
            r#"
            [port.eth0]
            dpmac = "dpmac.7"
            rate = 10000
            renamed = { from = "wan0" }

            [port.wan0]
            dpmac = "dpmac.8"
            rate = 10000
            "#,
        );
        assert!(err.contains("not itself renamed"), "states the rule: {err}");
        assert!(err.contains("wan0"), "names the from-target: {err}");
    }

    #[test]
    fn rename_swap_is_accepted() {
        // Both ends rename to each other in one edit: each target is itself renamed
        // away, so the refined rule (i) admits the swap.
        let intent = parse(
            r#"
            [port.wan0]
            dpmac = "dpmac.7"
            rate = 10000
            renamed = { from = "eth0" }

            [port.eth0]
            dpmac = "dpmac.8"
            rate = 10000
            renamed = { from = "wan0" }
            "#,
        );
        assert_eq!(intent.ports.len(), 2, "the swap parses to both ports");
    }

    #[test]
    fn rename_in_the_tenant_namespace_from_a_declared_tenant_is_rejected() {
        let err = parse_err(
            r#"
            [tenant.edge]
            dataplane = "userspace-poll"
            max_cores = 16
            renamed = { from = "c1" }

            [tenant.c1]
            dataplane = "userspace-poll"
            max_cores = 16
            "#,
        );
        assert!(err.contains("not itself renamed"), "states the rule: {err}");
        assert!(err.contains("c1"), "names the from-target: {err}");
    }

    #[test]
    fn plain_rename_from_an_undeclared_construct_is_accepted() {
        // The common after-a-rename case: the old name is gone, so the clause is inert.
        let intent = parse(
            r#"
            [port.eth0]
            dpmac = "dpmac.7"
            rate = 10000
            renamed = { from = "wan0" }
            "#,
        );
        assert_eq!(intent.ports[0].name.as_str(), "eth0");
    }

    #[test]
    fn accepted_rename_lands_as_renamed_and_a_sibling_is_none() {
        // An accepted `renamed = { from }` (the source undeclared, so inert) is carried
        // onto its construct as `Some`, and a sibling with no clause carries `None`
        // (task 6.5, ADR-0015 decision 10) — the matcher now sees the clause.
        let intent = parse(
            r#"
            [port.eth0]
            dpmac = "dpmac.7"
            rate = 10000
            renamed = { from = "old0" }

            [port.wan1]
            dpmac = "dpmac.8"
            rate = 10000
            "#,
        );
        let eth0 = intent
            .ports
            .iter()
            .find(|p| p.name.as_str() == "eth0")
            .expect("eth0 present");
        let wan1 = intent
            .ports
            .iter()
            .find(|p| p.name.as_str() == "wan1")
            .expect("wan1 present");
        assert_eq!(eth0.renamed.as_ref().map(|n| n.as_str()), Some("old0"));
        assert!(wan1.renamed.is_none(), "a construct without a clause is None");
    }

    #[test]
    fn rename_from_an_invalid_interface_name_is_rejected() {
        // The `from` value must satisfy the task-6.2 interface-name rule: `dpni.4`
        // trips the reserved `family.N` pattern.
        let err = parse_err(
            r#"
            [port.eth0]
            dpmac = "dpmac.7"
            rate = 10000
            renamed = { from = "dpni.4" }
            "#,
        );
        assert!(
            err.contains("invalid rename source"),
            "names the slot: {err}"
        );
        assert!(err.contains("reserved"), "cites the 6.2 rule: {err}");
    }

    #[test]
    fn rename_across_namespaces_is_inert() {
        // A port's `from` naming a declared TENANT is not the port namespace's target,
        // so rule (i) does not fire — it is a plain, inert rename (namespaces are
        // separate).
        let intent = parse(
            r#"
            [tenant.c1]
            dataplane = "userspace-poll"
            max_cores = 16

            [port.eth0]
            dpmac = "dpmac.7"
            rate = 10000
            renamed = { from = "c1" }
            "#,
        );
        assert_eq!(intent.ports[0].name.as_str(), "eth0");
    }

    #[test]
    fn shipped_example_topology_is_valid() {
        // The example installed to /etc/dpaa2/topology.toml must always parse.
        let example = include_str!("../../../packaging/dpaa2/topology.toml");
        let intent = parse_str(example).expect("shipped example topology parses");
        assert!(!intent.ports.is_empty());
    }
}
