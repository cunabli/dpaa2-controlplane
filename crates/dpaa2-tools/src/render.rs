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

use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::model::DprcId;
use dpaa2_api::families::dpio::SeatDisposition;
use dpaa2_api::families::dpmac::{CarrierReading, CounterRead, CounterReadout};
use dpaa2_api::families::pool_lifecycle::{
    PoolCensus, PoolDeltas, PoolDisposition, PoolFamily, ShrinkBelowDraw,
};
use dpaa2_api::intent::compiled::{
    AttachPoint, Attributes, CompiledPlan, Container, Measurement, ObjectKey, PlannedObject,
    ProvenanceKey,
};
use dpaa2_api::intent::refuse::{Refusal, Warning};
use dpaa2_api::plan::admission::AdmissionRefusal;
use dpaa2_api::plan::capacity::{GlobalBudget, PoolHeadroom};
use dpaa2_api::plan::dprc::{ConsumerConvergence, ContainerVerdict, FingerprintField, PruneItem};
use dpaa2_api::plan::pool::PoolDrift;
use dpaa2_api::plan::populate::ChildPlan;
use dpaa2_api::plan::rollback::{ActuationRecord, FailurePolicy, Residue};
use dpaa2_api::plan::rollback_exec::FailureDisposition;
use dpaa2_api::plan::{Class, Plan, Transition};

use dpaa2_api::contract::{DpseciPortalReadout, DpswIfFace};
use dpaa2_api::core::model::ObjectRef;
use dpaa2_api::families::dpsw::{
    BroadcastCfg, ComponentType, CounterReadout as DpswCounterReadout, DpswCounterRead,
    FdbAgingTime, FloodingCfg, IfLinkState, MaxFdbEntries, MaxFdbMcGroups, MaxFdbs, MaxVlans,
    NumIfs, ObservedOpts, Taildrop,
};
use dpaa2_api::plan::connect::ChildDeferredVisibility;

use crate::engine::{
    Admission, LinkConnection, LinkDryRun, LinkDryRunAction, LinkRow, SwitchVerdict, SwitchWork,
};
use crate::status::{DpseciRow, DpswRow, PortDetail};

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
        // A dpsw roots at its MAC-table sizing node, which inputs the dpsw node — so
        // the trace names the derived `max_fdb_entries` rule (mac-table /
        // mac-table-default) above the base dpsw rule (dpsw-typestate task 2.1).
        let root = dpsw_sizing_root(plan, obj).unwrap_or_else(|| obj.provenance().clone());
        render_prov_tree(plan, &root, 2, &mut path, &mut out);
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

/// The MAC-table sizing node's key for a dpsw object, when one exists: same tenant
/// and construct as the object's own `dpsw` provenance, rule `mac-table` (declared)
/// or `mac-table-default` (not). Rooting the tree here surfaces the derived
/// `max_fdb_entries` rule above the dpsw node it inputs (dpsw-typestate task 2.1).
fn dpsw_sizing_root(plan: &CompiledPlan, obj: &PlannedObject) -> Option<ProvenanceKey> {
    if !matches!(obj.attributes(), Attributes::Dpsw { .. }) {
        return None;
    }
    let p = obj.provenance();
    ["mac-table", "mac-table-default"]
        .into_iter()
        .map(|rule| ProvenanceKey::new(p.tenant.clone(), rule, p.construct.clone()))
        .find(|k| plan.provenance.contains_key(k))
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

/// Renders the dpni↔dpni link transitions `dry-run` would execute (cross-dprc-links task 5.5;
/// provisioning-cli delta req 1): one line per link in the `[class] verb` style, followed by the
/// link-edge provenance tree (the "link-edge" rule node, via `render_prov_tree`). A held-end is
/// the typed refusal line. The rendered transition is the exact one `ensure` actuates — the planner
/// ([`crate::engine::plan_links`]) shares its resolution and `plan_wire` authority.
#[must_use]
pub fn render_links(plan: &CompiledPlan, links: &[LinkDryRun]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "link transitions ({} link(s)) [exact plan ensure would execute]:",
        links.len()
    );
    if links.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for l in links {
        match &l.action {
            LinkDryRunAction::Connect => {
                let _ = writeln!(
                    out,
                    "  [{}] connect {} <-> {} @ {}",
                    Class::Disruptive,
                    l.a,
                    l.b,
                    l.ancestor
                );
            }
            LinkDryRunAction::Converged => {
                let _ = writeln!(
                    out,
                    "  [{}] {} <-> {} already connected",
                    Class::Hitless,
                    l.a,
                    l.b
                );
            }
            LinkDryRunAction::Pending => {
                let _ = writeln!(
                    out,
                    "  [{}] {} <-> {} pending (an end not yet resident)",
                    Class::Hitless,
                    l.a,
                    l.b
                );
            }
            LinkDryRunAction::Held(r) => {
                let _ = writeln!(out, "  REFUSED {} <-> {}: {r}", l.a, l.b);
            }
        }
        let mut path = BTreeSet::new();
        render_prov_tree(plan, &l.provenance, 2, &mut path, &mut out);
    }
    out
}

