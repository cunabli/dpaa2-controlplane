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

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::contract::{DpswDetail, DpswIfFace};
use crate::core::error::Error;
use crate::core::family::Family;
use crate::core::model::{DpmacId, DpniId, ObjectRef};
use crate::core::types::ConstructName;
use crate::families::dpsw::{
    BroadcastCfg, ComponentType, DpswCfg, DpswCreate, DpswOpt, FdbAgingTime, FloodingCfg,
    MaxFdbEntries, MaxFdbMcGroups, MaxFdbs, MaxVlans, NumIfs,
};
use crate::intent::compiled::Attributes;
use crate::plan::connect::dpsw_port_bound;
use crate::plan::{Class, SeveredProof, Transition};

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

// ---- the switched-domain plan surface (dpsw-typestate task 3.2) ----

/// Create-block fields the intent does not carry; the dpsw planner fills them with the
/// family create defaults (`dpsw.qnt` `BINDABLE_CFG`). They enter the census signature, so
/// a board switch created outside these defaults reads as drift (dpsw-typestate design D6).
const DEFAULT_MAX_VLANS: u16 = 16;
const DEFAULT_FDB_AGING_TIME: u16 = 300;
const DEFAULT_MAX_FDB_MC_GROUPS: u16 = 32;

/// Builds the immutable [`DpswCfg`] the planner reconciles from a compiled
/// [`Attributes::Dpsw`], routing it through the gated constructor [`DpswCfg::new`] so the
/// DPSW-I1 bindability law gates construction here too (dpsw-typestate design D2). The six
/// create fields the intent does not carry take the family defaults; the bindability flags
/// in the attributes are derivation-true (`compiled::Fabric::dpsw`), so a constructor
/// refusal is an internal error path, not a new operator-facing refusal vocabulary.
///
/// # Errors
/// [`Error::Config`] when an attribute is out of a refined field's range, or when the
/// derivation flags somehow fail the DPSW-I1 classification — a malformed attribute only.
pub fn cfg_from_attributes(attrs: &Attributes) -> Result<DpswCfg, Error> {
    let Attributes::Dpsw {
        num_ifs,
        max_fdbs,
        max_fdb_entries,
        per_fdb_flooding,
        per_fdb_broadcast,
        ctrl_if,
    } = attrs
    else {
        return Err(Error::Config(
            "the dpsw planner was given non-dpsw attributes".to_string(),
        ));
    };
    let narrow = |v: u32, field: &str| -> Result<u16, Error> {
        u16::try_from(v)
            .map_err(|_| Error::Config(format!("dpsw {field} {v} exceeds the 16-bit create field")))
    };
    let mut options = BTreeSet::new();
    if !*ctrl_if {
        options.insert(DpswOpt::CtrlIfDis);
    }
    let create = DpswCreate {
        num_ifs: NumIfs::new(narrow(*num_ifs, "num_ifs")?)?,
        options,
        max_vlans: MaxVlans::new(DEFAULT_MAX_VLANS)?,
        max_fdbs: MaxFdbs::new(narrow(*max_fdbs, "max_fdbs")?)?,
        max_fdb_entries: MaxFdbEntries::new(narrow(*max_fdb_entries, "max_fdb_entries")?)?,
        fdb_aging_time: FdbAgingTime::new(DEFAULT_FDB_AGING_TIME)?,
        max_fdb_mc_groups: MaxFdbMcGroups::new(DEFAULT_MAX_FDB_MC_GROUPS)?,
        mem_size: 0,
        component_type: ComponentType::CVlan,
        flooding_cfg: if *per_fdb_flooding {
            FloodingCfg::PerFdb
        } else {
            FloodingCfg::PerVlan
        },
        broadcast_cfg: if *per_fdb_broadcast {
            BroadcastCfg::PerFdb
        } else {
            BroadcastCfg::PerObject
        },
        max_meters_per_if: 0,
    };
    DpswCfg::new(create).map_err(Error::from)
}

