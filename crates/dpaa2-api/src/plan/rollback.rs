//! The rollback vocabulary: the run journal, the mid-run failure policy, and the
//! compile-checked total reversal table over the actuation vocabulary (plan-rollback
//! design D3/D4/D5; ADR-0023). Pure and sans-io — the undo side is a typed *description*
//! of a teardown verb, dispatched later by the `dpaa2-mc` executor, never a backend
//! closure here.
//!
//! **Borrowed-word caveat (plan-rollback design D1):** rollback unwinds *this run's*
//! observed actuations; it never restores a prior configuration generation — the `JunOS`
//! `rollback <n>` reading does not apply.
//!
//! [`reverse`] is the reversal table: one exhaustive `match` over [`ActuationRecord`]
//! with no wildcard arm, the total-dispatch idiom pointed backwards (ADR-0023 design D10,
//! the same discipline as [`crate::plan::dispatch`]). A runtime map is refused — a
//! missing insertion compiles, a missing `match` arm does not. A new [`ActuationRecord`]
//! kind therefore cannot ship without a reversal row.
//!
//! ## Verb → kind mapping
//!
//! The journal records *constructive* actuations only — the forward build a mid-run
//! failure must unwind (the model journals exactly these: creates and the switch-port
//! wire, `models/COVERAGE.md` plan-rollback D5/D6). Each successful mutating verb of
//! [`McControl`](crate::contract::McControl)/[`KernelControl`](crate::contract::KernelControl)
//! maps to exactly one place:
//!
//! - `create_dpni`, `create_dpni_in` → [`ActuationRecord::DpniCreated`]
//! - `dprc_create` → [`ActuationRecord::DprcCreated`]
//! - `dpbp_create`, `dpmcp_create`, `dpcon_create`, `dpio_create` →
//!   [`ActuationRecord::PoolObjectCreated`] (one kind: the observed payload and the
//!   `pool_destroy` reversal are identical; the family on the [`ObjectRef`] selects the
//!   root residue)
//! - `create_dpseci_in` → [`ActuationRecord::DpseciCreated`]
//! - `create_dpsw_in` → [`ActuationRecord::DpswCreated`]
//! - `connect`, `dprc_connect` → [`ActuationRecord::Connected`] (one kind: a `connect`
//!   peer is the `dpmac.N` [`ObjectRef`]; both reverse to `dprc_disconnect(ancestor, dpni)`)
//! - `connect_switch_port` → [`ActuationRecord::SwitchPortConnected`]
//! - `bind` → [`ActuationRecord::DpniBound`]
//! - `vfio_set_override`, `vfio_bind`, `dprc_kernel_bind` → [`ActuationRecord::DprcBound`]
//!   (one kind: the observed payload is just the child `dprc`, and `vfio_unbind` — the one
//!   child-dprc driver-detach verb — reverses all three)
//!
//! The teardown verbs are the reversals, carried in [`UndoVerb`], never journaled for
//! reversal (reversing a teardown restores a prior generation, a plan-rollback design D1
//! non-goal; the
//! model journals no teardown): `destroy`→[`UndoVerb::DestroyDpni`],
//! `dprc_destroy`→[`UndoVerb::DestroyDprc`], `pool_destroy`→[`UndoVerb::PoolDestroy`],
//! `destroy_dpseci`→[`UndoVerb::DestroyDpseci`], `destroy_dpsw`→[`UndoVerb::DestroyDpsw`],
//! `dprc_disconnect`→[`UndoVerb::Disconnect`], `unbind`→[`UndoVerb::Unbind`],
//! `vfio_unbind`→[`UndoVerb::VfioUnbind`]. `unbind_current` realizes the
//! [`Precondition::Unbound`] guard of a dpsw teardown (the root device is unbound before
//! `destroy_dpsw`). The attribute/placement verbs (`set_mac`, `set_label`,
//! `dprc_set_label`, `dprc_set_locked`, `dprc_assign`, `dprc_unassign`) overwrite state
//! whose prior value is not observed; on rollback they are subsumed by the object's own
//! destroy reversal, and the level-triggered loop re-stamps from intent — restoring a
//! prior value is a plan-rollback design D1 non-goal, so they are not journaled as
//! reversible kinds.

use crate::core::family::Family;
use crate::core::model::{DpniId, DprcId, ObjectRef};

