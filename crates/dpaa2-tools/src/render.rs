//! Pure text rendering of a compiled plan, its refusals, and the reconcile plan for
//! the `dpaa2ctl` `dry-run`/`ensure`/`status` surfaces (design D9/D10/D11; ADR-0006, restool-baseline).
//!
//! The frontend owns its text: `dpaa2-api` produces the object plan and the refusal
//! set as *data* — no `Display` impls (design D11; restool-baseline) — and these pure functions turn
//! them into the operator-facing strings `main` prints. Every function is a pure map
//! from borrowed plan data to a [`String`], so the dry-run output is snapshot-tested
//! with `insta` against a hand-built [`dpaa2_api::intent::refuse::compile`] result, board-free.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use dpaa2_api::core::family::Family;
use dpaa2_api::core::model::DprcId;
use dpaa2_api::intent::compiled::{
    AttachPoint, Attributes, CompiledPlan, Container, Measurement, ObjectKey, PlannedObject,
    ProvenanceKey,
};
use dpaa2_api::intent::refuse::{Refusal, Warning};
use dpaa2_api::plan::dprc::{ConsumerConvergence, ContainerVerdict, FingerprintField, PruneItem};
use dpaa2_api::plan::{Class, Plan, Transition};

use crate::engine::PoolDrift;

/// Renders the whole dry-run text: the compiled objects with their provenance trees
/// and edges, the transitions `reconcile` would execute, the plan-only report, and
/// any warnings (design D9/D10; ADR-0006, restool-baseline). This is the exact plan `ensure` executes, printed —
/// including the compiled `num_queues` each Create carries; a Create showing
/// `num_queues: 0` is the unsized port-only projection, which the backend sizes from the
/// host at execute time (synthesis L2/B3).
#[must_use]
pub fn render_dry_run(
    plan: &CompiledPlan,
    warnings: &BTreeSet<Warning>,
    reconcile: &Plan,
) -> String {
    let mut out = String::new();
    out.push_str(&render_plan(plan));
    out.push_str(&render_transitions(reconcile));
    out.push_str(&render_plan_only(&reconcile.plan_only));
    out.push_str(&render_warnings(warnings));
    out
}

/// Renders the compiled objects in emission order, each with its provenance tree,
/// then the connection edges (design D6; ADR-0004): part (a) and (b) of the dry-run.
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
            "  {key} label={} @ {}{}",
            obj.label(),
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
/// children (design D6; ADR-0004). Each line names the rule, its construct (elided when empty),
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

/// Renders the transitions, drift, and assertions `reconcile` produced (design D0; add-dpaa2-provisioning):
/// part (c) of the dry-run, the lines `dry-run` printed before this parcel. Each
/// transition leads with its disruption class and the block leads with the plan's
/// headline — the maximum class, the one converge gates on (ADR-0015 decision 12).
#[must_use]
pub fn render_transitions(plan: &Plan) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} planned transition(s) [headline: {}]:",
        plan.transitions.len(),
        plan.headline(),
    );
    for t in &plan.transitions {
        let _ = writeln!(out, "  [{}] {}", t.class(), render_transition(t));
    }
    for d in &plan.drift {
        let _ = writeln!(out, "  DRIFT {} {}: {}", d.dpni, d.attribute, d.detail);
    }
    for a in &plan.assertions {
        let _ = writeln!(out, "  ASSERT {} {}: {}", a.port, a.field, a.detail);
    }
    out
}

/// One dry-run transition line. `Create` gets a concise summary — its label, the
/// compiled `num_queues`, and the derived option names — rather than a full `DpniCfg`
/// Debug dump (dpni-typestate task 4.1); every other transition prints its Debug form.
fn render_transition(t: &Transition) -> String {
    match t {
        Transition::Create { port, label, cfg } => {
            let mut opts: Vec<&str> = cfg.options.flags().iter().map(|f| f.name()).collect();
            opts.extend(cfg.options.escapes().iter().map(|e| e.name()));
            format!(
                "Create {{ port: {port}, label: {label:?}, num_queues: {}, options: [{}] }}",
                cfg.num_queues.get(),
                opts.join(",")
            )
        }
        other => format!("{other:?}"),
    }
}

