//! Reader and comparator for frozen intent-model ITF traces
//! (`models/intent/traces/*.itf.json`, emitted by `models/intent/replay.qnt`).
//!
//! quint-connect evaluation (task 3.2, design D9): Informal's `quint-connect`
//! MBT crate was weighed against this hand-rolled reader and NOT adopted. It is
//! a *step-driver* framework — a `Driver::step(&mut self, &Step)` trait that
//! walks a system-under-test through a model's transitions — whereas an intent
//! trace is one state feeding a *pure* `compile`, so its paradigm does not fit;
//! and it maps trace states with `serde::Deserialize`, which `dpaa2-api` refuses
//! to carry (design D10), so adopting it would need serde mirror types in verify
//! plus ~7 new dependencies (quint-connect, -macros, itf, tempfile, colored,
//! rand, similar) — code and deps ADDED, none retired. The few-dependencies
//! tenet decides against it (design D9). Revisit if the intent model grows a
//! stepped state machine (then a step-driver earns its keep) or if `dpaa2-api`
//! ever gains serde.
//!
//! Structural comparison (design D9): the model is the oracle. Each trace's
//! frozen `intent`/`inv` are parsed into the Rust [`Intent`]/[`Inventory`] and
//! re-compiled by the Rust [`compile`]; the frozen `outcome` is parsed into the
//! same [`Outcome`] projection and the two are asserted equal — objects (key,
//! container, attributes, provenance key), edges (both ends and provenance key),
//! emission order, the provenance DAG, warnings, and (the refused arm) the
//! refusal set. `PlannedObject`/`Edge` have no public constructor (witness-only,
//! design D6), so both sides project to tuples of their *public* leaf types
//! ([`ObjectKey`], [`Container`], [`Attributes`], [`AttachPoint`], [`ProvenanceKey`], [`ProvenanceNode`]),
//! which carry the derived equality this diff rides on.
//!
//! The one mapping layer (the only place the two encodings are reconciled):
//! - `Compiled::Ok{plan,warnings}` / `Refused(set)` ⇒ Rust `Result<_, set>`;
//! - refusal tags `ReservedAnchor`/`ForeignAnchor` ⇒ [`Refusal::Reserved`] /
//!   [`Refusal::Foreign`] (refuse.qnt DEVIATION; the accepted ADR-0013 §5
//!   spelling);
//! - `#bigint` strings ⇒ `i64`/`u32` (`itf::int64`/`itf::num`);
//! - a refusal's `Set[str]` payload (`DoubleClaimed.constructs`) ⇒ a sorted
//!   `Vec<ConstructName>`, matching the `BTreeSet`-derived order the Rust compiler emits.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use dpaa2_api::{
    ALL_FAMILIES, AttachPoint, Attributes, Availability, Ceiling, Compiled, ConstructName,
    Container, Crypto, Dataplane, DpmacId, DpmacLinkType, DpmacOffer, EthInterface, Extra, Fabric,
    Family, Intent, Inventory, Isolation, Link, MacMode, Measurement, Member, ObjectKey,
    Permission, Port, ProvenanceKey, ProvenanceNode, Refusal, Switching, Tenant, TenantName,
    Warning, compile,
};

use crate::itf::{int64, num, tag};

// ---- the comparable projection ----

/// A planned object as its public leaf tuple: `(key, container, attributes, provenance)`.
type PlainObj = (ObjectKey, Container, Attributes, ProvenanceKey);
/// A connection as its public leaf tuple: `(end a, end b, provenance)`.
type PlainEdge = (AttachPoint, AttachPoint, ProvenanceKey);

/// The accepted arm's plan, projected to comparable public leaves plus warnings.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlainOk {
    /// The planned objects.
    pub objs: BTreeSet<PlainObj>,
    /// The connection edges.
    pub edges: BTreeSet<PlainEdge>,
    /// The emission order.
    pub order: Vec<ObjectKey>,
    /// The provenance DAG.
    pub provenance: BTreeMap<ProvenanceKey, ProvenanceNode>,
    /// The non-fatal warnings.
    pub warnings: BTreeSet<Warning>,
}

