//! The delta→id dispatch edge for the pool families (pool-objects design D2).
//!
//! The planner and disposition reason exclusively in counts per (container, family): a
//! [`PoolDeltas`] says grow by N, destroy M free-managed, prune K foreign-free. This
//! module is where counts become individuals — the one place identity enters, kept out of
//! the pure family surface (`dpaa2_api::families::pool_lifecycle`) exactly as the
//! pool-objects design D2 boundary decided. A grow emits N creates of the family's verb (companions wear their consumer's
//! name, ADR-0015); a destroy/prune selects victims from the *free* set only, first-N
//! (anonymity is the pattern's truth, so which free individual goes is not a policy
//! surface) — with the root prune narrowed to NEVER-PLUGGED foreigns (a plugged root foreign
//! is `fsl_mc_allocator`-bound residue, ADR-0020 decision 4; the [`CustodyScope`] carried into
//! the victim selection). Read-back is the observation returned, never an exit status
//! (mc-backend spec requirement 1).
//!
//! The two reclaim halves split on how they treat a busy refusal. The managed destroy path
//! discovers a per-victim draw: the MC's in-use refusal (`McStatus` `0x10`) on the unplug probe
//! is an observation, so the pass skips the held victim, tries the next free candidate, and
//! reports the typed [`ShrinkBelowDraw`] on [`PoolDispatch::refusal`] only when the free
//! candidates run out (pool-objects design D10). The foreign PRUNE path keeps its fail-fast
//! raw behavior — its count-level below-draw fold is a tracked refinement (ADR-0020).
//!
//! # Fail-fast, no rollback chain
//!
//! A verb error returns immediately and the objects already created this pass are left in
//! place — there is no reverse-order teardown, unlike a transactional rollback chain. Pool
//! capacity is anonymous and level-triggered: a partial grow is simply fewer companions
//! than the requirement, which the next converge pass tops up from the observed census.
//! Rolling a partial grow back would destroy capacity another consumer may already be
//! drawing, so the pass heals forward, it never unwinds.
//!
//! # dpio is not a pooled delta (pool-objects design D4)
//!
//! dpio is `pooled: false` (DPIO-I1): a seat, not pool custody, so it carries no
//! [`PoolDeltas`] and has no place in [`dispatch_pool_deltas`]. Its create is
//! [`create_dpio_seat`], which sequences the dpio→dpmcp probe-draw ordering procedurally.

use std::collections::BTreeSet;

use dpaa2_api::contract::McControl;
use dpaa2_api::core::error::Error;
use dpaa2_api::core::model::{DprcId, ObjectRef};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpio::{ChannelMode, DpioCfg, Priorities};
use dpaa2_api::families::pool_lifecycle::{
    CustodyScope, ObservedPoolObject, PoolDeltas, PoolFamily, PoolMembership, ShrinkBelowDraw,
};

/// The dpcon default priority count (`docs/baseline/dpcon.md` "Option inventory": 1–8,
/// **default 2**). Counts are the only plan input in phase 3.1, so a grown dpcon takes the
/// baseline default; a plan-carried priorities value is a later phase's concern.
fn dpcon_default_priorities() -> Priorities {
    Priorities::new(2).expect("the dpcon baseline default 2 is within the MC create range 1..=8")
}

/// The plain dpio create-cfg a grown seat takes — the ls-addni dpio defaults
/// (`DPIO_LOCAL_CHANNEL`, 8 priorities; `docs/baseline/dpio.md` "Option inventory"). The
/// compiled dpio companion is [`Attributes::Unsized`](dpaa2_api::intent::compiled::Attributes),
/// so a cfg drawn from the plan is a later tile — this is the deliberate stand-in until then,
/// shared by the child population and the root pool convergence (pool-objects design D4).
///
/// # Panics
/// Never in practice: the fixed priority count 8 is within the MC create range 1..=8.
#[must_use]
pub fn default_dpio_cfg() -> DpioCfg {
    DpioCfg {
        mode: ChannelMode::LocalChannel,
        priorities: Priorities::new(8)
            .expect("the ls-addni dpio default 8 is in the MC range 1..=8"),
    }
}

