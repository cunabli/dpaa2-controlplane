//! Reader for frozen dpni create-surface ITF traces (`models/traces/families/dpni/*.itf.json`,
//! emitted by `models/families/dpni.qnt` module `dpni_scenario` via `pnpm model:freeze-dpni`).
//!
//! The stepped-machine twin of the dprc reader ([`crate::intent::dprc_itf`]): the `world` var
//! walks a directed create → read-back → primary-MAC scenario, and this reader reduces
//! each frozen state to a `DpniWorld` built from the real
//! [`dpaa2_api::families::dpni`] value types, so `tests/dpni_replay.rs` can drive the
//! dpni-typestate task-2.1/2.2 core through the same transitions and compare. The
//! transition between two states is not named in the ITF; the replayer infers it from
//! the state delta, mirroring `dpni_scenario`'s action set.
//!
//! Decoding a `Created(CreateCfg)` state builds the full `DpniCfg` through the crate's
//! refined range constructors: a frozen accepted-create field outside the board-verified
//! envelope has no Rust constructor, so it surfaces here as a parse error — a model↔core
//! divergence made loud, never papered over (dpni-typestate design D2). The write-only
//! `dist_key_size` (dpni-typestate design D4) is decoded into the create block but is
//! dropped from the observation the replay compares on.
//!
//! Mapping (the only place the two encodings are reconciled) — every sum tag is the Rust
//! variant name verbatim (the ADR-0014 lint-bijection makes them identical spellings):
//! - `DpniState` (`Absent`/`Created`), `Outcome`/`Refusal`, `DpniOpt`, `RawEscape`,
//!   `Unrepresentable` ⇒ the [`dpaa2_api::families::dpni`] types of the same name;
//! - the `CreateCfg` numeric fields ⇒ the refined range newtypes;
//! - `primaryMac` (a six-element `List[int]`) ⇒ `MacAddr`'s octets.

use serde_json::Value;

use dpaa2_api::core::model::MacAddr;
use dpaa2_api::families::dpni::{
    DistKeySize, DpniCfg, DpniOpt, FsEntries, MacFilterEntries, NumCeetmCh, NumCgs, NumOpr,
    NumQueues, NumTcs, OptionMask, QosEntries, RawEscape, Unrepresentable, VlanFilterEntries,
};

use crate::itf::{field, num, set_items, tag};

/// The dpni create state (`dpni.qnt` `type DpniState`), carrying the full decoded create
/// block for a created dpni.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DpniPhase {
    /// No dpni — the `Absent` state (a refused create leaves this).
    Absent,
    /// A created dpni and its immutable create block (`Created(CreateCfg)`).
    Created(DpniCfg),
}

/// A create refusal (`dpni_lifecycle` `type Refusal`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DpniRefusal {
    /// A cfg field outside the restool envelope.
    RangeViolation,
    /// A dead option / `num_rx_tcs` requested, named.
    DeadOption(Unrepresentable),
    /// `UserspaceEvent` has no dpni profile (ADR-0012).
    UnpricedDataplane,
}

/// The recorded outcome of the transition that produced a state (`dpni_lifecycle`
/// `type Outcome`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DpniOutcome {
    /// The step was accepted.
    Accepted,
    /// The step was refused.
    Refused(DpniRefusal),
}

/// One frozen `world` state (`dpni_scenario` `type World`): the create state, the runtime
/// primary MAC, and the recorded outcome.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DpniWorld {
    /// The dpni create state (with the full create block for a created dpni).
    pub dpni: DpniPhase,
    /// The runtime primary MAC (`MacAddr::ZERO` when unset).
    pub primary_mac: MacAddr,
    /// The recorded outcome of the transition that produced this state.
    pub last_outcome: DpniOutcome,
}

/// One named MC flag (`dpni.qnt` `type DpniOpt`).
fn dpni_opt(v: &Value) -> Result<DpniOpt, String> {
    Ok(match tag(v)? {
        "SingleSender" => DpniOpt::SingleSender,
        "CustomCg" => DpniOpt::CustomCg,
        "HasKeyMasking" => DpniOpt::HasKeyMasking,
        "HasOpr" => DpniOpt::HasOpr,
        "OprPerTc" => DpniOpt::OprPerTc,
        "TxFrmRelease" => DpniOpt::TxFrmRelease,
        "HasPolicing" => DpniOpt::HasPolicing,
        "SharedCongestion" => DpniOpt::SharedCongestion,
        "NoMacFilter" => DpniOpt::NoMacFilter,
        "StashingDis" => DpniOpt::StashingDis,
        other => return Err(format!("unknown DpniOpt tag `{other}`")),
    })
}

/// One provenance-carrying raw escape (`dpni.qnt` `type RawEscape`).
fn raw_escape(v: &Value) -> Result<RawEscape, String> {
    match tag(v)? {
        "PfdrInPeb" => Ok(RawEscape::PfdrInPeb),
        other => Err(format!("unknown RawEscape tag `{other}`")),
    }
}

/// The option mask (`dpni.qnt` `type OptionMask`), built through the additive builder so
/// no unnamed flag or arbitrary bit can enter.
fn option_mask(v: &Value) -> Result<OptionMask, String> {
    let mut mask = OptionMask::empty();
    for f in set_items(field(v, "flags")?)? {
        mask = mask.with_flag(dpni_opt(f)?);
    }
    for e in set_items(field(v, "escapes")?)? {
        mask = mask.with_escape(raw_escape(e)?);
    }
    Ok(mask)
}