/// Renders the switched-domain create/recreate verdicts `dry-run` would execute (dpsw-hardening task 5.1):
/// a header naming the dpsw count and the gate headline (the max class over the work),
/// then one line per dpsw in the `[class] verb` style, each followed by the dpsw's provenance tree
/// (rooted at its MAC-table sizing node, as `render_plan` does). The rendered verdicts are the
/// exact set `ensure` judges — the planner ([`crate::engine::plan_switches`]) shares its verdict
/// authority, so the preview never disagrees with the convergence (review finding M4).
#[must_use]
pub fn render_switches(plan: &CompiledPlan, work: &[SwitchWork<'_>]) -> String {
    let mut out = String::new();
    let headline = work.iter().map(|w| w.class).max().unwrap_or_default();
    let _ = writeln!(
        out,
        "switch transitions ({} dpsw(s)) [headline: {headline}] [exact plan ensure would execute]:",
        work.len()
    );
    if work.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for w in work {
        match &w.verdict {
            SwitchVerdict::Create => {
                let _ = writeln!(out, "  [{}] create dpsw {}", w.class, w.obj.label());
            }
            SwitchVerdict::Recreate { destroy } => {
                let _ = writeln!(
                    out,
                    "  [{}] recreate {} (destroy {destroy})",
                    w.class,
                    w.obj.label()
                );
            }
            SwitchVerdict::Converged => {
                let _ = writeln!(out, "  [{}] {} converged", w.class, w.obj.label());
            }
        }
        let mut path = BTreeSet::new();
        let root = dpsw_sizing_root(plan, w.obj).unwrap_or_else(|| w.obj.provenance().clone());
        render_prov_tree(plan, &root, 2, &mut path, &mut out);
    }
    out
}

/// Renders the admission verdict beside the dry-run plan (resource-aware-orchestration task 5.1;
/// provisioning-cli delta req 1): a header carrying the drawn-budget count and the overall
/// verdict, then one data-only row per budget the intent draws on — the family (or `fdb`), the
/// gross draw, the observed headroom, and the per-row verdict. Headroom reads the face: a
/// reclaimable pool prints its free count; the dpio seats print `spent` (ADR-0003 §7
/// non-reclaimable); an unobserved family prints `unobserved`; an unknown FDB bracket prints
/// `unknown` (ADR-0011 admit-and-warn). One line per row, operator terms, no law IDs (ADR-0023 design D5).
/// A family with a proven MC-global budget renders that row in the `mc-global` voice beside its
/// census row (plan-rollback design D7), so a hardware-side drain the census cannot see still shows a refused budget.
#[must_use]
pub fn render_admission(adm: &Admission) -> String {
    let empty = Vec::new();
    let refusals = adm.verdict.as_ref().err().unwrap_or(&empty);
    let mut rows = Vec::new();
    for (&family, &draw) in &adm.draw.pool {
        if draw > 0 {
            rows.push(admission_pool_row(adm, family, draw, refusals));
            if let Some(g) = adm.face.globals.iter().find(|g| g.family == family) {
                rows.push(admission_global_row(g, draw, refusals));
            }
        }
    }
    if adm.draw.fdb > 0 {
        rows.push(admission_fdb_row(adm, refusals));
    }
    let verdict = if adm.verdict.is_ok() {
        "admitted"
    } else {
        "refused"
    };
    let mut out = String::new();
    let _ = writeln!(
        out,
        "admission ({} budget(s) drawn) [{verdict}]:",
        rows.len()
    );
    if rows.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for r in rows {
        let _ = writeln!(out, "  {r}");
    }
    out
}

/// One pool-budget row's text: family, draw, headroom, and the per-row verdict (refused when
/// this family is in the refusal set).
fn admission_pool_row(
    adm: &Admission,
    family: Family,
    draw: i64,
    refusals: &[AdmissionRefusal],
) -> String {
    let headroom = adm
        .face
        .pools
        .iter()
        .find(|p| p.family == family)
        .map_or_else(
            || "unobserved".to_owned(),
            |p| match p.headroom {
                PoolHeadroom::Free(n) => n.to_string(),
                PoolHeadroom::Spent(_) => "spent".to_owned(),
            },
        );
    let refused = refusals
        .iter()
        .any(|r| matches!(r, AdmissionRefusal::Pool { family: f, .. } if *f == family));
    format!(
        "{} draw={draw} headroom={headroom} [{}]",
        family.as_str(),
        admission_verdict(refused)
    )
}

/// One MC-global budget row's text in the distinguishable `mc-global` voice (plan-rollback design D7):
/// family, draw, the observed global free headroom, and the per-row verdict (refused when a
/// `GlobalPool` refusal names this family).
fn admission_global_row(g: &GlobalBudget, draw: i64, refusals: &[AdmissionRefusal]) -> String {
    let refused = refusals
        .iter()
        .any(|r| matches!(r, AdmissionRefusal::GlobalPool { family: f, .. } if *f == g.family));
    format!(
        "mc-global {} draw={draw} headroom={} [{}]",
        g.family.as_str(),
        g.free,
        admission_verdict(refused)
    )
}

/// The dpsw FDB budget row's text: draw, headroom (`unknown` on an unobserved bracket), and the
/// per-row verdict.
fn admission_fdb_row(adm: &Admission, refusals: &[AdmissionRefusal]) -> String {
    let headroom = adm
        .face
        .fdb
        .free()
        .map_or_else(|| "unknown".to_owned(), |n| n.to_string());
    let refused = refusals
        .iter()
        .any(|r| matches!(r, AdmissionRefusal::Fdb { .. }));
    format!(
        "fdb draw={} headroom={headroom} [{}]",
        adm.draw.fdb,
        admission_verdict(refused)
    )
}

/// The per-row verdict token.
fn admission_verdict(refused: bool) -> &'static str {
    if refused { "refused" } else { "admitted" }
}