/// The outcome of dispatching one (container, pool-family) delta: the concrete objects
/// created, destroyed, and pruned, plus the post-dispatch census read straight back from
/// the board (pool-objects task 3.1). `after` is the observation — the exit status of any
/// verb is never the observation (mc-backend spec requirement 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolDispatch {
    /// The objects a grow created, in creation order (all plugged and stamped `label`).
    pub created: Vec<ObjectRef>,
    /// The free-managed objects a surplus destroy removed (the model's `shrinkDestroyAt`).
    pub destroyed: Vec<ObjectRef>,
    /// The foreign-free objects a prune reclaimed (the model's `pruneAt`).
    pub pruned: Vec<ObjectRef>,
    /// The board-proven below-draw refusal the managed destroy pass discovered — `Some` only
    /// when every free managed candidate was held (`McStatus` `0x10`) and the destroy quota
    /// stayed unmet; the caller judges it, it is never an [`Error`] (pool-objects design D10).
    pub refusal: Option<ShrinkBelowDraw>,
    /// The post-dispatch census, re-observed from the board (read-back, not exit status).
    pub after: Vec<ObservedPoolObject>,
}

/// Dispatches a count-level [`PoolDeltas`] to concrete create/destroy verbs against
/// concrete ids for one (container, pool-family) pair — the count→individual boundary
/// (pool-objects design D2/D3).
///
/// - **grow**: `deltas.create` creates of `family`'s verb, each stamped `label` (dpcon
///   takes the baseline default of 2 priorities; counts are the only plan input in 3.1).
/// - **destroy**: `deltas.destroy` victims drawn from ALL the free ours-judged rows — never a
///   DPL-born or foreign one — each reclaimed through the unplug probe. The pass discovers
///   then judges: a victim the MC refuses to unplug (`McStatus` `0x10`) is held, so it is
///   skipped and the next candidate tried; only an exhausted candidate set with the quota
///   unmet proves the below-draw refusal, reported on [`PoolDispatch::refusal`] — never an
///   [`Error`] (pool-objects design D10).
/// - **prune**: `deltas.prune` victims selected from the foreign-judged rows, first-N,
///   reclaimed through the same probe — DPL-born rows are structurally exempt (roadmap #14).
///   At [`CustodyScope::RootScope`] only NEVER-PLUGGED foreign rows are eligible (a plugged
///   root foreign is `fsl_mc_allocator`-bound residue, ADR-0020 decision 4), matching the
///   never-plugged prune count the disposition emits; at [`CustodyScope::ChildScope`] a plugged
///   foreign is still pruned through the probe (the child reclaim law is unchanged).
///
/// Custody and ownership are judged against `declared`, the same declared-name recognition
/// set the census and the inventory use (ADR-0010 §4 refined by ADR-0015), so the victims
/// match the census that produced the deltas. Selection reads one fresh observation taken
/// after the grow. Each victim is reclaimed by the two-step unplug probe: unplug, then
/// destroy; a managed victim the MC refuses to unplug (in use) is the drawn signal — skipped,
/// not torn down — so the pass tries the next candidate and reports the exhausted-candidate
/// refusal typed on [`PoolDispatch::refusal`], while a foreign prune victim propagates the raw
/// refusal (pool-objects design D10). `requirement` is the family's derived requirement,
/// carried so the discovered refusal names the count the operator sees. After the mutations a
/// final observation is read back into [`PoolDispatch::after`].
///
/// Fails fast on the first verb error with no rollback — see the module docs. The managed
/// busy refusal is the one exception: it is an observation, not a verb error.
///
/// # Errors
/// Returns the first [`Error`] any create, destroy, or observation verb raises (the typed
/// `McStatus`/`RestoolGuard`/`Backend` funnel is inherited from the shim), except the managed
/// unplug's `McStatus` `0x10`, which is discovered as the drawn signal. Objects already created
/// this pass are left in place for the next converge pass to build on.
#[allow(clippy::too_many_arguments)] // the requirement joins the count→id inputs, all read-only
pub fn dispatch_pool_deltas<M: McControl>(
    mc: &M,
    container: Option<DprcId>,
    family: PoolFamily,
    deltas: PoolDeltas,
    requirement: i64,
    label: &ConstructName,
    declared: &BTreeSet<ConstructName>,
    scope: CustodyScope,
) -> Result<PoolDispatch, Error> {
    // grow: each count is one create of the family's verb (companions wear the consumer's
    // name, ADR-0015). Fail-fast, no rollback (module docs).
    let mut created = Vec::new();
    for _ in 0..deltas.create {
        created.push(create_one(mc, container, family, label)?);
    }

    // destroy + prune: resolve the counts to concrete reclaim victims from one fresh observation, each reclaimed through the unplug probe (pool-objects design D10).
    let mut destroyed = Vec::new();
    let mut pruned = Vec::new();
    let mut refusal = None;
    if deltas.destroy > 0 || deltas.prune > 0 {
        let rows = mc.observe_pool(container, family.family())?;

        // Managed destroy discovers then judges: try EVERY free candidate, skip a held one
        // (McStatus 0x10), and an exhausted set with the quota unmet proves the refusal (pool-objects design D10).
        if deltas.destroy > 0 {
            let quota = usize::try_from(deltas.destroy).unwrap_or(0);
            let candidates =
                select_reclaimable(&rows, declared, PoolMembership::Managed, i64::MAX, scope);
            for victim in &candidates {
                if destroyed.len() == quota {
                    break;
                }
                match reclaim(mc, container, victim) {
                    Ok(()) => destroyed.push(victim.object),
                    Err(Error::McStatus { status: 0x10 }) => {}
                    Err(e) => return Err(e),
                }
            }
            if destroyed.len() < quota {
                // Quota unmet ⇒ every survivor is held: the drawn count the requirement fell below.
                let drawn = i64::try_from(candidates.len() - destroyed.len()).unwrap_or(i64::MAX);
                refusal = Some(ShrinkBelowDraw {
                    family,
                    requirement,
                    drawn,
                });
            }
        }

        // Foreign prune keeps the fail-fast raw path (its below-draw fold is a tracked refinement, ADR-0020).
        for victim in select_reclaimable(
            &rows,
            declared,
            PoolMembership::Foreign,
            deltas.prune,
            scope,
        ) {
            reclaim(mc, container, victim)?;
            pruned.push(victim.object);
        }
    }

    // Read-back is the observation: the post-dispatch census, never a verb's exit status.
    let after = mc.observe_pool(container, family.family())?;
    Ok(PoolDispatch {
        created,
        destroyed,
        pruned,
        refusal,
        after,
    })
}

