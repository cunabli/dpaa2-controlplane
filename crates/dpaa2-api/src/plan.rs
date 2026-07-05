//! The output of reconciliation: an ordered [`Plan`] of [`Transition`]s plus
//! non-actuating [`DriftReport`]s and [`AssertMismatch`]es.
//!
//! Transitions that *create* an object reference the port's stable [`DpmacId`]
//! anchor rather than a DPNI index, because the index is not known until the MC
//! assigns it at create time (design D1). Transitions that *tear down* an existing
//! object reference the observed [`DpniId`].

use crate::model::{DpmacId, DpniId, MacAddr};

/// A single MC- or kernel-granularity action in a plan.
///
/// Operations are expressed one MC command at a time (mc-backend spec) so a future
/// ioctl backend maps one-to-one onto firmware commands behind the same trait.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Transition {
    /// Create a DPNI destined for the port anchored at this DPMAC.
    Create {
        /// The anchor the new DPNI will be connected to.
        port: DpmacId,
    },
    /// Connect the port's DPNI to its DPMAC (single edge).
    Connect {
        /// The anchor to connect to.
        port: DpmacId,
    },
    /// Ensure `dpaa2-eth` has bound the port's DPNI (wait-to-observe).
    Bind {
        /// The anchor whose DPNI should become bound.
        port: DpmacId,
    },
    /// Set the primary MAC of the port's DPNI (only in [`crate::MacMode::Actuate`]).
    SetMac {
        /// The anchor whose DPNI MAC is being written.
        port: DpmacId,
        /// The MAC to write.
        mac: MacAddr,
    },
    /// Disconnect an observed DPNI from its DPMAC (teardown).
    Disconnect {
        /// The observed DPNI to disconnect.
        dpni: DpniId,
    },
    /// Unbind an observed DPNI from `dpaa2-eth` (teardown).
    Unbind {
        /// The observed DPNI to unbind.
        dpni: DpniId,
    },
    /// Destroy an observed DPNI object (teardown, prune only).
    Destroy {
        /// The observed DPNI to destroy.
        dpni: DpniId,
    },
}

/// A refusal: an immutable, create-time-only attribute differs from desired.
///
/// Reconciliation reports this and plans no destructive change (design D8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DriftReport {
    /// The observed DPNI whose immutable attribute drifted.
    pub dpni: DpniId,
    /// The attribute name.
    pub attribute: String,
    /// Human-readable description of desired vs. observed.
    pub detail: String,
}

/// An assert-only field whose observed value does not match intent.
///
/// Reported, never actuated (design D9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AssertMismatch {
    /// The anchor whose port asserted a value that did not hold.
    pub port: DpmacId,
    /// The field name (e.g. `mac`).
    pub field: String,
    /// Human-readable description of asserted vs. observed.
    pub detail: String,
}

/// The full result of a reconcile pass.
///
/// A plan is *converged* when it carries no transitions. Drift and assert reports
/// are informational and never imply an actuation.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Plan {
    /// Ordered actions to move observed toward desired.
    pub transitions: Vec<Transition>,
    /// Immutable-attribute drift that was refused.
    pub drift: Vec<DriftReport>,
    /// Assert-only mismatches that were reported but not actuated.
    pub assertions: Vec<AssertMismatch>,
}

impl Plan {
    /// Creates an empty (converged) plan.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` when there is nothing to actuate.
    #[must_use]
    pub fn is_converged(&self) -> bool {
        self.transitions.is_empty()
    }

    /// Returns `true` when drift or an assert mismatch was reported.
    #[must_use]
    pub fn has_divergence(&self) -> bool {
        !self.drift.is_empty() || !self.assertions.is_empty()
    }
}
