use std::collections::BTreeMap;

use crate::core::family::Family;
use crate::core::model::{DpmacId, DpniId};
use crate::plan::{Class, Transition};

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
/// A plan is *converged* when it carries no transitions. Drift, assert, and
/// plan-only reports are informational and never imply an actuation.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Plan {
    /// Ordered actions to move observed toward desired.
    pub transitions: Vec<Transition>,
    /// Immutable-attribute drift that was refused.
    pub drift: Vec<DriftReport>,
    /// Assert-only mismatches that were reported but not actuated.
    pub assertions: Vec<AssertMismatch>,
    /// Derived objects the port facet has no executor for, counted by family
    /// (design D10). Reported so an operator sees the whole plan; never actuated,
    /// never drift, and — like drift and assertions — it does not affect convergence.
    pub plan_only: BTreeMap<Family, usize>,
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

    /// The plan's headline disruption class (ADR-0015 decision 12): the maximum class
    /// over its transitions, or [`Class::Hitless`] for a converged (empty) plan. Only
    /// actuating transitions carry a class — drift, assertions and plan-only are
    /// non-actuating reports, so they never raise the headline.
    #[must_use]
    pub fn headline(&self) -> Class {
        self.transitions
            .iter()
            .map(Transition::class)
            .max()
            .unwrap_or(Class::Hitless)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::MacAddr;

    #[test]
    fn plan_only_is_reported_but_never_drift_or_divergence() {
        // A plan whose sole content is a plan-only summary stays converged and
        // divergence-free: plan-only objects are reported, never reconciled (D10).
        let mut plan = Plan::new();
        plan.plan_only.insert(Family::Dpio, 3);
        plan.plan_only.insert(Family::Dpbp, 1);
        assert!(plan.is_converged());
        assert!(!plan.has_divergence());
    }

    #[test]
    fn headline_is_the_max_transition_class() {
        // A converged plan is hitless; a plan with a create and a set-mac headlines
        // disruptive (the create wins the max), per ADR-0015 decision 12.
        assert_eq!(Plan::new().headline(), Class::Hitless);
        let plan = Plan {
            transitions: vec![
                Transition::SetMac {
                    port: DpmacId::new(7),
                    mac: MacAddr::ZERO,
                },
                Transition::Create {
                    port: DpmacId::new(7),
                    label: "wan0".into(),
                    num_queues: 0,
                },
            ],
            ..Plan::new()
        };
        assert_eq!(plan.headline(), Class::Disruptive);
        // A bind is intrinsically hitless: it makes a netdev appear where nothing
        // carried traffic, so it perturbs no existing flow (ADR-0015 decision 12).
        assert_eq!(
            Transition::Bind {
                port: DpmacId::new(7)
            }
            .class(),
            Class::Hitless
        );
    }
}