/// Creates the paired dpmcp FIRST, then the dpio seat, stamping both `label`
/// (pool-objects design D4; DPIO-I1/DPMCP-I1).
///
/// The dpio driver draws one free dpmcp when a consumer probes the plugged dpio — the
/// hidden dependency behind ls-addni's dpmcp-per-dpio rule (`docs/baseline/dpio.md`
/// "Kernel-side behavior"). An exhausted dpmcp pool does not fail loudly; the probe defers
/// on a silent `-EPROBE_DEFER` loop. So the portal must exist before the dpio is probed,
/// and this helper creates it first — the deliberate order-flip versus ls-addni, which
/// creates the dpio then its dpmcp: the ordering is adapter-procedural, not a typed verb
/// obligation (mc-backend spec requirement 1; the dpni set-MAC-before-plug precedent).
/// Returns the created dpio.
///
/// # Errors
/// Returns the first [`Error`] the dpmcp or dpio create raises. A failed dpio create after
/// a successful dpmcp create leaves the dpmcp in place (the same forward-healing stance as
/// [`dispatch_pool_deltas`]).
pub fn create_dpio_seat<M: McControl>(
    mc: &M,
    container: Option<DprcId>,
    cfg: DpioCfg,
    label: &ConstructName,
) -> Result<ObjectRef, Error> {
    let _dpmcp = mc.dpmcp_create(container, label)?;
    mc.dpio_create(container, cfg, label)
}