/// Renders the `ensure` pre-dispatch admission refusals in the `refused:` voice
/// (resource-aware-orchestration task 5.1; provisioning-cli delta req 1): one line per
/// over-budget draw naming the budget, the draw, and the free headroom it exceeded — data only,
/// operator terms, no law IDs (ADR-0023 design D5). Printed before any verb, so a refused run
/// dispatched nothing (ADR-0023 design D7).
#[must_use]
pub fn render_admission_refusal(refusals: &[AdmissionRefusal]) -> String {
    let mut out = String::new();
    for r in refusals {
        match r {
            AdmissionRefusal::Pool {
                family,
                draw,
                headroom,
            } => {
                let _ = writeln!(
                    out,
                    "refused: pool {} draw {draw} exceeds free headroom {headroom}",
                    family.as_str()
                );
            }
            AdmissionRefusal::GlobalPool {
                family,
                draw,
                headroom,
            } => {
                let _ = writeln!(
                    out,
                    "refused: mc-global pool {} draw {draw} exceeds global free headroom {headroom}",
                    family.as_str()
                );
            }
            AdmissionRefusal::Fdb { draw, headroom } => {
                let _ = writeln!(
                    out,
                    "refused: dpsw fdb draw {draw} exceeds free headroom {headroom}"
                );
            }
        }
    }
    out
}

/// Renders the read-only `status --detail` link and obligation rows (cross-dprc-links task 5.5;
/// provisioning-cli delta req 3): one row per dpni↔dpni link — endpoints and connection state from
/// `dprc_get_connection` — and one per standing child-keyed obligation. Display-only: an
/// unavailable read renders as explicitly unknown (the honest-unknown idiom), and the header states
/// the view gates no convergence, so the command's exit code is untouched.
#[must_use]
pub fn render_link_detail(rows: &[LinkRow], obligations: &[ChildDeferredVisibility]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "link detail ({} link(s)) [read-only; never gates convergence]:",
        rows.len()
    );
    if rows.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for r in rows {
        let _ = writeln!(
            out,
            "  {link} {a} <-> {b} connection={conn}",
            link = r.link,
            a = render_opt_ref(r.a),
            b = render_opt_ref(r.b),
            conn = render_link_connection(&r.connection),
        );
    }
    let _ = writeln!(
        out,
        "obligations ({}) [read-only; gates no convergence]:",
        obligations.len()
    );
    if obligations.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for o in obligations {
        let _ = writeln!(out, "  {o}");
    }
    out
}

/// Renders the declined-consent drift refusal (cross-dprc-links task 5.5; provisioning-cli delta
/// req 2): the typed standing [`ChildDeferredVisibility`] residue, then the actionable re-run hint
/// in the `gate_refusal_message` idiom — the `--allow=disruptive` flag IS the consent machinery
/// (ADR-0015 decision 12), so the Disruptive rebind is named, never implied and never silent.
#[must_use]
pub fn render_drift_refusal(residue: &ChildDeferredVisibility, allowed: Class) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "refused: {residue}");
    let _ = writeln!(
        out,
        "the rebind cycle is `{}`, but the run allows only up to `{allowed}`; re-run with \
         `--allow=disruptive` to actuate it (disruptive is never implied).",
        Class::Disruptive
    );
    out
}

/// An observed link endpoint, or the honest-unknown token when the end is not resident.
fn render_opt_ref(obj: Option<ObjectRef>) -> String {
    obj.map_or_else(|| "unknown".to_owned(), |o| o.to_string())
}

/// The operator token for a link connection state; an unjudgeable read is explicitly unknown.
fn render_link_connection(c: &LinkConnection) -> String {
    match c {
        LinkConnection::Connected => "connected".to_owned(),
        LinkConnection::ConnectedElsewhere(peer) => format!("connected-elsewhere ({peer})"),
        LinkConnection::Disconnected => "disconnected".to_owned(),
        LinkConnection::Unknown => "unknown".to_owned(),
    }
}

/// Renders the plan-only report (design D10; restool-baseline): the families carrying no port-facet
/// `Transition` — the ensure engine passes actuate them — grouped and counted, presented as
/// plan-only; never drift, never error.
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

/// Writes one pool family's observed-vs-derived counts and its class-tagged disposition (or the
/// [`ShrinkBelowDraw`] refusal) at `indent`
/// — the shared block of the root drift and the child population renders (pool-objects design D3).
/// The per-surface residue line is left to the caller, whose indent differs.
fn render_family_disposition(
    out: &mut String,
    indent: &str,
    family: PoolFamily,
    census: PoolCensus,
    required: i64,
    disposition: Result<PoolDeltas, ShrinkBelowDraw>,
) {
    match disposition {
        Ok(deltas) => {
            let class = if deltas.is_empty() {
                Class::Hitless
            } else {
                Class::Disruptive
            };
            let _ = writeln!(
                out,
                "{indent}{} observed managed={}/{} required={required} [{class}] create={} destroy={} prune={}",
                family.name(),
                census.managed(),
                census.population(),
                deltas.create,
                deltas.destroy,
                deltas.prune,
            );
        }
        // A requirement below the drawn count is surfaced, never a teardown (pool-objects design D3).
        Err(refusal) => {
            let _ = writeln!(
                out,
                "{indent}{} observed managed={}/{} required={required} REFUSED: {refusal}",
                family.name(),
                census.managed(),
                census.population(),
            );
        }
    }
}

/// The class a grow-only dpio seat line reports: a deficit is [`Class::Disruptive`], parity is
/// [`Class::Hitless`] (pool-objects design D4). Shared by the root and child seat lines.
fn seat_class(required: i64, observed: i64) -> Class {
    if required > observed {
        Class::Disruptive
    } else {
        Class::Hitless
    }
}

