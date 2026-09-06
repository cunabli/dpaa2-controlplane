//! Reader for frozen edit-alphabet ITF traces (`models/intent/traces/editsSweep*.itf.json`,
//! emitted by `models/intent/edits.qnt` under `--invariant=identityLaws`).
//!
//! The edit machine (`edits.qnt`) strictly alternates a *perturb* phase (a synced
//! state, an intent edit or a board drift) with a *converge* phase (the plan applied),
//! carrying five vars — `intent`, `board`, `phase`, `touched`, `preBoard`. This reader
//! reduces each frozen state to the same value world the Rust matcher speaks
//! ([`MatchObject`]/[`BoardObject`], `matcher.rs`), so `intent_edit_replay.rs` can drive
//! the model's own converge through the Rust converge and diff the board, and re-check
//! the four identity laws at every synced state.
//!
//! Facet mapping (both sides MUST agree byte-for-byte with the intent projection the
//! replay renders through the config parser, `intent_edit_replay.rs`):
//! - **Anchor non-empty ⇒ [`ConfigFacet::Anchored`]**, the model's config int DISCARDED.
//!   The model's `apply` mutates an anchored object's config, but the Rust facet
//!   deliberately carries none (the anchor decides identity, ADR-0015 decision 9 —
//!   see `matcher.rs` `ConfigFacet` docs). That anchored-config repair is therefore
//!   invisible to this diff: both the intent projection and the board decode read
//!   [`ConfigFacet::Anchored`] whatever the int, so the two always compare equal. A
//!   documented ceiling, not fixed here.
//! - **Anchor empty ⇒ [`ConfigFacet::Link`]** with ends `(kernel, cfg<int>)` — the SAME
//!   "a then b" order the intent projection uses (`interface_a` = kernel, `interface_b` =
//!   cfgN). One flipped tuple diffs every state, so the order is pinned here.
//!
//! The model spells `from` and `label` as a bare `str`; `""` decodes to `None`, matching
//! the neutral `renamed`/`label` slots (`intent_itf.rs` `opt_name` idiom).

use std::collections::BTreeSet;

use serde_json::Value;

use dpaa2_api::{
    ALL_FAMILIES, BoardObject, ConfigFacet, ConstructName, DpmacId, Family, Handle, MatchObject,
    TenantName,
};

use crate::intent_itf::{cname, field, set_items, text};
use crate::itf::{int64, num, tag};

/// The kernel end every unanchored edit link carries (`edits.qnt`: the alphabet's
/// unanchored constructs are kernel↔cfgN links), the fixed near end of the [`ConfigFacet::Link`]
/// projection.
const KERNEL: &str = "kernel";

/// Which phase a frozen state sits in (`edits.qnt` `phase`): a synced `Perturb` state
/// (the four laws hold) or a `Converge` state (whose transition to the next `Perturb`
/// is one `convergeStep`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// A synced state — the board is a fixpoint of the current intent.
    Perturb,
    /// A perturbed state awaiting the converge that returns it to synced.
    Converge,
}

/// One frozen edit-machine state, decoded into the Rust matcher's value world.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EditState {
    /// The compiled intent objects the matcher re-associates.
    pub intent: BTreeSet<MatchObject>,
    /// The board a prior converge left.
    pub board: BTreeSet<BoardObject>,
    /// Synced or perturbed.
    pub phase: Phase,
    /// The construct names the last perturbation touched — the frame law's cone.
    pub touched: BTreeSet<ConstructName>,
    /// The board before the last perturbation — the frame law's `pre`.
    pub pre_board: BTreeSet<BoardObject>,
}

/// The [`Family`] whose ITF constructor tag this is (`edits.qnt` is Dpni-only, but the
/// decode stays family-general like `intent_itf.rs`).
fn family(v: &Value) -> Result<Family, String> {
    let t = tag(v)?;
    ALL_FAMILIES
        .into_iter()
        .find(|f| f.variant_name() == t)
        .ok_or_else(|| format!("unknown family tag `{t}`"))
}