/// One create of the pooled `family`'s verb, stamped `label` (dpcon takes the baseline
/// default priorities; pool-objects design D2). dpio is absent by construct — it is not a
/// [`PoolFamily`] variant (DPIO-I1); its create is [`create_dpio_seat`].
fn create_one<M: McControl>(
    mc: &M,
    container: Option<DprcId>,
    family: PoolFamily,
    label: &ConstructName,
) -> Result<ObjectRef, Error> {
    match family {
        PoolFamily::Dpbp => mc.dpbp_create(container, label),
        PoolFamily::Dpmcp => mc.dpmcp_create(container, label),
        PoolFamily::Dpcon => mc.dpcon_create(container, dpcon_default_priorities(), label),
    }
}

/// Selects the first `count` reclaim victims whose custody membership matches — the
/// arbitrary first-N victim pick (anonymity is the pattern's truth; pool-objects design D2).
/// A `Managed` filter yields shrink victims, a `Foreign` filter yields prune victims, and
/// DPL-born rows match neither so they are structurally exempt. Rows the observation already
/// knows drawn are skipped ([`ObservedPoolObject::is_free`]); the restool shim reports every
/// row undrawn, so the unplug probe in [`reclaim`] is what discovers a live draw
/// (pool-objects design D10) — no plugged⇒drawn proxy filters here anymore.
///
/// At [`CustodyScope::RootScope`] a foreign victim must additionally be NOT plugged: a plugged
/// root foreign is `fsl_mc_allocator`-bound residue prune must not reach (ADR-0020 decision 4;
/// the model's `prunable` gate `scope == ChildScope or not(plugged)`), so the never-plugged
/// foreign is chosen even when a plugged one sorts first, and the plugged one is left untouched
/// (never a wasted unplug attempt). The count the disposition emits is already the never-plugged
/// foreign count, so this filter aligns the concrete victims with that count. At
/// [`CustodyScope::ChildScope`] the plugged filter does not apply (the child prunes a plugged
/// foreign through the probe).
fn select_reclaimable<'a>(
    rows: &'a [ObservedPoolObject],
    declared: &BTreeSet<ConstructName>,
    membership: PoolMembership,
    count: i64,
    scope: CustodyScope,
) -> Vec<&'a ObservedPoolObject> {
    let n = usize::try_from(count).unwrap_or(0);
    rows.iter()
        .filter(|r| {
            r.is_free() && r.membership(declared) == membership && r.prunable_at(scope, membership)
        })
        .take(n)
        .collect()
}

/// Reclaims one victim through the two-step unplug probe (pool-objects design D10; DPBP-I2:
/// allocatable ⟺ plugged ∧ allocator-bound). A plugged victim is unplugged first (`dprc
/// assign --plugged=0`); the MC refusing that unplug because the object is in use IS the
/// per-victim drawn signal — read from the board, never inferred from a proxy. The managed
/// destroy caller treats that `McStatus` `0x10` as an observation: it skips the held victim,
/// tries the next candidate, and surfaces the typed
/// [`ShrinkBelowDraw`](dpaa2_api::families::pool_lifecycle::ShrinkBelowDraw) only when the free
/// candidates run out (a refusal, not data loss). A cleared (or already-unplugged) victim is
/// then destroyed.
///
/// The busy-on-unplug contract follows current MC behavior as observed through the restool
/// shim on the board (V-POOL-6; `docs/baseline/dprc.md`). A replacement transport (the
/// MC-portal ioctl) must re-verify it, not inherit it.
fn reclaim<M: McControl>(
    mc: &M,
    container: Option<DprcId>,
    victim: &ObservedPoolObject,
) -> Result<(), Error> {
    if victim.plugged {
        mc.dprc_assign(
            container.unwrap_or(DprcId::ROOT),
            victim.object,
            None,
            Some(false),
        )?;
    }
    mc.pool_destroy(&victim.object)
}

