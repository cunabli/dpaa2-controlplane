//! The observable-signature census for the dpsw create block — the pure convergence
//! judgment that compares a planned multiset of create signatures against the read-back
//! faces (dpsw-typestate design D6; `models/families/dpsw.qnt` `dpsw_lifecycle` census).
//!
//! Filed under `plan/` as `plan/dpsw.rs`, following the family-planner pattern
//! [`crate::plan::dpseci`] (ADR-0018). Convergence is a multiset census on the observable
//! cfg signature: the dpsw verb surface has exit-code truth gaps (DPSW-I7), so no verb
//! outcome is trusted without re-observation (dpsw-typestate design D6, on the
//! dpseci-typestate census precedent). The operators here are per-family literal twins of
//! the `dpsw.qnt` census section — isomorphism beats DRY, so they are not generalized across
//! families (dpsw-typestate design D6).

use std::collections::BTreeMap;

use crate::families::dpsw::{BroadcastCfg, ComponentType, DpswCfg, DpswOpt, FloodingCfg};

/// The observable cfg signature — the subset of the create block with a read-back
/// (`dpsw.qnt` `type ObservableSig`; dpsw-typestate design D6). It drops `mem_size` and
/// `max_meters_per_if`, the two fields `dpsw info` never prints, so two blocks differing
/// only in those share a signature (`docs/baseline/dpsw.md` "Command surface").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservableSig {
    /// The immutable port count (`dpsw.qnt` `numIfs`).
    pub num_ifs: u16,
    /// The MC option flags (`dpsw.qnt` `options`).
    pub options: std::collections::BTreeSet<DpswOpt>,
    /// `max_vlans` (`dpsw.qnt` `maxVlans`).
    pub max_vlans: u16,
    /// `max_fdbs` (`dpsw.qnt` `maxFdbs`).
    pub max_fdbs: u16,
    /// `max_fdb_entries` (`dpsw.qnt` `maxFdbEntries`).
    pub max_fdb_entries: u16,
    /// `fdb_aging_time` (`dpsw.qnt` `fdbAgingTime`).
    pub fdb_aging_time: u16,
    /// `max_fdb_mc_groups` (`dpsw.qnt` `maxFdbMcGroups`).
    pub max_fdb_mc_groups: u16,
    /// `component_type` (`dpsw.qnt` `componentType`).
    pub component_type: ComponentType,
    /// `flooding_cfg` (`dpsw.qnt` `floodingCfg`).
    pub flooding_cfg: FloodingCfg,
    /// `broadcast_cfg` (`dpsw.qnt` `broadcastCfg`).
    pub broadcast_cfg: BroadcastCfg,
}

/// The projection that drops the two unobservable fields (`dpsw.qnt` `observableSigOf`;
/// dpsw-typestate design D6): `mem_size` and `max_meters_per_if` have no read-back, so they
/// never enter the judgment (ADR-0018: the adapter never invents an unobserved value).
#[must_use]
pub fn sig_of(cfg: &DpswCfg) -> ObservableSig {
    ObservableSig {
        num_ifs: cfg.num_ifs(),
        options: cfg.options().clone(),
        max_vlans: cfg.max_vlans(),
        max_fdbs: cfg.max_fdbs(),
        max_fdb_entries: cfg.max_fdb_entries(),
        fdb_aging_time: cfg.fdb_aging_time(),
        max_fdb_mc_groups: cfg.max_fdb_mc_groups(),
        component_type: cfg.component_type(),
        flooding_cfg: cfg.flooding_cfg(),
        broadcast_cfg: cfg.broadcast_cfg(),
    }
}

/// One object's read-back face (`dpsw.qnt` `type ObservedFace`): the observed signature, or
/// typed unobservable — absence of evidence, never drift (dpsw-typestate design D6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservedFace {
    /// The portal answered with a nameable signature (`dpsw.qnt` `SigObserved`).
    SigObserved(ObservableSig),
    /// The signature cannot be judged this run (`dpsw.qnt` `SigUnobservable`).
    SigUnobservable,
}

