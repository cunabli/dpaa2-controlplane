//! Reader and registry twin for the frozen orchestration ITF traces
//! (`models/traces/orchestration/*.itf.json`, emitted by `models/orchestration/pipeline.qnt`
//! via `pnpm model:freeze-orch`; resource-aware-orchestration task 3.2).
//!
//! The orchestration model is an interface projection (ADR-0023 design D11): its state is the
//! census schema the engine can read — per-construct status, dpmac occupancy, the two budget
//! counts, the pending plan multisets, the admission verdict, and the typed-refusal record —
//! not the core object machine. So the replayable Rust truth is the plan-entry family dispatch
//! ([`dpaa2_api::plan::dispatch`]) plus the judgments its dispositions derive, NOT the engine's
//! IO passes. This reader reduces each frozen state to an `OrchState` over that schema, and
//! `tests/orchestration_replay.rs` walks the states asserting the dispatch agrees step by step
//! (the dpsw reader+replay precedent, [`crate::intent::dpsw_itf`]).
//!
//! The registry half (`row_set_findings`) is the twin of the dispatch's row set against the
//! model's `TABLE`: it text-parses `pipeline.qnt`'s `TABLE` Map (the model-text-is-truth
//! precedent of [`crate::intent::lint`]) and reconciles it with [`dpaa2_api::plan::dispatch::TABLE`].
//! The sole sanctioned asymmetry is dpmac — a model-only `Execute(NoCreate)` row with no plan
//! `Attributes` variant and so no Rust row (ADR-0023 design D10/D11; the dispatch.rs TABLE doc
//! states the same). Any other asymmetry — a family on one side only, a draw-kind or
//! disposition-kind disagreement, or a divergent `Unimplemented` trigger string — is a finding.

use std::collections::BTreeMap;

use serde_json::Value;

use dpaa2_api::core::family::Family;
use dpaa2_api::plan::dispatch::{Disposition, DrawKind};

use crate::itf::{family_of_tag, field, int64, tag};

/// Per-construct status (`pipeline.qnt` `type Status`): the four dispositions the engine observes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// No construct present.
    Missing,
    /// Present and at intent.
    Converged,
    /// Present but drifted from intent.
    Drifted,
    /// Present but unwanted (teardown pending).
    Orphan,
}

/// dpmac driver occupancy (`pipeline.qnt` `type Occupancy`; ADR-0023 design D9 live-read).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Occupancy {
    /// Driver-free.
    Free,
    /// Held by the standalone mac driver.
    MacHeld,
    /// Taken by the switch driver.
    SwitchHeld,
}

/// The admission verdict (`pipeline.qnt` `type Verdict`; ADR-0023 design D5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    /// Not yet judged this loop.
    Unjudged,
    /// Admitted — the gross draw fits.
    Admitted,
    /// Refused whole and typed — the gross draw overdraws a budget.
    AdmissionRefused,
}

/// One frozen orchestration state, reduced to the census-schema projection (ADR-0023 design D11).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OrchState {
    /// Per-construct status, keyed by family (`status`).
    pub status: BTreeMap<Family, Status>,
    /// Pending plan multiset per family (`pending`).
    pub pending: BTreeMap<Family, i64>,
    /// Typed whole-intent refusal recorded per family (`refused`).
    pub refused: BTreeMap<Family, bool>,
    /// The admission verdict for the compiled plan (`verdict`).
    pub verdict: Verdict,
    /// Live dpmac occupancy (`occ`).
    pub occ: Occupancy,
    /// Last-observed occupancy, the census-read snapshot (`censusOcc`).
    pub census_occ: Occupancy,
    /// The switch-port edge status (`portWired`).
    pub port_wired: bool,
    /// Container-pool budget, free count (`poolFree`; ADR-0011).
    pub pool_free: i64,
    /// dpsw FDB budget, free count (`fdbFree`; V-DPSW-2).
    pub fdb_free: i64,
}

fn status_of(v: &Value) -> Result<Status, String> {
    Ok(match tag(v)? {
        "Missing" => Status::Missing,
        "Converged" => Status::Converged,
        "Drifted" => Status::Drifted,
        "Orphan" => Status::Orphan,
        other => return Err(format!("unknown Status tag `{other}`")),
    })
}

fn occupancy_of(v: &Value) -> Result<Occupancy, String> {
    Ok(match tag(v)? {
        "Free" => Occupancy::Free,
        "MacHeld" => Occupancy::MacHeld,
        "SwitchHeld" => Occupancy::SwitchHeld,
        other => return Err(format!("unknown Occupancy tag `{other}`")),
    })
}

fn verdict_of(v: &Value) -> Result<Verdict, String> {
    Ok(match tag(v)? {
        "Unjudged" => Verdict::Unjudged,
        "Admitted" => Verdict::Admitted,
        "AdmissionRefused" => Verdict::AdmissionRefused,
        other => return Err(format!("unknown Verdict tag `{other}`")),
    })
}

