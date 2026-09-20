//! Reader for frozen Quint ITF traces (`models/traces/<aspect>/*.itf.json`).
//!
//! An ITF trace serializes every state of one directed run of the core
//! machine (`models/core/machine.qnt`). The replayer does not need the
//! whole `CoreState` — only the slice the reconciler can observe through
//! restool and the kernel — so each state is reduced here to a
//! [`ModelView`]: which DPNIs and DPMACs exist, which edges connect
//! them, and whether a DPNI is kernel-bound (the model's netdev proxy).
//! Everything else (pools, containers, visibility, pristine state) is
//! machinery the reconciler never sees.

use std::collections::BTreeMap;

use serde_json::Value;

use dpaa2_api::core::family::{ALL_FAMILIES, Family};

/// What the model exposes of one DPNI to an observer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DpniView {
    /// The DPMAC index this DPNI is connected to, from the model's
    /// connection edges (either endpoint order).
    pub connected_to: Option<u32>,
    /// Kernel-bound (`bind == BoundKernel`) — the model-side proxy for
    /// "a netdev exists".
    pub bound: bool,
}

/// The reconciler-observable slice of one model state.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ModelView {
    /// DPNIs by object number.
    pub dpnis: BTreeMap<u32, DpniView>,
    /// DPMAC object numbers present.
    pub dpmacs: Vec<u32>,
}

/// Parses a full ITF trace into the observable view of every state.
///
/// # Errors
///
/// Returns a description of the first structural mismatch — a trace not
/// produced by `quint test --out-itf` over the core machine.
pub fn parse_trace(json: &str) -> Result<Vec<ModelView>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    // The CoreState variable, tolerating a trace that also froze the
    // machine's `lastVerbs` var (the reconciler view never reads it): pick the
    // non-`mbt::`, non-`lastVerbs` var rather than assuming it is index 0.
    let var = root["vars"]
        .as_array()
        .ok_or("trace has no vars")?
        .iter()
        .filter_map(Value::as_str)
        .find(|v| !v.starts_with("mbt::") && !v.ends_with("lastVerbs"))
        .ok_or("trace has no state variable")?
        .to_owned();
    root["states"]
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(|state| view(&state[&var]))
        .collect()
}

/// The `#bigint`-encoded integer of an ITF value, as a `u32` (object numbers,
/// ordinals, ports). Shared with the intent replayer ([`crate::intent::intent_itf`]).
pub(crate) fn num(v: &Value) -> Result<u32, String> {
    v["#bigint"]
        .as_str()
        .ok_or_else(|| format!("not a #bigint: {v}"))?
        .parse()
        .map_err(|e| format!("bad integer: {e}"))
}

/// The `#bigint`-encoded integer of an ITF value, as an `i64` (rates, counts,
/// derived request/value — the intent model's wider integers).
pub(crate) fn int64(v: &Value) -> Result<i64, String> {
    v["#bigint"]
        .as_str()
        .ok_or_else(|| format!("not a #bigint: {v}"))?
        .parse()
        .map_err(|e| format!("bad integer: {e}"))
}

/// The constructor tag of an ITF sum-type value (e.g. `Dpni`, `Unbound`).
/// Shared with the intent replayer ([`crate::intent::intent_itf`]).
pub(crate) fn tag(v: &Value) -> Result<&str, String> {
    v["tag"]
        .as_str()
        .ok_or_else(|| format!("not a variant: {v}"))
}

/// A required object field, erroring when the key is absent (JSON `null`). The
/// generic record-field reader shared across the ITF alphabets.
pub(crate) fn field<'a>(v: &'a Value, name: &str) -> Result<&'a Value, String> {
    let f = &v[name];
    if f.is_null() {
        return Err(format!("missing field `{name}` in {v}"));
    }
    Ok(f)
}

/// An ITF value as an owned `String`.
pub(crate) fn text(v: &Value) -> Result<String, String> {
    v.as_str()
        .ok_or_else(|| format!("not a string: {v}"))
        .map(str::to_owned)
}

