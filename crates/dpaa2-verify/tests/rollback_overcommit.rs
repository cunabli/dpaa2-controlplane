//! Fake-MC overcommit rollback simulation (plan-rollback task 5.2; design D6/D7): offline,
//! against the in-memory `FakeBackend` through the `Recording` layer. A run whose admission
//! passes executes a prefix, a concurrent writer drains the budget, and the next create refuses
//! mid-run — the overcommit (plan-rollback design D1), discovered at dispatch. The two tests
//! witness the two policies end to end over the real `apply_failure_policy` seam: `Rollback`
//! unwinds the executed prefix back to the pre-run board up to typed residue, and `Abort` fires
//! no undo verb and names the executed prefix.
//!
//! The budget drain is expressed by composing the fake's existing one-shot
//! `with_dprc_create_refusal` seam at the mid-run create — no new fake surface is added
//! (plan-rollback design D7 fake-MC budget simulation): the prefix verbs succeed, and the create
//! that discovers the drain bounces the MC no-space refusal.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use dpaa2_api::contract::fake::FakeBackend;
use dpaa2_api::contract::{JournalHandle, KernelControl, McControl, Recording};
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::inventory::{Ceiling, Inventory};
use dpaa2_api::core::model::{DpniId, DprcId, ObjectRef, ObservedTopology};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpio::{ChannelMode, DpioCfg, Priorities};
use dpaa2_api::families::dpni::DpniCfg;
use dpaa2_api::families::dprc::Options;
use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};
use dpaa2_api::plan::admission::{GrossDraw, admit};
use dpaa2_api::plan::capacity::{CapacityFace, FdbBudget, PoolBudget, PoolHeadroom};
use dpaa2_api::plan::rollback::{ActuationRecord, FailurePolicy, Residue, RunJournal};
use dpaa2_api::plan::rollback_exec::{FailureDisposition, apply_failure_policy};

fn dpio_cfg() -> DpioCfg {
    DpioCfg {
        mode: ChannelMode::LocalChannel,
        priorities: Priorities::new(8).expect("8 is in 1..=8"),
    }
}

/// A capacity face with pool headroom for the prefix draw, so the planning-time admission admits
/// before the mid-run drain (plan-rollback design D6 "admission passes"). The fake models no
/// budget of its own, so this is the admission anchor; the drain itself is the mid-run create
/// refusal below.
fn admitting_face() -> CapacityFace {
    CapacityFace {
        pools: vec![
            PoolBudget {
                family: Family::Dpbp,
                headroom: PoolHeadroom::Free(4),
            },
            PoolBudget {
                family: Family::Dpio,
                headroom: PoolHeadroom::Spent(16),
            },
        ],
        fdb: FdbBudget {
            bracket: Ceiling::Observed {
                n: 4,
                provenance: "V-DPSW-2".to_owned(),
            },
            draw: 0,
        },
    }
}

/// The run's observed handles plus the pre-run board snapshot, for the net-identity comparison.
struct Overcommit {
    backend: Recording<FakeBackend>,
    journal: RunJournal,
    dpni: DpniId,
    dpio: ObjectRef,
    pre_topology: ObservedTopology,
    pre_inventory: Inventory,
    pre_dpio_pool: Vec<ObjectRef>,
}