#[cfg(test)]
mod tests {
    //! Dispatch over the shim mapping, driven through [`RestoolMc`] and a scripted runner
    //! (the transcript-test idiom, mirroring `restool.rs`): a grow becomes N creates and a
    //! re-observe (mc-backend scenario 1), a surplus destroy targets exactly the
    //! adapter-selected free-ours id and re-observes (scenario 2), a prune reclaims only
    //! foreign-free rows, and `create_dpio_seat` orders the dpmcp before the dpio.

    use std::cell::RefCell;
    use std::collections::HashMap;

    use dpaa2_api::core::family::Family;
    use dpaa2_api::core::types::ConstructName;
    use dpaa2_api::families::dpio::ChannelMode;

    use crate::restool::{DEFAULT_CONTAINER, RestoolMc};
    use crate::runner::{RunOutcome, Runner};

    use super::*;

    /// A [`Runner`] replaying a canned [`RunOutcome`] per joined argument line and
    /// recording every call — the same idiom `restool.rs`'s tests use, kept local so this
    /// module's tests stand alone.
    struct ScriptedRunner {
        outcomes: HashMap<String, RunOutcome>,
        calls: RefCell<Vec<Vec<String>>>,
    }

    fn ok(stdout: &str) -> RunOutcome {
        RunOutcome {
            stdout: stdout.to_owned(),
            stderr: String::new(),
            code: Some(0),
        }
    }

    impl ScriptedRunner {
        fn new(pairs: Vec<(String, RunOutcome)>) -> Self {
            Self {
                outcomes: pairs.into_iter().collect(),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }
    }

    impl Runner for ScriptedRunner {
        fn run(&self, args: &[&str]) -> Result<String, Error> {
            let out = self.run_capture(args)?;
            if out.code == Some(0) {
                Ok(out.stdout)
            } else {
                Err(Error::Backend(out.stderr))
            }
        }

        fn run_capture(&self, args: &[&str]) -> Result<RunOutcome, Error> {
            self.calls
                .borrow_mut()
                .push(args.iter().map(|s| (*s).to_owned()).collect());
            self.outcomes.get(&args.join(" ")).cloned().ok_or_else(|| {
                Error::Backend(format!("no scripted outcome for `{}`", args.join(" ")))
            })
        }
    }

    /// A `dprc show` body: header, contains-line, then `family.N [label] state` rows.
    fn dprc_show(rows: &[&str]) -> String {
        let mut s = format!("dprc.1 contains {} objects:\n", rows.len());
        s.push_str("object          label           plugged-state\n");
        for r in rows {
            s.push_str(r);
            s.push('\n');
        }
        s
    }

    fn declared(names: &[&str]) -> BTreeSet<ConstructName> {
        names.iter().map(|n| ConstructName::from(*n)).collect()
    }

    // The full create_and_plug script for one pool create: create → assign → set-label.
    fn create_script(create_line: &str, obj: &str, create_out: &str) -> Vec<(String, RunOutcome)> {
        vec![
            (create_line.to_owned(), ok(create_out)),
            (
                format!("dprc assign dprc.1 --object={obj} --plugged=1"),
                ok(""),
            ),
            (format!("dprc set-label {obj} --label=vpp"), ok("")),
        ]
    }