/// A refined range field: the `#bigint` at JSON key `key`, fed to the newtype constructor
/// `ctor`. A frozen value outside the board-verified envelope fails the constructor — a
/// loud model↔core divergence.
fn ranged<T>(
    cfg: &Value,
    key: &str,
    ctor: impl Fn(u16) -> Result<T, dpaa2_api::core::error::Error>,
) -> Result<T, String> {
    let raw = num(field(cfg, key)?)?;
    let v = u16::try_from(raw).map_err(|_| format!("{key} {raw} exceeds u16"))?;
    ctor(v).map_err(|e| format!("{key}: {e}"))
}

/// The create block (`dpni.qnt` `type CreateCfg`) as the full refined [`DpniCfg`].
fn create_cfg(v: &Value) -> Result<DpniCfg, String> {
    Ok(DpniCfg {
        options: option_mask(field(v, "options")?)?,
        num_queues: ranged(v, "numQueues", NumQueues::new)?,
        num_tcs: ranged(v, "numTcs", NumTcs::new)?,
        mac_filter_entries: ranged(v, "macFilterEntries", MacFilterEntries::new)?,
        vlan_filter_entries: ranged(v, "vlanFilterEntries", VlanFilterEntries::new)?,
        qos_entries: ranged(v, "qosEntries", QosEntries::new)?,
        fs_entries: ranged(v, "fsEntries", FsEntries::new)?,
        num_cgs: ranged(v, "numCgs", NumCgs::new)?,
        dist_key_size: ranged(v, "distKeySize", DistKeySize::new)?,
        num_ceetm_ch: ranged(v, "numCeetmCh", NumCeetmCh::new)?,
        num_opr: ranged(v, "numOpr", NumOpr::new)?,
        root_container: field(v, "rootContainer")?
            .as_bool()
            .ok_or("rootContainer is not a bool")?,
    })
}

/// The dpni create state (`dpni.qnt` `type DpniState`).
fn dpni_phase(v: &Value) -> Result<DpniPhase, String> {
    Ok(match tag(v)? {
        "Absent" => DpniPhase::Absent,
        "Created" => DpniPhase::Created(create_cfg(&v["value"])?),
        other => return Err(format!("unknown DpniState tag `{other}`")),
    })
}

/// An `Unrepresentable` sum value (`dpni_lifecycle` `type Unrepresentable`).
fn unrepresentable(v: &Value) -> Result<Unrepresentable, String> {
    Ok(match tag(v)? {
        "MacAddrCreate" => Unrepresentable::MacAddrCreate,
        "MaxSenders" => Unrepresentable::MaxSenders,
        "MaxTcs" => Unrepresentable::MaxTcs,
        "MaxDistPerTc" => Unrepresentable::MaxDistPerTc,
        "MaxFsEntriesPerTc" => Unrepresentable::MaxFsEntriesPerTc,
        "MaxUnicastFilters" => Unrepresentable::MaxUnicastFilters,
        "MaxMulticastFilters" => Unrepresentable::MaxMulticastFilters,
        "MaxVlanFilters" => Unrepresentable::MaxVlanFilters,
        "MaxQosEntries" => Unrepresentable::MaxQosEntries,
        "MaxQosKeySize" => Unrepresentable::MaxQosKeySize,
        "MaxDistKeySize" => Unrepresentable::MaxDistKeySize,
        "NumRxTcs" => Unrepresentable::NumRxTcs,
        other => return Err(format!("unknown Unrepresentable tag `{other}`")),
    })
}

/// A `Refusal` sum value (`dpni_lifecycle` `type Refusal`).
fn refusal(v: &Value) -> Result<DpniRefusal, String> {
    Ok(match tag(v)? {
        "RangeViolation" => DpniRefusal::RangeViolation,
        "DeadOptionRefusal" => DpniRefusal::DeadOption(unrepresentable(&v["value"])?),
        "UnpricedDataplane" => DpniRefusal::UnpricedDataplane,
        other => return Err(format!("unknown Refusal tag `{other}`")),
    })
}

/// An `Outcome` sum value (`dpni_lifecycle` `type Outcome`).
fn outcome(v: &Value) -> Result<DpniOutcome, String> {
    Ok(match tag(v)? {
        "Accepted" => DpniOutcome::Accepted,
        "Refused" => DpniOutcome::Refused(refusal(&v["value"])?),
        other => return Err(format!("unknown Outcome tag `{other}`")),
    })
}

/// The six-octet primary MAC (`dpni_scenario` `type MacBytes = List[int]`).
fn primary_mac(v: &Value) -> Result<MacAddr, String> {
    let items = v.as_array().ok_or("primaryMac is not a list")?;
    let octets: Vec<u8> = items
        .iter()
        .map(|b| {
            let n = num(b)?;
            u8::try_from(n).map_err(|_| format!("mac octet {n} exceeds a byte"))
        })
        .collect::<Result<_, String>>()?;
    let arr: [u8; 6] = octets
        .try_into()
        .map_err(|o: Vec<u8>| format!("primaryMac has {} octets, not 6", o.len()))?;
    Ok(MacAddr::new(arr))
}

/// One frozen `world` state as a [`DpniWorld`].
fn world_view(w: &Value) -> Result<DpniWorld, String> {
    Ok(DpniWorld {
        dpni: dpni_phase(field(w, "dpni")?)?,
        primary_mac: primary_mac(field(w, "primaryMac")?)?,
        last_outcome: outcome(field(w, "lastOutcome")?)?,
    })
}

/// Parses a frozen dpni-scenario trace into its per-state [`DpniWorld`]s.
///
/// # Errors
///
/// Returns a description of the first structural mismatch or a create block whose frozen
/// field falls outside the board-verified envelope (a model↔core divergence).
pub fn parse_dpni_trace(json: &str) -> Result<Vec<DpniWorld>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(|state| world_view(field(state, "world")?))
        .collect()
}