/// Drives the overcommit run against a fresh fake: the planning-time admission passes, the prefix
/// (a root dpni create, its bind, and a root dpio seat) executes and journals through the
/// `Recording` layer, then a concurrent writer has drained the budget so the next create — a child
/// container — refuses mid-run. The drain is the fake's one-shot `with_dprc_create_refusal` fired
/// at that create (plan-rollback design D7). Captures the pre-run board before the prefix.
fn run_overcommit() -> Overcommit {
    // The planning-time admission passes against headroom (plan-rollback design D6).
    let draw = GrossDraw::from_pending(&BTreeMap::from([(Family::Dpbp, 1)]));
    assert!(
        admit(&admitting_face(), &draw).is_ok(),
        "admission passes before the drain"
    );

    let journal: JournalHandle = Rc::new(RefCell::new(RunJournal::new()));
    // The drain: a concurrent writer ate the budget, so the mid-run child-container create bounces
    // the MC no-space refusal (0x8) — the overcommit discovered at dispatch (plan-rollback design D1).
    let backend = Recording::new(
        FakeBackend::new().with_dprc_create_refusal(Error::McStatus { status: 0x8 }),
        Rc::clone(&journal),
    );

    // The pre-run board: the empty topology the run began from, for the net-identity comparison.
    let pre_topology = backend.observe().unwrap();
    let pre_inventory = backend.read_inventory().unwrap();
    let pre_dpio_pool: Vec<ObjectRef> = backend
        .observe_pool(None, Family::Dpio)
        .unwrap()
        .iter()
        .map(|o| o.object)
        .collect();

    // The executed prefix: each verb journals its observed actuation (plan-rollback design D3).
    let lbl = ConstructName::from("port");
    let dpni = backend.create_dpni(&lbl, &DpniCfg::defaults()).unwrap();
    backend.bind(dpni).unwrap();
    let dpio = backend.dpio_create(None, dpio_cfg(), &lbl).unwrap();

    // The overcommit: the next create refuses, and a failed verb leaves no journal row.
    let refused = backend.dprc_create(
        DprcId::ROOT,
        Options::DEFAULT,
        &ConstructName::from("child"),
    );
    assert!(matches!(refused, Err(Error::McStatus { status: 0x8 })));

    let journal = journal.borrow().clone();
    assert_eq!(
        journal.iter().copied().collect::<Vec<_>>(),
        vec![
            ActuationRecord::DpniCreated {
                dpni,
                container: None,
            },
            ActuationRecord::DpniBound { dpni },
            ActuationRecord::PoolObjectCreated {
                object: dpio,
                container: None,
            },
        ],
        "the journal holds the executed prefix only — the refused create added no row",
    );

    Overcommit {
        backend,
        journal,
        dpni,
        dpio,
        pre_topology,
        pre_inventory,
        pre_dpio_pool,
    }
}

/// Spec "An overcommit unwinds the executed prefix": under the rollback policy the reverse-order
/// unwind returns the board to its pre-run state up to the typed dpio-seat residue (ADR-0003 §7).
#[test]
fn overcommit_rollback_unwinds_prefix_to_pre_run_board_up_to_residue() {
    let oc = run_overcommit();
    assert!(
        oc.pre_dpio_pool.is_empty(),
        "the run began with no root dpio seat"
    );

    let disposition = apply_failure_policy(
        FailurePolicy::Rollback,
        &oc.journal,
        &oc.backend,
        &oc.backend,
    );

    let FailureDisposition::RolledBack(report) = disposition else {
        panic!("the rollback policy rolls back");
    };
    // The reversible rows unwind in reverse journal order; the root dpio seat is irreversible, so
    // it reports typed residue and never enters the unwound set (plan-rollback design D4).
    assert_eq!(
        report.unwound,
        vec![
            ActuationRecord::DpniBound { dpni: oc.dpni },
            ActuationRecord::DpniCreated {
                dpni: oc.dpni,
                container: None,
            },
        ],
    );
    assert_eq!(report.residue, vec![Residue::DpioSeat { object: oc.dpio }]);
    assert!(report.unreversed.is_empty(), "every reversible row unwound");

    // Net identity up to residue: the reversible prefix (the dpni) is gone, so the post-unwind
    // topology and inventory equal the pre-run board...
    assert_eq!(oc.backend.observe().unwrap(), oc.pre_topology);
    assert_eq!(oc.backend.read_inventory().unwrap(), oc.pre_inventory);
    // ...and the sole surviving difference is the named residue: the root dpio seat stands.
    let post_dpio: Vec<ObjectRef> = oc
        .backend
        .observe_pool(None, Family::Dpio)
        .unwrap()
        .iter()
        .map(|o| o.object)
        .collect();
    assert_eq!(post_dpio, vec![oc.dpio]);
}

