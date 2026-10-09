//! Reader for frozen link-lifecycle ITF traces (`models/traces/families/link_lifecycle/*.itf.json`,
//! emitted by `models/families/link_lifecycle.qnt` module `link_lifecycle` via `pnpm model:freeze-link`).
//!
//! The dpni↔dpni wire twin of the dpmac/dpseci readers ([`crate::intent::dpmac_itf`],
//! [`crate::intent::dpseci_itf`]): the surface walks ONE state var — `world: World` — whose
//! record fields are the wire-typestate vocabulary the Rust [`dpaa2_api::plan::connect`] and
//! [`dpaa2_api::families::dprc`] surfaces reason at, so this reader reduces each frozen `world`
//! to a `LinkWorld` and `tests/link_replay.rs` drives the real judgments through the same
//! states. The transition between two states is not named in the ITF; the replayer infers it
//! from the field delta, mirroring `link_lifecycle`'s action set (cross-dprc-links design D7).
//!
//! A `.fail()` directed step (the cardinality-one guard refusal) froze no variable values (only
//! `#meta`); it decodes to `LinkStep::Refused`, the disabled-guard sentinel the replayer witnesses
//! against the prior world.
//!
//! The reduction keeps the slice the Rust surface judges and nothing else: the `World.core`
//! carries a full `CoreState`, but only the dpni endpoints (their container and bus visibility),
//! the standing dpni↔dpni edges, the wire phase, the container-bind facet, and the
//! obligation/residue carriers are decoded. Unknown/missing vocabulary — a non-Dpni object key,
//! an unknown phase or residue tag — fails loudly, which IS the freeze-freshness mechanism: a
//! regenerated trace whose shape the Rust surface no longer reproduces fails here, not silently.
//!
//! Mapping (the only place the two encodings are reconciled; the sum tags are the model's names
//! verbatim): the model's `WirePhase`/`ContainerBind`/`WireResidue` ⇒ the
//! `WirePhase`/`ContainerBind`/`LinkResidue` decode enums of the same cases; an `ObjId` ⇒ its ordinal (a dpni/dprc index);
//! a `conns` endpoint pair ⇒ the two dpni ordinals it joins.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::itf::{field, num, set_items, state_var, tag};

/// The four wire phases (`link_lifecycle.qnt` `type WirePhase`): the spec-level typestate the
/// wire advances through, decoded as a proper sum (ADR-0002 §3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WirePhase {
    /// Both dpni ends exist in their containers, no edge (`EndsExist`).
    EndsExist,
    /// The dprc-connect edge stands (`Connected`).
    Connected,
    /// The edge was torn; both ends still exist (`Disconnected`).
    Disconnected,
    /// A disconnected end was destroyed — teardown terminal (`EndDestroyLegal`).
    EndDestroyLegal,
}

/// The container-bind facet of the fresh partial order (`link_lifecycle.qnt` `type ContainerBind`;
/// cross-dprc-links design D4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContainerBind {
    /// Ends exist, container unplugged: connect is legal here (`Populated`).
    Populated,
    /// Container plugged, ends kernel-visible (`Bound`).
    Bound,
}

/// One dpni wire end as the Rust surface observes it (`link_lifecycle.qnt` `existingEnd` /
/// `populatedEnd` / `mcAcceptedInvisibleEnd`): the container it resides in and whether a scan
/// sees it kernel-visible — the [`VisibleEndpoint`](dpaa2_api::families::dprc::VisibleEndpoint)
/// witness and the `DeferredVisibility` invisibility both read this bit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkEnd {
    /// The container ordinal (`ObjState.parent`, a dprc index).
    pub container: u32,
    /// Kernel visibility (`ObjState.busVisible`).
    pub bus_visible: bool,
}

/// A typed standing residue (`link_lifecycle.qnt` `type WireResidue`), decoded carrying the
/// endpoint ordinal the model names — the container is not recorded on the model residue, so the
/// twin judges the [`dpaa2_api::plan::connect::WireResidue`] variant and endpoint identity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkResidue {
    /// A kernel node lingering after an MC destroy (`StaleNode(ObjId)`).
    StaleNode(u32),
    /// A `DeferredVisibility` whose rebind consent was declined (`DeclinedVisibility(ObjId)`).
    DeclinedVisibility(u32),
}

/// One frozen `world` state as the dpni↔dpni wire vocabulary (`link_lifecycle.qnt` `type World`):
/// the endpoints, the standing edges, the wire phase, the container-bind facet, and the
/// obligation/residue carriers the Rust judgments read back.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LinkWorld {
    /// The wire phase (`world.phase`).
    pub phase: WirePhase,
    /// The container-bind facet (`world.containerBind`; cross-dprc-links design D4).
    pub container_bind: ContainerBind,
    /// The dpni ends by ordinal (`world.core.objs`, Dpni keys only).
    pub endpoints: BTreeMap<u32, LinkEnd>,
    /// The standing dpni↔dpni edges as ordinal pairs (`world.core.conns`).
    pub conns: Vec<(u32, u32)>,
    /// The eager `DeferredVisibility` obligations by endpoint ordinal (`world.obligations`).
    pub obligations: BTreeSet<u32>,
    /// The typed standing residues (`world.residues`).
    pub residues: Vec<LinkResidue>,
}