/// One MC-granularity step in a switched-domain create plan.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SwitchStep {
    /// Create the dpsw in its own container, carrying the immutable kernel-bindable cfg.
    /// The gated [`DpswCfg`] is the construction law (DPSW-I1; dpsw-typestate design D2).
    CreateSwitch {
        /// The MC label — the fabric's construct name (ADR-0015 decisions 9 + 13).
        label: ConstructName,
        /// The validated bindable create block (`Attributes::Dpsw` through the gate).
        cfg: DpswCfg,
    },
    /// Connect one switch port `dpsw.N.ifx` to its endpoint — a member dpni or an uplink
    /// dpmac (the two ADR-0022 dpsw edge rows; bounded by `num_ifs`, DPSW-I10).
    ConnectPort {
        /// The switch interface index, within `num_ifs`.
        ifx: u32,
        /// The endpoint the port faces.
        endpoint: ObjectRef,
    },
    /// `dprc assign dprc.1 --object=dpsw.N --plugged=1`: plug the created dpsw itself — the
    /// final recipe step, on the dpsw object, not on any container (`docs/baseline/dpsw.md`
    /// "Lifecycle ordering"). Emitted last, after every port connect.
    PlugSwitch,
}

/// A switched-domain intent wiring a member port beyond the created `num_ifs` — refused
/// typed in planning before any step is emitted (DPSW-I10; `core/connect.qnt`
/// `dpswPortBound`). A typed value, never a silent drop.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PortOverflow {
    /// The created port count the domain is bounded by.
    pub num_ifs: u32,
    /// The member interface indices at or beyond `num_ifs`.
    pub offending: BTreeSet<u32>,
}

impl fmt::Display for PortOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "switch ports {:?} are beyond the created num_ifs {}: raise num_ifs or drop the members (DPSW-I10)",
            self.offending, self.num_ifs
        )
    }
}

/// The outcome of planning a switched-domain create (dpsw-typestate design D6).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SwitchPlan {
    /// The ordered create plan: the dpsw create, then the member/uplink connects, then the
    /// plug on the dpsw object (`--plugged=1`).
    Create(Vec<SwitchStep>),
    /// Refused: a member port is beyond `num_ifs` — no step is emitted (DPSW-I10).
    PortOverflow(PortOverflow),
}

/// Plans the switched-domain create in the baseline working-recipe order
/// (`docs/baseline/dpsw.md` "Lifecycle ordering"): create the dpsw, connect the
/// member/uplink ports, then `assign --plugged=1` on the dpsw object last. The dpsw and its
/// companions are CO-RESIDENT — "own container" is the container the dpsw is created in
/// (root, `dprc.1`, for the kernel regime), not a child dprc. The 1 dpmcp + 1 dpbp the probe
/// draws (DPSW-I9) are therefore NOT scaffolded here: the intent compiler derives that draw
/// into the root pool requirement (`crates/dpaa2-tools/tests/vdpsw5_intents.rs` pins it) and
/// the engine's `converge_pools` Grow pass runs before any create, so companion existence
/// before the dpsw is owned by that pool derivation plus the pass ordering, not by this plan
/// (dpsw-typestate task 6.4 / bead dpaa2-controlplane-dbf.22). The [`DpswCfg`] is built from
/// `attrs` through the gated constructor [`cfg_from_attributes`], so the DPSW-I1 construction
/// law stands.
///
/// Port overflow is refused typed before any step is emitted: a member `ifx >= num_ifs`
/// fails [`dpsw_port_bound`] and yields [`SwitchPlan::PortOverflow`] (DPSW-I10).
///
/// # Errors
/// [`Error::Config`] when `attrs` is malformed — the internal error path of
/// [`cfg_from_attributes`], never an operator refusal.
pub fn plan_switched_domain(
    label: ConstructName,
    attrs: &Attributes,
    ports: &[(u32, ObjectRef)],
) -> Result<SwitchPlan, Error> {
    let cfg = cfg_from_attributes(attrs)?;
    let num_ifs = u32::from(cfg.num_ifs());

    // Overflow refusal before any step (DPSW-I10): every member ifx must be within num_ifs.
    let offending: BTreeSet<u32> = ports
        .iter()
        .map(|&(ifx, _)| ifx)
        .filter(|&ifx| !dpsw_port_bound(Family::Dpsw, ifx, num_ifs))
        .collect();
    if !offending.is_empty() {
        return Ok(SwitchPlan::PortOverflow(PortOverflow {
            num_ifs,
            offending,
        }));
    }

    let mut steps = vec![SwitchStep::CreateSwitch { label, cfg }];
    for &(ifx, endpoint) in ports {
        steps.push(SwitchStep::ConnectPort { ifx, endpoint });
    }
    steps.push(SwitchStep::PlugSwitch);
    Ok(SwitchPlan::Create(steps))
}