/// Renders the root-scope pool convergence the run would drive (pool-objects task 3.4;
/// pool-objects design D3): a header with the pass headline, then per trio family its
/// observed-vs-derived counts, the class-tagged disposition (create/destroy/prune, or the
/// [`ShrinkBelowDraw`] refusal), and the
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
        render_family_disposition(
            &mut out,
            "  ",
            f.family,
            f.census,
            f.required,
            f.disposition,
        );
        // A root managed surplus renders as grow-only reboot-required residue on every surface (ADR-0020; same voice as the dpio residue below).
        if let PoolDisposition::RebootRequired(residue) = f.residue() {
            let _ = writeln!(out, "    reboot-required: {residue}");
        }
        render_root_provenance(plan, f.family.family(), &mut out);
    }
    // dpio is a seat, grown never shrunk (pool-objects design D4).
    let dpio_class = seat_class(drift.dpio_required, drift.dpio_observed);
    let _ = writeln!(
        out,
        "  dpio seats observed={} required={} [{dpio_class}]",
        drift.dpio_observed, drift.dpio_required,
    );
    // A surplus is the typed reboot-required residue, reported not reclaimed (pool-objects design D4/D10).
    if let SeatDisposition::RebootRequired(residue) = drift.dpio_disposition() {
        let _ = writeln!(out, "    reboot-required: {residue}");
    }
    render_root_provenance(plan, Family::Dpio, &mut out);
    out
}

/// Renders the child-population pass the run would drive (pool-objects design D11;
/// system-integration req 1): a header with the combined headline, then per child its
/// re-observation handle, converged/pending state and VFIO bind, the dpni arity (planned vs
/// present, connected vs peers), the trio families' observed-vs-derived counts and
/// class-tagged disposition (or the [`ShrinkBelowDraw`] refusal), and the dpio seats — the
/// same class-gating conventions the pool drift uses. A converged, bound board shows an
/// all-hitless headline and empty dispositions: the idempotent run's zero-action proof.
///
/// [`ShrinkBelowDraw`]: dpaa2_api::families::pool_lifecycle::ShrinkBelowDraw
#[must_use]
pub fn render_population(children: &[ChildPlan]) -> String {
    let headline = children
        .iter()
        .map(ChildPlan::headline)
        .max()
        .unwrap_or(Class::Hitless);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "child population ({} child(ren)) [headline: {headline}]:",
        children.len(),
    );
    if children.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for cp in children {
        let present = cp.dpnis.iter().filter(|d| d.observed.is_some()).count();
        let connected = cp.dpnis.iter().filter(|d| d.connected).count();
        let peers = cp.dpnis.iter().filter(|d| d.peer.is_some()).count();
        let state = if cp.is_converged() {
            "converged"
        } else {
            "pending"
        };
        let _ = writeln!(
            out,
            "  {label} [{child}] {state} [headline: {ch}] bound={bound}",
            label = cp.label,
            child = cp.child,
            ch = cp.headline(),
            bound = cp.vfio_bound(),
        );
        let _ = writeln!(
            out,
            "    dpni present={present}/{planned} connected={connected}/{peers}",
            planned = cp.dpnis.len(),
        );
        for (family, (required, census, disposition)) in &cp.families {
            render_family_disposition(&mut out, "    ", *family, *census, *required, *disposition);
        }
        let dpio_class = seat_class(cp.seats.0, cp.seats.1);
        let _ = writeln!(
            out,
            "    dpio seats observed={} required={} [{dpio_class}]",
            cp.seats.1, cp.seats.0,
        );
        if let SeatDisposition::RebootRequired(residue) = cp.dpio_disposition() {
            let _ = writeln!(out, "      reboot-required: {residue}");
        }
    }
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
        match r {
            // Hardware terms with the operator's own numbers, no process identifiers
            // (dpsw-typestate task 2.1): the product is derived for the line.
            Refusal::MacTableOverflow {
                fabric,
                num_ifs,
                mac_table_size,
                ..
            } => {
                let product = u64::from(*mac_table_size) * u64::from(*num_ifs);
                let _ = writeln!(
                    out,
                    "  switch '{fabric}': mac_table_size {mac_table_size} × {num_ifs} ports = {product} exceeds max_fdb_entries (16-bit field, max 65535)"
                );
            }
            Refusal::FabricMemberOverflow {
                fabric,
                num_ifs,
                bound,
            } => {
                let _ = writeln!(
                    out,
                    "  switch '{fabric}': {num_ifs} member interfaces exceeds max_fdbs (u8 field, max {bound})"
                );
            }
            Refusal::MacTableOnSoftwareFabric { fabric } => {
                let _ = writeln!(
                    out,
                    "  fabric '{fabric}' is software-switched: mac_table_size sizes the hardware switch MAC table; the software forwarder owns its own"
                );
            }
            Refusal::EmptyFabric { fabric } => {
                let _ = writeln!(
                    out,
                    "  switch '{fabric}' has no member interfaces: a hardware switched domain needs at least one (the MC accepts 1 to 65535)"
                );
            }
            _ => {
                let _ = writeln!(out, "  {r:?}");
            }
        }
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

/// Renders the read-only per-port detail view (provisioning-cli spec "The CLI exposes a
/// read-only port-detail view"): one block per port with arbitration, the MAC relation, the
/// link carrier, and the vocabulary counters — every value sourced from a read-back. The
/// header states the view is read-only and never gates convergence (link readings lag PHY
/// reality per V-LINK-2; counters are traffic-dependent), so the same text serves `status
/// --detail` without touching the exit code.
#[must_use]
pub fn render_port_details(details: &[PortDetail]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "port detail ({} port(s)) [read-only; never gates convergence]:",
        details.len()
    );
    if details.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for d in details {
        let _ = writeln!(
            out,
            "  {dpmac} {name}: arbitration={arb:?} mac={rel:?} carrier={carrier}",
            dpmac = d.dpmac,
            name = d.name.as_str(),
            arb = d.arbitration,
            rel = d.mac_relation,
            carrier = render_carrier(d.carrier),
        );
        render_counters(&mut out, &d.counters);
    }
    out
}