/// Renders the plan-only report (design D10; restool-baseline): the families the reconciler does not
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

/// Renders the child-DPRC (consumer container) convergence the dry-run would drive
/// (design D2/D6; ADR-0002, ADR-0004; reconciler delta): each declared consumer's container-only plan, its
/// re-observation verdict (DPRC-I6), and — the per-object provenance the operator
/// traces — the derived container's provenance node resolved to its baseline anchor via
/// the plan's DAG (the 4.1 `ConsumerContainer.provenance` key). A converged container
/// shows zero steps: the second run's zero-action proof, printed.
#[must_use]
pub fn render_container_convergence(
    plan: &CompiledPlan,
    convergences: &[ConsumerConvergence],
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "container convergence ({} consumer(s)) [container-only: no companion/dpni steps]:",
        convergences.len()
    );
    if convergences.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for c in convergences {
        let _ = writeln!(
            out,
            "  {label} [{verdict}] {n} step(s) [headline: {headline}]",
            label = c.container.label,
            verdict = render_verdict(&c.verdict),
            n = c.plan.steps.len(),
            headline = c.plan.headline(),
        );
        // The container's provenance: the rule node citing the baseline anchor.
        let mut path = BTreeSet::new();
        render_prov_tree(plan, &c.container.provenance, 2, &mut path, &mut out);
        for step in &c.plan.steps {
            let _ = writeln!(out, "    [{}] {step:?}", step.class());
        }
    }
    out
}

/// Renders the root-scope pool convergence the run would drive (pool-objects task 3.4;
/// pool-objects design D3): a header with the pass headline, then per trio family its
/// observed-vs-derived counts, the class-tagged disposition (create/destroy/prune, or the
/// [`ShrinkBelowDraw`](dpaa2_api::families::pool_lifecycle::ShrinkBelowDraw) refusal), and the
/// family's provenance node resolved to its baseline anchor — the same class-gating and
/// per-object provenance conventions the container steps use. The dpio seats follow as a
/// grow-only line. A converged root shows an all-hitless headline and empty dispositions:
/// the idempotent second run's zero-action proof, printed.
#[must_use]
pub fn render_pool_drift(plan: &CompiledPlan, drift: &PoolDrift) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "root pool convergence ({} famil(ies) + dpio seats) [headline: {}]:",
        drift.families.len(),
        drift.headline(),
    );
    for f in &drift.families {
        match f.disposition {
            Ok(deltas) => {
                let class = if deltas.is_empty() {
                    Class::Hitless
                } else {
                    Class::Disruptive
                };
                let _ = writeln!(
                    out,
                    "  {} observed managed={}/{} required={} [{class}] create={} destroy={} prune={}",
                    f.family.name(),
                    f.census.managed(),
                    f.census.population(),
                    f.required,
                    deltas.create,
                    deltas.destroy,
                    deltas.prune,
                );
            }
            // A requirement below the drawn count is surfaced, never a teardown (pool-objects design D3).
            Err(refusal) => {
                let _ = writeln!(
                    out,
                    "  {} observed managed={}/{} required={} REFUSED: {refusal}",
                    f.family.name(),
                    f.census.managed(),
                    f.census.population(),
                    f.required,
                );
            }
        }
        render_root_provenance(plan, f.family.family(), &mut out);
    }
    // dpio is a seat, grown never shrunk (pool-objects design D4).
    let dpio_class = if drift.dpio_required > drift.dpio_observed {
        Class::Disruptive
    } else {
        Class::Hitless
    };
    let _ = writeln!(
        out,
        "  dpio seats observed={} required={} [{dpio_class}]",
        drift.dpio_observed, drift.dpio_required,
    );
    render_root_provenance(plan, Family::Dpio, &mut out);
    out
}

