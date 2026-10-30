//! The rollback executor and the mid-run failure-policy application (plan-rollback design D6/D10;
//! spec "Rollback unwinds the journal in reverse order", "The mid-run failure policy is a
//! planning-time choice").
//!
//! Generic over the [`McControl`]/[`KernelControl`] seams (ADR-0018), like the imperative
//! shell: the executor drives injected ports and performs no transport itself, so dpaa2-verify
//! replays it against the in-memory fake. It is the interpreter half of the group-3/4
//! vocabulary ([`crate::plan::rollback`]), which stays the pure value side.

use crate::contract::{KernelControl, McControl};
use std::collections::BTreeSet;

use crate::core::error::Error;
use crate::core::model::ObjectRef;
use crate::plan::rollback::{
    ActuationRecord, FailurePolicy, Precondition, Residue, Reversal, RunJournal, UndoVerb, reverse,
};

/// The typed outcome of a reverse-order unwind (plan-rollback design D5/D10): the rows the
/// executor reversed, the residue it could not reclaim, and the rows whose undo the environment
/// refused. All three are in unwind (reverse-journal) order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RollbackReport {
    /// Actuations whose undo verb (after its preconditions) dispatched successfully.
    pub unwound: Vec<ActuationRecord>,
    /// Typed reboot-named residue from the irreversible rows (plan-rollback design D5; ADR-0020, ADR-0003 §7).
    pub residue: Vec<Residue>,
    /// Actuations whose undo (or a precondition verb) failed; reported, never retried — the next loop re-observes (plan-rollback design D6).
    pub unreversed: Vec<ActuationRecord>,
}

/// Replays `journal` in reverse order through the reversal table, dispatching each undo verb from
/// its observed values through the injected seams (plan-rollback design D5; spec "Rollback unwinds
/// the journal in reverse order").
///
/// Reverse order discharges the forward ordering guards for free: a reversible child resident
/// unwinds before its container, so [`Precondition::ResidentsDrained`] needs no verb (plan-rollback
/// design D5; ADR-0020 §3). A resident the walk cannot drain is left to the MC's refusal of the
/// non-empty destroy — the row reports unreversed (plan-rollback design D6/D8). [`Precondition::Unbound`] is
/// realized before the destroy (unbind for a dpni, `unbind_current` for an object).
///
/// An [`Reversal::Irreversible`] row reports its residue and the unwind continues; a failed undo
/// (or precondition) verb is reported unreversed and the unwind continues — no in-run retry, the
/// next level-triggered loop re-judges (plan-rollback design D6). An empty journal dispatches
/// nothing and reports an empty unwind — the identity fixpoint (process rule 6).
#[must_use]
pub fn execute<M: McControl, K: KernelControl>(
    journal: &RunJournal,
    mc: &M,
    kernel: &K,
) -> RollbackReport {
    let created_switches = switches_created_this_run(journal);
    let mut report = RollbackReport::default();
    for record in journal.iter().rev() {
        match reverse(record) {
            Reversal::Irreversible { residue } => report.residue.push(residue),
            // No trait verb severs a switch-port edge today (the `DisconnectSwitchPort` doc), and
            // the model undoes the wire as its own row, not via the dpsw teardown. So the edge is
            // covered only when its dpsw was created this run: the later destroy_dpsw row clears it,
            // so count it unwound with no verb. On a pre-existing dpsw the edge is undispatchable and
            // must surface unreversed, never silently vanish — the next loop re-judges (plan-rollback design D5).
            Reversal::Reversible {
                undo: UndoVerb::DisconnectSwitchPort { dpsw, .. },
                ..
            } => {
                if created_switches.contains(&dpsw) {
                    report.unwound.push(*record);
                } else {
                    report.unreversed.push(*record);
                }
            }
            Reversal::Reversible {
                undo,
                preconditions,
            } => match unwind_one(&undo, preconditions, mc, kernel) {
                Ok(()) => report.unwound.push(*record),
                Err(_) => report.unreversed.push(*record),
            },
        }
    }
    report
}

/// The dpsw objects this run created (plan-rollback design D5): a switch-port edge whose dpsw is in
/// this set is cleared by that dpsw's own teardown row, so the edge's no-verb undo is complete.
fn switches_created_this_run(journal: &RunJournal) -> BTreeSet<ObjectRef> {
    journal
        .iter()
        .filter_map(|record| match record {
            ActuationRecord::DpswCreated { object, .. } => Some(*object),
            _ => None,
        })
        .collect()
}