/// Renders the read-only dpseci detail view (dpseci-typestate task 4.1), mirroring
/// [`render_port_details`]: one line per object with its queue counts, tx priorities, and
/// plugged/drawn binding state, then the privileged portal readout (options + API version) or
/// the honest-unknown line when the portal was unavailable this run. The header states the view
/// is read-only and never gates convergence, so the same text serves `status --detail` without
/// touching the exit code (dpseci-typestate design D5).
#[must_use]
pub fn render_dpseci_details(rows: &[DpseciRow]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "dpseci detail ({} object(s)) [read-only; never gates convergence]:",
        rows.len()
    );
    if rows.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for r in rows {
        let _ = writeln!(
            out,
            "  {object} queues tx={tx}/rx={rx} tx-priorities=[{prios}] plugged={plugged} drawn={drawn}",
            object = r.object,
            tx = render_opt_queue(r.detail.num_tx_queues),
            rx = render_opt_queue(r.detail.num_rx_queues),
            prios = render_priorities(&r.detail.tx_priorities),
            plugged = r.plugged,
            drawn = r.drawn,
        );
        render_dpseci_portal(&mut out, &r.detail.portal);
    }
    out
}

/// A queue count the restool `info` parse witnessed, or the crate's absent-read idiom (`-`, the
/// same honest-unknown spelling the status netdev cell uses) when the line was absent.
fn render_opt_queue(count: Option<u8>) -> String {
    count.map_or_else(|| "-".to_owned(), |n| n.to_string())
}

/// The per-queue tx priorities as a comma-joined list (empty when the line was absent).
fn render_priorities(prios: &[u8]) -> String {
    prios
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Renders the dpseci portal readout under its object line. An [`DpseciPortalReadout::Observed`]
/// prints the named option flags, then each attributed raw escape as `raw:0x<value>`, and the
/// API version — the desired-face idiom of [`render_attrs`] (flags then escapes). An unnamed
/// bit therefore renders by its raw value, never a bare unknown token, closing the
/// readback-vs-desired asymmetry (dpseci-hardening design D1). An [`DpseciPortalReadout::Unobservable`]
/// prints the honest-unknown line carrying its reason — the carrier `no-observable (<reason>)`
/// idiom — never an error (dpseci-typestate design D5).
fn render_dpseci_portal(out: &mut String, portal: &DpseciPortalReadout) {
    match portal {
        DpseciPortalReadout::Observed {
            options,
            api_major,
            api_minor,
        } => {
            let mut opts: Vec<String> = options
                .flags()
                .iter()
                .map(|f| f.name().to_owned())
                .collect();
            opts.extend(
                options
                    .escapes()
                    .iter()
                    .map(|e| format!("raw:0x{:x}", e.raw_value())),
            );
            let opts = opts.join(",");
            let _ = writeln!(out, "    options=[{opts}] version={api_major}.{api_minor}");
        }
        DpseciPortalReadout::Unobservable { reason } => {
            let _ = writeln!(out, "    portal=no-observable ({reason})");
        }
    }
}

/// Renders the read-only dpsw switch detail view (dpsw-typestate task 5.1), mirroring
/// [`render_dpseci_details`]: one object line with the `dpsw info` attributes, decoded options,
/// and plugged/drawn binding state, then a per-interface table keyed by interface index carrying
/// the endpoint, link state, taildrop, and counters. Every honest-unknown face — an absent
/// attribute, an absent options line, an [`Unobservable`](DpswIfFace::Unobservable) port, an absent
/// link/taildrop/max-frame-length — renders the literal `unknown`, never blank or zero
/// (provisioning-cli spec "The CLI exposes a read-only switch detail view"; DPSW-I7). A genuinely
/// free port is the distinct `disconnected` token. The header states the view is read-only and
/// never gates convergence, so link and counters are display-only and the exit code stays
/// port-driven (dpsw-typestate design D6).
#[must_use]
pub fn render_dpsw_details(rows: &[DpswRow]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "dpsw detail ({} object(s)) [read-only; never gates convergence]:",
        rows.len()
    );
    if rows.is_empty() {
        let _ = writeln!(out, "  (none)");
    }
    for r in rows {
        let d = &r.detail;
        let _ = writeln!(
            out,
            "  {object} num_ifs={num_ifs} max_vlans={max_vlans} max_fdbs={max_fdbs} \
             max_fdb_entries={mfe} fdb_aging_time={aging} max_fdb_mc_groups={mcg} \
             component_type={ct} flooding_cfg={fc} broadcast_cfg={bc} mem_size={mem} \
             plugged={plugged} drawn={drawn}",
            object = r.object,
            num_ifs = render_opt(d.num_ifs.map(NumIfs::get)),
            max_vlans = render_opt(d.max_vlans.map(MaxVlans::get)),
            max_fdbs = render_opt(d.max_fdbs.map(MaxFdbs::get)),
            mfe = render_opt(d.max_fdb_entries.map(MaxFdbEntries::get)),
            aging = render_opt(d.fdb_aging_time.map(FdbAgingTime::get)),
            mcg = render_opt(d.max_fdb_mc_groups.map(MaxFdbMcGroups::get)),
            ct = render_opt(d.component_type.map(ComponentType::name)),
            fc = render_opt(d.flooding_cfg.map(FloodingCfg::name)),
            bc = render_opt(d.broadcast_cfg.map(BroadcastCfg::name)),
            mem = render_opt(d.mem_size),
            plugged = r.plugged,
            drawn = r.drawn,
        );
        render_dpsw_options(&mut out, d.options.as_ref());
        for (idx, iface) in &d.ifaces {
            let _ = writeln!(
                out,
                "    if {idx}: endpoint={ep} link={link} taildrop={td} max_frame_length={mfl}",
                ep = render_if_face(&iface.face),
                link = render_if_link(iface.link),
                td = render_taildrop(iface.taildrop.as_ref()),
                mfl = render_opt(iface.max_frame_length),
            );
            render_dpsw_counters(&mut out, &iface.counters);
        }
    }
    out
}

