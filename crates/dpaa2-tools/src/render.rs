//! Pure text rendering of a compiled plan, its refusals, and the reconcile plan for
//! the `dpaa2ctl` `dry-run`/`ensure`/`status` surfaces (design D9/D10/D11).
//!
//! The frontend owns its text: `dpaa2-api` produces the object plan and the refusal
//! set as *data* — no `Display` impls (design D11) — and these pure functions turn
//! them into the operator-facing strings `main` prints. Every function is a pure map
//! from borrowed plan data to a [`String`], so the dry-run output is snapshot-tested
//! with `insta` against a hand-built [`dpaa2_api::compile`] result, board-free.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use dpaa2_api::{
    AttachPoint, Attributes, CompiledPlan, Container, Edge, Family, Measurement, ObjectKey, Plan,
    PlannedObject, ProvenanceKey, Refusal, Warning,
};

/// Renders the whole dry-run text: the compiled objects with their provenance trees
/// and edges, the transitions `reconcile` would execute, the plan-only report, and
/// any warnings (design D9/D10). This is the exact plan `ensure` executes, printed.
#[must_use]
pub fn render_dry_run(
    plan: &CompiledPlan,
    warnings: &BTreeSet<Warning>,
    reconcile: &Plan,
) -> String {
    let mut out = String::new();
    out.push_str(&render_plan(plan));
    out.push_str(&render_transitions(reconcile));
    out.push_str(&render_plan_only(&plan_only_by_family(plan)));
    out.push_str(&render_warnings(warnings));
    out
}

/// Renders the compiled objects in emission order, each with its provenance tree,
/// then the connection edges (design D6): part (a) and (b) of the dry-run.
#[must_use]
pub fn render_plan(plan: &CompiledPlan) -> String {
    let by_key: BTreeMap<&ObjectKey, &PlannedObject> =
        plan.objects.iter().map(|o| (o.key(), o)).collect();

    let mut out = String::new();
    let _ = writeln!(
        out,
        "plan: {} object(s), {} edge(s)",
        plan.objects.len(),
        plan.edges.len()
    );
    for key in &plan.order {
        let Some(obj) = by_key.get(key) else {
            continue;
        };
        let _ = writeln!(
            out,
            "  {} @ {}{}",
            key.label(),
            render_container(obj.container()),
            render_attrs(obj.attributes()),
        );
        let mut path = BTreeSet::new();
        render_prov_tree(plan, obj.provenance(), 2, &mut path, &mut out);
    }
    let _ = writeln!(out, "edges:");
    for edge in &plan.edges {
        let _ = writeln!(
            out,
            "  {} <-> {}  [{}]",
            render_attach(edge.a()),
            render_attach(edge.b()),
            edge.provenance().rule,
        );
    }
    out
}

/// Walks the provenance DAG from `key` recursively through its `inputs`, indenting
/// children (design D6). Each line names the rule, its construct (elided when empty),
/// the request, the extra (only when declared), the effective value, the
/// [`Measurement`] mark, and the evidence anchor — the operator's trace path down to
/// the declared construct and the ADR/baseline it cites.
fn render_prov_tree(
    plan: &CompiledPlan,
    key: &ProvenanceKey,
    indent: usize,
    path: &mut BTreeSet<ProvenanceKey>,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    let Some(node) = plan.provenance.get(key) else {
        // A referenced key with no node is a compiler gap; surface it, never hide it.
        let _ = writeln!(out, "{pad}- {} (no provenance node)", key.rule);
        return;
    };
    let construct = if key.construct.is_empty() {
        String::new()
    } else {
        format!(" construct={}", key.construct)
    };
    let extra = node.extra.map_or(String::new(), |e| format!(" extra={e}"));
    let _ = writeln!(
        out,
        "{pad}- {rule}{construct} request={request}{extra} value={value} [{mark}] anchor: {anchor}",
        rule = node.rule,
        request = node.request,
        value = node.value,
        mark = mark_str(node.mark),
        anchor = node.anchor,
    );
    // Provenance is a DAG; the path guard keeps a hypothetical cycle from looping.
    if !path.insert(key.clone()) {
        let _ = writeln!(out, "{pad}  ... (cycle)");
        return;
    }
    for input in &node.inputs {
        render_prov_tree(plan, input, indent + 1, path, out);
    }
    path.remove(key);
}

/// Renders the transitions, drift, and assertions `reconcile` produced (design D0):
/// part (c) of the dry-run, the lines `dry-run` printed before this parcel.
#[must_use]
pub fn render_transitions(plan: &Plan) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} planned transition(s):", plan.transitions.len());
    for t in &plan.transitions {
        let _ = writeln!(out, "  {t:?}");
    }
    for d in &plan.drift {
        let _ = writeln!(out, "  DRIFT {} {}: {}", d.dpni, d.attribute, d.detail);
    }
    for a in &plan.assertions {
        let _ = writeln!(out, "  ASSERT {} {}: {}", a.port, a.field, a.detail);
    }
    out
}