/// The comparable projection of a `compile` result: the plan (Ok) or the
/// complete refusal set (Err). Both the Rust compiler's output and the model's
/// frozen `outcome` land here so a `==` is the whole conformance check.
pub type Outcome = Result<PlainOk, BTreeSet<Refusal>>;

/// One replay case: the two `compile` inputs and the model's frozen outcome.
pub struct ReplayCase {
    /// The parsed intent (`compile`'s first input).
    pub intent: Intent,
    /// The parsed inventory (`compile`'s second input).
    pub inv: Inventory,
    /// The model's own `compile` outcome, projected for comparison (the oracle).
    pub outcome: Outcome,
}

impl ReplayCase {
    /// Runs the Rust [`compile`] over this case's inputs and projects it into the
    /// comparable [`Outcome`], to assert against the model's frozen `outcome`.
    ///
    /// # Errors
    ///
    /// The `Err` arm is not a failure: it carries `compile`'s complete refusal
    /// set, the refused half of the [`Outcome`] the caller compares.
    pub fn rust_outcome(&self) -> Outcome {
        project(&compile(&self.intent, &self.inv))
    }
}

/// Projects a Rust `compile` result into the comparable [`Outcome`], reading the
/// witness-built `PlannedObject`/`Edge` through their public accessors.
fn project(r: &Result<Compiled, BTreeSet<Refusal>>) -> Outcome {
    match r {
        Ok(c) => Ok(PlainOk {
            objs: c
                .plan
                .objects
                .iter()
                .map(|o| {
                    (
                        o.key().clone(),
                        o.container().clone(),
                        o.attributes().clone(),
                        o.provenance().clone(),
                    )
                })
                .collect(),
            edges: c
                .plan
                .edges
                .iter()
                .map(|e| (e.a().clone(), e.b().clone(), e.provenance().clone()))
                .collect(),
            order: c.plan.order.clone(),
            provenance: c.plan.provenance.clone(),
            warnings: c.warnings.clone(),
        }),
        Err(rs) => Err(rs.clone()),
    }
}

// ---- generic ITF shape helpers (atop itf.rs's num/int64/tag) ----

fn field<'a>(v: &'a Value, name: &str) -> Result<&'a Value, String> {
    let f = &v[name];
    if f.is_null() {
        return Err(format!("missing field `{name}` in {v}"));
    }
    Ok(f)
}

fn text(v: &Value) -> Result<String, String> {
    v.as_str()
        .ok_or_else(|| format!("not a string: {v}"))
        .map(str::to_owned)
}

/// A JSON string as a [`TenantName`] — the boundary conversion for a name slot the
/// model spells as a bare `str` but the Rust intent types distinguish.
fn tname(v: &Value) -> Result<TenantName, String> {
    Ok(text(v)?.into())
}

/// A JSON string as a [`ConstructName`] (a port/link/fabric or any construct name).
fn cname(v: &Value) -> Result<ConstructName, String> {
    Ok(text(v)?.into())
}

fn flag(v: &Value) -> Result<bool, String> {
    v.as_bool().ok_or_else(|| format!("not a bool: {v}"))
}

/// The elements of an ITF `#set`.
fn set_items(v: &Value) -> Result<&Vec<Value>, String> {
    v["#set"]
        .as_array()
        .ok_or_else(|| format!("not a #set: {v}"))
}

/// The `[key, value]` pairs of an ITF `#map`.
fn map_items(v: &Value) -> Result<&Vec<Value>, String> {
    v["#map"]
        .as_array()
        .ok_or_else(|| format!("not a #map: {v}"))
}

fn list_items(v: &Value) -> Result<&Vec<Value>, String> {
    v.as_array().ok_or_else(|| format!("not a list: {v}"))
}

/// The [`Family`] whose ITF constructor tag this is (the intent side of the
/// `family_from_variant` idiom the retro adapter uses).
fn family(v: &Value) -> Result<Family, String> {
    let t = tag(v)?;
    ALL_FAMILIES
        .into_iter()
        .find(|f| f.variant_name() == t)
        .ok_or_else(|| format!("unknown family tag `{t}`"))
}

// ---- intent ----

fn dataplane(v: &Value) -> Result<Dataplane, String> {
    match tag(v)? {
        "KernelNetlink" => Ok(Dataplane::KernelNetlink),
        "UserspacePoll" => Ok(Dataplane::UserspacePoll),
        "UserspaceEvent" => Ok(Dataplane::UserspaceEvent),
        t => Err(format!("unknown dataplane `{t}`")),
    }
}