/// One frozen step of a directed link run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LinkStep {
    /// A state with the `world` var decoded.
    World(LinkWorld),
    /// A `.fail()` step: the guard was disabled, so quint froze no variable values. The replayer
    /// witnesses the refused action against the prior world (the cardinality-one guard).
    Refused,
}

fn wire_phase(v: &Value) -> Result<WirePhase, String> {
    Ok(match tag(v)? {
        "EndsExist" => WirePhase::EndsExist,
        "Connected" => WirePhase::Connected,
        "Disconnected" => WirePhase::Disconnected,
        "EndDestroyLegal" => WirePhase::EndDestroyLegal,
        other => return Err(format!("unknown WirePhase tag `{other}`")),
    })
}

fn container_bind(v: &Value) -> Result<ContainerBind, String> {
    Ok(match tag(v)? {
        "Populated" => ContainerBind::Populated,
        "Bound" => ContainerBind::Bound,
        other => return Err(format!("unknown ContainerBind tag `{other}`")),
    })
}

/// A dpni `ObjId` ordinal, failing loudly on any non-Dpni family — the only object family this
/// surface carries is the dpni end, so a foreign key is new vocabulary (a model↔core divergence).
fn dpni_ordinal(obj: &Value) -> Result<u32, String> {
    match tag(field(obj, "fam")?)? {
        "Dpni" => num(field(obj, "num")?),
        other => Err(format!("unexpected non-Dpni object family `{other}`")),
    }
}

/// The container ordinal an `ObjState.parent` names (always `Some(dprc)` on this surface).
fn parent_container(objstate: &Value) -> Result<u32, String> {
    let parent = field(objstate, "parent")?;
    match tag(parent)? {
        "Some" => dprc_ordinal(field(parent, "value")?),
        other => Err(format!("a wire end has no parent container (`{other}`)")),
    }
}

/// A dprc `ObjId` ordinal (the container a parent edge names).
fn dprc_ordinal(obj: &Value) -> Result<u32, String> {
    match tag(field(obj, "fam")?)? {
        "Dprc" => num(field(obj, "num")?),
        other => Err(format!("a container is not a Dprc (`{other}`)")),
    }
}

/// The dpni endpoints of `world.core.objs` (Dpni keys only, each with its container and visibility).
fn endpoints(core: &Value) -> Result<BTreeMap<u32, LinkEnd>, String> {
    let map = field(core, "objs")?["#map"]
        .as_array()
        .ok_or("objs is not a map")?;
    let mut out = BTreeMap::new();
    for entry in map {
        let n = dpni_ordinal(&entry[0])?;
        let objstate = &entry[1];
        out.insert(
            n,
            LinkEnd {
                container: parent_container(objstate)?,
                bus_visible: field(objstate, "busVisible")?
                    .as_bool()
                    .ok_or("busVisible is not a bool")?,
            },
        );
    }
    Ok(out)
}

/// The standing dpni↔dpni edges of `world.core.conns` as ordinal pairs.
fn conns(core: &Value) -> Result<Vec<(u32, u32)>, String> {
    set_items(field(core, "conns")?)?
        .iter()
        .map(|t| {
            let pair = t["#tup"].as_array().ok_or("conn is not a pair")?;
            Ok((
                dpni_ordinal(field(&pair[0], "obj")?)?,
                dpni_ordinal(field(&pair[1], "obj")?)?,
            ))
        })
        .collect()
}

/// The `DeferredVisibility` obligation endpoints of `world.obligations`.
fn obligations(v: &Value) -> Result<BTreeSet<u32>, String> {
    set_items(v)?
        .iter()
        .map(|o| dpni_ordinal(field(o, "endpoint")?))
        .collect()
}

/// A `WireResidue` sum value (`link_lifecycle.qnt` `type WireResidue`).
fn residue(v: &Value) -> Result<LinkResidue, String> {
    Ok(match tag(v)? {
        "StaleNode" => LinkResidue::StaleNode(dpni_ordinal(&v["value"])?),
        "DeclinedVisibility" => LinkResidue::DeclinedVisibility(dpni_ordinal(&v["value"])?),
        other => return Err(format!("unknown WireResidue tag `{other}`")),
    })
}

fn residues(v: &Value) -> Result<Vec<LinkResidue>, String> {
    set_items(v)?.iter().map(residue).collect()
}

/// One frozen step as a [`LinkStep`].
fn link_step(state: &Value) -> Result<LinkStep, String> {
    // A `.fail()` step froze only `#meta`; state_var finds no `world` and this is the sentinel.
    let Ok(world) = state_var(state, "world") else {
        return Ok(LinkStep::Refused);
    };
    let core = field(world, "core")?;
    Ok(LinkStep::World(LinkWorld {
        phase: wire_phase(field(world, "phase")?)?,
        container_bind: container_bind(field(world, "containerBind")?)?,
        endpoints: endpoints(core)?,
        conns: conns(core)?,
        obligations: obligations(field(world, "obligations")?)?,
        residues: residues(field(world, "residues")?)?,
    }))
}

/// Parses a frozen link-lifecycle trace into its per-state [`LinkStep`]s.
///
/// # Errors
///
/// Returns a description of the first structural mismatch or an unknown sum tag — a trace not
/// produced by `link_lifecycle` over the wire vocabulary (a model↔core divergence).
pub fn parse_link_trace(json: &str) -> Result<Vec<LinkStep>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(link_step)
        .collect()
}