/// How a mid-run verb failure was handled under the run's [`FailurePolicy`], folded into the run's
/// typed refusal (plan-rollback design D6; spec "The mid-run failure policy is a planning-time
/// choice").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureDisposition {
    /// The run rolled back: the reverse-order unwind of the journal (the default policy).
    RolledBack(RollbackReport),
    /// The run aborted: no undo verb fired, and `prefix` is the executed prefix the journal
    /// recorded (observed terms), left standing for the next loop to re-judge.
    Aborted {
        /// The executed prefix the journal recorded, in forward (execution) order.
        prefix: Vec<ActuationRecord>,
    },
}

/// Applies the run's planning-time [`FailurePolicy`] to a mid-run verb failure (plan-rollback design D6):
/// under [`FailurePolicy::Rollback`] it runs [`execute`] over the journal; under
/// [`FailurePolicy::Abort`] it dispatches no undo verb and names the executed prefix. The policy
/// is uniform over every failure class — no per-class branching and no in-run retry. The match
/// has no wildcard, so a future policy (the deferred `Continue`) cannot ship without an arm.
#[must_use]
pub fn apply_failure_policy<M: McControl, K: KernelControl>(
    policy: FailurePolicy,
    journal: &RunJournal,
    mc: &M,
    kernel: &K,
) -> FailureDisposition {
    match policy {
        FailurePolicy::Rollback => FailureDisposition::RolledBack(execute(journal, mc, kernel)),
        FailurePolicy::Abort => FailureDisposition::Aborted {
            prefix: journal.iter().copied().collect(),
        },
    }
}

/// Realizes the ordering preconditions then dispatches the undo verb (plan-rollback design D5). A
/// failed precondition or undo verb short-circuits to `Err`; the caller reports the row unreversed
/// and moves on — no retry (plan-rollback design D6).
fn unwind_one<M: McControl, K: KernelControl>(
    undo: &UndoVerb,
    preconditions: &[Precondition],
    mc: &M,
    kernel: &K,
) -> Result<(), Error> {
    for pc in preconditions {
        match pc {
            // Unbind precedes destroy: the dpni driver or the object's root device (plan-rollback design D5; ADR-0008 §8).
            Precondition::Unbound => unbind_for(undo, kernel)?,
            // Reverse order drains reversible children (ADR-0020 §3); what stands is the MC's to refuse — the row reports unreversed (plan-rollback design D6/D8).
            Precondition::ResidentsDrained => {}
        }
    }
    dispatch_undo(undo, mc, kernel)
}

/// The unbind that realizes [`Precondition::Unbound`] for `undo`'s target: a dpni driver detach, or
/// the driver-agnostic root-device detach for an object (plan-rollback design D5). Only the three
/// undo verbs the reversal table pairs with [`Precondition::Unbound`] carry one.
fn unbind_for<K: KernelControl>(undo: &UndoVerb, kernel: &K) -> Result<(), Error> {
    match undo {
        UndoVerb::DestroyDpni(dpni) => kernel.unbind(*dpni),
        UndoVerb::PoolDestroy(object) | UndoVerb::DestroyDpsw { object, .. } => {
            kernel.unbind_current(object)
        }
        _ => Ok(()),
    }
}

/// Dispatches one undo verb through the seams (plan-rollback design D5). `DisconnectSwitchPort` is
/// handled in [`execute`] by journal context (no trait verb severs a switch-port edge today), never
/// here, so its arm is unreachable — a regression that routes it here is a loud panic, not a silent
/// success.
fn dispatch_undo<M: McControl, K: KernelControl>(
    undo: &UndoVerb,
    mc: &M,
    kernel: &K,
) -> Result<(), Error> {
    match undo {
        UndoVerb::DestroyDpni(dpni) => mc.destroy(*dpni),
        UndoVerb::DestroyDprc(dprc) => mc.dprc_destroy(*dprc),
        UndoVerb::PoolDestroy(object) => mc.pool_destroy(object),
        UndoVerb::DestroyDpseci { container, object } => mc.destroy_dpseci(*container, object),
        UndoVerb::DestroyDpsw { container, object } => mc.destroy_dpsw(*container, object),
        UndoVerb::Disconnect { ancestor, dpni } => mc.dprc_disconnect(*ancestor, *dpni),
        UndoVerb::DisconnectSwitchPort { .. } => {
            unreachable!("switch-port edges are dispositioned in execute, never dispatched")
        }
        UndoVerb::Unbind(dpni) => kernel.unbind(*dpni),
        UndoVerb::VfioUnbind(dprc) => kernel.vfio_unbind(*dprc),
    }
}