fn bool_of(v: &Value) -> Result<bool, String> {
    v.as_bool().ok_or_else(|| format!("not a bool: {v}"))
}

/// A family-keyed ITF `#map`, each value decoded by `parse`.
fn family_map<T>(
    v: &Value,
    parse: impl Fn(&Value) -> Result<T, String>,
) -> Result<BTreeMap<Family, T>, String> {
    let mut m = BTreeMap::new();
    for entry in v["#map"]
        .as_array()
        .ok_or_else(|| format!("not a #map: {v}"))?
    {
        let pair = entry.as_array().ok_or("map entry is not a pair")?;
        let t = tag(&pair[0])?;
        let fam = family_of_tag(t).ok_or_else(|| format!("unknown family tag `{t}`"))?;
        m.insert(fam, parse(&pair[1])?);
    }
    Ok(m)
}

fn orch_state(s: &Value) -> Result<OrchState, String> {
    Ok(OrchState {
        status: family_map(field(s, "status")?, status_of)?,
        pending: family_map(field(s, "pending")?, int64)?,
        refused: family_map(field(s, "refused")?, bool_of)?,
        verdict: verdict_of(field(s, "verdict")?)?,
        occ: occupancy_of(field(s, "occ")?)?,
        census_occ: occupancy_of(field(s, "censusOcc")?)?,
        port_wired: bool_of(field(s, "portWired")?)?,
        pool_free: int64(field(s, "poolFree")?)?,
        fdb_free: int64(field(s, "fdbFree")?)?,
    })
}

/// Parses a frozen orchestration trace into its per-state [`OrchState`]s, skipping the
/// `#meta`-only sentinel a `.fail()` step freezes (the disabled-guard marker; the `itf::state_var`
/// doc records that shape).
///
/// # Errors
///
/// Returns a description of the first structural mismatch or unknown sum tag — a trace not
/// produced by `pipeline.qnt` over the census-schema vocabulary.
pub fn parse_orchestration_trace(json: &str) -> Result<Vec<OrchState>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for s in field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
    {
        // A `.fail()` step froze only `#meta`: no state variables, the disabled-guard sentinel.
        if s.get("status").is_none_or(Value::is_null) {
            continue;
        }
        out.push(orch_state(s)?);
    }
    Ok(out)
}

// ===================== registry twin (ADR-0023 design D10/D11) =====================

/// A model `TABLE` row's disposition (`pipeline.qnt` `type Disposition`), parsed from the qnt
/// source. The model carries no engine pass, so an `Execute` row is just its draw kind — the
/// twin compares draw kinds and disposition kinds, never the Rust pass string.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ModelDisp {
    /// A delivered family and the resource its create draws.
    Execute(DrawKind),
    /// No consuming pass — the typed whole-intent refusal and its retirement trigger.
    Unimplemented(String),
}

fn draw_of(s: &str) -> Result<DrawKind, String> {
    Ok(match s {
        "PoolDraw" => DrawKind::PoolDraw,
        "FdbDraw" => DrawKind::FdbDraw,
        "PlainCreate" => DrawKind::PlainCreate,
        "NoCreate" => DrawKind::NoCreate,
        other => return Err(format!("unknown ResKind `{other}`")),
    })
}

/// One `TABLE` row's right-hand side (`Execute(<ResKind>)` or `Unimplemented("<trigger>")`).
fn parse_disp(s: &str) -> Result<ModelDisp, String> {
    let s = s.trim_end_matches(',').trim();
    if let Some(inner) = s.strip_prefix("Execute(").and_then(|r| r.strip_suffix(')')) {
        Ok(ModelDisp::Execute(draw_of(inner.trim())?))
    } else if let Some(rest) = s.strip_prefix("Unimplemented(") {
        let inner = rest
            .strip_suffix(')')
            .ok_or_else(|| format!("malformed Unimplemented row `{s}`"))?;
        Ok(ModelDisp::Unimplemented(
            inner.trim().trim_matches('"').to_owned(),
        ))
    } else {
        Err(format!("unknown disposition `{s}`"))
    }
}