/// A model `Set[int]` anchor as the matcher's [`DpmacId`] set (`observed.qnt` `anchor`).
fn anchor(v: &Value) -> Result<BTreeSet<DpmacId>, String> {
    set_items(v)?
        .iter()
        .map(|e| Ok(DpmacId::new(num(e)?)))
        .collect()
}

/// The config facet from the anchor emptiness and the model's config int, branching on
/// anchor-emptiness FIRST (the facet mapping in the module docs): anchored ⇒
/// [`ConfigFacet::Anchored`] (int discarded); unanchored ⇒ [`ConfigFacet::Link`] with
/// ends `(kernel, cfgN)`.
fn facet(anchor: &BTreeSet<DpmacId>, config: &Value) -> Result<ConfigFacet, String> {
    if anchor.is_empty() {
        let n = int64(config)?;
        Ok(ConfigFacet::Link {
            ends: (
                TenantName::from(KERNEL),
                TenantName::from(format!("cfg{n}")),
            ),
        })
    } else {
        Ok(ConfigFacet::Anchored)
    }
}

/// The model's `str` name slot as the neutral optional: `""` ⇒ `None` (`intent_itf.rs`
/// `opt_name`), for `from` and `label`.
fn opt_cname(v: &Value) -> Result<Option<ConstructName>, String> {
    let s = text(v)?;
    Ok((!s.is_empty()).then(|| ConstructName::from(s)))
}

/// One `intent` element (`edits.qnt` `CompiledObject`) as a [`MatchObject`].
fn intent_obj(v: &Value) -> Result<MatchObject, String> {
    let anchor = anchor(field(v, "anchor")?)?;
    let config = facet(&anchor, field(v, "config")?)?;
    Ok(MatchObject {
        family: family(field(v, "family")?)?,
        name: cname(field(v, "name")?)?,
        anchor,
        config,
        from: opt_cname(field(v, "from")?)?,
    })
}

/// One `board` element (`edits.qnt` `Observed`) as a [`BoardObject`].
fn board_obj(v: &Value) -> Result<BoardObject, String> {
    let anchor = anchor(field(v, "anchor")?)?;
    let config = facet(&anchor, field(v, "config")?)?;
    Ok(BoardObject {
        family: family(field(v, "family")?)?,
        handle: Handle::new(num(field(v, "id")?)?),
        label: opt_cname(field(v, "label")?)?,
        anchor,
        config,
    })
}

fn board(v: &Value) -> Result<BTreeSet<BoardObject>, String> {
    set_items(v)?.iter().map(board_obj).collect()
}

fn phase(v: &Value) -> Result<Phase, String> {
    match text(v)?.as_str() {
        "perturb" => Ok(Phase::Perturb),
        "converge" => Ok(Phase::Converge),
        other => Err(format!("unknown phase `{other}`")),
    }
}

fn edit_state(state: &Value) -> Result<EditState, String> {
    Ok(EditState {
        intent: set_items(field(state, "intent")?)?
            .iter()
            .map(intent_obj)
            .collect::<Result<_, _>>()?,
        board: board(field(state, "board")?)?,
        phase: phase(field(state, "phase")?)?,
        touched: set_items(field(state, "touched")?)?
            .iter()
            .map(cname)
            .collect::<Result<_, _>>()?,
        pre_board: board(field(state, "preBoard")?)?,
    })
}

/// Parses a frozen edit-alphabet trace into its per-state [`EditState`]s.
///
/// # Errors
///
/// Returns a description of the first structural mismatch — a trace not produced by
/// `models/intent/edits.qnt` (missing `intent`/`board`/`phase`/`touched`/`preBoard`
/// vars, or a tag/shape the mapping layer does not recognise).
pub fn parse_edits_trace(json: &str) -> Result<Vec<EditState>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(edit_state)
        .collect()
}
