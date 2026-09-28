//! Child-container population plan value types — the pure, renderable read one child dprc's
//! convergence and its `dry-run`/`status` surfaces share (pool-objects design D11; ADR-0018
//! filing pattern).
//!
//! [`ChildPlan`] folds each planned [`PlannedChildDpni`], the trio census/disposition, and the
//! dpio seat counts computed from the compiled plan and the child's read-back. The adapter's
//! `dispatch_child_population` half actuates it; these types only describe (pool-objects design D11).

use std::collections::BTreeMap;

use crate::core::model::{DpniId, DprcId, ObjectRef};
use crate::core::types::ConstructName;
use crate::families::dpio::{SeatDisposition, SeatRegime, seat_disposition};
use crate::families::dpni::DpniCfg;
use crate::families::pool_lifecycle::{PoolCensus, PoolDeltas, PoolFamily, ShrinkBelowDraw};
use crate::intent::compiled::ObjectKey;
use crate::plan::Class;

/// One child dpni the population plans, read-only (pool-objects design D11): its compiled
/// create block and label, the planned peer to connect it to (the plan edge, `None` when
/// the peer is a cross-container dpni whose id this tile cannot resolve), and the observed
/// state — the id when a matching-label dpni already exists, and whether its endpoint
/// already equals the peer (the connect idempotence read). The arity of the containing
/// [`ChildPlan::dpnis`] IS the plan's per-port dpni count, never a constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedChildDpni {
    /// The plan key identity.
    pub key: ObjectKey,
    /// The MC label the create stamps (the port's construct name).
    pub label: ConstructName,
    /// The compiled create block.
    pub cfg: DpniCfg,
    /// The planned connect peer (a root dpmac for a child port-edge), or `None` when the
    /// plan wires a cross-container dpni↔dpni peer (unresolvable to an id this tile).
    pub peer: Option<ObjectRef>,
    /// The observed dpni id when a matching-label row already exists.
    pub observed: Option<DpniId>,
    /// Whether the observed endpoint already equals the peer (connect idempotence).
    pub connected: bool,
}

impl PlannedChildDpni {
    /// Whether the dpni is absent and must be created (no matching-label row observed).
    #[must_use]
    pub fn needs_create(&self) -> bool {
        self.observed.is_none()
    }

    /// Whether a connect must issue: the plan names a peer and the dpni is absent or not yet
    /// endpoint-equal to it (the connect idempotence read).
    #[must_use]
    pub fn needs_connect(&self) -> bool {
        self.peer.is_some() && (self.observed.is_none() || !self.connected)
    }
}

/// The read-only population plan for one child dprc (pool-objects design D11): pure and
/// renderable, computed from the compiled plan and the child's read-back census. Every
/// count comes from the plan and every observation from a read — no mutation. The
/// `dispatch_child_population` half actuates it; the dry-run and status surfaces render it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildPlan {
    /// The child's re-observation handle.
    pub child: DprcId,
    /// The child's MC label (the owning tenant/holder name).
    pub label: ConstructName,
    /// The per-port dpnis the plan places in this child, arity plan-derived.
    pub dpnis: Vec<PlannedChildDpni>,
    /// Per trio family: the derived requirement, the observed census, and the count-level
    /// disposition (or the below-draw refusal).
    pub families: BTreeMap<PoolFamily, (i64, PoolCensus, Result<PoolDeltas, ShrinkBelowDraw>)>,
    /// The dpio seats: `(required, observed)`.
    pub seats: (i64, i64),
    /// Whether the child reads back bound to `vfio-fsl-mc` (ADR-0017 drift gate).
    pub bound: bool,
}

impl ChildPlan {
    /// Whether the child is converged: every planned dpni present and connected, every trio
    /// census meets its requirement, and the dpio seats MEET their requirement — a surplus
    /// converges grow-only and is reported as the typed residue (`dpio.qnt` `seatDisposition`;
    /// pool-objects design D4/D10). The idempotence witness a second pass reproduces.
    #[must_use]
    pub fn is_converged(&self) -> bool {
        self.dpnis
            .iter()
            .all(|d| !d.needs_create() && !d.needs_connect())
            && self.seats.1 >= self.seats.0
            && self
                .families
                .values()
                .all(|(req, census, _)| census.converged(*req))
    }

    /// The pass headline (ADR-0015 decision 12): [`Class::Disruptive`] when any resident
    /// must be created (a dpni, a trio delta, or a dpio seat), else [`Class::Hitless`] — a
    /// connect or a bind is the hitless plug face. Matches the [`ContainerStep`] class
    /// taxonomy (`CreateResident` disruptive, `PlugContainer` hitless).
    ///
    /// [`ContainerStep`]: crate::plan::dprc::ContainerStep
    #[must_use]
    pub fn headline(&self) -> Class {
        let creates = self.dpnis.iter().any(PlannedChildDpni::needs_create)
            || self
                .families
                .values()
                .any(|(_, _, d)| d.is_ok_and(|d| !d.is_empty()))
            || self.seats.0 > self.seats.1;
        if creates {
            Class::Disruptive
        } else {
            Class::Hitless
        }
    }

    /// The child seat disposition — the `dpio.qnt` `seatDisposition` at child scope, DPDK
    /// regime (a VFIO child's dpios are userspace-consumed): a surplus is the typed
    /// grow-only reboot-required residue, never a destroy (pool-objects design D4/D10).
    #[must_use]
    pub fn dpio_disposition(&self) -> SeatDisposition {
        seat_disposition(SeatRegime::DpdkSeat, self.seats.1, self.seats.0)
    }

    /// The grow half's dpio seat deficit — the count the grow pass creates (ADR-0012 counts).
    #[must_use]
    pub fn seat_deficit(&self) -> i64 {
        (self.seats.0 - self.seats.1).max(0)
    }

    /// The first PRE-DISPATCH below-draw refusal across the trio, if any (pool-objects design D3):
    /// the census already read the draw, so the requirement sits below it before any probe runs.
    /// The population convergence consults it to refuse the pass typed before dispatch; its
    /// discovered-draw twin surfaces after dispatch on `ChildPopulation::refusal`.
    #[must_use]
    pub fn shrink_refusal(&self) -> Option<ShrinkBelowDraw> {
        self.families.values().find_map(|(_, _, d)| d.err())
    }
}
