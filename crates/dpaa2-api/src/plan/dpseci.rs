//! Plan semantics for the dpseci create block — the pure judgment that compares a newly
//! desired create cfg against the previously desired one, and how a mismatch must be
//! repaired (dpseci-typestate design D5; ADR-0015 decision 12).
//!
//! Filed under `plan/` as `plan/dpseci.rs`, following the family-planner pattern
//! [`crate::plan::dprc`] set (ADR-0018). The shipped census executor consumes
//! [`census_delta`]/[`observed_sig_of`], not this function; `classify_cfg_mismatch` remains
//! the plan-layer repair law for desired-versus-desired comparisons
//! (dpseci-typestate design D9).
//!
//! # The immutable-cfg repair law (dpseci-typestate design D5; DPSECI-I1)
//!
//! Every dpseci create-time value — the options mask, the queue count, and the per-queue
//! priorities — is immutable: there is no `dpseci_set_*` for any of them, so no live
//! mutation path exists (`docs/baseline/dpseci.md` "Attribute mutability";
//! [`crate::families::dpseci`]). A cfg difference therefore cannot be
//! repaired in place; it is a destroy+create replacement, the [`Class::Disruptive`] class
//! (ADR-0015 decision 12; `dpseci.qnt` `dpseci_lifecycle` `destroy` is the sole resize
//! path). Equal blocks need no repair.

use std::collections::BTreeMap;

use crate::contract::{DpseciDetail, DpseciPortalReadout};
use crate::families::dpseci::{DpseciCfg, OptionMask};
use crate::plan::Class;

/// Judge a newly desired dpseci create block against the previously desired (incumbent)
/// one, yielding the repair class when they differ and `None` when they already match
/// (dpseci-typestate design D5; D9 keeps this a desired-versus-desired law, because the
/// full create cfg has no observed operand — priorities have no read-back).
///
/// Any difference — an option bit, the queue count, or a priority entry — is an
/// immutable-cfg mismatch, and no setter exists to repair it in place, so the only repair
/// is a destroy+create replacement at [`Class::Disruptive`] (ADR-0015 decision 12;
/// DPSECI-I1). The judgment is total on the whole block: because [`DpseciCfg`] is immutable
/// and compares by value, a single `!=` captures every field without the caller
/// enumerating them, so no future field can silently escape the mismatch check.
#[must_use]
pub fn classify_cfg_mismatch(desired: &DpseciCfg, incumbent: &DpseciCfg) -> Option<Class> {
    (desired != incumbent).then_some(Class::Disruptive)
}

// ---- the observable-signature census (dpseci-typestate design D9) ----
// Structural Rust twins of the `dpseci.qnt` `dpseci_lifecycle` census operators.

/// The observable cfg signature — the queue shape and options, the only subset with a
/// read-back (`dpseci.qnt` `type ObservableSig`; dpseci-typestate design D9). Priorities are
/// create-time immutable with no read-back, so they never enter the judgment (ADR-0018: the
/// adapter never invents an unobserved value).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sig {
    /// The queue-pair count (`dpseci.qnt` `numQueues`).
    pub num_queues: usize,
    /// The options mask (`dpseci.qnt` `options`).
    pub options: OptionMask,
}

/// The projection that drops priorities (`dpseci.qnt` `observableSigOf`; this is the
/// dpseci-typestate design D9 observable subset).
#[must_use]
pub fn sig_of(cfg: &DpseciCfg) -> Sig {
    Sig {
        num_queues: cfg.num_queues(),
        options: cfg.options().clone(),
    }
}

/// One object's read-back face (`dpseci.qnt` `type ObservedFace`): the observed signature, or
/// typed unobservable — absence of evidence, never drift (dpseci-typestate design D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservedSig {
    /// The portal answered with a nameable signature (`dpseci.qnt` `SigObserved`).
    Observed(Sig),
    /// The signature cannot be judged this run (`dpseci.qnt` `SigUnobservable`).
    Unobservable,
}