    // mc-backend scenario 1: a grow deficit of 2 dpcon becomes two creates in the
    // container, and the post-dispatch census reads back the new count.
    #[test]
    fn grow_delta_of_two_dpcon_issues_two_creates_and_reobserves() {
        let mut pairs = create_script(
            "--script dpcon create --num-priorities=2 --container=dprc.1",
            "dpcon.5",
            "dpcon.5\n",
        );
        let show = dprc_show(&[
            "dpcon.5         vpp             plugged",
            "dpcon.6         vpp             plugged",
        ]);
        pairs.push(("dprc show dprc.1".to_owned(), ok(&show)));
        let mc = RestoolMc::with_runner(ScriptedRunner::new(pairs), DEFAULT_CONTAINER);

        let out = dispatch_pool_deltas(
            &mc,
            None,
            PoolFamily::Dpcon,
            PoolDeltas {
                create: 2,
                destroy: 0,
                prune: 0,
            },
            2,
            &ConstructName::from("vpp"),
            &declared(&["vpp"]),
            CustodyScope::ChildScope,
        )
        .expect("grow dispatches");

        assert_eq!(out.created.len(), 2, "two dpcon creates");
        // Exactly two dpcon create invocations issued in the container.
        let creates = mc
            .runner()
            .calls()
            .into_iter()
            .filter(|c| {
                c.first().map(String::as_str) == Some("--script")
                    && c.get(1).map(String::as_str) == Some("dpcon")
            })
            .count();
        assert_eq!(creates, 2);
        // The post-dispatch census is re-observed and reads back the new count.
        assert!(
            mc.runner()
                .calls()
                .iter()
                .any(|c| c == &["dprc", "show", "dprc.1"]),
            "a post-dispatch dprc show is re-issued"
        );
        assert_eq!(out.after.len(), 2);
    }

    // mc-backend scenario 2: a surplus destroy reclaims one ours-judged dpbp through the unplug probe (`--plugged=0` precedes `dpbp destroy`); DPL and foreign rows are exempt (pool-objects design D10).
    #[test]
    fn surplus_destroy_reclaims_the_ours_dpbp_through_the_probe() {
        let show = dprc_show(&[
            "dpbp.5          vpp             plugged", // ours ⇒ the only destroy victim
            "dpbp.2                          plugged", // empty ⇒ DPL, exempt
            "dpbp.3          vendor          plugged", // foreign ⇒ a prune target, not a destroy
        ]);
        let mc = RestoolMc::with_runner(
            ScriptedRunner::new(vec![
                ("dprc show dprc.1".to_owned(), ok(&show)),
                (
                    "dprc assign dprc.1 --object=dpbp.5 --plugged=0".to_owned(),
                    ok(""),
                ),
                ("dpbp destroy dpbp.5".to_owned(), ok("")),
                ("dprc sync".to_owned(), ok("")),
            ]),
            DEFAULT_CONTAINER,
        );

        let out = dispatch_pool_deltas(
            &mc,
            None,
            PoolFamily::Dpbp,
            PoolDeltas {
                create: 0,
                destroy: 1,
                prune: 0,
            },
            0,
            &ConstructName::from("vpp"),
            &declared(&["vpp"]),
            CustodyScope::ChildScope,
        )
        .expect("shrink dispatches");

        assert_eq!(out.destroyed, vec![ObjectRef::new(Family::Dpbp, 5)]);
        assert_eq!(out.pruned, [] as [dpaa2_api::core::model::ObjectRef; 0]);
        let calls = mc.runner().calls();
        let unplug_at = calls
            .iter()
            .position(|c| c == &["dprc", "assign", "dprc.1", "--object=dpbp.5", "--plugged=0"])
            .expect("the victim is unplugged (probed)");
        let destroy_at = calls
            .iter()
            .position(|c| c == &["dpbp", "destroy", "dpbp.5"])
            .expect("the victim is destroyed");
        assert!(unplug_at < destroy_at, "unplug probe precedes destroy");
        let destroys: Vec<_> = calls
            .iter()
            .filter(|c| c.get(1).map(String::as_str) == Some("destroy"))
            .collect();
        assert_eq!(
            destroys,
            vec![&vec!["dpbp".to_owned(), "destroy".into(), "dpbp.5".into()]]
        );
    }