fn isolation(v: &Value) -> Result<Isolation, String> {
    match tag(v)? {
        "Public" => Ok(Isolation::Public),
        "Restricted" => Ok(Isolation::Restricted),
        "Isolated" => Ok(Isolation::Isolated),
        t => Err(format!("unknown isolation `{t}`")),
    }
}

fn switching(v: &Value) -> Result<Switching, String> {
    match tag(v)? {
        "Hardware" => Ok(Switching::Hardware),
        "Software" => Ok(Switching::Software),
        t => Err(format!("unknown switching `{t}`")),
    }
}

fn member(v: &Value) -> Result<Member, String> {
    let p = &v["value"];
    match tag(v)? {
        "MPort" => Ok(Member::Port(cname(p)?)),
        "MTenant" => Ok(Member::Tenant(tname(p)?)),
        "MFabric" => Ok(Member::Fabric(cname(p)?)),
        t => Err(format!("unknown member `{t}`")),
    }
}

fn tenant(v: &Value) -> Result<Tenant, String> {
    Ok(Tenant {
        name: tname(field(v, "name")?)?,
        dataplane: dataplane(field(v, "dataplane")?)?,
        max_cores: int64(field(v, "maxCores")?)?,
        isolation: isolation(field(v, "isolation")?)?,
        pool: tname(field(v, "pool")?)?,
    })
}

fn port(v: &Value) -> Result<Port, String> {
    Ok(Port {
        name: cname(field(v, "name")?)?,
        dpmac: DpmacId::new(num(field(v, "dpmac")?)?),
        rate: int64(field(v, "rate")?)?,
        tenant: tname(field(v, "tenant")?)?,
        // `mac`/`mac_mode` are actuation-only facts the derivation never reads; the
        // Quint model omits them, so the ITF trace carries none — default them.
        mac: None,
        mac_mode: MacMode::default(),
    })
}

fn link(v: &Value) -> Result<Link, String> {
    Ok(Link {
        name: cname(field(v, "name")?)?,
        interface_a: tname(field(v, "interfaceA")?)?,
        interface_b: tname(field(v, "interfaceB")?)?,
    })
}

fn fabric(v: &Value) -> Result<Fabric, String> {
    Ok(Fabric {
        name: cname(field(v, "name")?)?,
        switching: switching(field(v, "switching")?)?,
        forwarded_by: tname(field(v, "forwardedBy")?)?,
        members: list_items(field(v, "members")?)?
            .iter()
            .map(member)
            .collect::<Result<_, _>>()?,
    })
}

fn crypto(v: &Value) -> Result<Crypto, String> {
    Ok(Crypto {
        tenant: tname(field(v, "tenant")?)?,
        flows: int64(field(v, "flows")?)?,
    })
}

fn extra(v: &Value) -> Result<Extra, String> {
    Ok(Extra {
        tenant: tname(field(v, "tenant")?)?,
        family: family(field(v, "family")?)?,
        count: int64(field(v, "count")?)?,
    })
}

fn intent(v: &Value) -> Result<Intent, String> {
    Ok(Intent {
        tenants: list_items(field(v, "tenants")?)?
            .iter()
            .map(tenant)
            .collect::<Result<_, _>>()?,
        ports: list_items(field(v, "ports")?)?
            .iter()
            .map(port)
            .collect::<Result<_, _>>()?,
        links: list_items(field(v, "links")?)?
            .iter()
            .map(link)
            .collect::<Result<_, _>>()?,
        fabrics: list_items(field(v, "fabrics")?)?
            .iter()
            .map(fabric)
            .collect::<Result<_, _>>()?,
        crypto: list_items(field(v, "crypto")?)?
            .iter()
            .map(crypto)
            .collect::<Result<_, _>>()?,
        extras: set_items(field(v, "extras")?)?
            .iter()
            .map(extra)
            .collect::<Result<_, _>>()?,
    })
}

// ---- inventory ----

