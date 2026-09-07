//! Reader, TOML emitter, and verdict matcher for frozen raw-conformance ITF
//! traces (`models/intent/traces/raw*Trace.itf.json`, emitted by
//! `models/intent/raw_replay.qnt`).
//!
//! MBT conformance (task 3.3e, bead gqf.48): the model is the oracle. Each trace's
//! frozen `raw` (a `RawIntent` mirroring the post-3.3b/3.3c/3.3d TOML surface) is
//! serialized here to a TOML document on the *real* `dpaa2-config` surface, fed
//! through `dpaa2_config::parse_str`, and the Rust verdict is asserted to agree
//! with the model's frozen `verdict` (`ModelVerdict`, the model's own `parse`).
//! The harness that drives it lives in `tests/raw_conformance.rs`.
//!
//! Two encodings are reconciled here, nowhere else:
//! - the raw surface: keyed maps for tenant/port/link/fabric/extra (a duplicate key
//!   is a TOML parse error, so the model spells them as maps — task 3.3d keyed ports
//!   and links too, ADR-0015 decision 1, so only a cross-family name collision still
//!   reaches DuplicateName), an array for crypto (genuinely anonymous, declaration
//!   order IS the dpseci ordinal), the model's
//!   `""` tenant/pool meaning absent (omitted from the emitted TOML), and the
//!   `dpmac` int emitted as the `"dpmac.N"` string the surface reads. Fields the
//!   model deliberately does not carry (`mac`, `mac_mode`, `dpni`) are omitted —
//!   they are optional, and `parse.rs` defaults them exactly as the accepted-arm
//!   model `Intent` does.
//! - the verdict: `ParseOk(Intent)` ⇒ the model `Intent` (parsed with the
//!   `crate::intent_itf` machinery), `ParseRefused(Set[RawRefusal])` ⇒ the model
//!   refusal set. `parse.rs` short-circuits with ONE error while the model carries
//!   the whole set (the `intent_raw.qnt` DEVIATION), so on the refused arm the
//!   harness asserts the Rust error corresponds to *at least one* refusal in the
//!   set, via `RawRefusal::matches_error` — a per-variant matcher anchored to the
//!   exact `parse.rs` spellings (read from `crates/dpaa2-config/src/parse.rs`, not
//!   guessed).
//!
//! Name slots cross into their dpaa2-api newtypes (`TenantName`, `ConstructName`)
//! at this ITF boundary, exactly as the config's serde layer does, so no bare
//! `String` name slot flows through the mirror (types.rs; the account name-slot rule).
//! Enum tokens (`dataplane`, `isolation`, `switching`) and the extra `family` are
//! not name slots — they stay `String` (a `family` may deliberately be unknown).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::Value;

use dpaa2_api::{ConstructName, Intent, TenantName};

use crate::intent_itf::{cname, field, intent, list_items, map_items, set_items, text, tname};
use crate::itf::{int64, tag};

// ---- the raw surface mirror (schema.rs / intent_raw.qnt) ----

/// A `[tenant.<name>]` value. `dataplane`/`isolation` are their TOML tokens (not
/// name slots); `pool` is `""` when absent.
#[derive(Clone, Debug)]
pub struct RawTenant {
    /// The `dataplane` TOML token (e.g. `"userspace-poll"`).
    pub dataplane: String,
    /// The core budget.
    pub max_cores: i64,
    /// The `isolation` TOML token (e.g. `"isolated"`).
    pub isolation: String,
    /// The pool holder, or `""` when absent (omitted from the emitted TOML).
    pub pool: TenantName,
    /// The `renamed.from` prior name, or `""` when absent (task 6.3; omitted).
    pub from: TenantName,
}

/// A `[port.<name>]` value (the name is the map key, task 3.3d). `dpmac` is the int
/// the model carries, emitted `"dpmac.N"`.
#[derive(Clone, Debug)]
pub struct RawPort {
    /// The DPMAC anchor index.
    pub dpmac: i64,
    /// The rate in Mbps.
    pub rate: i64,
    /// The owning tenant, or `""` for the kernel-defaulting owner (omitted).
    pub tenant: TenantName,
    /// The `renamed.from` prior name, or `""` when absent (task 6.3; omitted).
    pub from: ConstructName,
}

