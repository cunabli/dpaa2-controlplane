//! Root-scope pool drift value types — the read the pool convergence phase and the
//! `dry-run`/`status` surfaces share (pool-objects task 3.4; filed under `plan/` per ADR-0018).
//!
//! Pure plan data: [`PoolDrift`] folds each trio family's [`PoolFamilyDrift`] plus the dpio
//! seat counts, and [`PoolPass`] names which half of the grow-first/shrink-last walk a
//! convergence runs. The adapter drives; these types only describe (design D11; restool-baseline).

use crate::families::dpio::{SeatDisposition, SeatRegime, seat_disposition};
use crate::families::pool_lifecycle::{
    PoolCensus, PoolDeltas, PoolDisposition, PoolFamily, ShrinkBelowDraw, pool_disposition,
};
use crate::plan::Class;

/// Which half of the grow-first/shrink-last pool walk a `converge_pools` call runs
/// (pool-objects design D10 teardown ordering). The grow half runs BEFORE consumer
/// convergence — it dispatches only creates and dpio grows, and defers a below-draw shrink (a
/// consumer still holds the draw, torn down between the two passes). The shrink half runs
/// AFTER consumer teardown — it dispatches only destroys and prunes, and surfaces a genuine
/// below-draw refusal (a consumer the teardown could not release). Grow-first so a consumer
/// has capacity to draw; shrink-last so the teardown walk (consumers before pools) is the
/// executed order and an empty-intent ensure reaches prune instead of dying at a probe refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolPass {
    /// Grow capacity: creates and dpio seat grows only; below-draw deferred.
    Grow,
    /// Reclaim capacity: destroys and prunes only; below-draw surfaced.
    Shrink,
}

/// One root pool family's drift — its observed census, derived requirement, and the
/// count-level disposition [`drift_disposition`](crate::families::pool_lifecycle::drift_disposition)
/// would take (pool-objects task 3.4). The read seam `dry-run`/`status` render, carried as data
/// (the frontend owns its text; restool-baseline design D11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolFamilyDrift {
    /// The pooled family.
    pub family: PoolFamily,
    /// The observed census folded from the root's rows (the conservative restool proxy).
    pub census: PoolCensus,
    /// The plan-derived requirement (ADR-0012 counts, consumed unchanged).
    pub required: i64,
    /// The disposition the family would take, or the below-draw refusal (pool-objects design D3).
    pub disposition: Result<PoolDeltas, ShrinkBelowDraw>,
}

impl PoolFamilyDrift {
    /// The grow-only residue disposition this root family reports (ADR-0020, design D10
    /// amendment): the observed count is the labeled-plugged one, and above the requirement it
    /// renders reboot-required, else converged — root capacity cannot be reclaimed at runtime, so
    /// a surplus is reported and the reboot named, never a live destroy. The `dry-run`/`status`/
    /// `ensure` surfaces render it (the trio twin of the dpio [`SeatDisposition`] residue).
    ///
    /// Observed is the labeled-plugged count, not `managed` (ADR-0020 decision 2): at an empty
    /// intent the grown objects still wear a consumer label but declare no consumer, so `managed`
    /// reads zero and stays silent about capacity it grew and cannot reclaim. Any non-empty label
    /// proves runtime creation; the empty-label DPL boot pool is exempt. Where labels are
    /// declared the two counts agree, so every prior surplus case is unchanged.
    #[must_use]
    pub fn residue(&self) -> PoolDisposition {
        pool_disposition(self.family, self.census.labeled_plugged(), self.required)
    }
}

/// The root-scope pool drift across the trio plus the dpio seats — the read the pool
/// convergence phase and the `dry-run`/`status` surfaces share (pool-objects task 3.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolDrift {
    /// Per-trio-family drift, in traversal order (dpmcp→dpbp→dpcon; pool-objects design D8).
    pub families: Vec<PoolFamilyDrift>,
    /// The derived dpio seat count for the root (`derived_seats`).
    pub dpio_required: i64,
    /// The observed dpio seat count in the root.
    pub dpio_observed: i64,
}

impl PoolDrift {
    /// The pass headline (ADR-0015 decision 12): [`Class::Disruptive`] when any trio family
    /// needs a create/destroy/prune or a dpio seat is short, else [`Class::Hitless`] — the
    /// class the run gates on. A pool create/destroy is disruptive, matching the container
    /// steps' class-gating.
    #[must_use]
    pub fn headline(&self) -> Class {
        let trio_work = self
            .families
            .iter()
            .any(|f| f.disposition.is_ok_and(|d| !d.is_empty()));
        if trio_work || self.dpio_required > self.dpio_observed {
            Class::Disruptive
        } else {
            Class::Hitless
        }
    }

    /// The pass-specific headline (pool-objects design D10): the grow half is disruptive when a
    /// family needs a create or the dpio seats are short; the shrink half when a family needs a
    /// destroy or a prune. Split from [`headline`](Self::headline) so each pass gates only the
    /// work it dispatches — a shrink-only drift is hitless to the grow pass, and vice versa.
    #[must_use]
    pub fn headline_for(&self, pass: PoolPass) -> Class {
        let work = self.families.iter().any(|f| {
            f.disposition.is_ok_and(|d| match pass {
                PoolPass::Grow => d.create > 0,
                PoolPass::Shrink => d.destroy > 0 || d.prune > 0,
            })
        });
        let dpio = pass == PoolPass::Grow && self.dpio_required > self.dpio_observed;
        if work || dpio {
            Class::Disruptive
        } else {
            Class::Hitless
        }
    }

    /// The first below-draw refusal, if any (pool-objects design D3): a requirement under the drawn count
    /// surfaces to the operator by name and count, never a teardown of a live consumer.
    #[must_use]
    pub fn shrink_refusal(&self) -> Option<ShrinkBelowDraw> {
        self.families.iter().find_map(|f| f.disposition.err())
    }

    /// The grow half's dpio seat deficit — the count the grow pass creates (ADR-0012 counts).
    #[must_use]
    pub fn seat_deficit(&self) -> i64 {
        (self.dpio_required - self.dpio_observed).max(0)
    }

    /// The grow-only dpio seat disposition at root scope (pool-objects design D4/D10): the
    /// kernel-regime seats render [`SeatDisposition::RebootRequired`] when the observed count
    /// sits above the required one — a surplus no live destroy can reclaim (the ADR-0008 §4
    /// race), reported and reboot-named, never torn down. Root is the kernel container, so the
    /// regime is [`SeatRegime::KernelSeat`].
    #[must_use]
    pub fn dpio_disposition(&self) -> SeatDisposition {
        seat_disposition(
            SeatRegime::KernelSeat,
            self.dpio_observed,
            self.dpio_required,
        )
    }
}