/// The per-run mid-run failure policy, chosen by the caller at planning time and never
/// prompted mid-run (plan-rollback design D5; spec "The mid-run failure policy is a
/// planning-time choice"). Declared `#[non_exhaustive]` so a later policy joins without
/// breaking callers.
///
/// `Continue` — skip the failed verb and its guard-dependents, converge the rest, and let
/// the next loop finish — is the named deferred variant: its semantics are agreed, its
/// implementation is not licensed here (plan-rollback design D5), so it is documented but
/// not declared.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FailurePolicy {
    /// Unwind the run journal in reverse order (the default disposition).
    #[default]
    Rollback,
    /// Stop at the failure and leave the executed prefix standing as a typed refusal; the
    /// next level-triggered loop re-judges.
    Abort,
}

/// One journaled actuation, carrying the *observed* result (the MC-assigned handle and its
/// home container) — never the intended value (plan-rollback design D3; spec "The journal
/// records observations, not intent"). A create whose MC-assigned id differs from the
/// intended one is recorded by the id that exists, so a later rollback addresses the real
/// object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActuationRecord {
    /// A dpni create (`create_dpni` at root, `create_dpni_in` in a child). `container` is
    /// `None` for the shim root.
    DpniCreated {
        /// The MC-assigned dpni handle.
        dpni: DpniId,
        /// The home container, `None` for the shim root.
        container: Option<DprcId>,
    },
    /// A child container create (`dprc_create`).
    DprcCreated {
        /// The MC-assigned child-container handle.
        dprc: DprcId,
    },
    /// A pool-family create (`dpbp`/`dpmcp`/`dpcon`/`dpio`). The family on `object`
    /// selects the reversal: a child object is destroyed, a root one is irreversible
    /// residue (ADR-0020; ADR-0003 §7).
    PoolObjectCreated {
        /// The MC-assigned pool-object handle (its family selects the root residue).
        object: ObjectRef,
        /// The home container, `None` for the shim root.
        container: Option<DprcId>,
    },
    /// A dpseci create (`create_dpseci_in`).
    DpseciCreated {
        /// The MC-assigned dpseci handle.
        object: ObjectRef,
        /// The home container, `None` for the shim root.
        container: Option<DprcId>,
    },
    /// A dpsw create (`create_dpsw_in`).
    DpswCreated {
        /// The MC-assigned dpsw handle.
        object: ObjectRef,
        /// The home container, `None` for the shim root.
        container: Option<DprcId>,
    },
    /// A dpni↔peer edge (`connect` to a dpmac, or `dprc_connect` to any peer), issued at
    /// `ancestor` (the root [`DprcId::ROOT`] for a plain `connect`).
    Connected {
        /// The common ancestor the edge was issued at.
        ancestor: DprcId,
        /// The connected dpni.
        dpni: DpniId,
        /// The observed peer (a `dpmac.N` for `connect`, any object for `dprc_connect`).
        peer: ObjectRef,
    },
    /// A switch-port edge (`connect_switch_port`): interface `ifx` of `dpsw` to `peer`,
    /// issued at `ancestor`.
    SwitchPortConnected {
        /// The common ancestor the edge was issued at.
        ancestor: DprcId,
        /// The switch object.
        dpsw: ObjectRef,
        /// The switch interface index.
        ifx: u32,
        /// The observed peer.
        peer: ObjectRef,
    },
    /// A dpni driver bind (`bind`).
    DpniBound {
        /// The bound dpni.
        dpni: DpniId,
    },
    /// A child-dprc driver bind (`vfio_set_override`, `vfio_bind`, or `dprc_kernel_bind`).
    DprcBound {
        /// The bound child container.
        dprc: DprcId,
    },
}

/// The run journal: a run-scoped, append-only, in-memory record of the run's observed
/// actuations, created at execution start and dying with the run (plan-rollback design
/// D3). ADR-0008 is untouched — nothing persists past the run and nothing is reserved.
///
/// The type is deliberately minimal: append plus ordered iteration. It holds no shared
/// handle and no interior mutability — dispatch is serialized, so the group-4 recording
/// layer wraps this in single-threaded sharing (`Rc<RefCell<_>>`) when it owns the write
/// seam; that is not this type's concern.
#[derive(Debug, Clone, Default)]
pub struct RunJournal {
    records: Vec<ActuationRecord>,
}

impl RunJournal {
    /// Creates an empty journal at execution start.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one observed actuation. The recording layer calls this only for a verb that
    /// succeeded (spec "A failed verb leaves no journal row").
    pub fn append(&mut self, record: ActuationRecord) {
        self.records.push(record);
    }