/// A `[link.<name>]` value (the name is the map key, task 3.3d): a pseudo-wire between
/// two tenant ends.
#[derive(Clone, Debug)]
pub struct RawLink {
    /// One end.
    pub interface_a: TenantName,
    /// The other end.
    pub interface_b: TenantName,
    /// The `renamed.from` prior name, or `""` when absent (task 6.3; omitted).
    pub from: ConstructName,
}

/// A `[fabric.<name>]` value.
#[derive(Clone, Debug)]
pub struct RawFabric {
    /// The `switching` TOML token (`"hardware"`/`"software"`).
    pub switching: String,
    /// The forwarding tenant.
    pub forwarded_by: TenantName,
    /// The members, in declaration order.
    pub members: Vec<ConstructName>,
    /// The `renamed.from` prior name, or `""` when absent (task 6.3; omitted).
    pub from: ConstructName,
}

/// A `[[crypto]]` value.
#[derive(Clone, Debug)]
pub struct RawCrypto {
    /// The owning tenant.
    pub tenant: TenantName,
    /// The flow demand.
    pub flows: i64,
}

/// The whole raw document, mirroring `intent_raw.qnt`'s `RawIntent`.
#[derive(Clone, Debug, Default)]
pub struct RawIntent {
    /// `[tenant.<name>]` tables, keyed (a duplicate key is a TOML parse error).
    pub tenants: BTreeMap<TenantName, RawTenant>,
    /// `[port.<name>]` tables, keyed (task 3.3d — an intra-family duplicate is a TOML
    /// parse error; only a cross-family collision reaches `DuplicateName`).
    pub ports: BTreeMap<ConstructName, RawPort>,
    /// `[link.<name>]` tables, keyed (task 3.3d).
    pub links: BTreeMap<ConstructName, RawLink>,
    /// `[fabric.<name>]` tables, keyed.
    pub fabrics: BTreeMap<ConstructName, RawFabric>,
    /// `[[crypto]]` array.
    pub crypto: Vec<RawCrypto>,
    /// `[extra.<tenant>]` tables: tenant → (family token → count), keyed twice.
    pub extras: BTreeMap<TenantName, BTreeMap<String, i64>>,
}

// ---- the model verdict mirror (intent_raw.qnt Parsed / RawRefusal) ----

/// One model refusal, mirroring `intent_raw.qnt`'s `RawRefusal`. The `FromCompile`
/// wrapper is flattened: its three reused variants sit alongside the raw-surface ones.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RawRefusal {
    /// A construct naming an undeclared tenant (`FromCompile(TenantAbsent)`).
    TenantAbsent {
        /// The naming construct.
        construct: ConstructName,
        /// The absent tenant.
        tenant: TenantName,
    },
    /// A pool on a non-restricted tenant (raw-native `PoolWithoutRestricted`;
    /// vocabulary-v2 D1 removed the compile twin).
    PoolWithoutRestricted {
        /// The offending tenant.
        tenant: TenantName,
        /// The named pool.
        pool: TenantName,
    },
    /// A restricted tenant naming no pool (raw-native `RestrictedWithoutPool`;
    /// vocabulary-v2 D1 removed the compile twin).
    RestrictedWithoutPool {
        /// The offending tenant.
        tenant: TenantName,
    },
    /// `[tenant.kernel]` declares the reserved name.
    ReservedKernel {
        /// The reserved name (`kernel`).
        name: TenantName,
    },
    /// A port/link/fabric name TOML keys cannot dedupe.
    DuplicateName {
        /// The duplicated name.
        name: ConstructName,
    },
    /// A link naming one tenant at both ends.
    LinkSelfLoop {
        /// The link.
        link: ConstructName,
        /// The doubly-named tenant.
        tenant: TenantName,
    },
    /// A fabric member resolving to no port/tenant/fabric.
    RawMemberUnresolved {
        /// The fabric.
        fabric: ConstructName,
        /// The unresolved member.
        member: ConstructName,
    },
    /// An `[extra.<tenant>]` family not one of the 16.
    UnknownExtraFamily {
        /// The tenant.
        tenant: TenantName,
        /// The unknown family token.
        family: String,
    },
    /// A `renamed = { from }` naming a currently-declared construct not itself
    /// renamed away (ADR-0015 decision 10 rule (i) as refined, task 6.3).
    RenamedFromDeclared {
        /// The renaming construct.
        construct: ConstructName,
        /// The `from`-target that would be claimed twice.
        from: ConstructName,
    },
}