/// Parses `pipeline.qnt`'s `pure val TABLE: Family -> Disposition = Map(…)` rows, text-tolerant
/// of whitespace but intolerant of content drift (the [`crate::intent::lint`] parsing precedent).
///
/// # Errors
///
/// Returns a description if the `TABLE` block is absent/empty or a row names an unknown family,
/// draw kind, or disposition shape.
pub fn parse_model_table(qnt: &str) -> Result<Vec<(Family, ModelDisp)>, String> {
    let mut rows = Vec::new();
    let mut in_block = false;
    for line in qnt.lines() {
        if !in_block {
            if line.contains("pure val TABLE") && line.contains("Map(") {
                in_block = true;
            }
            continue;
        }
        let t = line.trim();
        if t.starts_with(')') {
            break; // the Map literal's close ends the block
        }
        let Some((lhs, rhs)) = t.split_once("->") else {
            continue;
        };
        let fam_tag: String = lhs
            .trim()
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if fam_tag.is_empty() {
            continue;
        }
        let fam = family_of_tag(&fam_tag)
            .ok_or_else(|| format!("TABLE names unknown family `{fam_tag}`"))?;
        rows.push((fam, parse_disp(rhs.trim())?));
    }
    if rows.is_empty() {
        return Err("pipeline.qnt has no TABLE rows".to_owned());
    }
    Ok(rows)
}

/// A disposition disagreement between the model row and the Rust row for family `f`.
fn push_disp_mismatch(f: Family, m: &ModelDisp, r: &Disposition, out: &mut Vec<String>) {
    let name = f.variant_name();
    match (m, r) {
        (ModelDisp::Execute(md), Disposition::Execute { draw, .. }) => {
            if md != draw {
                out.push(format!(
                    "row-set twin: {name} draw kind disagrees — model {md:?}, Rust {draw:?}"
                ));
            }
        }
        (ModelDisp::Unimplemented(mt), Disposition::Unimplemented { trigger }) => {
            if mt != trigger {
                out.push(format!(
                    "row-set twin: {name} Unimplemented trigger disagrees — model {mt:?}, Rust {trigger:?}"
                ));
            }
        }
        (ModelDisp::Execute(_), Disposition::Unimplemented { .. }) => out.push(format!(
            "row-set twin: {name} is Execute in the model but Unimplemented in Rust"
        )),
        (ModelDisp::Unimplemented(_), Disposition::Execute { .. }) => out.push(format!(
            "row-set twin: {name} is Unimplemented in the model but Execute in Rust"
        )),
    }
}