/// The elements of an ITF `#set`.
pub(crate) fn set_items(v: &Value) -> Result<&Vec<Value>, String> {
    v["#set"]
        .as_array()
        .ok_or_else(|| format!("not a #set: {v}"))
}

/// The [`Family`] whose `variant_name` is `tag` — the ITF constructor tag (`"Dpni"`,
/// `"Dprc"`, …). The single family-by-tag scan the intent, edits, and MBT-adapter
/// readers share.
pub(crate) fn family_of_tag(tag: &str) -> Option<Family> {
    ALL_FAMILIES.into_iter().find(|f| f.variant_name() == tag)
}

/// Family tag and object number of an ITF-encoded `ObjId`.
pub(crate) fn obj_id(v: &Value) -> Result<(&str, u32), String> {
    Ok((tag(&v["fam"])?, num(&v["num"])?))
}

/// An ITF-encoded `ObjId` (`{fam, num}`) as a corpus [`Family`] and ordinal — the strict decode
/// the `CoreState`-carrying readers ([`crate::intent::pool_itf`], [`crate::intent::dpio_itf`])
/// share. An unknown family tag fails, so the reader's types mirror the qnt `Family` sum rather
/// than flattening tags to strings (ADR-0002 §3).
pub(crate) fn obj_ref(v: &Value) -> Result<(Family, u32), String> {
    let (t, n) = obj_id(v)?;
    Ok((
        family_of_tag(t).ok_or_else(|| format!("unknown family tag `{t}`"))?,
        n,
    ))
}

/// An ITF-encoded `Option[ObjId]` (`{tag: Some|None}`) as an optional [`Family`]/ordinal —
/// the `ObjState` `parent`/`allocatedBy` custody edges the pool and dpio readers walk.
pub(crate) fn opt_obj_ref(v: &Value) -> Result<Option<(Family, u32)>, String> {
    match tag(v)? {
        "None" => Ok(None),
        "Some" => Ok(Some(obj_ref(&v["value"])?)),
        other => Err(format!("not an Option tag: `{other}`")),
    }
}

/// The value of machine variable `name` in a frozen state, matching the bare name or a
/// module-qualified `<module>::…::<name>` key. A run inheriting its vars from an imported
/// module qualifies them (`dpbp_lifecycle::pool_lifecycle::s`); a run owning them does not
/// (`s`). A value-less state (a `.fail()` step froze only `#meta`) has no match and errors —
/// the caller reads that as the disabled-guard sentinel.
pub(crate) fn state_var<'a>(state: &'a Value, name: &str) -> Result<&'a Value, String> {
    let obj = state.as_object().ok_or("state is not an object")?;
    let suffix = format!("::{name}");
    obj.iter()
        .find(|(k, _)| k.as_str() == name || k.ends_with(&suffix))
        .map(|(_, v)| v)
        .ok_or_else(|| format!("state has no `{name}` variable"))
}

/// Reduces one ITF-encoded `CoreState` to its observable view.
fn view(s: &Value) -> Result<ModelView, String> {
    // dpni↔dpmac edges first, so DPNI rows can carry their peer.
    let mut edges: BTreeMap<u32, u32> = BTreeMap::new();
    for conn in s["conns"]["#set"].as_array().ok_or("conns is not a set")? {
        let pair = conn["#tup"].as_array().ok_or("conn is not a pair")?;
        let a = obj_id(&pair[0]["obj"])?;
        let b = obj_id(&pair[1]["obj"])?;
        for ((fam_x, num_x), (fam_y, num_y)) in [(a, b), (b, a)] {
            if fam_x == "Dpni" && fam_y == "Dpmac" {
                edges.insert(num_x, num_y);
            }
        }
    }

    let mut out = ModelView::default();
    for entry in s["objs"]["#map"].as_array().ok_or("objs is not a map")? {
        let (fam, n) = obj_id(&entry[0])?;
        match fam {
            "Dpni" => {
                out.dpnis.insert(
                    n,
                    DpniView {
                        connected_to: edges.get(&n).copied(),
                        bound: tag(&entry[1]["bind"])? == "BoundKernel",
                    },
                );
            }
            "Dpmac" => out.dpmacs.push(n),
            _ => {}
        }
    }
    Ok(out)
}