/// The near-miss dimension a [`RawRefusal`] belongs to, for corpus coverage. One
/// per near-miss the dirty alphabet reaches (task 3.3e); an unpaired near-miss —
/// a kind absent from the corpus — is the gqf.39 hole shape, so the harness fails
/// on a missing kind.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Kind {
    /// `TenantAbsent` — the dangling tenant reference.
    TenantAbsent,
    /// `PoolWithoutRestricted` — a pool on a non-restricted tenant.
    PoolWithoutRestricted,
    /// `RestrictedWithoutPool` — a restricted tenant with no pool.
    RestrictedWithoutPool,
    /// `ReservedKernel` — the reserved kernel declared.
    ReservedKernel,
    /// `DuplicateName` — a duplicate construct name.
    DuplicateName,
    /// `LinkSelfLoop` — a self-loop link.
    LinkSelfLoop,
    /// `RawMemberUnresolved` — an unresolved fabric member.
    RawMemberUnresolved,
    /// `UnknownExtraFamily` — an unknown extra family.
    UnknownExtraFamily,
    /// `RenamedFromDeclared` — a rename from a declared, not-itself-renamed construct.
    RenamedFromDeclared,
}

impl RawRefusal {
    /// The near-miss dimension this refusal belongs to.
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            RawRefusal::TenantAbsent { .. } => Kind::TenantAbsent,
            RawRefusal::PoolWithoutRestricted { .. } => Kind::PoolWithoutRestricted,
            RawRefusal::RestrictedWithoutPool { .. } => Kind::RestrictedWithoutPool,
            RawRefusal::ReservedKernel { .. } => Kind::ReservedKernel,
            RawRefusal::DuplicateName { .. } => Kind::DuplicateName,
            RawRefusal::LinkSelfLoop { .. } => Kind::LinkSelfLoop,
            RawRefusal::RawMemberUnresolved { .. } => Kind::RawMemberUnresolved,
            RawRefusal::UnknownExtraFamily { .. } => Kind::UnknownExtraFamily,
            RawRefusal::RenamedFromDeclared { .. } => Kind::RenamedFromDeclared,
        }
    }

    /// Whether a `parse.rs` [`dpaa2_api::Error::Config`] message `e` corresponds to
    /// this model refusal. Each arm anchors on the distinctive substring of the
    /// exact `parse.rs` spelling (plus the payload name where it disambiguates), so
    /// the harness can assert the short-circuited Rust error is *one of* the model's
    /// refusal set (the `intent_raw.qnt` DEVIATION). The spellings are read from
    /// `crates/dpaa2-config/src/parse.rs`; see the module for the source clauses.
    #[must_use]
    pub fn matches_error(&self, e: &str) -> bool {
        match self {
            // convert: "... declares the reserved tenant name `kernel`, which is reserved ...".
            RawRefusal::ReservedKernel { .. } => e.contains("reserved"),
            // convert: "duplicate construct name `<n>`" (the cross-family collision, the
            // only DuplicateName path left once ports/links are keyed, task 3.3d).
            RawRefusal::DuplicateName { name } => {
                e.contains("duplicate") && e.contains(name.as_str())
            }
            // convert_link: "... names the same tenant `<t>` at both ends ...".
            RawRefusal::LinkSelfLoop { .. } => e.contains("both ends"),
            // classify_member: "... names member `<m>`, which is not a declared port, tenant, or fabric".
            RawRefusal::RawMemberUnresolved { member, .. } => {
                e.contains("not a declared") && e.contains(member.as_str())
            }
            // parse_family: "`[extra.<t>]` names unknown family `<f>`".
            RawRefusal::UnknownExtraFamily { family, .. } => {
                e.contains("unknown family") && e.contains(family.as_str())
            }
            // convert_{port,link,fabric,crypto,extra}: "... `<t>`, which is not declared".
            // Disjoint from RawMemberUnresolved's "not a declared" (the article "a").
            RawRefusal::TenantAbsent { tenant, .. } => {
                e.contains("not declared") && e.contains(tenant.as_str())
            }
            // convert_tenant: "... names a `pool` (`<p>`) but is not `restricted`; ...".
            RawRefusal::PoolWithoutRestricted { .. } => e.contains("but is not `restricted`"),
            // convert_tenant: "... is `restricted` but names no `pool` holder".
            RawRefusal::RestrictedWithoutPool { .. } => e.contains("names no `pool`"),
            // check_renames: "... `<from>` is currently declared and not itself renamed;
            // a construct cannot be claimed twice" (task 6.3).
            RawRefusal::RenamedFromDeclared { from, .. } => {
                e.contains("not itself renamed") && e.contains(from.as_str())
            }
        }
    }
}

