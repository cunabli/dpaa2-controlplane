//! The ITF trace replayer (task 3.2): frozen model traces vs. the Rust
//! reconciler, board-free.
//!
//! A frozen trace supplies a sequence of MC-legal states produced by the
//! core machine's guards. The reconciler is level-triggered — observe,
//! plan, apply the whole plan, re-observe — so each trace is segmented
//! into *epochs* by the observation points recorded in its
//! [`RetroTrace`] spec. At each observation state the replayer projects
//! the model state to an [`ObservedTopology`], runs [`reconcile_with`],
//! and diffs the plan against what the model actually did next.
//!
//! The model-action ↔ plan-step mapping (the granularity decision of
//! design D2, settled in `models/retro/reconciler.qnt`) lives in
//! `deltas`: one plan step is coarser than one machine action, and
//! only reconciler-observable changes classify — dep-chain objects
//! (dpio/dpmcp/dpbp/dpcon), plug flips, and bus rescans are sub-steps
//! that never surface as transitions of their own. `Bind` is
//! wait-to-observe: the plan carries it as soon as the port's DPNI is
//! connected but unbound, while the model fires the kernel probe in a
//! later epoch — `expected_plan` encodes exactly that.

use std::collections::BTreeMap;

use dpaa2_api::{
    DesiredPort, DesiredTopology, DpmacId, DpniId, LinkType, ObservedDpmac, ObservedDpni,
    ObservedTopology, Plan, Presence, ReconcileOptions, Transition, reconcile_with,
};

use crate::itf::ModelView;

/// One frozen retro trace and the reconciler context it replays under.
#[derive(Clone, Copy, Debug)]
pub struct RetroTrace {
    /// Trace file name under `models/traces/`.
    pub file: &'static str,
    /// The DPMAC anchor of the single desired port.
    pub port: u32,
    /// Whether the operator wants the port present or torn down.
    pub presence: Presence,
    /// Reconciliation policy for the replay.
    pub options: ReconcileOptions,
    /// State indices where the reconciler observes; the last index MUST
    /// be the trace's final state, where the plan must be converged.
    pub observations: &'static [usize],
}

/// Projects the model's observable slice to what `restool` + sysfs
/// read-back would return: MC truth for objects and edges, kernel bind
/// as the netdev. The model carries no MAC values and no link types, so
/// those project as unknown/PHY (out of retro scope, see
/// `models/retro/reconciler.qnt`).
fn project(v: &ModelView) -> ObservedTopology {
    ObservedTopology {
        dpnis: v
            .dpnis
            .iter()
            .map(|(n, d)| ObservedDpni {
                id: DpniId::new(*n),
                connected_to: d.connected_to.map(DpmacId::new),
                mac: None,
                netdev: d.bound.then(|| format!("eth{n}")),
                attributes: BTreeMap::new(),
            })
            .collect(),
        dpmacs: v
            .dpmacs
            .iter()
            .map(|n| ObservedDpmac {
                id: DpmacId::new(*n),
                link_type: LinkType::Phy,
                mac: None,
            })
            .collect(),
    }
}

/// Classifies one model step into the plan step it serves, if any.
/// Steps that change nothing an observer can see (companion creates,
/// plugs, rescans, pool draws) return `None`.
fn deltas(prev: &ModelView, next: &ModelView, port: u32) -> Option<Transition> {
    let anchor = DpmacId::new(port);
    for (n, d) in &next.dpnis {
        match prev.dpnis.get(n) {
            None => return Some(Transition::Create { port: anchor }),
            Some(p) => {
                if p.connected_to.is_none() && d.connected_to == Some(port) {
                    return Some(Transition::Connect { port: anchor });
                }
                if !p.bound && d.bound {
                    return Some(Transition::Bind { port: anchor });
                }
                if p.bound && !d.bound {
                    return Some(Transition::Unbind {
                        dpni: DpniId::new(*n),
                    });
                }
                if p.connected_to.is_some() && d.connected_to.is_none() {
                    return Some(Transition::Disconnect {
                        dpni: DpniId::new(*n),
                    });
                }
            }
        }
    }
    for n in prev.dpnis.keys() {
        if !next.dpnis.contains_key(n) {
            return Some(Transition::Destroy {
                dpni: DpniId::new(*n),
            });
        }
    }
    None
}

/// The plan the model expects for one epoch: the classified steps the
/// machine took, plus the trailing wait-to-observe `Bind` when the
/// epoch ends with the port's DPNI connected but not yet kernel-bound.
fn expected_plan(epoch: &[ModelView], port: u32) -> Vec<Transition> {
    let mut plan: Vec<Transition> = epoch
        .windows(2)
        .filter_map(|w| deltas(&w[0], &w[1], port))
        .collect();
    let end = epoch.last().expect("epoch has at least one state");
    if end
        .dpnis
        .values()
        .any(|d| d.connected_to == Some(port) && !d.bound)
    {
        plan.push(Transition::Bind {
            port: DpmacId::new(port),
        });
    }
    plan
}

/// Replays one frozen trace against the reconciler.
///
/// # Errors
///
/// Returns a description of the first divergence: a plan that does not
/// match the model's expected steps, a drift/assert report the model
/// never predicted, or a final state the reconciler does not consider
/// converged.
pub fn replay(views: &[ModelView], spec: &RetroTrace) -> Result<(), String> {
    let mut port = DesiredPort::new(DpmacId::new(spec.port), "retro0");
    port.presence = spec.presence;
    let desired = DesiredTopology::from_ports([port]);

    if spec.observations.last() != Some(&(views.len() - 1)) {
        return Err(format!(
            "{}: last observation must be the final state {} (got {:?})",
            spec.file,
            views.len() - 1,
            spec.observations.last()
        ));
    }

    for (i, &obs) in spec.observations.iter().enumerate() {
        let plan: Plan = reconcile_with(&desired, &project(&views[obs]), spec.options);
        if plan.has_divergence() {
            return Err(format!(
                "{}@{obs}: unexpected divergence: {:?} {:?}",
                spec.file, plan.drift, plan.assertions
            ));
        }
        let expected = match spec.observations.get(i + 1) {
            Some(&next) => expected_plan(&views[obs..=next], spec.port),
            // Final observation: the model is done, so must the plan be.
            None => Vec::new(),
        };
        if plan.transitions != expected {
            return Err(format!(
                "{}@{obs}: reconciler planned {:?}, model expects {expected:?}",
                spec.file, plan.transitions
            ));
        }
    }
    Ok(())
}