fn eth_if(v: &Value) -> Result<EthInterface, String> {
    match tag(v)? {
        "XFI" => Ok(EthInterface::Xfi),
        "CAUI" => Ok(EthInterface::Caui),
        "RGMII" => Ok(EthInterface::Rgmii),
        t => Err(format!("unknown ethIf `{t}`")),
    }
}

fn dpmac_link_type(v: &Value) -> Result<DpmacLinkType, String> {
    match tag(v)? {
        "LinkNone" => Ok(DpmacLinkType::None),
        "LinkFixed" => Ok(DpmacLinkType::Fixed),
        "LinkPhy" => Ok(DpmacLinkType::Phy),
        "LinkBackplane" => Ok(DpmacLinkType::Backplane),
        t => Err(format!("unknown linkType `{t}`")),
    }
}

fn availability(v: &Value) -> Result<Availability, String> {
    match tag(v)? {
        "Free" => Ok(Availability::Free),
        "Reserved" => Ok(Availability::Reserved(text(&v["value"])?)),
        "Foreign" => Ok(Availability::Foreign(text(&v["value"])?)),
        t => Err(format!("unknown availability `{t}`")),
    }
}

fn dpmac_offer(v: &Value) -> Result<DpmacOffer, String> {
    Ok(DpmacOffer {
        id: DpmacId::new(num(field(v, "id")?)?),
        max_rate: int64(field(v, "maxRate")?)?,
        eth_if: eth_if(field(v, "ethIf")?)?,
        link_type: dpmac_link_type(field(v, "linkType")?)?,
        avail: availability(field(v, "avail")?)?,
    })
}

fn ceiling(v: &Value) -> Result<Ceiling, String> {
    match tag(v)? {
        "Counted" => Ok(Ceiling::Counted(int64(&v["value"])?)),
        "Observed" => Ok(Ceiling::Observed {
            n: int64(field(&v["value"], "n")?)?,
            provenance: text(field(&v["value"], "provenance")?)?,
        }),
        "Unknown" => Ok(Ceiling::Unknown),
        t => Err(format!("unknown ceiling `{t}`")),
    }
}

/// An ITF-encoded `ObjId` `{ fam, num }` (the inventory `foreign` map's key).
fn obj_id(v: &Value) -> Result<(Family, u32), String> {
    Ok((family(field(v, "fam")?)?, num(field(v, "num")?)?))
}

fn inventory(v: &Value) -> Result<Inventory, String> {
    let mut dpmacs = BTreeMap::new();
    for pair in map_items(field(v, "dpmacs")?)? {
        let offer = dpmac_offer(&pair[1])?;
        dpmacs.insert(DpmacId::new(num(&pair[0])?), offer);
    }
    let mut foreign = BTreeMap::new();
    for pair in map_items(field(v, "foreign")?)? {
        foreign.insert(obj_id(&pair[0])?, text(&pair[1])?);
    }
    let mut ceilings = BTreeMap::new();
    for pair in map_items(field(v, "ceilings")?)? {
        ceilings.insert(family(&pair[0])?, ceiling(&pair[1])?);
    }
    Ok(Inventory {
        cpus: num(field(v, "cpus")?)?,
        dpmacs,
        foreign,
        ceilings,
    })
}

// ---- the plan (outcome, accepted arm) ----

fn perm(v: &Value) -> Result<Permission, String> {
    match tag(v)? {
        "Spawn" => Ok(Permission::Spawn),
        "Alloc" => Ok(Permission::Alloc),
        "ObjCreate" => Ok(Permission::ObjCreate),
        "IrqCfg" => Ok(Permission::IrqCfg),
        "TopologyChanges" => Ok(Permission::TopologyChanges),
        t => Err(format!("unknown perm `{t}`")),
    }
}

fn key(v: &Value) -> Result<ObjectKey, String> {
    Ok(ObjectKey::new(
        text(field(v, "tenant")?)?,
        family(field(v, "family")?)?,
        num(field(v, "ordinal")?)?,
    ))
}

fn container(v: &Value) -> Result<Container, String> {
    match tag(v)? {
        "Root" => Ok(Container::Root),
        "Child" => Ok(Container::Child(tname(&v["value"])?)),
        t => Err(format!("unknown container `{t}`")),
    }
}