/// The model's frozen `parse` verdict: accepted (with the resulting `Intent`, the
/// oracle for the accepted-arm comparison) or refused (with the whole refusal set).
#[derive(Clone, Debug)]
pub enum ModelVerdict {
    /// `ParseOk(Intent)` — accepted; carries the model's resulting neutral intent.
    Ok(Box<Intent>),
    /// `ParseRefused(Set[RawRefusal])` — the complete model refusal set.
    Refused(Vec<RawRefusal>),
}

/// One replay case: the raw document and the model's frozen verdict over it.
pub struct RawCase {
    /// The raw surface document to serialize and re-parse through `dpaa2-config`.
    pub raw: RawIntent,
    /// The model's own `parse` verdict (the oracle).
    pub verdict: ModelVerdict,
}

// ---- TOML emission (the real dpaa2-config surface) ----

/// A table-key component (`[tenant.<k>]`, `[fabric.<k>]`, `[extra.<k>]`) must be a
/// bare TOML identifier for the emitter to place it unquoted. Every name the dirty
/// alphabet draws is one; a state that cannot be expressed on the surface fails
/// loudly here rather than emitting malformed TOML (task 3.3e).
fn bare_key(k: &str) -> &str {
    assert!(
        !k.is_empty()
            && k.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "raw state carries a table key `{k}` that is not a bare TOML identifier; it cannot be \
         expressed on the config surface without fixing it up"
    );
    k
}