    /// Child-scoped prune reclaims a plugged foreign through the probe (DPL exempt; pool-objects design D10) — the probe-prune of a plugged foreign is the CHILD law now, at root it is residue (ADR-0020 decision 4).
    #[test]
    fn prune_reclaims_foreign_rows_through_the_probe() {
        let show = dprc_show(&[
            "dpbp.0                          plugged", // empty ⇒ DPL, exempt
            "dpbp.1          vendor          plugged", // foreign ⇒ the only prune victim
        ]);
        let mc = RestoolMc::with_runner(
            ScriptedRunner::new(vec![
                ("dprc show dprc.1".to_owned(), ok(&show)),
                (
                    "dprc assign dprc.1 --object=dpbp.1 --plugged=0".to_owned(),
                    ok(""),
                ),
                ("dpbp destroy dpbp.1".to_owned(), ok("")),
                ("dprc sync".to_owned(), ok("")),
            ]),
            DEFAULT_CONTAINER,
        );

        let out = dispatch_pool_deltas(
            &mc,
            None,
            PoolFamily::Dpbp,
            PoolDeltas {
                create: 0,
                destroy: 0,
                prune: 1,
            },
            0,
            &ConstructName::from("vpp"),
            &declared(&["vpp"]),
            CustodyScope::ChildScope,
        )
        .expect("prune dispatches");

        assert_eq!(out.pruned, vec![ObjectRef::new(Family::Dpbp, 1)]);
        assert_eq!(out.destroyed, [] as [dpaa2_api::core::model::ObjectRef; 0]);
        let destroys: Vec<_> = mc
            .runner()
            .calls()
            .into_iter()
            .filter(|c| c.get(1).map(String::as_str) == Some("destroy"))
            .collect();
        assert_eq!(destroys, vec![vec!["dpbp", "destroy", "dpbp.1"]]);
    }

    /// Root: with a MIXED census (a plugged foreign sorted first + a never-plugged one), only the never-plugged is a prune victim; the plugged residue is untouched (ADR-0020 decision 4).
    #[test]
    fn root_prune_selects_the_never_plugged_foreign_only() {
        use dpaa2_api::contract::fake::FakeBackend;
        use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};

        let declared = declared(&["vpp"]);
        let plugged_foreign = ObjectRef::new(Family::Dpbp, 1);
        let never_plugged_foreign = ObjectRef::new(Family::Dpbp, 2);
        let foreign = |object, plugged| ObservedPoolObject {
            object,
            label: RawLabel::from("vendor"),
            plugged,
            drawn: false,
        };
        // The plugged foreign is seeded FIRST so it sorts ahead of the never-plugged one.
        let mc = FakeBackend::new()
            .with_pool_object(DprcId::ROOT, foreign(plugged_foreign, true))
            .with_pool_object(DprcId::ROOT, foreign(never_plugged_foreign, false));

        let out = dispatch_pool_deltas(
            &mc,
            None,
            PoolFamily::Dpbp,
            PoolDeltas {
                create: 0,
                destroy: 0,
                prune: 1,
            },
            0,
            &ConstructName::from("vpp"),
            &declared,
            CustodyScope::RootScope,
        )
        .expect("root prune dispatches");

        assert_eq!(out.pruned, vec![never_plugged_foreign]);

        let rows = mc.observe_pool(None, Family::Dpbp).unwrap();
        assert!(
            !rows.iter().any(|r| r.object == never_plugged_foreign),
            "the never-plugged foreign is the chosen root prune victim"
        );
        let survivor = rows
            .iter()
            .find(|r| r.object == plugged_foreign)
            .expect("the plugged foreign survives as residue");
        assert!(
            survivor.plugged,
            "the plugged residue is untouched: no `dprc assign --plugged=0` was dispatched against it"
        );
    }