#[cfg(test)]
mod tests {
    use super::{FailureDisposition, RollbackReport, apply_failure_policy, execute};
    use crate::contract::fake::FakeBackend;
    use crate::core::family::Family;
    use crate::core::model::{DpniId, DprcId, ObjectRef};
    use crate::plan::rollback::{ActuationRecord, FailurePolicy, Residue, RunJournal};

    fn dpni(id: u32) -> ActuationRecord {
        ActuationRecord::DpniCreated {
            dpni: DpniId::new(id),
            container: None,
        }
    }

    // Spec "Rollback unwinds the journal in reverse order": the undo verbs are observed in reverse
    // journal order, and each dpni's unbind precedes its destroy (the reversed ordering guard).
    #[test]
    fn unwind_is_reverse_order_unbind_before_destroy() {
        let backend = FakeBackend::new();
        let mut journal = RunJournal::new();
        journal.append(dpni(5));
        journal.append(dpni(6));

        let report = execute(&journal, &backend, &backend);

        assert_eq!(
            backend.audit(),
            vec![
                format!("unbind:{}", DpniId::new(6)),
                format!("destroy:{}", DpniId::new(6)),
                format!("unbind:{}", DpniId::new(5)),
                format!("destroy:{}", DpniId::new(5)),
            ],
        );
        assert_eq!(
            report,
            RollbackReport {
                unwound: vec![dpni(6), dpni(5)],
                ..RollbackReport::default()
            },
        );
    }

    // Spec "Irreversible work becomes residue, not pretense": a root pool object reports residue
    // and the row that unwinds after it (in reverse) still fires — the irreversible row never blocks.
    #[test]
    fn irreversible_row_does_not_stop_later_rows() {
        let backend = FakeBackend::new();
        let obj = ObjectRef::new(Family::Dpbp, 1);
        let mut journal = RunJournal::new();
        journal.append(dpni(5));
        journal.append(ActuationRecord::PoolObjectCreated {
            object: obj,
            container: None,
        });

        let report = execute(&journal, &backend, &backend);

        assert_eq!(
            report,
            RollbackReport {
                unwound: vec![dpni(5)],
                residue: vec![Residue::RootPopulation { object: obj }],
                unreversed: vec![],
            },
        );
        // The dpni destroy still fired after the irreversible row was reported.
        assert_eq!(
            backend.audit(),
            vec![
                format!("unbind:{}", DpniId::new(5)),
                format!("destroy:{}", DpniId::new(5)),
            ],
        );
    }

    // Spec "A failed undo verb does not wedge the unwind": a disconnect of a dpni that never
    // existed fails; the row is reported unreversed and the remaining row still unwinds.
    #[test]
    fn failed_undo_is_reported_and_unwind_continues() {
        let backend = FakeBackend::new();
        let dangling = ActuationRecord::Connected {
            ancestor: DprcId::ROOT,
            dpni: DpniId::new(99),
            peer: ObjectRef::new(Family::Dpmac, 7),
        };
        let mut journal = RunJournal::new();
        journal.append(dpni(5));
        journal.append(dangling);

        let report = execute(&journal, &backend, &backend);

        assert_eq!(report.unreversed, vec![dangling]);
        assert_eq!(report.unwound, vec![dpni(5)]);
        // No in-run retry, and the surviving row's destroy still fired.
        assert!(
            backend
                .audit()
                .contains(&format!("destroy:{}", DpniId::new(5)))
        );
    }

