//! Reader for frozen dpsw-lifecycle ITF traces (`models/traces/families/dpsw/*.itf.json`,
//! emitted by `models/families/dpsw.qnt` module `dpsw_lifecycle` via `pnpm model:freeze-dpsw`).
//!
//! The create-plus-pool twin of the dpseci reader ([`crate::intent::dpseci_itf`]): the `world`
//! var walks a directed create/probe/defer/destroy run over a shared container pool, and this
//! reader reduces each frozen state to a `DpswWorld` over the dpsw create/census vocabulary,
//! so `tests/dpsw_replay.rs` can drive the real [`dpaa2_api::families::dpsw`] judgments through
//! the same states and compare (dpsw-typestate task 6.1).
//!
//! As in the dpseci reader, a `Switch` state is decoded into its *reported* fields (`RawCfg`)
//! — the twelve create fields verbatim — never through the family constructor. The adapter
//! reports, the core judges (the ITF-replay rule that the classification lives once, core-side):
//! [`dpaa2_api::families::dpsw::DpswCfg::new`] is the judgment under test, so the replay runs it
//! over these reported fields and asserts the verdict, rather than this reader pre-judging at
//! decode. A frozen range field that does not fit its 16-bit create width surfaces here as a
//! decode error — a model↔core divergence made loud.
//!
//! Mapping (the only place the two encodings are reconciled; the sum tags are the family type
//! names, the flooding/broadcast cases carrying the model's `FloodingPer…`/`BroadcastPer…`
//! prefix): `DpswState` (`Absent`/`Switch`), `DpswOpt`, `ComponentType`, `FloodingCfg`,
//! `BroadcastCfg`, `SwitchPhase` (`Phase…`), and `Refusal` ⇒ the
//! [`dpaa2_api::families::dpsw`] types of the same name; the `Outcome` sum ⇒ `Option<Refusal>`
//! (`None` is `Accepted`, `Some(r)` is `Refused(r)`), the crate's accepted-or-refused idiom; the
//! `Pool` record ⇒ plain `Pool` counts the replay does the draw arithmetic over.

use std::collections::BTreeSet;

use serde_json::Value;

use dpaa2_api::families::dpsw::{
    BroadcastCfg, ComponentType, DpswOpt, FloodingCfg, Refusal, SwitchPhase,
};

use crate::itf::{field, int64, num, set_items, tag};

/// The frozen dpsw create block as reported fields (`dpsw.qnt` `dpsw_lifecycle` `type
/// CreateCfg`): the twelve create fields verbatim, held raw so the replay can run
/// [`dpaa2_api::families::dpsw::DpswCfg::new`] over them itself (the adapter does not
/// pre-validate). The six range fields ride as plain `u16`, the two dead/unobservable fields
/// (`mem_size`, `max_meters_per_if`) as `u32`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RawCfg {
    /// `--num-ifs` — the immutable port count (`world.dpsw` `Switch` `cfg.numIfs`; DPSW-I10).
    pub num_ifs: u16,
    /// `--options` — the named MC flags (`cfg.options`); the model carries no raw escape.
    pub options: BTreeSet<DpswOpt>,
    /// `--max-vlans` (`cfg.maxVlans`).
    pub max_vlans: u16,
    /// `--max-fdbs` (`cfg.maxFdbs`; DPSW-I1 reads `>= num_ifs`).
    pub max_fdbs: u16,
    /// `--max-fdb-entries` (`cfg.maxFdbEntries`).
    pub max_fdb_entries: u16,
    /// `--fdb-aging-time` (`cfg.fdbAgingTime`).
    pub fdb_aging_time: u16,
    /// `--max-fdb-mc-groups` (`cfg.maxFdbMcGroups`).
    pub max_fdb_mc_groups: u16,
    /// `--mem-size` — a dead/unobservable field the census signature drops (`cfg.memSize`).
    pub mem_size: u32,
    /// `--component-type` (`cfg.componentType`).
    pub component_type: ComponentType,
    /// `--flooding-cfg` (`cfg.floodingCfg`; DPSW-I1 requires `PER_FDB`).
    pub flooding_cfg: FloodingCfg,
    /// `--broadcast-cfg` (`cfg.broadcastCfg`; DPSW-I1 requires `PER_FDB`).
    pub broadcast_cfg: BroadcastCfg,
    /// A dead flib field the census signature drops (`cfg.maxMetersPerIf`).
    pub max_meters_per_if: u32,
}