/// Maps one dpseci's witnessable detail to its observed face (dpseci-typestate design D9). A
/// missing queue line, an unavailable portal, or a mask carrying an attributed raw escape is
/// typed [`ObservedSig::Unobservable`] — absence of evidence, so the census judges nothing
/// rather than invent a value (ADR-0018; V-LIFE-DPSECI-1). The classification lives core-side;
/// the adapter only reports the [`DpseciDetail`] (PASS5-F1).
///
/// An attributed escape is nameable evidence of an *unnameable* option bit, so the census
/// treats it exactly as the old unnamed-bit gap did: it judges nothing. Folding an escape into
/// the signature would make the readback differ from every planned cfg and drive a
/// destroy+create loop (dpseci-hardening design D2; V-LIFE-DPSECI-1). The escape thus changes
/// only what the detail row displays, never the plan.
#[must_use]
pub fn observed_sig_of(detail: &DpseciDetail) -> ObservedSig {
    match &detail.portal {
        // No portal: the face is unobservable (dpseci-typestate design D9).
        DpseciPortalReadout::Unobservable { .. } => ObservedSig::Unobservable,
        // An attributed escape is an unnameable bit: the census judges nothing (dpseci-hardening design D2).
        DpseciPortalReadout::Observed { options, .. } if !options.escapes().is_empty() => {
            ObservedSig::Unobservable
        }
        DpseciPortalReadout::Observed { options, .. } => match detail.num_tx_queues {
            Some(queues) => ObservedSig::Observed(Sig {
                num_queues: usize::from(queues),
                options: options.clone(),
            }),
            // A missing queue line leaves the shape half unjudgeable (dpseci-typestate design D9).
            None => ObservedSig::Unobservable,
        },
    }
}

/// Observable-subset drift for one desired object against its read-back (`dpseci.qnt`
/// `classifyObservableMismatch`, rendered in [`classify_cfg_mismatch`]'s `Option<Class>`
/// shape): an unobservable face is no drift judged (ADR-0018); an observed signature equal to
/// the desired projection is no drift; any other observed signature is the destroy+create
/// disruption of dpseci-typestate design D5.
#[must_use]
pub fn classify_observable_mismatch(desired: &DpseciCfg, face: &ObservedSig) -> Option<Class> {
    match face {
        ObservedSig::Unobservable => None,
        ObservedSig::Observed(sig) => (*sig != sig_of(desired)).then_some(Class::Disruptive),
    }
}

/// A census is the multiset of signatures as a count map (`dpseci.qnt` `sigCensus`): the
/// planned census is built from compiled cfgs via [`sig_of`], the observed from read-backs
/// via [`observed_sig_of`].
#[must_use]
pub fn sig_census(sigs: impl IntoIterator<Item = Sig>) -> BTreeMap<Sig, i64> {
    let mut census = BTreeMap::new();
    for sig in sigs {
        *census.entry(sig).or_insert(0) += 1;
    }
    census
}

/// The multiset delta (`dpseci.qnt` `type CensusDelta`; dpseci-typestate design D9): per
/// signature, the planned surplus is a create count and the observed surplus a destroy count.
/// A signature mismatch therefore surfaces as one create AND one destroy — the destroy+create
/// disruption of dpseci-typestate design D5 read off the two maps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CensusDelta {
    /// Per signature, the count to create (planned surplus).
    pub creates: BTreeMap<Sig, i64>,
    /// Per signature, the count to destroy (observed surplus).
    pub destroys: BTreeMap<Sig, i64>,
}

impl CensusDelta {
    /// Whether the delta is empty both ways — the converged verdict (`dpseci.qnt`
    /// `censusConverged`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.creates.is_empty() && self.destroys.is_empty()
    }
}