/// What a cfg-drift repair plans for a matched dpsw (dpsw-typestate design D6; ADR-0015
/// matcher).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DriftRepair {
    /// A create-time field drifted: destroy the observed dpsw and recreate it — never an
    /// in-place update, since every dpsw cfg field is immutable (`docs/baseline/dpsw.md`
    /// "Attribute mutability"). Always [`Class::Disruptive`].
    Recreate {
        /// The observed dpsw to destroy before the recreate.
        destroy: ObjectRef,
    },
    /// The observable signature matches, or the face is unobservable: nothing to repair
    /// (absence of evidence is never drift, DPSW-I7).
    Converged,
}

impl DriftRepair {
    /// The repair's disruption class (ADR-0015 decision 12): a destroy-and-recreate is
    /// [`Class::Disruptive`]; a converged repair is [`Class::Hitless`].
    #[must_use]
    pub fn class(&self) -> Class {
        match self {
            Self::Recreate { .. } => Class::Disruptive,
            Self::Converged => Class::Hitless,
        }
    }
}

/// Plans the repair for a dpsw the ADR-0015 matcher bound to an intent by anchor
/// (provenance identity) against its read-back `face` (dpsw-typestate design D6). Reuses
/// 3.1's [`classify_observable_mismatch`] as the drift detector: any create-time-field
/// difference plans a destroy-and-recreate classed [`Class::Disruptive`], never an in-place
/// update. The recreate follows the ADR-0008 §8 teardown order already stated in
/// reconcile.rs — disconnect, destroy (neither dpsw edge kind has a kernel-face unbind,
/// dpsw-typestate design D5), then re-run [`plan_switched_domain`]. An unobservable or
/// matching face is [`DriftRepair::Converged`].
#[must_use]
pub fn plan_cfg_drift(desired: &DpswCfg, observed: ObjectRef, face: &ObservedFace) -> DriftRepair {
    if classify_observable_mismatch(desired, face) {
        DriftRepair::Recreate { destroy: observed }
    } else {
        DriftRepair::Converged
    }
}

/// One step of the flat-to-switched interposition plan (dpsw-typestate design D10; the
/// baseline sharing topology).
#[derive(PartialEq, Eq, Debug)]
pub enum InterposeStep {
    /// Sever the flat dpni↔dpmac edge through the dpmac edge law — the [`Transition`] a
    /// [`Transition::sever`] yields, minting the [`SeveredProof`] a kernel-face unbind would
    /// demand (ADR-0008 §8). Going through sever, not a bare disconnect, pins the dpmac
    /// handback before the port is re-homed.
    Sever(Transition),
    /// Connect the uplink `dpsw.N.ifx` ↔ dpmac (the ADR-0022 switch-uplink edge row).
    ConnectUplink {
        /// The switch interface index the dpmac faces.
        ifx: u32,
        /// The uplink dpmac — the port handed to the switch regime.
        dpmac: DpmacId,
    },
    /// Connect the member dpni ↔ `dpsw.N.ifx` (the ADR-0022 switch-port membership row).
    ConnectMember {
        /// The switch interface index the dpni faces.
        ifx: u32,
        /// The member dpni re-homed from the flat topology into the switch.
        dpni: DpniId,
    },
}