/// Spec "Abort leaves the prefix for the loop": under the abort policy no undo verb fires and the
/// typed refusal names the executed prefix in forward order. The abort disposition provably
/// dispatches nothing, so a fresh fake is a sufficient witness for the empty undo audit — its
/// board is immaterial because abort never touches it (plan-rollback design D5).
#[test]
fn overcommit_abort_fires_no_undo_and_names_the_prefix() {
    let oc = run_overcommit();
    let fake = FakeBackend::new();

    let disposition = apply_failure_policy(FailurePolicy::Abort, &oc.journal, &fake, &fake);

    assert_eq!(
        disposition,
        FailureDisposition::Aborted {
            prefix: vec![
                ActuationRecord::DpniCreated {
                    dpni: oc.dpni,
                    container: None,
                },
                ActuationRecord::DpniBound { dpni: oc.dpni },
                ActuationRecord::PoolObjectCreated {
                    object: oc.dpio,
                    container: None,
                },
            ],
        },
    );
    assert!(
        fake.audit().is_empty(),
        "abort dispatches no undo verb — the fake's teardown audit stays empty"
    );
}

/// A pre-seeded plugged orphan in the child (never journaled) blocks the container destroy: the
/// row reports unreversed and the walk continues (plan-rollback design D5/D6; ORCH-I7). The root
/// dpio residue is an independent fact — root residue never strands a container, a child dpio is
/// reversible (ADR-0020 §3; ADR-0003 §7) — so the model's stranded-container shape is not mirrored
/// literally.
#[test]
fn rollback_leaves_orphaned_container_unreversed_and_continues_past_it() {
    let child = DprcId::new(2);
    let orphan = ObjectRef::new(Family::Dpbp, 99);
    let journal: JournalHandle = Rc::new(RefCell::new(RunJournal::new()));
    let backend = Recording::new(
        FakeBackend::new().with_pool_object(
            child,
            ObservedPoolObject {
                object: orphan,
                label: RawLabel::from("orphan"),
                plugged: true,
                drawn: false,
            },
        ),
        Rc::clone(&journal),
    );

    let lbl = ConstructName::from("port");
    let dpni = backend.create_dpni(&lbl, &DpniCfg::defaults()).unwrap();
    let created_child = backend
        .dprc_create(
            DprcId::ROOT,
            Options::DEFAULT,
            &ConstructName::from("child"),
        )
        .unwrap();
    assert_eq!(created_child, child, "the fake mints dprc.2 first");
    let dpio = backend.dpio_create(None, dpio_cfg(), &lbl).unwrap();

    let journal = journal.borrow().clone();
    assert_eq!(
        journal.iter().copied().collect::<Vec<_>>(),
        vec![
            ActuationRecord::DpniCreated {
                dpni,
                container: None,
            },
            ActuationRecord::DprcCreated { dprc: child },
            ActuationRecord::PoolObjectCreated {
                object: dpio,
                container: None,
            },
        ],
    );

    let disposition = apply_failure_policy(FailurePolicy::Rollback, &journal, &backend, &backend);
    let FailureDisposition::RolledBack(report) = disposition else {
        panic!("the rollback policy rolls back");
    };

    // The root dpio seat is irreversible, so it reports typed residue (ADR-0003 §7; ADR-0020).
    assert_eq!(report.residue, vec![Residue::DpioSeat { object: dpio }]);
    // The MC refuses the orphan's container destroy: unreversed, never silently unwound.
    assert_eq!(
        report.unreversed,
        vec![ActuationRecord::DprcCreated { dprc: child }],
    );
    assert_eq!(
        report.unwound,
        vec![ActuationRecord::DpniCreated {
            dpni,
            container: None,
        }],
    );

    assert!(
        backend.observe_containers().unwrap().contains_key(&child),
        "the refused container destroy left the dprc standing"
    );
    // The orphan was never journaled, so the unwind never reached it.
    assert_eq!(
        backend
            .observe_pool(Some(child), Family::Dpbp)
            .unwrap()
            .iter()
            .map(|o| o.object)
            .collect::<Vec<_>>(),
        vec![orphan],
    );
}