/// Serializes a [`RawIntent`] to a TOML document on the real `dpaa2-config` surface,
/// including the mandatory `[intent] schema = 1` envelope. A near-miss serializes to
/// exactly its pathological TOML (a cross-family name collision, dangling references,
/// `[tenant.kernel]`, unknown family keys) — the emitter never fixes up a dirty state.
#[must_use]
pub fn to_toml(raw: &RawIntent) -> String {
    let mut s = String::from("[intent]\nschema = 1\n");

    for (name, t) in &raw.tenants {
        let _ = writeln!(s, "\n[tenant.{}]", bare_key(name.as_str()));
        let _ = writeln!(s, "dataplane = \"{}\"", t.dataplane);
        let _ = writeln!(s, "max_cores = {}", t.max_cores);
        let _ = writeln!(s, "isolation = \"{}\"", t.isolation);
        if !t.pool.as_str().is_empty() {
            let _ = writeln!(s, "pool = \"{}\"", t.pool);
        }
        if !t.from.as_str().is_empty() {
            let _ = writeln!(s, "renamed = {{ from = \"{}\" }}", t.from);
        }
    }

    for (name, p) in &raw.ports {
        let _ = writeln!(s, "\n[port.{}]", bare_key(name.as_str()));
        let _ = writeln!(s, "dpmac = \"dpmac.{}\"", p.dpmac);
        let _ = writeln!(s, "rate = {}", p.rate);
        if !p.tenant.as_str().is_empty() {
            let _ = writeln!(s, "tenant = \"{}\"", p.tenant);
        }
        if !p.from.as_str().is_empty() {
            let _ = writeln!(s, "renamed = {{ from = \"{}\" }}", p.from);
        }
    }

    for (name, l) in &raw.links {
        let _ = writeln!(s, "\n[link.{}]", bare_key(name.as_str()));
        let _ = writeln!(s, "interface_a = \"{}\"", l.interface_a);
        let _ = writeln!(s, "interface_b = \"{}\"", l.interface_b);
        if !l.from.as_str().is_empty() {
            let _ = writeln!(s, "renamed = {{ from = \"{}\" }}", l.from);
        }
    }

    for (name, f) in &raw.fabrics {
        let _ = writeln!(s, "\n[fabric.{}]", bare_key(name.as_str()));
        let _ = writeln!(s, "switching = \"{}\"", f.switching);
        let _ = writeln!(s, "forwarded_by = \"{}\"", f.forwarded_by);
        let members: Vec<String> = f.members.iter().map(|m| format!("\"{m}\"")).collect();
        let _ = writeln!(s, "members = [{}]", members.join(", "));
        if !f.from.as_str().is_empty() {
            let _ = writeln!(s, "renamed = {{ from = \"{}\" }}", f.from);
        }
    }

    for k in &raw.crypto {
        s.push_str("\n[[crypto]]\n");
        let _ = writeln!(s, "tenant = \"{}\"", k.tenant);
        let _ = writeln!(s, "flows = {}", k.flows);
    }

    for (t, families) in &raw.extras {
        let _ = writeln!(s, "\n[extra.{}]", bare_key(t.as_str()));
        for (family, count) in families {
            let _ = writeln!(s, "{family} = {count}");
        }
    }

    s
}

// ---- ITF reading ----

/// The TOML token for a `Dataplane` ITF tag (matches schema.rs `RawDataplane`).
fn dataplane_token(v: &Value) -> Result<String, String> {
    Ok(match tag(v)? {
        "KernelNetlink" => "kernel-netlink",
        "UserspacePoll" => "userspace-poll",
        "UserspaceEvent" => "userspace-event",
        t => return Err(format!("unknown dataplane `{t}`")),
    }
    .to_owned())
}

/// The TOML token for a `RawIsolation` ITF tag (matches schema.rs `RawIsolation`).
/// The model's raw surface carries a payload-free `RawIsolation` (`RPublic` /
/// `RRestricted` / `RIsolated`) distinct from the neutral `Isolation`, so its pool
/// key stays a separate field the contradictions can disagree with (vocabulary-v2 D1).
fn isolation_token(v: &Value) -> Result<String, String> {
    Ok(match tag(v)? {
        "RPublic" => "public",
        "RRestricted" => "restricted",
        "RIsolated" => "isolated",
        t => return Err(format!("unknown isolation `{t}`")),
    }
    .to_owned())
}

/// The TOML token for a `Switching` ITF tag (matches schema.rs `RawSwitching`).
fn switching_token(v: &Value) -> Result<String, String> {
    Ok(match tag(v)? {
        "Hardware" => "hardware",
        "Software" => "software",
        t => return Err(format!("unknown switching `{t}`")),
    }
    .to_owned())
}

fn raw_tenant(v: &Value) -> Result<RawTenant, String> {
    Ok(RawTenant {
        dataplane: dataplane_token(field(v, "dataplane")?)?,
        max_cores: int64(field(v, "maxCores")?)?,
        isolation: isolation_token(field(v, "isolation")?)?,
        pool: tname(field(v, "pool")?)?,
        from: tname(field(v, "from")?)?,
    })
}

