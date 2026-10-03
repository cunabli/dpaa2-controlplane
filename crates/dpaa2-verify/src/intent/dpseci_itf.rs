//! Reader for frozen dpseci create-surface ITF traces (`models/traces/families/dpseci/*.itf.json`,
//! emitted by `models/families/dpseci.qnt` module `dpseci_lifecycle` via `pnpm model:freeze-dpseci`).
//!
//! The create-surface twin of the dpni reader ([`crate::intent::dpni_itf`]): the `world` var
//! walks a directed create/refuse/destroy run, and this reader reduces each frozen state to a
//! `DpseciWorld` over the dpseci create vocabulary, so `tests/dpseci_replay.rs` can drive the
//! real [`dpaa2_api::families::dpseci`] judgments through the same states and compare.
//!
//! Unlike the dpni reader, a `Created` state is decoded into its *reported* fields
//! (`RawCfg`) — the options mask, the queue count and the priorities — rather than through
//! the family constructor. The adapter reports, the core judges (the ITF-replay rule that the
//! classification lives once, core-side): `DpseciCfg::new` is the judgment under test, so the
//! replay runs it over these reported fields and asserts the verdict, instead of this reader
//! pre-judging at decode. A frozen queue count or priority that does not fit the reported width
//! surfaces here as a decode error — a model↔core divergence made loud.
//!
//! Mapping (the only place the two encodings are reconciled; the sum tags are the family
//! variant names verbatim): `DpseciState` (`Absent`/`Created`), `DpseciOpt`, the `RawBits`
//! escape, and `Refusal` ⇒ the [`dpaa2_api::families::dpseci`] types of the same name; the
//! `Outcome` sum ⇒ `Option<Refusal>` (`None` is `Accepted`, `Some(r)` is `Refused(r)`), the
//! crate's accepted-or-refused idiom rather than a restated two-entry vocabulary.

use serde_json::Value;

use dpaa2_api::families::dpseci::{DpseciOpt, OptionMask, RawEscape, Refusal};

use crate::itf::{field, num, set_items, tag};

/// The frozen dpseci create block as reported fields (`dpseci.qnt` `dpseci_lifecycle`
/// `type CreateCfg`): the options mask, the single queue count driving both tx and rx, and the
/// per-queue priorities. These are the inputs `DpseciCfg::new` judges; the replay runs that
/// judgment itself (the adapter does not pre-validate).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RawCfg {
    /// The reported option mask (named flags plus provenance-carrying raw escapes).
    pub options: OptionMask,
    /// The reported `--num-queues` value (`world.dpseci` `Created` `numQueues`).
    pub num_queues: usize,
    /// The reported per-queue priorities (`world.dpseci` `Created` `priorities`).
    pub priorities: Vec<u8>,
}

/// The dpseci create state (`dpseci.qnt` `dpseci_lifecycle` `type DpseciState`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DpseciPhase {
    /// No dpseci — the `Absent` state (`init`, a destroyed object, or a refused create).
    Absent,
    /// A created dpseci and its reported create block (`Created(CreateCfg)`).
    Created(RawCfg),
}

/// One frozen `world` state (`dpseci_lifecycle` `type World`): the create state and the
/// recorded verdict of the transition that produced it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DpseciWorld {
    /// The dpseci create state (with the reported block for a created dpseci).
    pub dpseci: DpseciPhase,
    /// The recorded outcome (`world.lastOutcome`): `None` is `Accepted`, `Some(r)` is
    /// `Refused(r)`.
    pub last_outcome: Option<Refusal>,
}

/// One named MC flag (`dpseci.qnt` `type DpseciOpt`).
fn dpseci_opt(v: &Value) -> Result<DpseciOpt, String> {
    Ok(match tag(v)? {
        "HasCg" => DpseciOpt::HasCg,
        "HasOpr" => DpseciOpt::HasOpr,
        "OprShared" => DpseciOpt::OprShared,
        other => return Err(format!("unknown DpseciOpt tag `{other}`")),
    })
}

/// The option mask (`dpseci.qnt` `type OptionMask`), built through the additive builder so no
/// unnamed flag enters and each raw bit rides attributed as a [`RawEscape`], never merged into
/// the named flags.
fn option_mask(v: &Value) -> Result<OptionMask, String> {
    let mut mask = OptionMask::empty();
    for f in set_items(field(v, "flags")?)? {
        mask = mask.with_flag(dpseci_opt(f)?);
    }
    for e in set_items(field(v, "escapes")?)? {
        mask = mask.with_escape(RawEscape::new(num(field(e, "value")?)?));
    }
    Ok(mask)
}

/// The reported create block (`dpseci.qnt` `type CreateCfg`) as a [`RawCfg`]. A priority that
/// does not fit a byte fails the decode — a loud model↔core divergence.
fn raw_cfg(v: &Value) -> Result<RawCfg, String> {
    let priorities = field(v, "priorities")?
        .as_array()
        .ok_or("priorities is not a list")?
        .iter()
        .map(|p| {
            let n = num(p)?;
            u8::try_from(n).map_err(|_| format!("priority {n} exceeds a byte"))
        })
        .collect::<Result<Vec<u8>, String>>()?;
    Ok(RawCfg {
        options: option_mask(field(v, "options")?)?,
        num_queues: num(field(v, "numQueues")?)? as usize,
        priorities,
    })
}

/// The dpseci create state (`dpseci.qnt` `type DpseciState`).
fn dpseci_phase(v: &Value) -> Result<DpseciPhase, String> {
    Ok(match tag(v)? {
        "Absent" => DpseciPhase::Absent,
        "Created" => DpseciPhase::Created(raw_cfg(&v["value"])?),
        other => return Err(format!("unknown DpseciState tag `{other}`")),
    })
}

/// A `Refusal` sum value (`dpseci_lifecycle` `type Refusal`).
fn refusal(v: &Value) -> Result<Refusal, String> {
    Ok(match tag(v)? {
        "QueueCountOutOfRange" => Refusal::QueueCountOutOfRange,
        "PriorityCountMismatch" => Refusal::PriorityCountMismatch,
        "PriorityOutOfRange" => Refusal::PriorityOutOfRange,
        other => return Err(format!("unknown Refusal tag `{other}`")),
    })
}

/// An `Outcome` sum value (`dpseci_lifecycle` `type Outcome`) as `Option<Refusal>`.
fn outcome(v: &Value) -> Result<Option<Refusal>, String> {
    Ok(match tag(v)? {
        "Accepted" => None,
        "Refused" => Some(refusal(&v["value"])?),
        other => return Err(format!("unknown Outcome tag `{other}`")),
    })
}

/// One frozen `world` state as a [`DpseciWorld`].
fn world_view(w: &Value) -> Result<DpseciWorld, String> {
    Ok(DpseciWorld {
        dpseci: dpseci_phase(field(w, "dpseci")?)?,
        last_outcome: outcome(field(w, "lastOutcome")?)?,
    })
}

/// Parses a frozen dpseci-lifecycle trace into its per-state [`DpseciWorld`]s.
///
/// # Errors
///
/// Returns a description of the first structural mismatch or an unknown sum tag — a trace not
/// produced by `dpseci_lifecycle` over the create vocabulary (a model↔core divergence).
pub fn parse_dpseci_trace(json: &str) -> Result<Vec<DpseciWorld>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(|state| world_view(field(state, "world")?))
        .collect()
}