    /// The number of recorded actuations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the journal is empty — the rollback-of-nothing identity (spec "Rollback of
    /// an empty journal is the identity").
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Iterates the records in append order. The rollback executor consumes `.rev()` for
    /// the reverse-order unwind (plan-rollback design D4; the reverse is deterministic, so
    /// ORCH-I6 gains no scheduler choice).
    pub fn iter(&self) -> std::slice::Iter<'_, ActuationRecord> {
        self.records.iter()
    }
}

impl<'a> IntoIterator for &'a RunJournal {
    type Item = &'a ActuationRecord;
    type IntoIter = std::slice::Iter<'a, ActuationRecord>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An ordering guard the unwind honors — the forward dependency reversed, a
/// plan-rollback design D4 guard (spec "Reversal honors ordering in reverse"). The
/// executor realizes each guard with the appropriate verb before it fires the destroy undo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precondition {
    /// The object is unbound (or unplugged) before its destroy — unbind precedes destroy
    /// (ADR-0008 §8). Realized by `unbind` for a dpni, `unbind_current` for a dpsw.
    Unbound,
    /// A container's residents unwind before the container's destroy — a child unwinds
    /// before its container (ADR-0004 containment).
    ResidentsDrained,
}

const UNBIND_THEN_DESTROY: &[Precondition] = &[Precondition::Unbound];
const DRAIN_THEN_DESTROY: &[Precondition] = &[Precondition::ResidentsDrained];
const NO_PRECONDITION: &[Precondition] = &[];

/// Irreversible residue: a grow-only hardware population a runtime unwind cannot reclaim,
/// reported as a reboot-named disposition rather than pretending reversal — the
/// plan-rollback design D4 irreversible row (spec "Irreversible work becomes residue, not
/// pretense").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Residue {
    /// A root pool object the kernel's `fsl_mc_allocator` claims on plug; runtime reclaim
    /// is refused and capacity shrinks only across a reboot (ADR-0020).
    RootPopulation {
        /// The stranded root pool object.
        object: ObjectRef,
    },
    /// A root dpio seat — the grow-only, reboot-bounded case ADR-0020 generalized from
    /// (ADR-0003 §7).
    DpioSeat {
        /// The stranded root dpio object.
        object: ObjectRef,
    },
}

/// A typed description of a teardown verb the rollback executor will dispatch from observed
/// values (plan-rollback design D4). Sans-io: this names the verb and its observed
/// operands, it is not a closure over a backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoVerb {
    /// Destroy a dpni (`McControl::destroy`).
    DestroyDpni(DpniId),
    /// Destroy a child container (`McControl::dprc_destroy`).
    DestroyDprc(DprcId),
    /// Destroy a pool object (`McControl::pool_destroy`).
    PoolDestroy(ObjectRef),
    /// Destroy a dpseci (`McControl::destroy_dpseci`).
    DestroyDpseci {
        /// The home container, `None` for the shim root.
        container: Option<DprcId>,
        /// The dpseci to destroy.
        object: ObjectRef,
    },
    /// Destroy a dpsw (`McControl::destroy_dpsw`).
    DestroyDpsw {
        /// The home container, `None` for the shim root.
        container: Option<DprcId>,
        /// The dpsw to destroy.
        object: ObjectRef,
    },
    /// Sever a dpni edge (`McControl::dprc_disconnect`), issued at the recorded ancestor.
    Disconnect {
        /// The common ancestor the edge was issued at.
        ancestor: DprcId,
        /// The dpni to disconnect.
        dpni: DpniId,
    },
    /// Sever a switch-port edge. No single trait verb carries this today; the executor
    /// realizes it (via the dpsw teardown or a later disconnect verb) from these operands.
    DisconnectSwitchPort {
        /// The common ancestor the edge was issued at.
        ancestor: DprcId,
        /// The switch object.
        dpsw: ObjectRef,
        /// The switch interface index.
        ifx: u32,
    },
    /// Unbind a dpni driver (`KernelControl::unbind`).
    Unbind(DpniId),
    /// Unbind a child-dprc driver and clear its override (`KernelControl::vfio_unbind`).
    VfioUnbind(DprcId),
}