/// One created switch (`dpsw.qnt` `type SwitchData`): its reported block, its phase, and
/// whether MC-default state still stands (`mcDefaults`; reset by the bind, DPSW-I4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SwitchView {
    /// The reported create block ([`RawCfg`]).
    pub cfg: RawCfg,
    /// The switch phase (`SwitchData.phase`).
    pub phase: SwitchPhase,
    /// Whether MC-default state still stands (`SwitchData.mcDefaults`; DPSW-I4).
    pub mc_defaults: bool,
}

/// The dpsw create state (`dpsw.qnt` `type DpswState`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DpswState {
    /// No dpsw — the `Absent` state (`init`, a destroyed object, or a refused create).
    Absent,
    /// A created switch and its reported view (`Switch(SwitchData)`).
    Switch(SwitchView),
}

/// The container pool the kernel probe draws from (`dpsw.qnt` `type Pool`), as plain counts —
/// the replay does the 1 dpmcp + 1 dpbp draw arithmetic over these (`PROBE_DRAW`; DPSW-I9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pool {
    /// Free dpmcp (`pool.dpmcp`).
    pub dpmcp: i64,
    /// Free dpbp (`pool.dpbp`).
    pub dpbp: i64,
}

/// One frozen `world` state (`dpsw_lifecycle` `type World`): the create state, the pool, and
/// the recorded verdict of the transition that produced it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DpswWorld {
    /// The dpsw create state (with the reported view for a created switch).
    pub dpsw: DpswState,
    /// The container pool.
    pub pool: Pool,
    /// The recorded outcome (`world.lastOutcome`): `None` is `Accepted`, `Some(r)` is
    /// `Refused(r)`.
    pub last_outcome: Option<Refusal>,
}

/// A 16-bit range field (`dpsw.qnt` `CreateCfg` range fields): a frozen value beyond the
/// create width is a loud model↔core divergence, never silently truncated.
fn range_field(v: &Value, key: &str) -> Result<u16, String> {
    let n = num(v)?;
    u16::try_from(n).map_err(|_| format!("dpsw {key} {n} exceeds the 16-bit create field"))
}

/// One named MC flag (`dpsw.qnt` `type DpswOpt`).
fn dpsw_opt(v: &Value) -> Result<DpswOpt, String> {
    Ok(match tag(v)? {
        "FloodingDis" => DpswOpt::FloodingDis,
        "MulticastDis" => DpswOpt::MulticastDis,
        "CtrlIfDis" => DpswOpt::CtrlIfDis,
        "FloodingMeteringDis" => DpswOpt::FloodingMeteringDis,
        "MeteringEn" => DpswOpt::MeteringEn,
        "BpPerIf" => DpswOpt::BpPerIf,
        "LagDis" => DpswOpt::LagDis,
        other => return Err(format!("unknown DpswOpt tag `{other}`")),
    })
}

/// The component type (`dpsw.qnt` `type ComponentType`).
fn component_type(v: &Value) -> Result<ComponentType, String> {
    Ok(match tag(v)? {
        "CVlan" => ComponentType::CVlan,
        "SVlan" => ComponentType::SVlan,
        other => return Err(format!("unknown ComponentType tag `{other}`")),
    })
}

/// The flooding domain (`dpsw.qnt` `type FloodingCfg`; the cases carry the model prefix).
fn flooding_cfg(v: &Value) -> Result<FloodingCfg, String> {
    Ok(match tag(v)? {
        "FloodingPerVlan" => FloodingCfg::PerVlan,
        "FloodingPerFdb" => FloodingCfg::PerFdb,
        other => return Err(format!("unknown FloodingCfg tag `{other}`")),
    })
}

/// The broadcast domain (`dpsw.qnt` `type BroadcastCfg`; the cases carry the model prefix).
fn broadcast_cfg(v: &Value) -> Result<BroadcastCfg, String> {
    Ok(match tag(v)? {
        "BroadcastPerObject" => BroadcastCfg::PerObject,
        "BroadcastPerFdb" => BroadcastCfg::PerFdb,
        other => return Err(format!("unknown BroadcastCfg tag `{other}`")),
    })
}

