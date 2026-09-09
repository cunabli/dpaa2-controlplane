//! Reader for frozen dprc-lifecycle ITF traces (`models/families/traces/*.itf.json`,
//! emitted by `models/families/dprc.qnt` module `dprc_lifecycle` via `pnpm
//! model:freeze-dprc`).
//!
//! Unlike the intent trace (one state feeding the pure `compile`, `intent_itf.rs`), a
//! dprc trace is a *stepped machine* run: the `world` var walks a directed sequence of
//! container states. This reader reduces each frozen state to a `WorldView` built
//! from the real [`dpaa2_api::dprc`] value types, so `tests/dprc_replay.rs` can drive
//! the task-2.1 typestate core through the same transitions and assert the Rust world
//! equals the frozen world at every step. The transition between two states is not
//! named in the ITF; the replayer infers it from the state delta, mirroring
//! `dprc.qnt`'s action set (see `dprc_replay.rs`).
//!
//! Mapping (the only place the two encodings are reconciled) — every sum tag is the
//! Rust variant name verbatim (the ADR-0014 lint-bijection makes them identical
//! spellings), so the decode is a direct tag match:
//! - `ContainerState` incl. `Plugged(VfioBind)`, `Outcome`/`Refusal`, `ResidentKind`,
//!   `VfioBind` ⇒ the [`dpaa2_api::dprc`] enums of the same name;
//! - `Resident { id, kind, plugged }` ⇒ a `ResidentId`-keyed `Resident` entry;
//! - `#bigint` strings ⇒ `u32` (`itf::num`).

use std::collections::BTreeMap;

use serde_json::Value;

use dpaa2_api::ConstructName;
use dpaa2_api::dprc::{
    ContainerState, Identity, Options, Outcome, Refusal, Resident, ResidentId, ResidentKind,
    VfioBind,
};

use crate::intent_itf::{field, set_items, text};
use crate::itf::{num, tag};

/// The comparable projection of one frozen `world` state (`dprc.qnt` `type World`):
/// the child container plus the parent's residents and the recorded outcome. Built
/// from the real [`dpaa2_api::dprc`] types so the replay's `==` against a Rust-driven
/// container is the whole conformance check.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WorldView {
    /// The child container's lifecycle phase (with the VFIO bind for `Plugged`).
    pub phase: ContainerState,
    /// The child's pool-assigned identity (DPRC-I10).
    pub identity: Identity,
    /// The child's create-time option mask.
    pub options: Options,
    /// The child's MC label.
    pub label: ConstructName,
    /// The child's residents, keyed by id.
    pub residents: BTreeMap<ResidentId, Resident>,
    /// The parent's residents — the eviction destination.
    pub parent_residents: BTreeMap<ResidentId, Resident>,
    /// The recorded outcome of the transition that produced this state.
    pub last_outcome: Outcome,
}

/// The `bool` at an ITF record field.
fn boolean(v: &Value) -> Result<bool, String> {
    v.as_bool().ok_or_else(|| format!("not a bool: {v}"))
}

/// A `VfioBind` sum value (`dprc.qnt` `type VfioBind`).
fn vfio_bind(v: &Value) -> Result<VfioBind, String> {
    match tag(v)? {
        "Unbound" => Ok(VfioBind::Unbound),
        "BoundVfioFslMc" => Ok(VfioBind::BoundVfioFslMc),
        other => Err(format!("unknown VfioBind tag `{other}`")),
    }
}

/// A `ContainerState` sum value (`dprc.qnt` `type ContainerState`).
fn container_state(v: &Value) -> Result<ContainerState, String> {
    Ok(match tag(v)? {
        "Declared" => ContainerState::Declared,
        "Created" => ContainerState::Created,
        "Populated" => ContainerState::Populated,
        "Plugged" => ContainerState::Plugged(vfio_bind(&v["value"])?),
        "Locked" => ContainerState::Locked,
        "Emptied" => ContainerState::Emptied,
        "Destroyed" => ContainerState::Destroyed,
        other => return Err(format!("unknown ContainerState tag `{other}`")),
    })
}

/// A `ResidentKind` sum value (`dprc.qnt` `type ResidentKind`).
fn resident_kind(v: &Value) -> Result<ResidentKind, String> {
    match tag(v)? {
        "CreatedIn" => Ok(ResidentKind::CreatedIn),
        "AssignedIn" => Ok(ResidentKind::AssignedIn),
        other => Err(format!("unknown ResidentKind tag `{other}`")),
    }
}

/// A `Refusal` sum value (`dprc.qnt` `type Refusal`).
fn refusal(v: &Value) -> Result<Refusal, String> {
    match tag(v)? {
        "SpawnViolation" => Ok(Refusal::SpawnViolation),
        "AllocViolation" => Ok(Refusal::AllocViolation),
        "TopologyLockGate" => Ok(Refusal::TopologyLockGate),
        other => Err(format!("unknown Refusal tag `{other}`")),
    }
}

/// An `Outcome` sum value (`dprc.qnt` `type Outcome`).
fn outcome(v: &Value) -> Result<Outcome, String> {
    match tag(v)? {
        "Accepted" => Ok(Outcome::Accepted),
        "Refused" => Ok(Outcome::Refused(refusal(&v["value"])?)),
        other => Err(format!("unknown Outcome tag `{other}`")),
    }
}

/// One `Resident` record (`dprc.qnt` `type Resident`) as an id-keyed entry.
fn resident(v: &Value) -> Result<(ResidentId, Resident), String> {
    Ok((
        ResidentId::new(num(field(v, "id")?)?),
        Resident {
            kind: resident_kind(field(v, "kind")?)?,
            plugged: boolean(field(v, "plugged")?)?,
        },
    ))
}

/// A `Set[Resident]` as an id-keyed map.
fn residents(v: &Value) -> Result<BTreeMap<ResidentId, Resident>, String> {
    set_items(v)?.iter().map(resident).collect()
}

/// The option mask record (`dprc.qnt` `type Options`).
fn options(v: &Value) -> Result<Options, String> {
    Ok(Options {
        spawn: boolean(field(v, "spawn")?)?,
        alloc: boolean(field(v, "alloc")?)?,
        obj_create: boolean(field(v, "objCreate")?)?,
        topology_changes: boolean(field(v, "topologyChanges")?)?,
    })
}

/// The identity record (`dprc.qnt` `type Identity`).
fn identity(v: &Value) -> Result<Identity, String> {
    Ok(Identity {
        icid: num(field(v, "icid")?)?,
        portal_id: num(field(v, "portalId")?)?,
    })
}

/// One frozen `world` state as a [`WorldView`].
fn world_view(w: &Value) -> Result<WorldView, String> {
    let child = field(w, "child")?;
    Ok(WorldView {
        phase: container_state(field(child, "phase")?)?,
        identity: identity(field(child, "identity")?)?,
        options: options(field(child, "options")?)?,
        label: ConstructName::from(text(field(child, "label")?)?),
        residents: residents(field(child, "residents")?)?,
        parent_residents: residents(field(w, "parentResidents")?)?,
        last_outcome: outcome(field(w, "lastOutcome")?)?,
    })
}

/// Parses a frozen dprc-lifecycle trace into its per-state [`WorldView`]s.
///
/// # Errors
///
/// Returns a description of the first structural mismatch — a trace not produced by
/// `models/families/dprc.qnt` (missing `world`/`child` fields, or a tag the mapping
/// layer does not recognise).
pub fn parse_dprc_trace(json: &str) -> Result<Vec<WorldView>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(|state| world_view(field(state, "world")?))
        .collect()
}