/// A reversal-table row (plan-rollback design D4): a reversible actuation names its undo
/// verb and the ordering guards the unwind honors in reverse; an irreversible one names its
/// typed reboot residue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reversal {
    /// The actuation is undone by `undo`, after `preconditions` hold.
    Reversible {
        /// The teardown verb that undoes the actuation.
        undo: UndoVerb,
        /// The ordering guards the unwind honors before the undo fires.
        preconditions: &'static [Precondition],
    },
    /// The actuation cannot be undone at runtime; `residue` names the reboot remedy.
    Irreversible {
        /// The reboot-named residue the run reports.
        residue: Residue,
    },
}

/// The reversal table: the undo of one journaled actuation (plan-rollback design D4;
/// ADR-0023 design D10). The exhaustive `match` has no wildcard arm, so a new
/// [`ActuationRecord`] kind without a row fails the build — omission is a compile error,
/// never a runtime discovery (spec "A new actuation kind without a row cannot ship").
#[must_use]
pub fn reverse(record: &ActuationRecord) -> Reversal {
    match record {
        ActuationRecord::DpniCreated { dpni, .. } => Reversal::Reversible {
            undo: UndoVerb::DestroyDpni(*dpni),
            preconditions: UNBIND_THEN_DESTROY,
        },
        ActuationRecord::DprcCreated { dprc } => Reversal::Reversible {
            undo: UndoVerb::DestroyDprc(*dprc),
            preconditions: DRAIN_THEN_DESTROY,
        },
        // A root pool object is allocator-claimed and reboot-bounded (ADR-0020); a child
        // one is VFIO-owned and reclaimed by destroy (ADR-0020 §3).
        ActuationRecord::PoolObjectCreated { object, container } => match container {
            Some(_) => Reversal::Reversible {
                undo: UndoVerb::PoolDestroy(*object),
                preconditions: UNBIND_THEN_DESTROY,
            },
            None => Reversal::Irreversible {
                residue: root_pool_residue(*object),
            },
        },
        ActuationRecord::DpseciCreated { object, container } => Reversal::Reversible {
            undo: UndoVerb::DestroyDpseci {
                container: *container,
                object: *object,
            },
            preconditions: NO_PRECONDITION,
        },
        ActuationRecord::DpswCreated { object, container } => Reversal::Reversible {
            undo: UndoVerb::DestroyDpsw {
                container: *container,
                object: *object,
            },
            preconditions: UNBIND_THEN_DESTROY,
        },
        ActuationRecord::Connected { ancestor, dpni, .. } => Reversal::Reversible {
            undo: UndoVerb::Disconnect {
                ancestor: *ancestor,
                dpni: *dpni,
            },
            preconditions: NO_PRECONDITION,
        },
        ActuationRecord::SwitchPortConnected {
            ancestor,
            dpsw,
            ifx,
            ..
        } => Reversal::Reversible {
            undo: UndoVerb::DisconnectSwitchPort {
                ancestor: *ancestor,
                dpsw: *dpsw,
                ifx: *ifx,
            },
            preconditions: NO_PRECONDITION,
        },
        ActuationRecord::DpniBound { dpni } => Reversal::Reversible {
            undo: UndoVerb::Unbind(*dpni),
            preconditions: NO_PRECONDITION,
        },
        ActuationRecord::DprcBound { dprc } => Reversal::Reversible {
            undo: UndoVerb::VfioUnbind(*dprc),
            preconditions: NO_PRECONDITION,
        },
    }
}