/// The flat-to-switched interposition plan: the dpni↔dpmac sever FIRST, then the uplink
/// connect, then the member connect (dpsw-typestate design D10; `docs/baseline/dpsw.md`
/// "Lifecycle ordering" sharing topology). No step strands the port driverless — the
/// dpmac-typestate edge law holds across the rewire (ADR-0008 §8).
///
/// The sever path mints a [`SeveredProof`]; because that proof is the only key a
/// [`Transition::unbind`] accepts and only [`Transition::sever`] mints it, a plan that
/// unbinds before severing does not construct — the guarantee lives on [`SeveredProof`]
/// already (transition.rs), claimed here, not re-invented.
#[derive(PartialEq, Eq, Debug)]
pub struct Interposition {
    /// The ordered steps: sever → connect uplink → connect member.
    pub steps: Vec<InterposeStep>,
    /// The witness the sever minted — the dpmac edge law's proof, available to a later
    /// kernel-face unbind the re-home may demand.
    pub proof: SeveredProof,
}

/// Plans the interposition of a dpsw between a flat dpni↔dpmac pair (dpni↔dpmac becomes
/// dpni↔dpsw↔dpmac; dpsw-typestate design D10). The dpni–dpmac sever is planned FIRST
/// through [`Transition::sever`] (minting the [`SeveredProof`]), then the uplink dpsw↔dpmac
/// connect, then the member dpni↔dpsw connect — the baseline recipe's order
/// (`docs/baseline/dpsw.md` "Lifecycle ordering"), so no step strands the port driverless.
#[must_use]
pub fn plan_interposition(
    dpni: DpniId,
    dpmac: DpmacId,
    uplink_ifx: u32,
    member_ifx: u32,
) -> Interposition {
    let (disconnect, proof) = Transition::sever(dpni);
    Interposition {
        steps: vec![
            InterposeStep::Sever(disconnect),
            InterposeStep::ConnectUplink {
                ifx: uplink_ifx,
                dpmac,
            },
            InterposeStep::ConnectMember {
                ifx: member_ifx,
                dpni,
            },
        ],
        proof,
    }
}

// ---- the read-back projection into the census face (dpsw-typestate task 3.3; design D6) ----

/// Projects one dpsw's read-back [`DpswDetail`] to its census face (dpsw-typestate design D6;
/// [`ObservedFace`]). Any attributed raw escape makes the face [`ObservedFace::SigUnobservable`]:
/// an escape is nameable evidence of an *unnameable* option bit, so folding it into the signature
/// would make the read-back differ from every planned cfg and drive a destroy+create loop — the
/// dpseci-hardening design D2 rationale, kept verbatim. An absent options line (`None`) is itself
/// [`ObservedFace::SigUnobservable`]: a blanked option list is a typed decode escape, never the
/// empty set. Any absent required sig line is likewise [`ObservedFace::SigUnobservable`] (absence
/// of evidence is never drift, DPSW-I7). Only an all-present, escape-free detail yields
/// [`ObservedFace::SigObserved`] from the named options and the decoded fields.
#[must_use]
pub fn observed_face_of(detail: &DpswDetail) -> ObservedFace {
    let Some(options) = &detail.options else {
        return ObservedFace::SigUnobservable;
    };
    if !options.escapes().is_empty() {
        return ObservedFace::SigUnobservable;
    }
    let (
        Some(num_ifs),
        Some(max_vlans),
        Some(max_fdbs),
        Some(max_fdb_entries),
        Some(fdb_aging_time),
        Some(max_fdb_mc_groups),
        Some(component_type),
        Some(flooding_cfg),
        Some(broadcast_cfg),
    ) = (
        detail.num_ifs,
        detail.max_vlans,
        detail.max_fdbs,
        detail.max_fdb_entries,
        detail.fdb_aging_time,
        detail.max_fdb_mc_groups,
        detail.component_type,
        detail.flooding_cfg,
        detail.broadcast_cfg,
    )
    else {
        return ObservedFace::SigUnobservable;
    };
    ObservedFace::SigObserved(ObservableSig {
        num_ifs: num_ifs.get(),
        options: options.named().clone(),
        max_vlans: max_vlans.get(),
        max_fdbs: max_fdbs.get(),
        max_fdb_entries: max_fdb_entries.get(),
        fdb_aging_time: fdb_aging_time.get(),
        max_fdb_mc_groups: max_fdb_mc_groups.get(),
        component_type,
        flooding_cfg,
        broadcast_cfg,
    })
}

