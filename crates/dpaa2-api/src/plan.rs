//! The output of reconciliation: an ordered [`Plan`] of [`Transition`]s plus
//! non-actuating [`DriftReport`]s and [`AssertMismatch`]es.
//!
//! Transitions that *create* an object reference the port's stable [`DpmacId`]
//! anchor rather than a DPNI index, because the index is not known until the MC
//! assigns it at create time (design D1). Transitions that *tear down* an existing
//! object reference the observed [`DpniId`].

use core::fmt;
use std::collections::BTreeMap;

use crate::family::Family;
use crate::model::{DpmacId, DpniId, MacAddr};
use crate::types::ConstructName;

/// The disruption class of a plan or one of its transitions (ADR-0015 decision 12).
///
/// The three classes are ordered low to high, so the derived [`Ord`] lets a plan's
/// headline be the maximum over its parts ([`Plan::headline`], the matcher's
/// [`crate::MatchPlan::headline`]). Dry-run reports the headline; converge gates on an explicit
/// allow of that class, and [`Class::Disruptive`] is never implied — the default allow
/// is [`Class::Hitless`] only. Both the port-edge reconciler ([`Transition::class`])
/// and the identity matcher share this one classing so a mixed plan has a single
/// comparable headline.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Class {
    /// No traffic effect: a `set-label` relabel, an attribute assert, or a
    /// wait-to-observe nudge (ADR-0015 decision 12). The default and the floor.
    #[default]
    Hitless,
    /// No traffic-path change, but an externally-held name changes — the netdev name
    /// systemd-networkd matches, or a VPP allowlist token held as text (ADR-0015
    /// decision 12, the decision-6 boundary wearing a new face).
    Boundary,
    /// A destroy/create, a link flap, a disconnect, or a rewired path (ADR-0015
    /// decision 12).
    Disruptive,
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Hitless => "hitless",
            Self::Boundary => "boundary",
            Self::Disruptive => "disruptive",
        })
    }
}

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
        /// The construct name stamped on the object at create — the owning
        /// construct's name (ADR-0015 decisions 9 + 13), written as the MC label the
        /// moment the object is minted so no read-back window ever shows it
        /// unlabelled (ADR-0010 §4 ABA guard). The name IS the label, byte-for-byte
        /// (decision 13).
        label: ConstructName,
        /// The compiled transmit-queue sizing (`Attributes::Dpni`) carried so the shim
        /// never re-derives it; 0 = unsized port-only projection, the backend falls
        /// back to its host-derived default (synthesis L2/B3).
        num_queues: u32,
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
    /// Relabel an observed DPNI to its intended construct name — the actuation the
    /// matcher's re-association lowers to (ADR-0015 decisions 9-10). A relabel is
    /// `set-label`, decision 9's repair verb, never a destroy/create; whether it is a
    /// hitless drift-repair or a boundary rename is the matcher's
    /// [`crate::matcher::MatchPlan::headline`] to judge from the prior label.
    /// [`reconcile`](crate::reconcile::reconcile) emits this for a port dpni whose
    /// observed label differs from its construct name.
    SetLabel {
        /// The observed DPNI to relabel.
        dpni: DpniId,
        /// The construct name to write as the label (ADR-0015 decision 13: the name
        /// IS the label, byte-for-byte).
        label: ConstructName,
    },
}

impl Transition {
    /// The disruption class of this transition (ADR-0015 decision 12). Each port-edge
    /// action is classed by its honest traffic effect:
    // Each variant keeps its own arm and rationale even where two share a class
    // (decision 12 asks the classing be stated per variant), so the identical bodies
    // are deliberate.
    #[allow(clippy::match_same_arms)]
    #[must_use]
    pub fn class(&self) -> Class {
        match self {
            // Minting the DPNI object — a create is disruptive by definition.
            Self::Create { .. } => Class::Disruptive,
            // Brings the dpni↔dpmac traffic path up: a new path, disruptive.
            Self::Connect { .. } => Class::Disruptive,
            // A class is intrinsic — a transition's effect on traffic already flowing
            // (ADR-0015 decision 12). Binding a DPNI to `dpaa2-eth` makes a netdev
            // appear where nothing yet carried traffic, so it perturbs no existing
            // flow: hitless, a wait-to-observe nudge. Containment never softens this —
            // a bind inside a destructive plan is already refused by the join
            // ([`Plan::headline`]). A future re-bind-after-unbind is a distinct
            // variant with its own intrinsic class, not a context-sensitive `class()`.
            Self::Bind { .. } => Class::Hitless,
            // An attribute actuate/assert on the DPNI primary MAC — no traffic
            // effect, so hitless (decision 12).
            Self::SetMac { .. } => Class::Hitless,
            // Teardown verbs: a disconnect flaps the link, an unbind drops the
            // netdev, a destroy removes the object — all disruptive.
            Self::Disconnect { .. } | Self::Unbind { .. } | Self::Destroy { .. } => {
                Class::Disruptive
            }
            // A relabel is never disruptive (ADR-0015 decision 12). A bare transition
            // cannot tell a hitless drift-repair from a boundary rename — that needs
            // the prior label the matcher holds — so it takes the conservative
            // boundary reading for gating; the matcher's headline judges the exact
            // per-pair class where the prior label is known.
            Self::SetLabel { .. } => Class::Boundary,
        }
    }
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