/// Renders the provenance node of a planned root object of `family`, when one exists — the
/// operator's trace down to the declared construct and the ADR/baseline it cites, matching
/// the container-convergence provenance render (ADR-0004 design D6).
fn render_root_provenance(plan: &CompiledPlan, family: Family, out: &mut String) {
    let Some(obj) = plan
        .objects
        .iter()
        .find(|o| o.container() == &Container::Root && o.key().family == family)
    else {
        return;
    };
    let mut path = BTreeSet::new();
    render_prov_tree(plan, obj.provenance(), 2, &mut path, out);
}

/// Renders a container verdict as a short operator token.
fn render_verdict(verdict: &ContainerVerdict) -> String {
    match verdict {
        ContainerVerdict::Converged => "converged".to_owned(),
        ContainerVerdict::Diverged(reasons) => format!("diverged: {reasons:?}"),
    }
}

/// Renders the undeclared-consumer prune report (dprc-encapsulation task 4.3; reconciler
/// spec "Undeclared consumer containers are pruned under the double gate"): a header with
/// the candidate and report-only counts, then per container its id, [`PruneBucket`] name,
/// matched/unmatched fingerprint fields, and — for a candidate — the eviction-law teardown
/// steps and predicted post-state (ADR-0007 §3). Nothing is dispatched without `--prune`
/// and `--allow disruptive`, which the header states, so the same text serves the dry-run
/// (which never dispatches) and the `ensure` report.
///
/// [`PruneBucket`]: dpaa2_api::plan::dprc::PruneBucket
#[must_use]
pub fn render_prune(items: &BTreeMap<DprcId, PruneItem>) -> String {
    let candidates = items.values().filter(|i| i.plan.is_some()).count();
    let report_only = items.len() - candidates;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "prune ({candidates} candidate(s), {report_only} report-only) \
         [double gate: --prune --allow disruptive]:"
    );
    if items.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for (id, item) in items {
        let c = &item.classification;
        let _ = writeln!(
            out,
            "  {id} [{}] matched: {} unmatched: {}",
            c.bucket.name(),
            render_fields(&c.matched),
            render_fields(&c.unmatched),
        );
        let Some(plan) = &item.plan else {
            continue;
        };
        for step in &plan.steps {
            let _ = writeln!(out, "    [{}] {step:?}", step.class());
        }
        if let Some(pred) = &plan.predicted {
            // Origin is unobservable through restool, so a gained resident of unknown
            // origin is counted conservatively and flagged rather than lying that the
            // parent gains nothing (review M2; PASS3-F2).
            let unknown = pred
                .parent_gained
                .values()
                .filter(|r| r.origin.is_none())
                .count();
            let _ = write!(
                out,
                "    post-state: {:?}, parent gains {} resident(s)",
                pred.final_state,
                pred.parent_gained.len(),
            );
            if unknown > 0 {
                let _ = write!(
                    out,
                    " ({unknown} origin-unobservable, conservatively evicted)"
                );
            }
            let _ = writeln!(out);
        }
    }
    out
}

/// Renders a set of fingerprint fields as a bracketed, comma-joined list.
fn render_fields(fields: &BTreeSet<FingerprintField>) -> String {
    let names: Vec<String> = fields.iter().map(|f| format!("{f:?}")).collect();
    format!("[{}]", names.join(", "))
}

/// Renders every refusal with its named rule and offending construct (design D5/D10; ADR-0003, restool-baseline),
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

/// Renders the non-fatal warnings an accepted compile carried (design D2/D3; ADR-0002), or the
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

fn render_attach(ap: &AttachPoint) -> String {
    match ap {
        AttachPoint::Object { key, port } => format!("{key}#{port}"),
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
        Attributes::Dpni { cfg } => {
            // num_queues plus the derived option set (flag then escape names), so dry-run
            // shows the chosen profile options (dpni-typestate design D3).
            let mut opts: Vec<&str> = cfg.options.flags().iter().map(|f| f.name()).collect();
            opts.extend(cfg.options.escapes().iter().map(|e| e.name()));
            format!(
                " num_queues={} options=[{}]",
                cfg.num_queues.get(),
                opts.join(",")
            )
        }
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