/// Whether a switch port reads back unconnected (dpsw-typestate design D6; reconciler spec "A
/// blanked connection line is not freedom"). An [`DpswIfFace::Unobservable`] port judges nothing
/// (`None`) — no plan treats a blanked `connection:` line as unconnected, the `has_endpoint`
/// misread (DPSW-I7); a connected port is `Some(false)`, an explicitly disconnected port
/// `Some(true)`.
#[must_use]
pub fn port_unconnected(face: &DpswIfFace) -> Option<bool> {
    match face {
        DpswIfFace::Unobservable => None,
        DpswIfFace::Connected(_) => Some(false),
        DpswIfFace::Disconnected => Some(true),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::families::dpsw::{
        DpswCreate, FdbAgingTime, MaxFdbEntries, MaxFdbMcGroups, MaxFdbs, MaxVlans, NumIfs,
        ObservedOpts, RawEscape,
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

    // ---- the switched-domain plan surface (dpsw-typestate task 3.2) ----

    /// The DPSW-I1 derivation-true attributes (`compiled::Fabric::dpsw`): ctrl-if on,
    /// `PER_FDB` flooding+broadcast, `max_fdbs == num_ifs`, the default mac table.
    fn dpsw_attrs(num_ifs: u32) -> Attributes {
        Attributes::Dpsw {
            num_ifs,
            max_fdbs: num_ifs,
            max_fdb_entries: 1024,
            per_fdb_flooding: true,
            per_fdb_broadcast: true,
            ctrl_if: true,
        }
    }

    fn oref(fam: Family, ordinal: u32) -> ObjectRef {
        ObjectRef::new(fam, ordinal)
    }

    #[test]
    fn cfg_from_attributes_maps_the_derivation_shape() {
        // The derivation-true attributes build a bindable cfg with the family defaults for
        // the fields the intent does not carry (dpsw-typestate design D2).
        let cfg = cfg_from_attributes(&dpsw_attrs(4)).expect("derivation-true attrs build");
        assert_eq!(cfg.num_ifs(), 4);
        assert_eq!(cfg.max_fdbs(), 4);
        assert_eq!(cfg.flooding_cfg(), FloodingCfg::PerFdb);
        assert_eq!(cfg.broadcast_cfg(), BroadcastCfg::PerFdb);
        assert_eq!(cfg.max_vlans(), 16);
        assert_eq!(cfg.fdb_aging_time(), 300);
        assert_eq!(cfg.max_fdb_mc_groups(), 32);

        // A malformed (unbindable) attribute is a typed internal error, not a refusal vocab.
        let bad = Attributes::Dpsw {
            num_ifs: 2,
            max_fdbs: 2,
            max_fdb_entries: 1024,
            per_fdb_flooding: true,
            per_fdb_broadcast: true,
            ctrl_if: false,
        };
        assert!(matches!(cfg_from_attributes(&bad), Err(Error::Config(_))));
    }

    #[test]
    fn companion_starvation_is_planned_away() {
        // Co-residency law (DPSW-I9; see `plan_switched_domain` rustdoc): no scaffolding and no
        // companion creates — only CreateSwitch → connects → PlugSwitch last.
        let SwitchPlan::Create(steps) = plan_switched_domain(
            "fabric".into(),
            &dpsw_attrs(2),
            &[(0, oref(Family::Dpmac, 3)), (1, oref(Family::Dpni, 7))],
        )
        .expect("derivation-true attrs build") else {
            panic!("an in-bound domain plans a create");
        };
        assert!(
            matches!(steps.first(), Some(SwitchStep::CreateSwitch { .. })),
            "the dpsw create leads the plan, no companion scaffolding ahead of it"
        );
        assert_eq!(steps.last(), Some(&SwitchStep::PlugSwitch));
        let connects = steps
            .iter()
            .filter(|s| matches!(s, SwitchStep::ConnectPort { .. }))
            .count();
        assert_eq!(connects, 2);
        assert_eq!(steps.len(), 4, "create + two connects + plug, nothing else");
    }

    #[test]
    fn port_overflow_is_refused_typed_before_dispatch() {
        // A member ifx at/beyond num_ifs is refused typed, emitting no step (DPSW-I10).
        let plan = plan_switched_domain(
            "fabric".into(),
            &dpsw_attrs(2),
            &[(0, oref(Family::Dpmac, 3)), (2, oref(Family::Dpni, 7))],
        )
        .expect("derivation-true attrs build");
        match plan {
            SwitchPlan::PortOverflow(o) => {
                assert_eq!(o.num_ifs, 2);
                assert_eq!(o.offending, BTreeSet::from([2]));
            }
            SwitchPlan::Create(_) => panic!("an overflowing port must refuse, not plan"),
        }
    }

    #[test]
    fn cfg_drift_plans_a_disruption_classed_recreate() {
        // A matched dpsw whose observed signature differs in a create-time field plans a
        // destroy-and-recreate classed Disruptive; an identical signature plans nothing
        // (dpsw-typestate design D6; ADR-0015).
        let desired = bindable(); // 2-port
        let observed = oref(Family::Dpsw, 4);

        let drift = plan_cfg_drift(&desired, observed, &ObservedFace::SigObserved(sig_four()));
        assert_eq!(drift, DriftRepair::Recreate { destroy: observed });
        assert_eq!(drift.class(), Class::Disruptive);

        // An identical signature is converged — no recreate.
        let same = plan_cfg_drift(
            &desired,
            observed,
            &ObservedFace::SigObserved(sig_bindable()),
        );
        assert_eq!(same, DriftRepair::Converged);
        assert_eq!(same.class(), Class::Hitless);

        // An unobservable face is never drift (DPSW-I7).
        let unobs = plan_cfg_drift(&desired, observed, &ObservedFace::SigUnobservable);
        assert_eq!(unobs, DriftRepair::Converged);
    }

    #[test]
    fn interposition_sequences_sever_then_uplink_then_member() {
        // The flat-to-switched rewire plans sever → connect uplink → connect member
        // (dpsw-typestate design D10; ADR-0008 §8).
        let dpni = DpniId::new(7);
        let dpmac = DpmacId::new(3);
        let plan = plan_interposition(dpni, dpmac, 0, 1);
        assert!(matches!(
            plan.steps.as_slice(),
            [
                InterposeStep::Sever(Transition::Disconnect { dpni: d }),
                InterposeStep::ConnectUplink { ifx: 0, dpmac: m },
                InterposeStep::ConnectMember { ifx: 1, dpni: n },
            ] if *d == dpni && *m == dpmac && *n == dpni
        ));
        // The sever minted the dpmac edge law's proof, bound to the severed dpni.
        assert_eq!(plan.proof.dpni(), dpni);
    }

    // ---- read-back-only convergence and honest-unknown faces (dpsw-typestate task 3.3; design D6) ----

    /// A fully-observed detail synthesized from a cfg — every sig line present, no escape, no
    /// per-interface faces (faces are not part of the signature).
    fn observed_detail(cfg: &DpswCfg) -> DpswDetail {
        DpswDetail {
            num_ifs: NumIfs::new(cfg.num_ifs()).ok(),
            max_vlans: MaxVlans::new(cfg.max_vlans()).ok(),
            max_fdbs: MaxFdbs::new(cfg.max_fdbs()).ok(),
            max_fdb_entries: MaxFdbEntries::new(cfg.max_fdb_entries()).ok(),
            fdb_aging_time: FdbAgingTime::new(cfg.fdb_aging_time()).ok(),
            max_fdb_mc_groups: MaxFdbMcGroups::new(cfg.max_fdb_mc_groups()).ok(),
            component_type: Some(cfg.component_type()),
            flooding_cfg: Some(cfg.flooding_cfg()),
            broadcast_cfg: Some(cfg.broadcast_cfg()),
            mem_size: None,
            options: Some(
                cfg.options()
                    .iter()
                    .fold(ObservedOpts::empty(), |opts, &flag| opts.with_named(flag)),
            ),
            ifaces: BTreeMap::new(),
        }
    }

    #[test]
    fn silent_destroy_failure_does_not_converge() {
        // reconciler spec "A silent destroy failure does not converge" (DPSW-I7): the plan omits
        // the victim, but its read-back face still carries the survivor's observed sig, so the
        // census judges a destroy residue, never success.
        let survivor = observed_face_of(&observed_detail(&bindable()));
        let planned: Vec<ObservableSig> = Vec::new();
        match judge_census(&planned, &[survivor]) {
            FamilyJudgment::JudgesNothing => panic!("an all-observed census must judge a delta"),
            FamilyJudgment::Judges(d) => {
                assert!(!d.is_empty(), "a surviving object is not converged");
                assert_eq!(d.destroys.get(&sig_bindable()), Some(&1));
            }
        }
    }

    #[test]
    fn blanked_connection_line_is_not_freedom() {
        // reconciler spec "A blanked connection line is not freedom": a blanked connection line
        // is Unobservable, so port_unconnected judges nothing, never Some(true) (the has_endpoint
        // misread, DPSW-I7).
        assert_eq!(port_unconnected(&DpswIfFace::Unobservable), None);
        assert_ne!(port_unconnected(&DpswIfFace::Unobservable), Some(true));
        assert_eq!(
            port_unconnected(&DpswIfFace::Connected(oref(Family::Dpni, 7))),
            Some(false)
        );
        assert_eq!(port_unconnected(&DpswIfFace::Disconnected), Some(true));
    }

    #[test]
    fn escape_poisons_the_face_display_only() {
        // dpseci-hardening design D2: an attributed raw escape poisons the census face, yet the
        // detail still names the escape for display.
        let mut detail = observed_detail(&bindable());
        let poisoned = detail
            .options
            .take()
            .unwrap()
            .with_escape(RawEscape::new(0x100));
        assert!(poisoned.escapes().contains(&RawEscape::new(0x100)));
        detail.options = Some(poisoned);
        assert_eq!(observed_face_of(&detail), ObservedFace::SigUnobservable);
    }

    #[test]
    fn missing_sig_line_is_unobservable() {
        // DPSW-I7: any absent required sig line leaves the face unobservable, never defaulted.
        let mut detail = observed_detail(&bindable());
        detail.num_ifs = None;
        assert_eq!(observed_face_of(&detail), ObservedFace::SigUnobservable);
    }

    #[test]
    fn absent_options_line_is_unobservable() {
        // mc-backend spec: a blanked option list is a typed decode escape — an absent options
        // line (None) is never the empty set, so the face is unobservable (DPSW-I7).
        let mut detail = observed_detail(&bindable());
        detail.options = None;
        assert_eq!(observed_face_of(&detail), ObservedFace::SigUnobservable);
    }
}