/// An observed optional attribute, or the crate honest-unknown token when `info` omitted the
/// line (provisioning-cli spec: an absent attribute is `unknown`, never blank or zero —
/// distinct from the dpseci queue `-`, which is a separate surface).
fn render_opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or_else(|| "unknown".to_owned(), |x| x.to_string())
}

/// Renders the dpsw observed options under its object line, the [`render_dpseci_portal`] idiom:
/// a present [`ObservedOpts`] prints the named MC flags then each attributed raw escape as
/// `raw:0x<value>`, so an unnamed bit renders by its raw value (dpsw-typestate design D6). An
/// absent options line (`None`) is the honest-unknown token, never an empty list.
fn render_dpsw_options(out: &mut String, options: Option<&ObservedOpts>) {
    match options {
        Some(o) => {
            let mut opts: Vec<String> = o.named().iter().map(|f| f.name().to_owned()).collect();
            opts.extend(
                o.escapes()
                    .iter()
                    .map(|e| format!("raw:0x{:x}", e.raw_value())),
            );
            let _ = writeln!(out, "    options=[{}]", opts.join(","));
        }
        None => {
            let _ = writeln!(out, "    options=unknown");
        }
    }
}

/// The operator token for one interface's connection face. A
/// [`Connected`](DpswIfFace::Connected) port names its endpoint ref; a
/// [`Disconnected`](DpswIfFace::Disconnected) port is genuinely free; an
/// [`Unobservable`](DpswIfFace::Unobservable) face is honest-unknown, never read as free
/// (DPSW-I7; provisioning-cli spec "A blanked connection line is not freedom").
fn render_if_face(face: &DpswIfFace) -> String {
    match face {
        DpswIfFace::Connected(obj) => obj.to_string(),
        DpswIfFace::Disconnected => "disconnected".to_owned(),
        DpswIfFace::Unobservable => "unknown".to_owned(),
    }
}

/// The operator token for an interface link state; an absent `link state:` line is honest-unknown,
/// never a defaulted `down` (DPSW-I7).
fn render_if_link(link: Option<IfLinkState>) -> String {
    match link {
        Some(IfLinkState::Up) => "up".to_owned(),
        Some(IfLinkState::Down) => "down".to_owned(),
        Some(IfLinkState::NotApplicable) => "n/a".to_owned(),
        None => "unknown".to_owned(),
    }
}

/// Renders the taildrop readout inline; an absent block is honest-unknown, never defaulted zeros.
fn render_taildrop(td: Option<&Taildrop>) -> String {
    td.map_or_else(
        || "unknown".to_owned(),
        |t| {
            format!(
                "enabled={},units={},threshold={}",
                t.enabled, t.units, t.threshold
            )
        },
    )
}

/// Renders a dpsw per-interface counter read-back, the [`render_counters`] idiom for the dpsw
/// vocabulary type. A truncated mid-list row reads `unread`, never `0`, so a silent truncation is
/// never shown as zeros (DPSW-I7).
fn render_dpsw_counters(out: &mut String, counters: &DpswCounterReadout) {
    let rows = counters.rows();
    let _ = writeln!(out, "      counters ({}):", rows.len());
    for (name, read) in rows {
        let value = match read {
            DpswCounterRead::Known(v) => v.to_string(),
            DpswCounterRead::Unread => "unread".to_owned(),
        };
        let _ = writeln!(out, "        {name} = {value}");
    }
}

/// The operator token for a carrier reading. `NoObservable` renders as the driverless-port
/// diagnosis, never as `down` (provisioning-cli spec).
fn render_carrier(carrier: CarrierReading) -> &'static str {
    match carrier {
        CarrierReading::Up => "up",
        CarrierReading::Down => "down",
        CarrierReading::NoObservable => "no-observable (driverless port)",
    }
}

/// Renders a counter read-back under a port block. A [`CounterReadout::Vocabulary`] prints one
/// row per counter under its carried verbatim name (the shim parsed it; DPMAC-I7), each name
/// travelling in the same tuple as its value. A [`CounterReadout::VersionSignal`] prints the deviation
/// line, never zero rows (DPMAC-I7).
fn render_counters(out: &mut String, counters: &CounterReadout) {
    match counters {
        CounterReadout::Vocabulary(rows) => {
            let _ = writeln!(out, "    counters ({}):", rows.len());
            for (name, read) in rows {
                let value = match read {
                    CounterRead::Known(v) => v.to_string(),
                    CounterRead::NotInVocabulary => "n/a".to_owned(),
                };
                let _ = writeln!(out, "      {name} = {value}");
            }
        }
        CounterReadout::VersionSignal { expected, got } => {
            let _ = writeln!(
                out,
                "    counters: version-signal (expected {expected}, got {got})"
            );
        }
    }
}

/// Renders the one-line mid-run failure-policy preview the dry-run prints last.
/// Anchor: plan-rollback design D6; spec "The dry-run output SHALL state the run's selected
/// policy" — the operator sees the failure disposition the run would execute under beside the
/// plan. Pure text, operator data terms, no type names.
#[must_use]
pub fn render_failure_policy(policy: FailurePolicy) -> String {
    match policy {
        FailurePolicy::Rollback => {
            "on failure: rollback — a mid-run failure unwinds this run's recorded actuations in \
             reverse order\n"
                .to_owned()
        }
        FailurePolicy::Abort => {
            "on failure: abort — a mid-run failure stops the run and leaves the executed prefix \
             standing for the next run\n"
                .to_owned()
        }
        // FailurePolicy is #[non_exhaustive]; the deferred `continue` variant lands by adding an
        // arm here with its own preview wording (plan-rollback design D6).
        _ => "on failure: a policy with no preview wording yet\n".to_owned(),
    }
}

