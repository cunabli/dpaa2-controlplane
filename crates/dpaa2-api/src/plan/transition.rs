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

/// Proof a specific dpni↔dpmac edge was severed before its kernel face unbinds (ADR-0008 §8).
///
/// The Rust twin of the model's severed witness (`models/core/connect.qnt`
/// `edgeDemandsSeveredWitness`; `models/families/dpmac.qnt` `severAt`/`unbindKernelFaceAt`,
/// `SeveredWitness`): `sever` consumes the bound edge and yields `Offered` plus this proof
/// for that edge's dpni, and the kernel-face unbind demands it. The law lives on the
/// dpni↔dpmac edge kind only (ADR-0019 edge facet; dpmac-typestate design D3); the shared
/// predicate is [`crate::plan::connect::edge_demands_severed_witness`] and this carrier is
/// the delivered machinery that table claims for the kind, not a parallel copy (ADR-0022).
///
/// The private [`DpniId`] field is the whole mechanism: there is no public constructor, so
/// the only mint is [`Transition::sever`], and [`Transition::unbind`] reads its target back
/// from the proof — a consumer can neither fabricate one nor retarget it at a different
/// dpni. It is not [`Copy`], so one sever's proof unbinds exactly once. The
/// unbind-before-sever order that strands a driverless port (ADR-0008 §8) therefore does not
/// typecheck. This is the plan-surface proof token; it is distinct from
/// [`crate::families::dpmac::SeveredWitness`], the freely-constructible observation
/// vocabulary — that enum judges a read-back and cannot serve as the token.
#[derive(PartialEq, Eq, Debug)]
pub struct SeveredProof(DpniId);

impl SeveredProof {
    /// The dpni whose edge this proof witnesses — read-only, so a consumer reads the unbind
    /// target back from the proof but can neither forge nor retarget it (ADR-0008 §8).
    #[must_use]
    pub fn dpni(&self) -> DpniId {
        self.0
    }
}