/// Counts the objects the port-facet reconciler has no executor for, by family
/// (design D10): every planned object except the port-edge dpnis it actuates. Task
/// 3.6 folds this into `reconcile`; until then it is a small pure fn the frontend
/// computes from the plan the reconciler already carries in its [`dpaa2_api::DesiredTopology`].
#[must_use]
pub fn plan_only_by_family(plan: &CompiledPlan) -> BTreeMap<Family, usize> {
    let actuated = port_edge_dpni_keys(plan);
    let mut summary: BTreeMap<Family, usize> = BTreeMap::new();
    for obj in &plan.objects {
        if !actuated.contains(obj.key()) {
            *summary.entry(obj.key().family).or_insert(0) += 1;
        }
    }
    summary
}

/// Renders the plan-only report (design D10): the families the reconciler does not
/// execute, grouped and counted, presented as plan-only — never drift, never error.
#[must_use]
pub fn render_plan_only(summary: &BTreeMap<Family, usize>) -> String {
    let total: usize = summary.values().sum();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "plan-only ({total}) [no port-facet executor; reported, not drift or error]:"
    );
    if summary.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for (fam, n) in summary {
        let _ = writeln!(out, "  {} x{n}", fam.as_str());
    }
    out
}

/// Renders every refusal with its named rule and offending construct (design D5/D10),
/// in the deterministic order the [`BTreeSet`] gives. The [`std::fmt::Debug`] form
/// leads with the exact token [`Refusal::name`] returns, so the rule name leads and
/// its fields — the construct/tenant/family and needed-vs-available amounts — follow.
#[must_use]
pub fn render_refusals(refusals: &BTreeSet<Refusal>) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "compilation refused: {} rule(s) broken",
        refusals.len()
    );
    for r in refusals {
        let _ = writeln!(out, "  {r:?}");
    }
    out
}

/// Renders the non-fatal warnings an accepted compile carried (design D2/D3), or the
/// empty string when there are none.
#[must_use]
pub fn render_warnings(warnings: &BTreeSet<Warning>) -> String {
    if warnings.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let _ = writeln!(out, "warnings ({}):", warnings.len());
    for w in warnings {
        let _ = writeln!(out, "  {w:?}");
    }
    out
}

// ---- endpoint / attribute / container formatting ----

/// The dpni object keys the port-facet reconciler actuates: the dpni end of every
/// dpni<->dpmac port-edge (design D10). Everything else in the plan is plan-only.
fn port_edge_dpni_keys(plan: &CompiledPlan) -> BTreeSet<ObjectKey> {
    plan.edges.iter().filter_map(port_edge_dpni_key).collect()
}

/// The dpni key a dpni<->dpmac port-edge connects, or `None` for any other edge (a
/// dpni<->dpni link/wire, a dpsw<->dpmac fabric-edge): the same discriminator
/// [`dpaa2_api::DesiredTopology::from_parts`] pairs the port projection on.
fn port_edge_dpni_key(edge: &Edge) -> Option<ObjectKey> {
    match (edge.a(), edge.b()) {
        (AttachPoint::Object { key, .. }, AttachPoint::Mac(_))
        | (AttachPoint::Mac(_), AttachPoint::Object { key, .. })
            if key.family == Family::Dpni =>
        {
            Some(key.clone())
        }
        _ => None,
    }
}

fn render_attach(ap: &AttachPoint) -> String {
    match ap {
        AttachPoint::Object { key, port } => format!("{}#{port}", key.label()),
        AttachPoint::Mac(dpmac) => dpmac.to_string(),
    }
}

fn render_container(c: &Container) -> String {
    match c {
        Container::Root => "root".to_owned(),
        Container::Child(t) => format!("child:{t}"),
    }
}

fn render_attrs(a: &Attributes) -> String {
    match a {
        Attributes::Unsized => String::new(),
        Attributes::Dpni { num_queues } => format!(" num_queues={num_queues}"),
        Attributes::Dpseci { num_queues, has_cg } => {
            format!(" num_queues={num_queues} has_cg={has_cg}")
        }
        Attributes::Dpsw { num_ifs, .. } => format!(" num_ifs={num_ifs}"),
        Attributes::Dprc { .. } => " (dprc default options)".to_owned(),
    }
}

fn mark_str(m: Measurement) -> &'static str {
    match m {
        Measurement::Measured => "measured",
        Measurement::Unmeasured => "unmeasured",
    }
}