fn attributes(v: &Value) -> Result<Attributes, String> {
    let p = &v["value"];
    match tag(v)? {
        "Unsized" => Ok(Attributes::Unsized),
        "DpniAttrs" => Ok(Attributes::Dpni {
            num_queues: num(field(p, "numQueues")?)?,
        }),
        "DpseciAttrs" => Ok(Attributes::Dpseci {
            num_queues: num(field(p, "numQueues")?)?,
            has_cg: flag(field(p, "hasCg")?)?,
        }),
        "DpswAttrs" => Ok(Attributes::Dpsw {
            num_ifs: num(field(p, "numIfs")?)?,
            max_fdbs: num(field(p, "maxFdbs")?)?,
            per_fdb_flooding: flag(field(p, "perFdbFlooding")?)?,
            per_fdb_broadcast: flag(field(p, "perFdbBroadcast")?)?,
            ctrl_if: flag(field(p, "ctrlIf")?)?,
        }),
        "DprcAttrs" => Ok(Attributes::Dprc {
            options: set_items(field(p, "options")?)?
                .iter()
                .map(perm)
                .collect::<Result<_, _>>()?,
        }),
        t => Err(format!("unknown attributes `{t}`")),
    }
}

fn prov_key(v: &Value) -> Result<ProvenanceKey, String> {
    Ok(ProvenanceKey::new(
        text(field(v, "tenant")?)?,
        text(field(v, "rule")?)?,
        text(field(v, "construct")?)?,
    ))
}

fn end(v: &Value) -> Result<AttachPoint, String> {
    match tag(v)? {
        "ObjectAttach" => Ok(AttachPoint::object(
            key(field(&v["value"], "key")?)?,
            num(field(&v["value"], "port")?)?,
        )),
        "MacAttach" => Ok(AttachPoint::mac(DpmacId::new(num(&v["value"])?))),
        t => Err(format!("unknown end `{t}`")),
    }
}

fn mark(v: &Value) -> Result<Measurement, String> {
    match tag(v)? {
        "Measured" => Ok(Measurement::Measured),
        "Unmeasured" => Ok(Measurement::Unmeasured),
        t => Err(format!("unknown mark `{t}`")),
    }
}

fn opt_int(v: &Value) -> Result<Option<i64>, String> {
    match tag(v)? {
        "None" => Ok(None),
        "Some" => Ok(Some(int64(&v["value"])?)),
        t => Err(format!("unknown Option `{t}`")),
    }
}

fn prov_node(v: &Value) -> Result<ProvenanceNode, String> {
    Ok(ProvenanceNode {
        rule: text(field(v, "rule")?)?.into(),
        anchor: text(field(v, "anchor")?)?,
        mark: mark(field(v, "mark")?)?,
        request: int64(field(v, "request")?)?,
        extra: opt_int(field(v, "extra")?)?,
        value: int64(field(v, "value")?)?,
        inputs: set_items(field(v, "inputs")?)?
            .iter()
            .map(prov_key)
            .collect::<Result<_, _>>()?,
        constructs: set_items(field(v, "constructs")?)?
            .iter()
            .map(cname)
            .collect::<Result<_, _>>()?,
    })
}

fn plan_obj(v: &Value) -> Result<PlainObj, String> {
    Ok((
        key(field(v, "key")?)?,
        container(field(v, "container")?)?,
        attributes(field(v, "attributes")?)?,
        prov_key(field(v, "provenance")?)?,
    ))
}

fn edge(v: &Value) -> Result<PlainEdge, String> {
    Ok((
        end(field(v, "a")?)?,
        end(field(v, "b")?)?,
        prov_key(field(v, "provenance")?)?,
    ))
}

fn warning(v: &Value) -> Result<Warning, String> {
    let p = &v["value"];
    match tag(v)? {
        "UnknownCeiling" => Ok(Warning::UnknownCeiling {
            family: family(field(p, "family")?)?,
            needed: int64(field(p, "needed")?)?,
        }),
        // `rates` is a model `Set[int]`; sorted matches the `BTreeSet`-derived
        // order the Rust compiler emits (the mapping-layer note). `UnknownRateClass`
        // below carries a `List[int]` instead, kept in declaration order.
        "UnmeasuredCombination" => Ok(Warning::UnmeasuredCombination {
            tenant: tname(field(p, "tenant")?)?,
            rates: ints_sorted(field(p, "rates")?)?,
        }),
        t => Err(format!("unknown warning `{t}`")),
    }
}