/// A single MC- or kernel-granularity action in a plan.
///
/// Operations are expressed one MC command at a time (mc-backend spec) so a future
/// ioctl backend maps one-to-one onto firmware commands behind the same trait.
#[derive(PartialEq, Eq, Debug)]
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
    /// Unbind an observed DPNI's kernel face from `dpaa2-eth` (teardown).
    ///
    /// This is the dpni↔dpmac edge's kernel-face unbind, the only `Unbind` the planner
    /// emits — it is always dpmac-facing (the port teardown in
    /// [`reconcile`](crate::plan::reconcile::reconcile)), so it carries the
    /// [`SeveredProof`] its edge law demands (ADR-0008 §8; dpmac-typestate design D3). The
    /// proof is mintable only by [`Transition::sever`], so no consumer can express
    /// unbind-before-sever — build this through [`Transition::unbind`]. The variant carries
    /// only the proof (no separate target field), and [`SeveredProof`] has no public
    /// constructor, so neither forging the witness nor retargeting its edge is representable;
    /// a struct literal buys nothing. No dpni-facing `Unbind` variant exists, because none is
    /// emitted — the law types exactly this one edge kind (dpmac-typestate design D3).
    Unbind {
        /// Proof this edge was severed first (ADR-0008 §8) — unforgeable and edge-bound, so
        /// neither the unbind-before-sever order nor a cross-edge reuse typechecks. The target
        /// dpni is read back through [`SeveredProof::dpni`]; the variant carries no separate
        /// target field, so retargeting is unrepresentable.
        proof: SeveredProof,
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

// Hand-written: `SeveredProof` is deliberately not `Clone` (dpmac-hardening design D1), so the
// `Unbind` arm re-mints its edge-bound proof in-crate — unforgeable outside, equal within.
impl Clone for Transition {
    fn clone(&self) -> Self {
        match self {
            Self::Create { port, label, cfg } => Self::Create {
                port: *port,
                label: label.clone(),
                cfg: cfg.clone(),
            },
            Self::Connect { port } => Self::Connect { port: *port },
            Self::Bind { port } => Self::Bind { port: *port },
            Self::SetMac { port, mac } => Self::SetMac {
                port: *port,
                mac: *mac,
            },
            Self::Disconnect { dpni } => Self::Disconnect { dpni: *dpni },
            Self::Unbind { proof } => Self::Unbind {
                proof: SeveredProof(proof.0),
            },
            Self::Destroy { dpni } => Self::Destroy { dpni: *dpni },
            Self::SetLabel { dpni, label } => Self::SetLabel {
                dpni: *dpni,
                label: label.clone(),
            },
        }
    }
}

impl Transition {
    /// Sever the dpni↔dpmac edge, minting the [`SeveredProof`] its kernel-face unbind
    /// demands (ADR-0008 §8; `models/families/dpmac.qnt` `severAt`). Returns the
    /// [`Transition::Disconnect`] step and the proof together — the planner pushes the
    /// disconnect, then threads the proof into [`Transition::unbind`], which reads its
    /// target dpni back from the proof. The proof binds this edge, so the sever-then-unbind
    /// order is owned by the types per edge, not by planner discipline: no proof means no
    /// unbind, and one edge's proof cannot unbind another (dpmac-typestate design D3). The
    /// disconnect step is unchanged from before.
    #[must_use]
    pub fn sever(dpni: DpniId) -> (Self, SeveredProof) {
        (Self::Disconnect { dpni }, SeveredProof(dpni))
    }

    /// Unbind a dpmac-facing dpni's kernel face, consuming the [`SeveredProof`] its
    /// [`sever`](Self::sever) minted and reading the target dpni back from it (ADR-0008 §8;
    /// `models/families/dpmac.qnt` `unbindKernelFaceAt`, whose guard routes through
    /// `connect.edgeDemandsSeveredWitness`). Because the proof has no public constructor and
    /// carries its own edge, both the unbind-before-sever order and any cross-edge or
    /// twice-over reuse are unrepresentable for a library consumer — neither snippet compiles:
    ///
    /// ```compile_fail,E0423
    /// use dpaa2_api::core::model::DpniId;
    /// use dpaa2_api::plan::{SeveredProof, Transition};
    /// // `SeveredProof`'s only mint is `Transition::sever` — its field is private, so no
    /// // consumer can forge the witness a kernel-face unbind demands (ADR-0008 §8; the
    /// // dpni↔dpmac edge law, dpmac-typestate design D3). The illegal order is rejected.
    /// let forged = SeveredProof(DpniId::new(7));
    /// let _ = Transition::unbind(forged);
    /// ```
    ///
    /// ```compile_fail,E0382
    /// use dpaa2_api::core::model::DpniId;
    /// use dpaa2_api::plan::Transition;
    /// // The proof is not `Copy` and binds its own edge, so one sever unbinds exactly once:
    /// // reuse (for the same or a different edge) moves the witness away, so it is gone the
    /// // second time (ADR-0008 §8; dpmac-typestate design D3).
    /// let (_disconnect, proof) = Transition::sever(DpniId::new(7));
    /// let _first = Transition::unbind(proof);
    /// let _second = Transition::unbind(proof);
    /// ```
    ///
    /// Retargeting is likewise unrepresentable: the variant carries no separate target field,
    /// so a struct literal that names a different dpni beside a legitimate proof does not name
    /// a field that exists:
    ///
    /// ```compile_fail,E0559
    /// use dpaa2_api::core::model::DpniId;
    /// use dpaa2_api::plan::Transition;
    /// let (_disconnect, proof) = Transition::sever(DpniId::new(7));
    /// let _ = Transition::Unbind { dpni: DpniId::new(9), proof };
    /// ```
    #[must_use]
    pub fn unbind(proof: SeveredProof) -> Self {
        Self::Unbind { proof }
    }

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

#[cfg(test)]
mod tests {
    //! The dpni↔dpmac sever-then-unbind proof path (ADR-0008 §8; the Rust twin of
    //! `models/families/dpmac.qnt` `severThenUnbindTest`). The negative face — that neither
    //! unbind-before-sever, cross-edge/twice-over reuse, nor edge retargeting typechecks — is
    //! the three `compile_fail` doctests on [`Transition::unbind`].

    use super::*;

    #[test]
    fn sever_mints_the_proof_unbind_consumes_it() {
        let (disconnect, proof) = Transition::sever(DpniId::new(7));
        assert_eq!(
            disconnect,
            Transition::Disconnect {
                dpni: DpniId::new(7)
            }
        );
        let unbind = Transition::unbind(proof);
        assert!(matches!(&unbind, Transition::Unbind { proof } if proof.dpni() == DpniId::new(7)));
        assert_eq!(unbind.class(), Class::Disruptive);
    }

    #[test]
    fn the_proof_is_equal_by_construction() {
        // Two proofs for one edge carry the same id, so plans stay equal (ADR-0015 decision 12).
        let (_, p1) = Transition::sever(DpniId::new(7));
        let (_, p2) = Transition::sever(DpniId::new(7));
        assert_eq!(Transition::unbind(p1), Transition::unbind(p2));
    }
}
