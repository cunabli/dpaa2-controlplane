//! The transparent recording layer: a decorator over the southbound control traits that
//! journals each successful mutating actuation as it is observed (plan-rollback design D3/D4).
//! It forwards every verb unchanged — it never alters, reorders, or refuses one, and
//! never inspects an argument beyond what the record needs.
//!
//! The wrapper lives with the trait seams it wraps (plan-rollback design D4) and is a per-trait
//! decorator, not a generic middleware/layer trait: a uniform call shape would need a verb-enum
//! funnel that erases the typestate at the exact seam this project types, so that promotion is
//! refused until a second cross-cutting layer earns it (plan-rollback design D3).
//!
//! **Ordering law (plan-rollback design D3):** recording sits *inside* retry — the wrapped
//! backend verbs carry their own retries (`dpaa2-mc`), so a row is appended only once a verb
//! finally returns `Ok`, and a verb that errors appends nothing. The journal therefore holds
//! the actuation that succeeded, never a failed attempt.
//!
//! The record carries the **observed** result (the MC-assigned id/ref, the home container),
//! never an intended input, so a later rollback addresses the object that exists. The
//! verb→record mapping is the authoritative one in [`crate::plan::rollback`]; this layer only
//! applies it. ADR-0008 is untouched: the journal is in-memory and dies with the run —
//! nothing persists and nothing is reserved.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::contract::{DpseciDetail, DpswDetail, KernelControl, McControl};
use crate::core::error::Error;
use crate::core::family::Family;
use crate::core::inventory::Inventory;
use crate::core::model::{DpmacId, DpniId, DprcId, MacAddr, ObjectRef, ObservedTopology};
use crate::core::types::ConstructName;
use crate::families::dpio::{DpioCfg, Priorities};
use crate::families::dpmac::{CarrierSource, DpmacObservation};
use crate::families::dpni::{DpniCfg, LinkState};
use crate::families::dprc;
use crate::families::dpseci::DpseciCfg;
use crate::families::dpsw::DpswCfg;
use crate::families::pool_lifecycle::{ObservedPoolObject, RawDriver};
use crate::plan::dprc::ObservedContainer;
use crate::plan::rollback::{ActuationRecord, RunJournal};

/// A single-threaded shared handle to the run journal. Dispatch is serialized, so interior
/// mutability under `Rc<RefCell<_>>` suffices — no `Mutex` (plan-rollback design D3).
pub type JournalHandle = Rc<RefCell<RunJournal>>;

/// Wraps a southbound backend `B`, forwarding every verb to it and appending each successful
/// mutating actuation to a shared [`RunJournal`] (plan-rollback design D3/D4). One generic
/// struct serves both seams: it is [`McControl`] when `B` is, and [`KernelControl`] when `B`
/// is, so a composed backend records both sides against one journal.
pub struct Recording<B> {
    backend: B,
    journal: JournalHandle,
}

impl<B> Recording<B> {
    /// Wraps `backend`, sharing `journal` so a composed MC/kernel pair records against one run
    /// journal.
    #[must_use]
    pub fn new(backend: B, journal: JournalHandle) -> Self {
        Self { backend, journal }
    }

    /// Appends one observed actuation. Called only on a verb that returned `Ok`.
    fn record(&self, record: ActuationRecord) {
        self.journal.borrow_mut().append(record);
    }
}

impl<B: McControl> McControl for Recording<B> {
    fn observe(&self) -> Result<ObservedTopology, Error> {
        self.backend.observe()
    }

    fn read_inventory(&self) -> Result<Inventory, Error> {
        self.backend.read_inventory()
    }

    fn observe_dpmac(&self, dpmac: DpmacId) -> Result<DpmacObservation, Error> {
        self.backend.observe_dpmac(dpmac)
    }

    fn observe_dpseci(&self, container: DprcId, dpseci: ObjectRef) -> Result<DpseciDetail, Error> {
        self.backend.observe_dpseci(container, dpseci)
    }

    fn create_dpni(&self, label: &ConstructName, cfg: &DpniCfg) -> Result<DpniId, Error> {
        let dpni = self.backend.create_dpni(label, cfg)?;
        self.record(ActuationRecord::DpniCreated {
            dpni,
            container: None,
        });
        Ok(dpni)
    }

    fn create_dpni_in(
        &self,
        container: DprcId,
        cfg: &DpniCfg,
        label: &ConstructName,
    ) -> Result<DpniId, Error> {
        let dpni = self.backend.create_dpni_in(container, cfg, label)?;
        self.record(ActuationRecord::DpniCreated {
            dpni,
            container: Some(container),
        });
        Ok(dpni)
    }