fn plain_ok(v: &Value) -> Result<PlainOk, String> {
    let plan = field(v, "plan")?;
    let mut provenance = BTreeMap::new();
    for pair in map_items(field(plan, "provenance")?)? {
        provenance.insert(prov_key(&pair[0])?, prov_node(&pair[1])?);
    }
    Ok(PlainOk {
        objs: set_items(field(plan, "objs")?)?
            .iter()
            .map(plan_obj)
            .collect::<Result<_, _>>()?,
        edges: set_items(field(plan, "edges")?)?
            .iter()
            .map(edge)
            .collect::<Result<_, _>>()?,
        order: list_items(field(plan, "order")?)?
            .iter()
            .map(key)
            .collect::<Result<_, _>>()?,
        provenance,
        warnings: set_items(field(v, "warnings")?)?
            .iter()
            .map(warning)
            .collect::<Result<_, _>>()?,
    })
}

// ---- refusals (outcome, refused arm) ----

fn constructs_sorted(v: &Value) -> Result<Vec<ConstructName>, String> {
    // A model `Set[str]` payload; a BTreeSet gives the sorted order the Rust
    // compiler emits from its own `BTreeSet` (the mapping-layer note).
    let s: BTreeSet<ConstructName> = set_items(v)?.iter().map(cname).collect::<Result<_, _>>()?;
    Ok(s.into_iter().collect())
}

fn ints_list(v: &Value) -> Result<Vec<i64>, String> {
    list_items(v)?.iter().map(int64).collect()
}

fn ints_sorted(v: &Value) -> Result<Vec<i64>, String> {
    // A model `Set[int]` payload; a BTreeSet gives the sorted order the Rust
    // compiler emits from its own `BTreeSet`.
    let s: BTreeSet<i64> = set_items(v)?.iter().map(int64).collect::<Result<_, _>>()?;
    Ok(s.into_iter().collect())
}