/// Renders a mid-run failure and how the run's failure policy disposed of it.
/// Anchor: plan-rollback design D6; spec "Rollback outcomes are rendered in operator data terms".
/// The failure class, then — under
/// rollback — the reverse-order unwind (the actuations undone, each typed reboot residue, and any row
/// the environment left unreversed), or — under abort — the executed prefix left standing for the next
/// run. Every line is the operator's data vocabulary, never an internal type name or status code.
///
/// Overcommit (the firmware's no-resources status `0x8`) is rendered distinct from a planning-time
/// headroom refusal: admission passed, but a concurrent writer drained the budget mid-run, and under
/// rollback the report ends stating re-apply refuses until headroom returns (plan-rollback design D6).
///
/// A portal-busy give-up (restool could not open the root portal before the shim's attempt bound —
/// the single-opener collision, ADR-0006; bead dpaa2-controlplane-f59) is rendered in honest mid-run
/// voice, never the refusal voice: nothing reached the MC, so it is a transient the operator re-runs,
/// not a decided refusal.
#[must_use]
pub fn render_failure_disposition(failure: &Error, disposition: &FailureDisposition) -> String {
    // 0x8 is the only status classified here; the fuller attribution matrix stays core-side.
    let overcommit = matches!(failure, Error::McStatus { status: 0x8 });
    let portal_busy = matches!(failure, Error::PortalBusy { .. });
    let mut out = String::new();
    if overcommit {
        let _ = writeln!(
            out,
            "refused: overcommit — admission passed, but a concurrent writer drained the resource budget mid-run"
        );
    } else if portal_busy {
        // A busy portal never reached the MC: honest mid-run voice, not "refused" (bead dpaa2-controlplane-f59).
        let _ = writeln!(out, "mid-run failure: {failure}");
    } else {
        // Error's Display is already operator text.
        let _ = writeln!(out, "refused: a verb failed mid-run: {failure}");
    }
    match disposition {
        FailureDisposition::RolledBack(report) => {
            for record in &report.unwound {
                let _ = writeln!(out, "  unwound: {}", describe_actuation(record));
            }
            for residue in &report.residue {
                let _ = writeln!(out, "  residue: {}", describe_residue(residue));
            }
            for record in &report.unreversed {
                let _ = writeln!(
                    out,
                    "  not reversed, the next run re-judges: {}",
                    describe_actuation(record)
                );
            }
            if overcommit {
                let _ = writeln!(out, "re-apply refuses until headroom returns.");
            }
        }
        FailureDisposition::Aborted { prefix } => {
            let _ = writeln!(
                out,
                "  aborted without unwinding: {} actuation(s) left standing for the next run",
                prefix.len()
            );
            for record in prefix {
                let _ = writeln!(out, "  standing: {}", describe_actuation(record));
            }
        }
    }
    out
}

/// Describes one journaled actuation in the operator's data vocabulary (plan-rollback design D6). A
/// total match over the actuation record, full-word naming, never an internal type name.
fn describe_actuation(record: &ActuationRecord) -> String {
    match record {
        ActuationRecord::DpniCreated { dpni, container } => {
            format!("created dpni {dpni} in {}", describe_container(*container))
        }
        ActuationRecord::DprcCreated { dprc } => format!("created container {dprc}"),
        ActuationRecord::PoolObjectCreated { object, container } => {
            format!(
                "created pool object {object} in {}",
                describe_container(*container)
            )
        }
        ActuationRecord::DpseciCreated { object, container } => {
            format!(
                "created dpseci {object} in {}",
                describe_container(*container)
            )
        }
        ActuationRecord::DpswCreated { object, container } => {
            format!(
                "created switch {object} in {}",
                describe_container(*container)
            )
        }
        ActuationRecord::Connected {
            ancestor,
            dpni,
            peer,
        } => format!("connected dpni {dpni} to {peer} at container {ancestor}"),
        ActuationRecord::SwitchPortConnected {
            ancestor,
            dpsw,
            ifx,
            peer,
        } => format!("connected switch {dpsw} interface {ifx} to {peer} at container {ancestor}"),
        ActuationRecord::DpniBound { dpni } => format!("bound dpni {dpni} to its driver"),
        ActuationRecord::DprcBound { dprc } => format!("bound container {dprc} to its driver"),
    }
}

/// Names a home container in operator terms, or the shim root when none.
fn describe_container(container: Option<DprcId>) -> String {
    container.map_or_else(|| "the root container".to_owned(), |c| c.to_string())
}

/// Describes a typed reboot residue in plain words, naming the object and that a reboot reclaims it
/// (plan-rollback design D6; spec "Residue names its remedy"). A total match over the residue.
fn describe_residue(residue: &Residue) -> String {
    match residue {
        Residue::RootPopulation { object } => {
            format!(
                "root pool object {object}: the kernel claims it on plug and frees it only at reboot"
            )
        }
        Residue::DpioSeat { object } => {
            format!("dpio seat {object}: the kernel claims it on plug and frees it only at reboot")
        }
    }
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
        Attributes::Dpseci { cfg } => {
            // num_queues plus the derived option flag names, so dry-run shows the chosen
            // profile (dpseci-typestate design D3/D4).
            let opts: Vec<&str> = cfg.options().flags().iter().map(|f| f.name()).collect();
            format!(
                " num_queues={} options=[{}]",
                cfg.num_queues(),
                opts.join(",")
            )
        }
        Attributes::Dpsw {
            num_ifs,
            max_fdb_entries,
            ..
        } => format!(" num_ifs={num_ifs} max_fdb_entries={max_fdb_entries}"),
        Attributes::Dprc { .. } => " (dprc default options)".to_owned(),
    }
}