    fn connect(&self, dpni: DpniId, dpmac: DpmacId) -> Result<(), Error> {
        self.backend.connect(dpni, dpmac)?;
        // A plain connect is issued at the root; the peer is the `dpmac.N` ref (rollback.rs mapping).
        self.record(ActuationRecord::Connected {
            ancestor: DprcId::ROOT,
            dpni,
            peer: ObjectRef::new(Family::Dpmac, dpmac.into_inner()),
        });
        Ok(())
    }

    fn dprc_connect(&self, ancestor: DprcId, dpni: DpniId, peer: ObjectRef) -> Result<(), Error> {
        self.backend.dprc_connect(ancestor, dpni, peer)?;
        self.record(ActuationRecord::Connected {
            ancestor,
            dpni,
            peer,
        });
        Ok(())
    }

    fn dprc_get_connection(&self, dpni: DpniId) -> Result<Option<ObjectRef>, Error> {
        self.backend.dprc_get_connection(dpni)
    }

    fn dpni_get_link_state(&self, dpni: DpniId) -> Result<LinkState, Error> {
        self.backend.dpni_get_link_state(dpni)
    }

    // Attribute overwrite: the prior value is not observed, so rollback leans on the object's
    // own destroy reversal and the level-triggered re-stamp — not journaled (rollback.rs mapping).
    fn set_mac(&self, dpni: DpniId, mac: MacAddr) -> Result<(), Error> {
        self.backend.set_mac(dpni, mac)
    }

    fn set_label(&self, dpni: DpniId, label: &ConstructName) -> Result<(), Error> {
        self.backend.set_label(dpni, label)
    }

    // Teardown verb: reversing it would restore a prior generation, a plan-rollback D1 non-goal.
    fn dprc_disconnect(&self, ancestor: DprcId, dpni: DpniId) -> Result<(), Error> {
        self.backend.dprc_disconnect(ancestor, dpni)
    }

    fn destroy(&self, dpni: DpniId) -> Result<(), Error> {
        self.backend.destroy(dpni)
    }

    fn observe_containers(&self) -> Result<BTreeMap<DprcId, ObservedContainer>, Error> {
        self.backend.observe_containers()
    }

    fn observe_container(&self, id: DprcId) -> Result<Option<ObservedContainer>, Error> {
        self.backend.observe_container(id)
    }

    fn dprc_create(
        &self,
        parent: DprcId,
        options: dprc::Options,
        label: &ConstructName,
    ) -> Result<DprcId, Error> {
        let dprc = self.backend.dprc_create(parent, options, label)?;
        self.record(ActuationRecord::DprcCreated { dprc });
        Ok(dprc)
    }

    fn dprc_destroy(&self, container: DprcId) -> Result<(), Error> {
        self.backend.dprc_destroy(container)
    }

    // Placement overwrite: subsumed by the object's destroy reversal — not journaled.
    fn dprc_assign(
        &self,
        container: DprcId,
        object: ObjectRef,
        child: Option<DprcId>,
        plugged: Option<bool>,
    ) -> Result<(), Error> {
        self.backend.dprc_assign(container, object, child, plugged)
    }

    fn dprc_unassign(&self, parent: DprcId, child: DprcId, object: ObjectRef) -> Result<(), Error> {
        self.backend.dprc_unassign(parent, child, object)
    }

    fn dprc_set_label(&self, container: DprcId, label: &ConstructName) -> Result<(), Error> {
        self.backend.dprc_set_label(container, label)
    }

    fn dprc_set_locked(&self, child: DprcId, locked: bool) -> Result<(), Error> {
        self.backend.dprc_set_locked(child, locked)
    }

    fn dpbp_create(
        &self,
        container: Option<DprcId>,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        let object = self.backend.dpbp_create(container, label)?;
        self.record(ActuationRecord::PoolObjectCreated { object, container });
        Ok(object)
    }

    fn dpmcp_create(
        &self,
        container: Option<DprcId>,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        let object = self.backend.dpmcp_create(container, label)?;
        self.record(ActuationRecord::PoolObjectCreated { object, container });
        Ok(object)
    }

    fn dpcon_create(
        &self,
        container: Option<DprcId>,
        priorities: Priorities,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        let object = self.backend.dpcon_create(container, priorities, label)?;
        self.record(ActuationRecord::PoolObjectCreated { object, container });
        Ok(object)
    }

    fn dpio_create(
        &self,
        container: Option<DprcId>,
        cfg: DpioCfg,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        let object = self.backend.dpio_create(container, cfg, label)?;
        self.record(ActuationRecord::PoolObjectCreated { object, container });
        Ok(object)
    }