/// Observable-subset drift for one desired object against its read-back (`dpsw.qnt`
/// `classifyObservableMismatch`): an unobservable face is no drift judged (`false`,
/// ADR-0018); an observed signature equal to the desired projection is no drift; any other
/// observed signature is drift (`true`). The dpsw model returns a bare `bool` — this family
/// has no in-place repair class, so the twin keeps `bool`.
#[must_use]
pub fn classify_observable_mismatch(desired: &DpswCfg, face: &ObservedFace) -> bool {
    match face {
        ObservedFace::SigUnobservable => false,
        ObservedFace::SigObserved(sig) => *sig != sig_of(desired),
    }
}

/// A census is the multiset of signatures as a count map (`dpsw.qnt` `sigCensus`): a missing
/// signature reads as zero. The planned census is built from compiled cfgs via [`sig_of`],
/// the observed from the [`ObservedFace::SigObserved`] faces via [`observed_sigs_of`].
#[must_use]
pub fn sig_census(sigs: impl IntoIterator<Item = ObservableSig>) -> BTreeMap<ObservableSig, i64> {
    let mut census = BTreeMap::new();
    for sig in sigs {
        *census.entry(sig).or_insert(0) += 1;
    }
    census
}

/// The multiset delta (`dpsw.qnt` `type CensusDelta`; dpsw-typestate design D6): per
/// signature, the planned surplus is a create count and the observed surplus a destroy
/// count. A signature mismatch surfaces as one create AND one destroy — the destroy+create
/// disruption, since every dpsw cfg field is immutable (`docs/baseline/dpsw.md` "Attribute
/// mutability").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CensusDelta {
    /// Per signature, the count to create (planned surplus).
    pub creates: BTreeMap<ObservableSig, i64>,
    /// Per signature, the count to destroy (observed surplus).
    pub destroys: BTreeMap<ObservableSig, i64>,
}

impl CensusDelta {
    /// Whether the delta is empty both ways — the converged verdict (`dpsw.qnt`
    /// `censusConverged`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.creates.is_empty() && self.destroys.is_empty()
    }
}

/// The multiset delta between a planned and observed census (`dpsw.qnt` `censusDelta`): a
/// missing signature reads as zero, so a positive difference either way names exactly one
/// create or destroy count.
#[must_use]
pub fn census_delta(
    planned: &BTreeMap<ObservableSig, i64>,
    observed: &BTreeMap<ObservableSig, i64>,
) -> CensusDelta {
    let count = |census: &BTreeMap<ObservableSig, i64>, sig: &ObservableSig| {
        census.get(sig).copied().unwrap_or(0)
    };
    let mut delta = CensusDelta::default();
    for sig in planned.keys().chain(observed.keys()) {
        let want = count(planned, sig);
        let have = count(observed, sig);
        if want > have {
            delta.creates.insert(sig.clone(), want - have);
        } else if have > want {
            delta.destroys.insert(sig.clone(), have - want);
        }
    }
    delta
}

/// Converged when the delta is empty both ways — the planned and observed multisets coincide
/// (`dpsw.qnt` `censusConverged`; dpsw-typestate design D6).
#[must_use]
pub fn census_converged(
    planned: &BTreeMap<ObservableSig, i64>,
    observed: &BTreeMap<ObservableSig, i64>,
) -> bool {
    census_delta(planned, observed).is_empty()
}

/// The signatures of the observed faces, dropping the unobservable ones (`dpsw.qnt`
/// `observedSigsOf`).
#[must_use]
pub fn observed_sigs_of(faces: &[ObservedFace]) -> Vec<ObservableSig> {
    faces
        .iter()
        .filter_map(|face| match face {
            ObservedFace::SigObserved(sig) => Some(sig.clone()),
            ObservedFace::SigUnobservable => None,
        })
        .collect()
}