/// The multiset delta between a planned and observed census (`dpseci.qnt` `censusDelta`): a
/// missing signature reads as zero, so a positive difference either way names exactly one
/// create or destroy count.
#[must_use]
pub fn census_delta(planned: &BTreeMap<Sig, i64>, observed: &BTreeMap<Sig, i64>) -> CensusDelta {
    let sig_count = |census: &BTreeMap<Sig, i64>, sig: &Sig| census.get(sig).copied().unwrap_or(0);
    let mut delta = CensusDelta::default();
    for sig in planned.keys().chain(observed.keys()) {
        let want = sig_count(planned, sig);
        let have = sig_count(observed, sig);
        if want > have {
            delta.creates.insert(sig.clone(), want - have);
        } else if have > want {
            delta.destroys.insert(sig.clone(), have - want);
        }
    }
    delta
}

/// Converged when the delta is empty both ways — the planned and observed multisets coincide
/// (`dpseci.qnt` `censusConverged`; dpseci-typestate design D9).
#[must_use]
pub fn census_converged(planned: &BTreeMap<Sig, i64>, observed: &BTreeMap<Sig, i64>) -> bool {
    census_delta(planned, observed).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::dpseci::{DpseciOpt, OptionMask};

    fn cfg(flag: DpseciOpt, queues: usize) -> DpseciCfg {
        DpseciCfg::new(OptionMask::empty().with_flag(flag), queues, vec![2; queues]).unwrap()
    }

    #[test]
    fn equal_blocks_need_no_repair() {
        let a = cfg(DpseciOpt::HasCg, 4);
        let b = cfg(DpseciOpt::HasCg, 4);
        assert_eq!(classify_cfg_mismatch(&a, &b), None);
    }

    #[test]
    fn options_mismatch_is_disruptive_replacement() {
        let desired = cfg(DpseciOpt::HasCg, 4);
        let incumbent = cfg(DpseciOpt::HasOpr, 4);
        assert_eq!(
            classify_cfg_mismatch(&desired, &incumbent),
            Some(Class::Disruptive)
        );
    }

    #[test]
    fn queue_shape_mismatch_is_disruptive_replacement() {
        let desired = cfg(DpseciOpt::HasCg, 4);
        let incumbent = cfg(DpseciOpt::HasCg, 8);
        assert_eq!(
            classify_cfg_mismatch(&desired, &incumbent),
            Some(Class::Disruptive)
        );
    }

    #[test]
    fn priority_mismatch_is_disruptive_replacement() {
        let desired = cfg(DpseciOpt::HasCg, 2);
        let incumbent = DpseciCfg::new(
            OptionMask::empty().with_flag(DpseciOpt::HasCg),
            2,
            vec![2, 3],
        )
        .unwrap();
        assert_eq!(
            classify_cfg_mismatch(&desired, &incumbent),
            Some(Class::Disruptive)
        );
    }

    // Structural twins of the `dpseci.qnt` `dpseci_lifecycle` census `run` tests (dpseci-typestate design D9):
    // two board-verified signatures plus a 4-queue sibling sharing production's options.

    fn production_cfg() -> DpseciCfg {
        let options = OptionMask::empty()
            .with_flag(DpseciOpt::HasCg)
            .with_flag(DpseciOpt::HasOpr)
            .with_flag(DpseciOpt::OprShared);
        DpseciCfg::new(options, 8, vec![2; 8]).unwrap()
    }

    fn no_cg_cfg() -> DpseciCfg {
        let options = OptionMask::empty()
            .with_flag(DpseciOpt::HasOpr)
            .with_flag(DpseciOpt::OprShared);
        DpseciCfg::new(options, 8, vec![2; 8]).unwrap()
    }

    fn sig_prod() -> Sig {
        sig_of(&production_cfg())
    }

    fn sig_no_cg() -> Sig {
        sig_of(&no_cg_cfg())
    }

    fn sig_four() -> Sig {
        Sig {
            num_queues: 4,
            options: production_cfg().options().clone(),
        }
    }

    #[test]
    fn census_absent_creates() {
        // Twin of `dpseci.qnt` `censusAbsentCreatesTest`.
        let planned = sig_census([sig_prod()]);
        let observed = sig_census([]);
        let d = census_delta(&planned, &observed);
        assert_eq!(d.creates.get(&sig_prod()), Some(&1));
        assert!(d.destroys.is_empty());
        assert!(!census_converged(&planned, &observed));
    }

    #[test]
    fn census_surplus_destroys() {
        // Twin of `dpseci.qnt` `censusSurplusDestroysTest`.
        let planned = sig_census([]);
        let observed = sig_census([sig_prod()]);
        let d = census_delta(&planned, &observed);
        assert_eq!(d.destroys.get(&sig_prod()), Some(&1));
        assert!(d.creates.is_empty());
        assert!(!census_converged(&planned, &observed));
    }

    #[test]
    fn census_signature_mismatch() {
        // Twin of `dpseci.qnt` `censusSignatureMismatchTest`: one create AND one destroy (dpseci-typestate design D5).
        let planned = sig_census([sig_prod()]);
        let observed = sig_census([sig_no_cg()]);
        let d = census_delta(&planned, &observed);
        assert_eq!(d.creates.get(&sig_prod()), Some(&1));
        assert_eq!(d.destroys.get(&sig_no_cg()), Some(&1));
        assert_eq!(d.creates.len(), 1);
        assert_eq!(d.destroys.len(), 1);
        assert!(!census_converged(&planned, &observed));
    }

    #[test]
    fn census_match_empty_delta() {
        // Twin of `dpseci.qnt` `censusMatchEmptyDeltaTest`.
        let planned = sig_census([sig_four(), sig_prod()]);
        let observed = sig_census([sig_prod(), sig_four()]);
        let d = census_delta(&planned, &observed);
        assert!(d.is_empty());
        assert!(census_converged(&planned, &observed));
    }

    #[test]
    fn census_two_signatures() {
        // Twin of `dpseci.qnt` `censusTwoSignaturesTest`.
        let planned = sig_census([sig_four(), sig_prod()]);
        let observed = sig_census([sig_prod()]);
        let d = census_delta(&planned, &observed);
        assert_eq!(d.creates.get(&sig_four()), Some(&1));
        assert!(d.destroys.is_empty());
        assert!(!census_converged(&planned, &observed));
    }

    #[test]
    fn classify_observable_mismatch_three_faces() {
        // Twin of `dpseci.qnt` `classifyObservableMismatchTest`: unobservable judges no drift.
        let prod = production_cfg();
        assert_eq!(
            classify_observable_mismatch(&prod, &ObservedSig::Unobservable),
            None
        );
        assert_eq!(
            classify_observable_mismatch(&prod, &ObservedSig::Observed(sig_prod())),
            None
        );
        assert_eq!(
            classify_observable_mismatch(&prod, &ObservedSig::Observed(sig_no_cg())),
            Some(Class::Disruptive)
        );
    }

    #[test]
    fn an_attributed_escape_projects_unobservable() {
        // An escape is unobservable; the same detail without it is observed (dpseci-hardening design D2).
        use crate::contract::{DpseciDetail, DpseciPortalReadout};
        use crate::families::dpseci::RawEscape;

        let detail = |options: OptionMask| DpseciDetail {
            num_tx_queues: Some(8),
            num_rx_queues: Some(8),
            tx_priorities: vec![2; 8],
            portal: DpseciPortalReadout::Observed {
                options,
                api_major: 5,
                api_minor: 4,
            },
        };
        let named = OptionMask::empty().with_flag(DpseciOpt::HasCg);

        assert_eq!(
            observed_sig_of(&detail(named.clone().with_escape(RawEscape::new(0x100)))),
            ObservedSig::Unobservable
        );
        assert_eq!(
            observed_sig_of(&detail(named.clone())),
            ObservedSig::Observed(Sig {
                num_queues: 8,
                options: named,
            })
        );
    }

    #[test]
    fn census_position_independent() {
        // Twin of `dpseci.qnt` `censusPositionIndependentTest`: position is not identity (ADR-0015 decision 5).
        assert_eq!(
            sig_census([sig_four(), sig_prod()]),
            sig_census([sig_prod(), sig_four()])
        );
        assert!(census_converged(
            &sig_census([sig_four(), sig_prod()]),
            &sig_census([sig_prod(), sig_four()])
        ));
    }
}