    fn pool_destroy(&self, object: &ObjectRef) -> Result<(), Error> {
        self.backend.pool_destroy(object)
    }

    fn create_dpseci_in(
        &self,
        container: Option<DprcId>,
        cfg: &DpseciCfg,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        let object = self.backend.create_dpseci_in(container, cfg, label)?;
        self.record(ActuationRecord::DpseciCreated { object, container });
        Ok(object)
    }

    fn destroy_dpseci(&self, container: Option<DprcId>, dpseci: &ObjectRef) -> Result<(), Error> {
        self.backend.destroy_dpseci(container, dpseci)
    }

    fn observe_dpsw(&self, container: DprcId, dpsw: ObjectRef) -> Result<DpswDetail, Error> {
        self.backend.observe_dpsw(container, dpsw)
    }

    fn create_dpsw_in(
        &self,
        container: Option<DprcId>,
        cfg: &DpswCfg,
        label: &ConstructName,
    ) -> Result<ObjectRef, Error> {
        let object = self.backend.create_dpsw_in(container, cfg, label)?;
        self.record(ActuationRecord::DpswCreated { object, container });
        Ok(object)
    }

    fn destroy_dpsw(&self, container: Option<DprcId>, dpsw: &ObjectRef) -> Result<(), Error> {
        self.backend.destroy_dpsw(container, dpsw)
    }

    fn connect_switch_port(
        &self,
        ancestor: DprcId,
        dpsw: ObjectRef,
        ifx: u32,
        peer: ObjectRef,
    ) -> Result<(), Error> {
        self.backend
            .connect_switch_port(ancestor, dpsw, ifx, peer)?;
        self.record(ActuationRecord::SwitchPortConnected {
            ancestor,
            dpsw,
            ifx,
            peer,
        });
        Ok(())
    }

    fn observe_pool(
        &self,
        container: Option<DprcId>,
        family: Family,
    ) -> Result<Vec<ObservedPoolObject>, Error> {
        self.backend.observe_pool(container, family)
    }
}

impl<B: KernelControl> KernelControl for Recording<B> {
    fn bind(&self, dpni: DpniId) -> Result<(), Error> {
        self.backend.bind(dpni)?;
        self.record(ActuationRecord::DpniBound { dpni });
        Ok(())
    }

    // Teardown verb, a plan-rollback design D1 non-goal to reverse (it restores a prior generation).
    fn unbind(&self, dpni: DpniId) -> Result<(), Error> {
        self.backend.unbind(dpni)
    }

    fn netdev_of(&self, dpni: DpniId) -> Result<Option<String>, Error> {
        self.backend.netdev_of(dpni)
    }

    fn carrier(&self, source: CarrierSource) -> Result<Option<bool>, Error> {
        self.backend.carrier(source)
    }

    fn dpni_driver(&self, dpni: DpniId) -> Result<Option<RawDriver>, Error> {
        self.backend.dpni_driver(dpni)
    }

    // The three child-dprc bind verbs all record `DprcBound`; one `vfio_unbind` reverses them
    // (rollback.rs mapping).
    fn vfio_set_override(&self, dprc: DprcId) -> Result<(), Error> {
        self.backend.vfio_set_override(dprc)?;
        self.record(ActuationRecord::DprcBound { dprc });
        Ok(())
    }

    fn vfio_bind(&self, dprc: DprcId) -> Result<(), Error> {
        self.backend.vfio_bind(dprc)?;
        self.record(ActuationRecord::DprcBound { dprc });
        Ok(())
    }

    fn dprc_kernel_bind(&self, dprc: DprcId) -> Result<(), Error> {
        self.backend.dprc_kernel_bind(dprc)?;
        self.record(ActuationRecord::DprcBound { dprc });
        Ok(())
    }

    fn vfio_unbind(&self, dprc: DprcId) -> Result<(), Error> {
        self.backend.vfio_unbind(dprc)
    }

    fn bound_driver(&self, dprc: DprcId) -> Result<Option<RawDriver>, Error> {
        self.backend.bound_driver(dprc)
    }

    fn driver_override(&self, dprc: DprcId) -> Result<Option<String>, Error> {
        self.backend.driver_override(dprc)
    }

    fn iommu_group(&self, dprc: DprcId) -> Result<Option<u32>, Error> {
        self.backend.iommu_group(dprc)
    }

    fn unbind_current(&self, object: &ObjectRef) -> Result<(), Error> {
        self.backend.unbind_current(object)
    }
}