/// The registry twin: reconciles the Rust dispatch's row set with `pipeline.qnt`'s `TABLE`,
/// returning one finding per drift (an empty vector is the green verdict). Red when a family
/// sits on one side only (except the sanctioned dpmac exemption), when an `Execute` draw kind
/// or a disposition kind disagrees, or when an `Unimplemented` trigger string differs
/// (resource-aware-orchestration task 3.2; ADR-0023 design D10/D11).
#[must_use]
pub fn row_set_findings(pipeline_qnt: &str, rust_table: &[(Family, Disposition)]) -> Vec<String> {
    let mut out = Vec::new();
    let model = match parse_model_table(pipeline_qnt) {
        Ok(m) => m,
        Err(e) => {
            out.push(format!(
                "row-set twin: cannot parse pipeline.qnt TABLE: {e}"
            ));
            return out;
        }
    };
    let model_disp = |f: Family| model.iter().find(|(x, _)| *x == f).map(|(_, d)| d);
    let rust_disp = |f: Family| rust_table.iter().find(|(x, _)| *x == f).map(|(_, d)| d);

    // model -> Rust: every model row has a Rust row, except dpmac — the one sanctioned
    // model-only asymmetry (DPC-born, no plan `Attributes` variant), which must be exactly
    // Execute(NoCreate) and absent from the Rust table (ADR-0023 design D10/D11).
    for (f, mdisp) in &model {
        match rust_disp(*f) {
            Some(rdisp) => push_disp_mismatch(*f, mdisp, rdisp, &mut out),
            None if *f == Family::Dpmac => {
                if *mdisp != ModelDisp::Execute(DrawKind::NoCreate) {
                    out.push(format!(
                        "row-set twin: Dpmac is the sanctioned model-only exemption but its row is {mdisp:?}, not Execute(NoCreate)"
                    ));
                }
            }
            None => out.push(format!(
                "row-set twin: model TABLE has {} with no Rust dispatch row",
                f.variant_name()
            )),
        }
    }
    // Rust -> model: every Rust row has a model row (no exemption this direction).
    for (f, _) in rust_table {
        if model_disp(*f).is_none() {
            out.push(format!(
                "row-set twin: Rust dispatch has {} with no model TABLE row",
                f.variant_name()
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real `TABLE` block (`pipeline.qnt` lines ~77-87) plus a trailing declaration the
    /// parser must not fold in, so the block boundary (`)` close) is exercised.
    const TABLE_QNT: &str = "\
  pure val TABLE: Family -> Disposition = Map(
    Dprc   -> Execute(PlainCreate),
    Dpni   -> Execute(PlainCreate),
    Dpbp   -> Execute(PoolDraw),
    Dpio   -> Execute(PoolDraw),
    Dpcon  -> Execute(PoolDraw),
    Dpmcp  -> Execute(PoolDraw),
    Dpseci -> Unimplemented(\"crypto-provisioning change (ADR-0023 D10)\"),
    Dpsw   -> Execute(FdbDraw),
    Dpmac  -> Execute(NoCreate)
  )
  pure val TABLE_FAMS: Set[Family] = TABLE.keys()";

    // The Rust dispatch's rows, single-sourced from the real const so the positive test tracks
    // the production table (DrawKind/pass verbatim; the pass is not part of the comparison).
    fn rust_rows() -> Vec<(Family, Disposition)> {
        dpaa2_api::plan::dispatch::TABLE.to_vec()
    }

    #[test]
    fn parses_the_model_table_rows() {
        let rows = parse_model_table(TABLE_QNT).unwrap();
        assert_eq!(rows.len(), 9);
        assert_eq!(
            rows[0],
            (Family::Dprc, ModelDisp::Execute(DrawKind::PlainCreate))
        );
        assert_eq!(
            rows[8],
            (Family::Dpmac, ModelDisp::Execute(DrawKind::NoCreate))
        );
        assert_eq!(
            rows[6],
            (
                Family::Dpseci,
                ModelDisp::Unimplemented("crypto-provisioning change (ADR-0023 D10)".to_owned())
            )
        );
    }

    #[test]
    fn twin_is_green_when_the_tables_agree() {
        // The real dispatch rows against the real-shaped TABLE: dpmac is the lone model-only
        // row and must not be flagged.
        let out = row_set_findings(TABLE_QNT, &rust_rows());
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn twin_flags_a_rust_row_without_a_model_row() {
        // The model text drops the Dprc row; the Rust table still carries it.
        let qnt_bad = TABLE_QNT.replace("    Dprc   -> Execute(PlainCreate),\n", "");
        let out = row_set_findings(&qnt_bad, &rust_rows());
        assert!(
            out.iter()
                .any(|m| m.contains("Rust dispatch has Dprc with no model TABLE row")),
            "{out:?}"
        );
    }

    #[test]
    fn twin_flags_a_model_row_without_a_rust_row() {
        // A phantom model family (Dpdmux) with no Rust row — not the dpmac exemption, so red.
        let qnt_bad = TABLE_QNT.replace(
            "    Dpsw   -> Execute(FdbDraw),\n",
            "    Dpsw    -> Execute(FdbDraw),\n    Dpdmux  -> Execute(PlainCreate),\n",
        );
        let out = row_set_findings(&qnt_bad, &rust_rows());
        assert!(
            out.iter()
                .any(|m| m.contains("model TABLE has Dpdmux with no Rust dispatch row")),
            "{out:?}"
        );
    }

    #[test]
    fn twin_flags_a_draw_kind_disagreement() {
        // The model recasts Dpni's plain create as a pool draw.
        let qnt_bad = TABLE_QNT.replace(
            "    Dpni   -> Execute(PlainCreate),",
            "    Dpni   -> Execute(PoolDraw),",
        );
        let out = row_set_findings(&qnt_bad, &rust_rows());
        assert!(
            out.iter().any(|m| m.contains("Dpni draw kind disagrees")),
            "{out:?}"
        );
    }

    #[test]
    fn twin_flags_a_divergent_unimplemented_trigger() {
        let qnt_bad = TABLE_QNT.replace(
            "crypto-provisioning change (ADR-0023 D10)",
            "some other change",
        );
        let out = row_set_findings(&qnt_bad, &rust_rows());
        assert!(
            out.iter()
                .any(|m| m.contains("Dpseci Unimplemented trigger disagrees")),
            "{out:?}"
        );
    }

    #[test]
    fn twin_flags_a_disposition_kind_disagreement() {
        // The Rust side (constructed, not the real const) recasts Dpseci as an Execute row.
        let mut rows = rust_rows();
        for row in &mut rows {
            if row.0 == Family::Dpseci {
                row.1 = Disposition::Execute {
                    draw: DrawKind::PlainCreate,
                    pass: "phantom",
                };
            }
        }
        let out = row_set_findings(TABLE_QNT, &rows);
        assert!(
            out.iter()
                .any(|m| m.contains("Dpseci is Unimplemented in the model but Execute in Rust")),
            "{out:?}"
        );
    }

    #[test]
    fn twin_flags_a_corrupted_dpmac_exemption() {
        // dpmac stays model-only (the exemption) but its row is no longer Execute(NoCreate).
        let qnt_bad = TABLE_QNT.replace(
            "    Dpmac  -> Execute(NoCreate)",
            "    Dpmac  -> Execute(PoolDraw)",
        );
        let out = row_set_findings(&qnt_bad, &rust_rows());
        assert!(
            out.iter()
                .any(|m| m.contains("Dpmac is the sanctioned model-only exemption")),
            "{out:?}"
        );
    }
}