fn mark_str(m: Measurement) -> &'static str {
    match m {
        Measurement::Measured => "measured",
        Measurement::Unmeasured => "unmeasured",
    }
}

#[cfg(test)]
mod tests {
    //! The admission render is the operator's data-only verdict (ADR-0023 design D5): an
    //! over-budget face renders the refusal rows with exact budget/draw/headroom, an admitted
    //! face renders the full table with the admitted verdict, and the `ensure` refusal voice
    //! names the budget, draw, and free headroom it exceeded.

    use super::*;
    use crate::engine::Admission;
    use dpaa2_api::core::inventory::Ceiling;
    use dpaa2_api::plan::admission::GrossDraw;
    use dpaa2_api::plan::capacity::{CapacityFace, FdbBudget, GlobalBudget, PoolBudget};

    fn face(dpbp: PoolHeadroom, fdb_bracket: Ceiling, fdb_draw: i64) -> CapacityFace {
        CapacityFace {
            pools: vec![
                PoolBudget {
                    family: Family::Dpbp,
                    headroom: dpbp,
                },
                PoolBudget {
                    family: Family::Dpio,
                    headroom: PoolHeadroom::Spent(16),
                },
            ],
            globals: vec![],
            fdb: FdbBudget {
                bracket: fdb_bracket,
                draw: fdb_draw,
            },
        }
    }

    fn face_with_global(dpbp: PoolHeadroom, free: i64) -> CapacityFace {
        let mut f = face(dpbp, Ceiling::Unknown, 0);
        f.globals = vec![GlobalBudget {
            family: Family::Dpbp,
            free,
        }];
        f
    }

    fn observed(n: i64) -> Ceiling {
        Ceiling::Observed {
            n,
            provenance: "V-DPSW-2".to_owned(),
        }
    }

    fn draw(dpbp: i64, fdb: i64) -> GrossDraw {
        GrossDraw {
            pool: BTreeMap::from([(Family::Dpbp, dpbp)]),
            fdb,
        }
    }

    #[test]
    fn an_over_budget_face_renders_the_refusal_row() {
        let adm = Admission {
            face: face(PoolHeadroom::Free(1), observed(2), 1),
            draw: draw(2, 0),
            verdict: Err(vec![AdmissionRefusal::Pool {
                family: Family::Dpbp,
                draw: 2,
                headroom: 1,
            }]),
        };
        let text = render_admission(&adm);
        assert!(
            text.contains("admission (1 budget(s) drawn) [refused]:"),
            "{text}"
        );
        assert!(
            text.contains("  dpbp draw=2 headroom=1 [refused]"),
            "{text}"
        );
    }

    #[test]
    fn an_admitted_face_renders_the_table_and_verdict() {
        let adm = Admission {
            face: face(PoolHeadroom::Free(3), observed(4), 0),
            draw: draw(2, 1),
            verdict: Ok(()),
        };
        let text = render_admission(&adm);
        assert!(
            text.contains("admission (2 budget(s) drawn) [admitted]:"),
            "{text}"
        );
        assert!(
            text.contains("  dpbp draw=2 headroom=3 [admitted]"),
            "{text}"
        );
        assert!(
            text.contains("  fdb draw=1 headroom=4 [admitted]"),
            "{text}"
        );
        assert!(!text.contains("mc-global"), "{text}");
    }

    #[test]
    fn a_proven_global_row_renders_beside_the_census_row() {
        // plan-rollback design D7: a hardware-side drain the census cannot see refuses against
        // the global row though the census row admits — both render, and the global row counts.
        let adm = Admission {
            face: face_with_global(PoolHeadroom::Free(5), 1),
            draw: draw(2, 0),
            verdict: Err(vec![AdmissionRefusal::GlobalPool {
                family: Family::Dpbp,
                draw: 2,
                headroom: 1,
            }]),
        };
        let text = render_admission(&adm);
        assert!(
            text.contains("admission (2 budget(s) drawn) [refused]:"),
            "{text}"
        );
        assert!(
            text.contains("  dpbp draw=2 headroom=5 [admitted]"),
            "{text}"
        );
        assert!(
            text.contains("  mc-global dpbp draw=2 headroom=1 [refused]"),
            "{text}"
        );
    }

    #[test]
    fn an_unknown_fdb_bracket_renders_unknown_headroom() {
        // No banked bracket ⇒ free() is None, so the row reads `unknown`, never an invented number.
        let adm = Admission {
            face: face(PoolHeadroom::Free(3), Ceiling::Unknown, 0),
            draw: draw(0, 2),
            verdict: Ok(()),
        };
        let text = render_admission(&adm);
        assert!(
            text.contains("  fdb draw=2 headroom=unknown [admitted]"),
            "{text}"
        );
    }

    #[test]
    fn the_ensure_refusal_voice_names_budget_draw_and_headroom() {
        let text = render_admission_refusal(&[
            AdmissionRefusal::Pool {
                family: Family::Dpbp,
                draw: 2,
                headroom: 1,
            },
            AdmissionRefusal::GlobalPool {
                family: Family::Dpbp,
                draw: 2,
                headroom: 0,
            },
            AdmissionRefusal::Fdb {
                draw: 3,
                headroom: 1,
            },
        ]);
        assert!(
            text.contains("refused: pool dpbp draw 2 exceeds free headroom 1"),
            "{text}"
        );
        assert!(
            text.contains("refused: mc-global pool dpbp draw 2 exceeds global free headroom 0"),
            "{text}"
        );
        assert!(
            text.contains("refused: dpsw fdb draw 3 exceeds free headroom 1"),
            "{text}"
        );
    }
}