#[cfg(test)]
mod tests {
    use super::{JournalHandle, Recording};
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::contract::fake::FakeBackend;
    use crate::contract::{KernelControl, McControl};
    use crate::core::error::Error;
    use crate::core::family::Family;
    use crate::core::model::{DpmacId, DpniId, LinkType, MacAddr};
    use crate::core::types::ConstructName;
    use crate::families::dpni::DpniCfg;
    use crate::plan::rollback::ActuationRecord;

    fn handle() -> JournalHandle {
        Rc::new(RefCell::new(crate::plan::rollback::RunJournal::new()))
    }

    fn rows(journal: &JournalHandle) -> Vec<ActuationRecord> {
        journal.borrow().iter().copied().collect()
    }

    // Spec "The journal records observations, not intent": the row carries the id the fake
    // assigned, which the wrapper cannot know until the verb returns.
    #[test]
    fn create_journals_the_observed_id() {
        let journal = handle();
        let backend = Recording::new(FakeBackend::new(), Rc::clone(&journal));
        let label = ConstructName::from("port");

        let first = backend.create_dpni(&label, &DpniCfg::defaults()).unwrap();
        let second = backend.create_dpni(&label, &DpniCfg::defaults()).unwrap();
        assert_ne!(first, second, "the fake assigns distinct ids");

        assert_eq!(
            rows(&journal),
            vec![
                ActuationRecord::DpniCreated {
                    dpni: first,
                    container: None,
                },
                ActuationRecord::DpniCreated {
                    dpni: second,
                    container: None,
                },
            ],
        );
    }

    // Spec "A failed verb leaves no journal row": a refused create appends nothing.
    #[test]
    fn a_failed_verb_leaves_no_row() {
        let journal = handle();
        let backend = Recording::new(
            FakeBackend::new().with_dprc_create_refusal(Error::McStatus { status: 0x6 }),
            Rc::clone(&journal),
        );

        let refused = backend.dprc_create(
            crate::core::model::DprcId::ROOT,
            crate::families::dprc::Options::DEFAULT,
            &ConstructName::from("child"),
        );
        assert!(refused.is_err());
        assert_eq!(rows(&journal), Vec::new());
    }

    // The layer never alters a verb's result or the backend's observed state: a wrapped run and
    // an unwrapped one over the same sequence agree on ids and on re-observation.
    #[test]
    fn passthrough_is_result_and_state_equivalent() {
        let mac = MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);
        let dpmac = DpmacId::new(1);
        let label = ConstructName::from("port");

        let plain = FakeBackend::new().with_dpmac(dpmac, LinkType::Phy, mac);
        let plain_id = plain.create_dpni(&label, &DpniCfg::defaults()).unwrap();
        plain.connect(plain_id, dpmac).unwrap();

        let journal = handle();
        let wrapped = Recording::new(
            FakeBackend::new().with_dpmac(dpmac, LinkType::Phy, mac),
            Rc::clone(&journal),
        );
        let wrapped_id = wrapped.create_dpni(&label, &DpniCfg::defaults()).unwrap();
        wrapped.connect(wrapped_id, dpmac).unwrap();

        assert_eq!(plain_id, wrapped_id);
        assert_eq!(plain.observe().unwrap(), wrapped.observe().unwrap());
        assert_eq!(
            plain.observe_pool(None, Family::Dpni).unwrap(),
            wrapped.observe_pool(None, Family::Dpni).unwrap(),
        );
    }

    // A teardown verb is a reversal, never a reversible actuation: destroy adds no row.
    #[test]
    fn teardown_journals_nothing() {
        let journal = handle();
        let backend = Recording::new(FakeBackend::new(), Rc::clone(&journal));
        let created = backend
            .create_dpni(&ConstructName::from("port"), &DpniCfg::defaults())
            .unwrap();

        backend.destroy(created).unwrap();

        assert_eq!(
            rows(&journal),
            vec![ActuationRecord::DpniCreated {
                dpni: created,
                container: None,
            }],
            "the create journals; the destroy does not",
        );
    }

    // Kernel side: bind journals DpniBound; an excluded observing verb (netdev_of) journals nothing.
    #[test]
    fn bind_journals_but_observation_does_not() {
        let journal = handle();
        let backend = Recording::new(FakeBackend::new(), Rc::clone(&journal));
        let dpni = DpniId::new(3);

        backend.bind(dpni).unwrap();
        backend.netdev_of(dpni).unwrap();

        assert_eq!(rows(&journal), vec![ActuationRecord::DpniBound { dpni }]);
    }
}