/// The verdict of the census judge (`dpsw.qnt` `type FamilyJudgment`): either it judges
/// nothing, or it judges a delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FamilyJudgment {
    /// The census is poisoned — it judges no create and no destroy (`dpsw.qnt` `JudgesNothing`).
    JudgesNothing,
    /// The census is all-observed — it judges this delta (`dpsw.qnt` `Judges`).
    Judges(CensusDelta),
}

/// The DPSW-I7 census judge — the whole-census-poisoning law (`dpsw.qnt` `judgeCensus`;
/// dpsw-typestate design D6; ADR-0018): any unobservable member makes the whole face judge
/// nothing (absence of evidence is never drift, a spurious destroy+create is loss); only an
/// all-observed census yields a delta.
#[must_use]
pub fn judge_census(planned: &[ObservableSig], faces: &[ObservedFace]) -> FamilyJudgment {
    if faces
        .iter()
        .any(|face| matches!(face, ObservedFace::SigUnobservable))
    {
        FamilyJudgment::JudgesNothing
    } else {
        let planned_census = sig_census(planned.iter().cloned());
        let observed_census = sig_census(observed_sigs_of(faces));
        FamilyJudgment::Judges(census_delta(&planned_census, &observed_census))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::families::dpsw::{
        DpswCreate, FdbAgingTime, MaxFdbEntries, MaxFdbMcGroups, MaxFdbs, MaxVlans, NumIfs,
    };

    /// The bindable 2-port shape (`dpsw.qnt` `BINDABLE_CFG`), with `mem_size`/
    /// `max_meters_per_if` as parameters so the unobservable-drop can be witnessed.
    fn cfg(num_ifs: u16, max_fdbs: u16, mem_size: u32, max_meters_per_if: u32) -> DpswCfg {
        DpswCfg::new(DpswCreate {
            num_ifs: NumIfs::new(num_ifs).unwrap(),
            options: BTreeSet::new(),
            max_vlans: MaxVlans::new(16).unwrap(),
            max_fdbs: MaxFdbs::new(max_fdbs).unwrap(),
            max_fdb_entries: MaxFdbEntries::new(1024).unwrap(),
            fdb_aging_time: FdbAgingTime::new(300).unwrap(),
            max_fdb_mc_groups: MaxFdbMcGroups::new(32).unwrap(),
            mem_size,
            component_type: ComponentType::CVlan,
            flooding_cfg: FloodingCfg::PerFdb,
            broadcast_cfg: BroadcastCfg::PerFdb,
            max_meters_per_if,
        })
        .unwrap()
    }

    fn bindable() -> DpswCfg {
        cfg(2, 2, 0, 0)
    }

    fn bindable4() -> DpswCfg {
        cfg(4, 4, 0, 0)
    }

    fn sig_bindable() -> ObservableSig {
        sig_of(&bindable())
    }

    fn sig_four() -> ObservableSig {
        sig_of(&bindable4())
    }

    #[test]
    fn sig_of_drops_the_unobservable_fields() {
        // dpsw.qnt `observableSigOf`: mem_size and max_meters_per_if never enter the
        // signature, so two blocks differing only in those share a signature.
        let a = cfg(2, 2, 0, 0);
        let b = cfg(2, 2, 4096, 7);
        assert_ne!(a, b);
        assert_eq!(sig_of(&a), sig_of(&b));
    }

    #[test]
    fn census_absent_creates() {
        // Twin of `dpsw.qnt` `censusAbsentCreatesTest`: a planned signature with no observed
        // peer yields one create of it, no destroy, and a non-converged verdict.
        let planned = sig_census([sig_bindable()]);
        let observed = sig_census([]);
        let d = census_delta(&planned, &observed);
        assert_eq!(d.creates.get(&sig_bindable()), Some(&1));
        assert!(d.destroys.is_empty());
        assert!(!census_converged(&planned, &observed));
    }

    #[test]
    fn census_signature_mismatch() {
        // Twin of `dpsw.qnt` `censusSignatureMismatchTest`: 2-port planned, 4-port observed
        // is one create AND one destroy — the destroy+create disruption.
        let planned = sig_census([sig_bindable()]);
        let observed = sig_census([sig_four()]);
        let d = census_delta(&planned, &observed);
        assert_eq!(d.creates.get(&sig_bindable()), Some(&1));
        assert_eq!(d.destroys.get(&sig_four()), Some(&1));
        assert_eq!(d.creates.len(), 1);
        assert_eq!(d.destroys.len(), 1);
        assert!(!census_converged(&planned, &observed));
    }

    #[test]
    fn census_match_empty_delta_position_independent() {
        // Twin of `dpsw.qnt` `censusMatchEmptyDeltaTest`: matching multisets (reordered)
        // converge — position is not a hardware identity (ADR-0015 decision 5).
        let planned = sig_census([sig_four(), sig_bindable()]);
        let observed = sig_census([sig_bindable(), sig_four()]);
        let d = census_delta(&planned, &observed);
        assert!(d.is_empty());
        assert!(census_converged(&planned, &observed));
        assert_eq!(planned, observed);
    }

    #[test]
    fn classify_observable_mismatch_three_faces() {
        // Twin of `dpsw.qnt` `classifyObservableMismatchTest`: unobservable is no drift, the
        // matching signature is no drift, any other observed signature is drift.
        let desired = bindable();
        assert!(!classify_observable_mismatch(
            &desired,
            &ObservedFace::SigUnobservable
        ));
        assert!(!classify_observable_mismatch(
            &desired,
            &ObservedFace::SigObserved(sig_bindable())
        ));
        assert!(classify_observable_mismatch(
            &desired,
            &ObservedFace::SigObserved(sig_four())
        ));
    }

    #[test]
    fn unobservable_member_poisons_the_census() {
        // Twin of `dpsw.qnt` `censusUnobservableMemberJudgesNothingTest` (DPSW-I7; ADR-0018):
        // a poisoned census (one observed, one unobservable) judges nothing — it does NOT
        // create the missing sig_four — while the unpoisoned control over the same plan
        // judges one create of sig_four, no destroy.
        let planned = [sig_four(), sig_bindable()];
        let poisoned = [
            ObservedFace::SigObserved(sig_bindable()),
            ObservedFace::SigUnobservable,
        ];
        assert_eq!(
            judge_census(&planned, &poisoned),
            FamilyJudgment::JudgesNothing
        );

        let control = [ObservedFace::SigObserved(sig_bindable())];
        match judge_census(&planned, &control) {
            FamilyJudgment::JudgesNothing => panic!("an all-observed census must judge a delta"),
            FamilyJudgment::Judges(d) => {
                assert_eq!(d.creates.get(&sig_four()), Some(&1));
                assert!(d.destroys.is_empty());
            }
        }
    }

    #[test]
    fn judge_census_clean_match_is_converged() {
        // A clean all-observed match judges a converged (empty) delta.
        let planned = [sig_bindable(), sig_four()];
        let faces = [
            ObservedFace::SigObserved(sig_four()),
            ObservedFace::SigObserved(sig_bindable()),
        ];
        match judge_census(&planned, &faces) {
            FamilyJudgment::JudgesNothing => panic!("an all-observed census must judge a delta"),
            FamilyJudgment::Judges(d) => assert!(d.is_empty()),
        }
    }

    #[test]
    fn observed_sigs_of_drops_unobservable() {
        // dpsw.qnt `observedSigsOf`: only the SigObserved payloads survive.
        let faces = [
            ObservedFace::SigObserved(sig_bindable()),
            ObservedFace::SigUnobservable,
            ObservedFace::SigObserved(sig_four()),
        ];
        assert_eq!(observed_sigs_of(&faces), vec![sig_bindable(), sig_four()]);
    }
}