fn raw_port(v: &Value) -> Result<RawPort, String> {
    Ok(RawPort {
        dpmac: int64(field(v, "dpmac")?)?,
        rate: int64(field(v, "rate")?)?,
        tenant: tname(field(v, "tenant")?)?,
        from: cname(field(v, "from")?)?,
    })
}

fn raw_link(v: &Value) -> Result<RawLink, String> {
    Ok(RawLink {
        interface_a: tname(field(v, "interfaceA")?)?,
        interface_b: tname(field(v, "interfaceB")?)?,
        from: cname(field(v, "from")?)?,
    })
}

fn raw_fabric(v: &Value) -> Result<RawFabric, String> {
    Ok(RawFabric {
        switching: switching_token(field(v, "switching")?)?,
        forwarded_by: tname(field(v, "forwardedBy")?)?,
        members: list_items(field(v, "members")?)?
            .iter()
            .map(cname)
            .collect::<Result<_, _>>()?,
        from: cname(field(v, "from")?)?,
    })
}

fn raw_crypto(v: &Value) -> Result<RawCrypto, String> {
    Ok(RawCrypto {
        tenant: tname(field(v, "tenant")?)?,
        flows: int64(field(v, "flows")?)?,
    })
}

fn raw_intent(v: &Value) -> Result<RawIntent, String> {
    let mut tenants = BTreeMap::new();
    for pair in map_items(field(v, "tenants")?)? {
        tenants.insert(tname(&pair[0])?, raw_tenant(&pair[1])?);
    }
    let mut ports = BTreeMap::new();
    for pair in map_items(field(v, "ports")?)? {
        ports.insert(cname(&pair[0])?, raw_port(&pair[1])?);
    }
    let mut links = BTreeMap::new();
    for pair in map_items(field(v, "links")?)? {
        links.insert(cname(&pair[0])?, raw_link(&pair[1])?);
    }
    let mut fabrics = BTreeMap::new();
    for pair in map_items(field(v, "fabrics")?)? {
        fabrics.insert(cname(&pair[0])?, raw_fabric(&pair[1])?);
    }
    let mut extras = BTreeMap::new();
    for pair in map_items(field(v, "extras")?)? {
        let mut inner = BTreeMap::new();
        for fam in map_items(&pair[1])? {
            inner.insert(text(&fam[0])?, int64(&fam[1])?);
        }
        extras.insert(tname(&pair[0])?, inner);
    }
    Ok(RawIntent {
        tenants,
        ports,
        links,
        fabrics,
        crypto: list_items(field(v, "crypto")?)?
            .iter()
            .map(raw_crypto)
            .collect::<Result<_, _>>()?,
        extras,
    })
}

fn raw_refusal(v: &Value) -> Result<RawRefusal, String> {
    // `TenantAbsent` arrives wrapped in `FromCompile` (the one compile refusal the
    // raw surface still reuses); the two pool contradictions are raw-native tags
    // now (vocabulary-v2 D1). Unwrap one level for `FromCompile`, then read the tag;
    // a top-level pool refusal reads its own tag directly.
    let (outer, payload) = match tag(v)? {
        "FromCompile" => {
            let inner = &v["value"];
            (tag(inner)?, &inner["value"])
        }
        other => (other, &v["value"]),
    };
    Ok(match outer {
        "TenantAbsent" => RawRefusal::TenantAbsent {
            construct: cname(field(payload, "construct")?)?,
            tenant: tname(field(payload, "tenant")?)?,
        },
        "PoolWithoutRestricted" => RawRefusal::PoolWithoutRestricted {
            tenant: tname(field(payload, "tenant")?)?,
            pool: tname(field(payload, "pool")?)?,
        },
        "RestrictedWithoutPool" => RawRefusal::RestrictedWithoutPool {
            tenant: tname(field(payload, "tenant")?)?,
        },
        "ReservedKernel" => RawRefusal::ReservedKernel {
            name: tname(field(payload, "name")?)?,
        },
        "DuplicateName" => RawRefusal::DuplicateName {
            name: cname(field(payload, "name")?)?,
        },
        "LinkSelfLoop" => RawRefusal::LinkSelfLoop {
            link: cname(field(payload, "link")?)?,
            tenant: tname(field(payload, "tenant")?)?,
        },
        "RawMemberUnresolved" => RawRefusal::RawMemberUnresolved {
            fabric: cname(field(payload, "fabric")?)?,
            member: cname(field(payload, "member")?)?,
        },
        "UnknownExtraFamily" => RawRefusal::UnknownExtraFamily {
            tenant: tname(field(payload, "tenant")?)?,
            family: text(field(payload, "family")?)?,
        },
        "RenamedFromDeclared" => RawRefusal::RenamedFromDeclared {
            construct: cname(field(payload, "construct")?)?,
            from: cname(field(payload, "from")?)?,
        },
        t => return Err(format!("unknown raw refusal `{t}`")),
    })
}