/// The switch phase (`dpsw.qnt` `type SwitchPhase`; the model `Phase…` cases).
fn switch_phase(v: &Value) -> Result<SwitchPhase, String> {
    Ok(match tag(v)? {
        "PhaseCreated" => SwitchPhase::Created,
        "PhaseDeferred" => SwitchPhase::Deferred,
        "PhaseBound" => SwitchPhase::Bound,
        other => return Err(format!("unknown SwitchPhase tag `{other}`")),
    })
}

/// The reported create block (`dpsw.qnt` `type CreateCfg`) as a [`RawCfg`].
fn raw_cfg(v: &Value) -> Result<RawCfg, String> {
    let mut options = BTreeSet::new();
    for o in set_items(field(v, "options")?)? {
        options.insert(dpsw_opt(o)?);
    }
    Ok(RawCfg {
        num_ifs: range_field(field(v, "numIfs")?, "num_ifs")?,
        options,
        max_vlans: range_field(field(v, "maxVlans")?, "max_vlans")?,
        max_fdbs: range_field(field(v, "maxFdbs")?, "max_fdbs")?,
        max_fdb_entries: range_field(field(v, "maxFdbEntries")?, "max_fdb_entries")?,
        fdb_aging_time: range_field(field(v, "fdbAgingTime")?, "fdb_aging_time")?,
        max_fdb_mc_groups: range_field(field(v, "maxFdbMcGroups")?, "max_fdb_mc_groups")?,
        mem_size: num(field(v, "memSize")?)?,
        component_type: component_type(field(v, "componentType")?)?,
        flooding_cfg: flooding_cfg(field(v, "floodingCfg")?)?,
        broadcast_cfg: broadcast_cfg(field(v, "broadcastCfg")?)?,
        max_meters_per_if: num(field(v, "maxMetersPerIf")?)?,
    })
}

/// The dpsw create state (`dpsw.qnt` `type DpswState`).
fn dpsw_state(v: &Value) -> Result<DpswState, String> {
    Ok(match tag(v)? {
        "Absent" => DpswState::Absent,
        "Switch" => {
            let sw = &v["value"];
            DpswState::Switch(SwitchView {
                cfg: raw_cfg(field(sw, "cfg")?)?,
                phase: switch_phase(field(sw, "phase")?)?,
                mc_defaults: field(sw, "mcDefaults")?
                    .as_bool()
                    .ok_or("mcDefaults is not a bool")?,
            })
        }
        other => return Err(format!("unknown DpswState tag `{other}`")),
    })
}

/// A `Refusal` sum value (`dpsw_lifecycle` `type Refusal`).
fn refusal(v: &Value) -> Result<Refusal, String> {
    Ok(match tag(v)? {
        "VendorRecipeShape" => Refusal::VendorRecipeShape,
        "UnbindableCfg" => Refusal::UnbindableCfg,
        other => return Err(format!("unknown Refusal tag `{other}`")),
    })
}

/// An `Outcome` sum value (`dpsw_lifecycle` `type Outcome`) as `Option<Refusal>`.
fn outcome(v: &Value) -> Result<Option<Refusal>, String> {
    Ok(match tag(v)? {
        "Accepted" => None,
        "Refused" => Some(refusal(&v["value"])?),
        other => return Err(format!("unknown Outcome tag `{other}`")),
    })
}

/// The container pool (`dpsw.qnt` `type Pool`) as plain counts.
fn pool(v: &Value) -> Result<Pool, String> {
    Ok(Pool {
        dpmcp: int64(field(v, "dpmcp")?)?,
        dpbp: int64(field(v, "dpbp")?)?,
    })
}

/// One frozen `world` state as a [`DpswWorld`].
fn world_view(w: &Value) -> Result<DpswWorld, String> {
    Ok(DpswWorld {
        dpsw: dpsw_state(field(w, "dpsw")?)?,
        pool: pool(field(w, "pool")?)?,
        last_outcome: outcome(field(w, "lastOutcome")?)?,
    })
}

/// Parses a frozen dpsw-lifecycle trace into its per-state [`DpswWorld`]s.
///
/// # Errors
///
/// Returns a description of the first structural mismatch or an unknown sum tag — a trace not
/// produced by `dpsw_lifecycle` over the create/census vocabulary (a model↔core divergence).
pub fn parse_dpsw_trace(json: &str) -> Result<Vec<DpswWorld>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(|state| world_view(field(state, "world")?))
        .collect()
}