    // plan-rollback design D5: a switch-port edge whose dpsw was created this run is cleared by the
    // later destroy_dpsw row, so it counts unwound with no switch-port verb of its own.
    #[test]
    fn switch_port_on_same_run_dpsw_counts_unwound_no_extra_verb() {
        let backend = FakeBackend::new();
        let dpsw = ObjectRef::new(Family::Dpsw, 1);
        let created = ActuationRecord::DpswCreated {
            object: dpsw,
            container: None,
        };
        let wire = ActuationRecord::SwitchPortConnected {
            ancestor: DprcId::ROOT,
            dpsw,
            ifx: 0,
            peer: ObjectRef::new(Family::Dpmac, 7),
        };
        let mut journal = RunJournal::new();
        journal.append(created);
        journal.append(wire);

        let report = execute(&journal, &backend, &backend);

        // Reverse: the wire is covered (no verb), then the dpsw tears down (unbind_current, destroy).
        assert_eq!(
            report,
            RollbackReport {
                unwound: vec![wire, created],
                ..RollbackReport::default()
            },
        );
        assert_eq!(
            backend.audit(),
            vec![format!("unbind_current:{dpsw}"), format!("destroy:{dpsw}"),],
        );
    }

    // plan-rollback design D5: a switch-port edge on a pre-existing dpsw (no DpswCreated row) is
    // undispatchable — it surfaces unreversed, never silently vanishes, and the unwind continues.
    #[test]
    fn switch_port_on_pre_existing_dpsw_is_unreversed() {
        let backend = FakeBackend::new();
        let wire = ActuationRecord::SwitchPortConnected {
            ancestor: DprcId::ROOT,
            dpsw: ObjectRef::new(Family::Dpsw, 9),
            ifx: 0,
            peer: ObjectRef::new(Family::Dpmac, 7),
        };
        let mut journal = RunJournal::new();
        journal.append(dpni(5));
        journal.append(wire);

        let report = execute(&journal, &backend, &backend);

        assert_eq!(report.unreversed, vec![wire]);
        assert_eq!(report.unwound, vec![dpni(5)]);
        // The unwind continued: the surviving dpni still destroyed, and no switch-port verb fired.
        assert!(
            backend
                .audit()
                .contains(&format!("destroy:{}", DpniId::new(5)))
        );
    }

    // Spec "Rollback of an empty journal is the identity": nothing dispatched, empty report.
    #[test]
    fn empty_journal_is_identity() {
        let backend = FakeBackend::new();

        let report = execute(&RunJournal::new(), &backend, &backend);

        assert_eq!(report, RollbackReport::default());
        assert_eq!(backend.audit(), Vec::<String>::new());
    }

    // plan-rollback design D5: a container reversal drains residents by reverse order alone, so it
    // issues the destroy with no extra unbind verb for ResidentsDrained.
    #[test]
    fn container_reversal_issues_only_the_destroy() {
        let backend = FakeBackend::new();
        let mut journal = RunJournal::new();
        journal.append(ActuationRecord::DprcCreated {
            dprc: DprcId::new(9),
        });

        let report = execute(&journal, &backend, &backend);

        assert_eq!(
            report.unwound,
            vec![ActuationRecord::DprcCreated {
                dprc: DprcId::new(9),
            }]
        );
        assert_eq!(
            backend.audit(),
            vec![format!("dprc_destroy:{}", DprcId::new(9))]
        );
    }

    // Spec "Abort leaves the prefix for the loop": no undo verb fires and the refusal names the
    // executed prefix in forward order.
    #[test]
    fn abort_dispatches_no_undo_and_names_the_prefix() {
        let backend = FakeBackend::new();
        let bound = ActuationRecord::DpniBound {
            dpni: DpniId::new(5),
        };
        let mut journal = RunJournal::new();
        journal.append(dpni(5));
        journal.append(bound);

        let disposition = apply_failure_policy(FailurePolicy::Abort, &journal, &backend, &backend);

        assert_eq!(
            disposition,
            FailureDisposition::Aborted {
                prefix: vec![dpni(5), bound],
            },
        );
        assert_eq!(
            backend.audit(),
            Vec::<String>::new(),
            "abort dispatches no undo verb"
        );
    }

    // Spec "The default rolls back": the default policy runs the reverse-order unwind.
    #[test]
    fn default_policy_rolls_back() {
        let backend = FakeBackend::new();
        let mut journal = RunJournal::new();
        journal.append(dpni(5));

        let disposition =
            apply_failure_policy(FailurePolicy::default(), &journal, &backend, &backend);

        match disposition {
            FailureDisposition::RolledBack(report) => assert_eq!(report.unwound, vec![dpni(5)]),
            FailureDisposition::Aborted { .. } => panic!("the default policy must roll back"),
        }
        assert_eq!(
            backend.audit(),
            vec![
                format!("unbind:{}", DpniId::new(5)),
                format!("destroy:{}", DpniId::new(5)),
            ],
        );
    }
}