#[allow(clippy::too_many_lines)]
fn refusal(v: &Value) -> Result<Refusal, String> {
    let p = &v["value"];
    Ok(match tag(v)? {
        "TenantAbsent" => Refusal::TenantAbsent {
            construct: cname(field(p, "construct")?)?,
            tenant: tname(field(p, "tenant")?)?,
        },
        "MemberUnresolved" => Refusal::MemberUnresolved {
            fabric: cname(field(p, "fabric")?)?,
            member: member(field(p, "member")?)?,
        },
        "SelfMember" => Refusal::SelfMember {
            fabric: cname(field(p, "fabric")?)?,
            member: member(field(p, "member")?)?,
        },
        "Unanchored" => Refusal::Unanchored {
            port: cname(field(p, "port")?)?,
            dpmac: DpmacId::new(num(field(p, "dpmac")?)?),
        },
        // refuse.qnt DEVIATION: model `ReservedAnchor`/`ForeignAnchor` carry the
        // accepted ADR-0013 §5 spelling here (the one mapping-layer rename).
        "ReservedAnchor" => Refusal::Reserved {
            port: cname(field(p, "port")?)?,
            dpmac: DpmacId::new(num(field(p, "dpmac")?)?),
            why: text(field(p, "why")?)?,
        },
        "ForeignAnchor" => Refusal::Foreign {
            port: cname(field(p, "port")?)?,
            dpmac: DpmacId::new(num(field(p, "dpmac")?)?),
            owner: text(field(p, "owner")?)?,
        },
        "DoubleClaimed" => Refusal::DoubleClaimed {
            dpmac: DpmacId::new(num(field(p, "dpmac")?)?),
            constructs: constructs_sorted(field(p, "constructs")?)?,
        },
        "OverRate" => Refusal::OverRate {
            port: cname(field(p, "port")?)?,
            rate: int64(field(p, "rate")?)?,
            max_rate: int64(field(p, "maxRate")?)?,
        },
        "FabricNotKernelForwarded" => Refusal::FabricNotKernelForwarded {
            fabric: cname(field(p, "fabric")?)?,
            forwarded_by: tname(field(p, "forwardedBy")?)?,
        },
        "PortTenantMismatch" => Refusal::PortTenantMismatch {
            fabric: cname(field(p, "fabric")?)?,
            port: cname(field(p, "port")?)?,
            tenant: tname(field(p, "tenant")?)?,
        },
        "UnsupportedEdge" => Refusal::UnsupportedEdge {
            fabric: cname(field(p, "fabric")?)?,
            member: cname(field(p, "member")?)?,
        },
        "UnknownRateClass" => Refusal::UnknownRateClass {
            tenant: tname(field(p, "tenant")?)?,
            rates: ints_list(field(p, "rates")?)?,
        },
        "CoreBudgetExceeded" => Refusal::CoreBudgetExceeded {
            tenant: tname(field(p, "tenant")?)?,
            t: int64(field(p, "t")?)?,
            max_cores: int64(field(p, "maxCores")?)?,
        },
        "ExtraNotCompanion" => Refusal::ExtraNotCompanion {
            tenant: tname(field(p, "tenant")?)?,
            family: family(field(p, "family")?)?,
        },
        "ExtraNotPositive" => Refusal::ExtraNotPositive {
            tenant: tname(field(p, "tenant")?)?,
            family: family(field(p, "family")?)?,
            count: int64(field(p, "count")?)?,
        },
        "CryptoFlowsNotPositive" => Refusal::CryptoFlowsNotPositive {
            tenant: tname(field(p, "tenant")?)?,
            ordinal: num(field(p, "ordinal")?)?,
            flows: int64(field(p, "flows")?)?,
        },
        "CryptoFlowsOverDevice" => Refusal::CryptoFlowsOverDevice {
            tenant: tname(field(p, "tenant")?)?,
            ordinal: num(field(p, "ordinal")?)?,
            flows: int64(field(p, "flows")?)?,
            max_flows: int64(field(p, "maxFlows")?)?,
        },
        "Infeasible" => Refusal::Infeasible {
            family: family(field(p, "family")?)?,
            needed: int64(field(p, "needed")?)?,
            available: int64(field(p, "available")?)?,
        },
        "UnpricedDataplane" => Refusal::UnpricedDataplane {
            tenant: tname(field(p, "tenant")?)?,
            dataplane: dataplane(field(p, "dataplane")?)?,
        },
        "PoolWithoutRestricted" => Refusal::PoolWithoutRestricted {
            tenant: tname(field(p, "tenant")?)?,
            pool: tname(field(p, "pool")?)?,
        },
        "RestrictedWithoutPool" => Refusal::RestrictedWithoutPool {
            tenant: tname(field(p, "tenant")?)?,
        },
        "HolderNotPublic" => Refusal::HolderNotPublic {
            tenant: tname(field(p, "tenant")?)?,
            holder: tname(field(p, "holder")?)?,
        },
        "PoolChain" => Refusal::PoolChain {
            tenant: tname(field(p, "tenant")?)?,
            holder: tname(field(p, "holder")?)?,
        },
        "PoolDataplaneMismatch" => Refusal::PoolDataplaneMismatch {
            tenant: tname(field(p, "tenant")?)?,
            drawer: dataplane(field(p, "drawer")?)?,
            holder: dataplane(field(p, "holder")?)?,
        },
        t => return Err(format!("unknown refusal `{t}`")),
    })
}

fn outcome(v: &Value) -> Result<Outcome, String> {
    match tag(v)? {
        "Ok" => Ok(Ok(plain_ok(&v["value"])?)),
        "Refused" => Ok(Err(set_items(&v["value"])?
            .iter()
            .map(refusal)
            .collect::<Result<_, _>>()?)),
        t => Err(format!("unknown Compiled `{t}`")),
    }
}

/// Parses a frozen intent trace into its [`ReplayCase`].
///
/// # Errors
///
/// Returns a description of the first structural mismatch — a trace not produced
/// by `models/intent/replay.qnt` (missing `intent`/`inv`/`outcome` vars, or a
/// tag/shape the mapping layer does not recognise).
pub fn parse_case(json: &str) -> Result<ReplayCase, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let state = &field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .first()
        .ok_or("trace has no first state")?;
    Ok(ReplayCase {
        intent: intent(field(state, "intent")?)?,
        inv: inventory(field(state, "inv")?)?,
        outcome: outcome(field(state, "outcome")?)?,
    })
}
