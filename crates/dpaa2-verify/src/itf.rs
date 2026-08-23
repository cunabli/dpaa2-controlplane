//! Reader for frozen Quint ITF traces (`models/traces/*.itf.json`).
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
    let var = root["vars"][0]
        .as_str()
        .ok_or("trace has no state variable")?
        .to_owned();
    root["states"]
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(|state| view(&state[&var]))
        .collect()
}

/// The `#bigint`-encoded integer of an ITF value.
fn num(v: &Value) -> Result<u32, String> {
    v["#bigint"]
        .as_str()
        .ok_or_else(|| format!("not a #bigint: {v}"))?
        .parse()
        .map_err(|e| format!("bad integer: {e}"))
}

/// The constructor tag of an ITF sum-type value (e.g. `Dpni`, `Unbound`).
fn tag(v: &Value) -> Result<&str, String> {
    v["tag"]
        .as_str()
        .ok_or_else(|| format!("not a variant: {v}"))
}

/// Family tag and object number of an ITF-encoded `ObjId`.
fn obj_id(v: &Value) -> Result<(&str, u32), String> {
    Ok((tag(&v["fam"])?, num(&v["num"])?))
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