fn verdict(v: &Value) -> Result<ModelVerdict, String> {
    match tag(v)? {
        "ParseOk" => Ok(ModelVerdict::Ok(Box::new(intent(&v["value"])?))),
        "ParseRefused" => Ok(ModelVerdict::Refused(
            set_items(&v["value"])?
                .iter()
                .map(raw_refusal)
                .collect::<Result<_, _>>()?,
        )),
        t => Err(format!("unknown Parsed `{t}`")),
    }
}

/// Parses a frozen raw trace into its [`RawCase`].
///
/// # Errors
///
/// Returns a description of the first structural mismatch — a trace not produced by
/// `models/intent/raw_replay.qnt` (missing `raw`/`verdict` vars, or a tag/shape the
/// mapping layer does not recognise).
pub fn parse_raw_case(json: &str) -> Result<RawCase, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let state = field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .first()
        .ok_or("trace has no first state")?;
    Ok(RawCase {
        raw: raw_intent(field(state, "raw")?)?,
        verdict: verdict(field(state, "verdict")?)?,
    })
}

/// Normalizes a Rust [`Intent`] for order-insensitive comparison against the model's:
/// tenants and fabrics come from unordered maps on both sides (Rust's `BTreeMap`, the
/// model's `keys()` fold), so they are sorted here. Ports and links are keyed maps now
/// too (task 3.3d), and both sides linearize them in NAME order — the model's `toIntent`
/// sorts by name, and the emitted TOML lists `[port.<name>]`/`[link.<name>]` tables in
/// this `BTreeMap`'s name order, which `dpaa2-config` preserves — so they already agree
/// without sorting. crypto is a declaration-ordered array and extras a `BTreeSet`, so
/// those need no sorting either.
pub fn normalize(i: &mut Intent) {
    i.tenants.sort();
    i.fabrics.sort();
}