    // The unplug probe both directions over the stateful fake: a free managed victim unplugs then destroys; a drawn one with no free alternative bounces `-EBUSY`, is never destroyed, and the pass reports the typed below-draw refusal — not an error (pool-objects design D10).
    #[test]
    fn unplug_probe_reclaims_free_but_refuses_drawn() {
        use dpaa2_api::contract::fake::FakeBackend;
        use dpaa2_api::families::pool_lifecycle::{ObservedPoolObject, RawLabel};

        let vpp = ConstructName::from("vpp");
        let declared = declared(&["vpp"]);
        let free = ObjectRef::new(Family::Dpbp, 7);
        let drawn = ObjectRef::new(Family::Dpbp, 8);
        let row = |object| ObservedPoolObject {
            object,
            label: RawLabel::from("vpp"),
            plugged: true,
            drawn: false, // restool never reads the draw; the probe discovers it
        };

        // succeeds-on-free: a plugged, undrawn managed victim reclaims cleanly.
        let mc = FakeBackend::new().with_pool_object(DprcId::ROOT, row(free));
        let out = dispatch_pool_deltas(
            &mc,
            None,
            PoolFamily::Dpbp,
            PoolDeltas {
                create: 0,
                destroy: 1,
                prune: 0,
            },
            0,
            &vpp,
            &declared,
            CustodyScope::ChildScope,
        )
        .expect("a free victim reclaims");
        assert_eq!(out.destroyed, vec![free]);
        assert!(out.refusal.is_none(), "a free victim raises no refusal");
        assert!(
            mc.observe_pool(None, Family::Dpbp).unwrap().is_empty(),
            "the reclaimed victim is gone"
        );

        // refused-on-drawn: the sole managed row reads free but is held, no free alternative — the pass returns the typed refusal, not an error.
        let mc = FakeBackend::new().with_in_use_pool_object(DprcId::ROOT, row(drawn));
        let out = dispatch_pool_deltas(
            &mc,
            None,
            PoolFamily::Dpbp,
            PoolDeltas {
                create: 0,
                destroy: 1,
                prune: 0,
            },
            0,
            &vpp,
            &declared,
            CustodyScope::ChildScope,
        )
        .expect("a drawn victim is a refusal, not an error");
        assert_eq!(
            out.refusal,
            Some(ShrinkBelowDraw {
                family: PoolFamily::Dpbp,
                requirement: 0,
                drawn: 1,
            }),
            "the exhausted-candidate below-draw refusal"
        );
        assert!(
            out.destroyed.is_empty(),
            "the drawn victim is never destroyed"
        );
        assert_eq!(
            mc.observe_pool(None, Family::Dpbp).unwrap().len(),
            1,
            "the drawn victim survives"
        );
    }

    // create_dpio_seat creates the paired dpmcp BEFORE the dpio (the probe-draw ordering;
    // DPIO-I1/DPMCP-I1; pool-objects design D4).
    #[test]
    fn create_dpio_seat_orders_dpmcp_before_dpio() {
        let mut pairs = create_script(
            "--script dpmcp create --container=dprc.1",
            "dpmcp.0",
            "dpmcp.0\n",
        );
        pairs.extend(create_script(
            "--script dpio create --channel-mode=DPIO_LOCAL_CHANNEL --container=dprc.1 \
             --num-priorities=8",
            "dpio.1",
            "dpio.1\n",
        ));
        let mc = RestoolMc::with_runner(ScriptedRunner::new(pairs), DEFAULT_CONTAINER);

        let cfg = DpioCfg {
            mode: ChannelMode::LocalChannel,
            priorities: Priorities::new(8).expect("8 is in 1..=8"),
        };
        let dpio =
            create_dpio_seat(&mc, None, cfg, &ConstructName::from("vpp")).expect("seat created");
        assert_eq!(dpio, ObjectRef::new(Family::Dpio, 1));

        // The dpmcp create precedes the dpio create in the issued command order.
        let calls = mc.runner().calls();
        let dpmcp_at = calls
            .iter()
            .position(|c| c.get(1).map(String::as_str) == Some("dpmcp"))
            .expect("a dpmcp create was issued");
        let dpio_at = calls
            .iter()
            .position(|c| c.get(1).map(String::as_str) == Some("dpio"))
            .expect("a dpio create was issued");
        assert!(dpmcp_at < dpio_at, "the dpmcp is created before the dpio");
    }
}