/// The residue a root pool object strands: a dpio seat (ADR-0003 §7), every other root pool
/// family a grow-only root population (ADR-0020). Both are reboot-bounded.
fn root_pool_residue(object: ObjectRef) -> Residue {
    if object.family() == Family::Dpio {
        Residue::DpioSeat { object }
    } else {
        Residue::RootPopulation { object }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActuationRecord, FailurePolicy, Precondition, Residue, Reversal, RunJournal, UndoVerb,
        reverse,
    };
    use crate::core::family::Family;
    use crate::core::model::{DpmacId, DpniId, DprcId, ObjectRef};

    #[test]
    fn records_carry_the_observed_id_not_the_intended_one() {
        // Spec "The journal records observations, not intent": the MC assigned dpni.7 while
        // the intent meant some other index; the row carries what exists.
        let observed = DpniId::new(7);
        let record = ActuationRecord::DpniCreated {
            dpni: observed,
            container: None,
        };
        let ActuationRecord::DpniCreated { dpni, .. } = record else {
            panic!("wrong kind");
        };
        assert_eq!(dpni, observed);
    }

    #[test]
    fn default_policy_is_rollback() {
        // Spec "The default rolls back".
        assert_eq!(FailurePolicy::default(), FailurePolicy::Rollback);
    }

    #[test]
    fn dpni_reversal_unbinds_before_destroy() {
        // Spec "Reversal honors ordering in reverse": the create's row destroys the dpni,
        // and its precondition carries unbind-before-destroy.
        let record = ActuationRecord::DpniCreated {
            dpni: DpniId::new(7),
            container: None,
        };
        let Reversal::Reversible {
            undo,
            preconditions,
        } = reverse(&record)
        else {
            panic!("a dpni create is reversible");
        };
        assert_eq!(undo, UndoVerb::DestroyDpni(DpniId::new(7)));
        assert!(preconditions.contains(&Precondition::Unbound));
    }

    #[test]
    fn root_dpio_create_is_dpio_seat_residue() {
        // Spec "Irreversible work becomes residue": a root dpio seat is grow-only
        // (ADR-0003 §7).
        let seat = ObjectRef::new(Family::Dpio, 16);
        let record = ActuationRecord::PoolObjectCreated {
            object: seat,
            container: None,
        };
        assert_eq!(
            reverse(&record),
            Reversal::Irreversible {
                residue: Residue::DpioSeat { object: seat },
            }
        );
    }

    #[test]
    fn root_dpbp_create_is_root_population_residue() {
        // ADR-0020: every non-dpio root pool family is grow-only root population.
        let obj = ObjectRef::new(Family::Dpbp, 1);
        let record = ActuationRecord::PoolObjectCreated {
            object: obj,
            container: None,
        };
        assert_eq!(
            reverse(&record),
            Reversal::Irreversible {
                residue: Residue::RootPopulation { object: obj },
            }
        );
    }

    #[test]
    fn child_pool_object_is_reversible_by_destroy() {
        // ADR-0020 §3: a child object is VFIO-owned and reclaimed by destroy.
        let obj = ObjectRef::new(Family::Dpio, 16);
        let record = ActuationRecord::PoolObjectCreated {
            object: obj,
            container: Some(DprcId::new(2)),
        };
        let Reversal::Reversible {
            undo,
            preconditions,
        } = reverse(&record)
        else {
            panic!("a child pool object is reversible");
        };
        assert_eq!(undo, UndoVerb::PoolDestroy(obj));
        assert!(preconditions.contains(&Precondition::Unbound));
    }

    #[test]
    fn container_reversal_drains_residents_first() {
        // Spec "Reversal honors ordering in reverse": a child unwinds before its container.
        let record = ActuationRecord::DprcCreated {
            dprc: DprcId::new(2),
        };
        let Reversal::Reversible {
            undo,
            preconditions,
        } = reverse(&record)
        else {
            panic!("a container create is reversible");
        };
        assert_eq!(undo, UndoVerb::DestroyDprc(DprcId::new(2)));
        assert!(preconditions.contains(&Precondition::ResidentsDrained));
    }

    #[test]
    fn edge_reverses_to_a_disconnect_at_the_ancestor() {
        let record = ActuationRecord::Connected {
            ancestor: DprcId::ROOT,
            dpni: DpniId::new(7),
            peer: ObjectRef::new(Family::Dpmac, DpmacId::new(7).into_inner()),
        };
        assert_eq!(
            reverse(&record),
            Reversal::Reversible {
                undo: UndoVerb::Disconnect {
                    ancestor: DprcId::ROOT,
                    dpni: DpniId::new(7),
                },
                preconditions: &[],
            }
        );
    }

    #[test]
    fn journal_starts_empty_and_appends_in_order() {
        // Spec "Rollback of an empty journal is the identity" (the empty harvest) plus the
        // deterministic append order the reverse unwind consumes.
        let mut journal = RunJournal::new();
        assert!(journal.is_empty());
        assert_eq!(journal.len(), 0);

        let first = ActuationRecord::DpniCreated {
            dpni: DpniId::new(7),
            container: None,
        };
        let second = ActuationRecord::DpniBound {
            dpni: DpniId::new(7),
        };
        journal.append(first);
        journal.append(second);

        assert_eq!(journal.len(), 2);
        let order: Vec<_> = journal.iter().copied().collect();
        assert_eq!(order, vec![first, second]);
        // The unwind is reverse order: the bind reverses (unbind) before the create (destroy).
        let reversed: Vec<_> = journal.iter().rev().copied().collect();
        assert_eq!(reversed, vec![second, first]);
    }
}