/// The near-miss kinds present in a verdict (the refused arm; empty when accepted).
#[must_use]
pub fn kinds(v: &ModelVerdict) -> BTreeSet<Kind> {
    match v {
        ModelVerdict::Ok(_) => BTreeSet::new(),
        ModelVerdict::Refused(rs) => rs.iter().map(RawRefusal::kind).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tenant(pool: &str) -> RawTenant {
        RawTenant {
            dataplane: "userspace-poll".into(),
            max_cores: 8,
            isolation: "isolated".into(),
            pool: pool.into(),
            from: "".into(),
        }
    }

    #[test]
    fn emits_the_schema_envelope_and_omits_empty_optional_slots() {
        let mut raw = RawIntent::default();
        raw.tenants.insert("router".into(), tenant(""));
        raw.ports.insert(
            "wan0".into(),
            RawPort {
                dpmac: 7,
                rate: 10000,
                tenant: "".into(),
                from: "".into(),
            },
        );
        let doc = to_toml(&raw);
        assert!(doc.starts_with("[intent]\nschema = 1\n"), "{doc}");
        assert!(doc.contains("[tenant.router]"), "{doc}");
        assert!(doc.contains("[port.wan0]"), "{doc}");
        assert!(doc.contains("dpmac = \"dpmac.7\""), "{doc}");
        // Empty pool and empty port tenant are omitted, not emitted as "".
        assert!(!doc.contains("pool ="), "{doc}");
        assert!(!doc.contains("tenant ="), "{doc}");
    }

    #[test]
    fn emits_a_cross_family_name_collision_verbatim() {
        // The only DuplicateName near-miss left once ports/links are keyed (task 3.3d):
        // a port and a fabric of one name serialize to exactly that TOML — separate
        // `[port.p1]` and `[fabric.p1]` tables the parser refuses cross-family; the
        // emitter never fixes it up.
        let mut raw = RawIntent::default();
        raw.ports.insert(
            "p1".into(),
            RawPort {
                dpmac: 7,
                rate: 10000,
                tenant: "".into(),
                from: "".into(),
            },
        );
        raw.fabrics.insert(
            "p1".into(),
            RawFabric {
                switching: "hardware".into(),
                forwarded_by: "kernel".into(),
                members: vec![],
                from: "".into(),
            },
        );
        let doc = to_toml(&raw);
        assert!(doc.contains("[port.p1]"), "{doc}");
        assert!(doc.contains("[fabric.p1]"), "{doc}");
    }

    #[test]
    fn matcher_anchors_each_variant_on_its_parse_rs_spelling() {
        // The exact spellings from crates/dpaa2-config/src/parse.rs.
        let cases: &[(RawRefusal, &str, bool)] = &[
            (
                RawRefusal::ReservedKernel {
                    name: "kernel".into(),
                },
                "`[tenant.kernel]` declares the reserved tenant name `kernel`, which is reserved",
                true,
            ),
            (
                RawRefusal::DuplicateName { name: "p1".into() },
                "duplicate construct name `p1`",
                true,
            ),
            (
                RawRefusal::LinkSelfLoop {
                    link: "l1".into(),
                    tenant: "c1".into(),
                },
                "link `l1` names the same tenant `c1` at both ends; a link joins two distinct tenants",
                true,
            ),
            (
                RawRefusal::RawMemberUnresolved {
                    fabric: "f1".into(),
                    member: "ghost".into(),
                },
                "fabric `f1` names member `ghost`, which is not a declared port, tenant, or fabric",
                true,
            ),
            (
                RawRefusal::UnknownExtraFamily {
                    tenant: "c1".into(),
                    family: "dpwidget".into(),
                },
                "`[extra.c1]` names unknown family `dpwidget`",
                true,
            ),
            (
                RawRefusal::TenantAbsent {
                    construct: "p1".into(),
                    tenant: "ghost".into(),
                },
                "port `p1` names tenant `ghost`, which is not declared",
                true,
            ),
            (
                RawRefusal::PoolWithoutRestricted {
                    tenant: "stray".into(),
                    pool: "restr".into(),
                },
                "tenant `stray` names a `pool` (`restr`) but is not `restricted`; a pool is legal only on a restricted tenant",
                true,
            ),
            (
                RawRefusal::RestrictedWithoutPool {
                    tenant: "restr".into(),
                },
                "tenant `restr` is `restricted` but names no `pool` holder",
                true,
            ),
            (
                RawRefusal::RenamedFromDeclared {
                    construct: "e0".into(),
                    from: "wan0".into(),
                },
                "`[port.e0]` declares `renamed = { from = \"wan0\" }` but `wan0` is currently \
                 declared and not itself renamed; a construct cannot be claimed twice",
                true,
            ),
            // A member-unresolved message must NOT read as a TenantAbsent ("not a
            // declared" is disjoint from "not declared").
            (
                RawRefusal::TenantAbsent {
                    construct: "f1".into(),
                    tenant: "ghost".into(),
                },
                "fabric `f1` names member `ghost`, which is not a declared port, tenant, or fabric",
                false,
            ),
        ];
        for (r, msg, want) in cases {
            assert_eq!(r.matches_error(msg), *want, "{r:?} vs `{msg}`");
        }
    }
}
