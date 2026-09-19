use core::fmt;

use crate::core::model::{DpmacId, DpniId, MacAddr};
use crate::core::types::ConstructName;
use crate::families::dpni::DpniCfg;

/// The disruption class of a plan or one of its transitions (ADR-0015 decision 12).
///
/// The three classes are ordered low to high, so the derived [`Ord`] lets a plan's
/// headline be the maximum over its parts ([`Plan::headline`](crate::plan::Plan::headline), the matcher's
/// [`crate::plan::matcher::MatchPlan::headline`]). Dry-run reports the headline; converge gates on an explicit
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
        /// The compiled, in-envelope create block (`Attributes::Dpni`) carried verbatim
        /// so the shim renders it without re-deriving anything (dpni-typestate task 4.1;
        /// dpni-typestate design D3). Sizing rides inside as [`DpniCfg::num_queues`]; an unsized
        /// port-only projection carries [`DpniCfg::defaults`] (`num_queues` 0), which the
        /// backend sizes from the host (synthesis L2/B3).
        cfg: DpniCfg,
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
    /// Set the primary MAC of the port's DPNI (only in [`crate::core::model::MacMode::Actuate`]).
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
    /// [`crate::plan::matcher::MatchPlan::headline`] to judge from the prior label.
    /// [`reconcile`](crate::plan::reconcile::reconcile) emits this for a port dpni whose
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
