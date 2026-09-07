//! The compiled object plan and its witness-taking constructors (design D6;
//! ADR-0013 §6).
//!
//! Transcribed from `models/intent/derive.qnt`. The plan is data — every object
//! keyed by `(tenant, family, ordinal)`, every connection an [`Edge`], every
//! derived value carrying its rule as a provenance node. Its *relationships* are
//! locked by construction: the only way to make a [`PlannedObject`] companion, an
//! [`Edge`], or an [`Interface`] is through a witness that takes the deriving construct
//! ([`Tenant`], [`Port`], [`Link`], [`Fabric`]). Because [`PlannedObject`], [`Edge`] and
//! [`Interface`] have no public constructor of their own, four wrong shapes are not
//! values this module admits, proved by the `compile_fail` doctests on [`PlannedObject`],
//! [`Container`], [`Interface`] and [`Link::wire`]:
//!
//! 1. a free-standing companion — only [`Tenant::companion`] emits one;
//! 2. a dpmac at a link end — [`Link::wire`] takes [`Interface`], never an [`AttachPoint`];
//! 3. re-wiring one interface handle — an [`Interface`] is moved when it is wired, so
//!    the same handle cannot be wired twice (plan-wide single connect is the runtime
//!    invariant `INTENT_I2`, not this type lock: [`Interface::attach_point`] re-mints a
//!    fresh [`AttachPoint`] view on demand);
//! 4. a tenant's object in the root dprc — [`Tenant::companion`]/[`Tenant::dpni`]
//!    place in the tenant's own [`Container::Child`], and no witness places one in
//!    [`Container::Root`].
//!
//! Emission order (`object-model.md` §5: pool companions before the objects that
//! draw them) is a property of the order witnesses append keys in, not a sort
//! applied afterwards.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::family::{Family, Permission};
use crate::intent::{Fabric, Link, Port, Tenant};
use crate::model::DpmacId;
use crate::types::{ConstructName, RuleName, TenantName};

/// A derived object's identity (design D6; `derive.qnt` `ObjectKey`). This is the plan
/// key, the thing every collection sorts and traces by. The MC label the object carries
/// is *not* rendered from it: the label is the owning construct's name (ADR-0015
/// decisions 9+13), stamped on the [`PlannedObject`]. Ordinals are 1-based positions.
/// [`Display`](fmt::Display) renders the key as `<tenant>/<family>/<ordinal>` for
/// human-readable plan output, never as the label the MC carries.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ObjectKey {
    /// The owning tenant's name.
    pub tenant: TenantName,
    /// The object family.
    pub family: Family,
    /// The 1-based ordinal within `(tenant, family)`.
    pub ordinal: u32,
}

impl ObjectKey {
    /// Builds a key.
    #[must_use]
    pub fn new(tenant: impl Into<TenantName>, family: Family, ordinal: u32) -> Self {
        Self {
            tenant: tenant.into(),
            family,
            ordinal,
        }
    }
}

impl fmt::Display for ObjectKey {
    /// The plan-key identity path, `<tenant>/<family>/<ordinal>`, for human-readable
    /// plan output (ADR-0015 decision 9). This is never the label the MC carries — the
    /// label is the owning construct's name on the [`PlannedObject`] — only the key the
    /// plan sorts and traces by.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{}/{}",
            self.tenant,
            self.family.as_str(),
            self.ordinal
        )
    }
}

/// Where an object lives (design D6; `derive.qnt` `Container`).
///
/// [`Container::Root`] is dprc.1 — the kernel's own container, never a tenant's.
/// A non-kernel tenant's objects live in [`Container::Child`], its own child DPRC;
/// no witness places one in [`Container::Root`], so a tenant floating in root is
/// unrepresentable:
///
/// ```compile_fail
/// use dpaa2_api::compiled::{Attributes, Container, ObjectKey, PlannedObject, ProvenanceKey};
/// use dpaa2_api::{ConstructName, Family};
/// // A tenant's dpni cannot be placed in root: `PlannedObject` has private fields, and
/// // the only constructors (`Tenant::dpni`/`companion`) use the tenant's own
/// // `Container::Child`.
/// let _ = PlannedObject {
///     key: ObjectKey::new("vpp", Family::Dpni, 1),
///     container: Container::Root,
///     attributes: Attributes::Dpni { num_queues: 1 },
///     provenance: ProvenanceKey::new("vpp", "dpnis", ""),
///     label: ConstructName::from("vpp"),
/// };
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Container {
    /// The kernel's own container, dprc.1.
    Root,
    /// The named tenant's child DPRC.
    Child(TenantName),
}

/// Whether a value stands on measured evidence or on the declared, visibly
/// unmeasured rate-class table (design D3; `derive.qnt` `Measurement`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Measurement {
    /// Measured evidence (a baseline or ADR anchor).
    Measured,
    /// The rate-class worker table, visibly unmeasured — only rule "T" may carry
    /// this today (ADR-0013 §6 `INTENT_I6`).
    Unmeasured,
}

/// Per-family create-config the plan carries (design D6; `derive.qnt` `Attributes`).
/// Unsized families (dpio, dpbp, dpmcp, dpcon, dprtc) expose no sizing knob.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Attributes {
    /// No sizing knob.
    Unsized,
    /// A dpni's transmit-queue count (≥ T poll-mode, `cpus` kernel).
    Dpni {
        /// Transmit queues.
        num_queues: u32,
    },
    /// A dpseci's queue count and the `DPSECI_OPT_HAS_CG` safety bit.
    Dpseci {
        /// Queue pairs (its crypto block's flows).
        num_queues: u32,
        /// The congestion-group safety bit.
        has_cg: bool,
    },
    /// A dpsw's kernel-bindable configuration (`dpsw.md`, read-not-verified).
    Dpsw {
        /// Interface count.
        num_ifs: u32,
        /// FDB count (≥ `num_ifs`).
        max_fdbs: u32,
        /// `PER_FDB` flooding.
        per_fdb_flooding: bool,
        /// `PER_FDB` broadcast.
        per_fdb_broadcast: bool,
        /// Control interface enabled.
        ctrl_if: bool,
    },
    /// A child DPRC's create options.
    Dprc {
        /// The restool-default option mask.
        options: BTreeSet<Permission>,
    },
}

/// A provenance node's address (design D6; `derive.qnt` `ProvenanceKey`): tenant-level
/// count rules use construct `""`; per-construct rules carry the construct name
/// (the fabric for dpsw, the port for port-edge), so two constructs of one owner do
/// not collide on a single node.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ProvenanceKey {
    /// The owning tenant.
    pub tenant: TenantName,
    /// The rule that emitted the value.
    pub rule: RuleName,
    /// The declared construct, or the empty name for a tenant-level count.
    pub construct: ConstructName,
}

impl ProvenanceKey {
    /// Builds a provenance key.
    #[must_use]
    pub fn new(
        tenant: impl Into<TenantName>,
        rule: impl Into<RuleName>,
        construct: impl Into<ConstructName>,
    ) -> Self {
        Self {
            tenant: tenant.into(),
            rule: rule.into(),
            construct: construct.into(),
        }
    }
}

/// A node of the provenance DAG (design D6; `derive.qnt` `ProvenanceNode`): `inputs` are
/// the [`ProvenanceKey`]s it consumed, `constructs` the declared names it bottoms out in,
/// `anchor` the ADR/baseline section the rule cites. `value = request + extra` when
/// an extra exists, else `request` (design D5).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ProvenanceNode {
    /// The rule name.
    pub rule: RuleName,
    /// The evidence anchor (free-text ADR/baseline citation).
    pub anchor: String,
    /// Whether the value is measured or unmeasured.
    pub mark: Measurement,
    /// The derived request (before any extra).
    pub request: i64,
    /// The additive extra, if one was declared.
    pub extra: Option<i64>,
    /// The effective value (`request + extra`).
    pub value: i64,
    /// The provenance keys this node consumed.
    pub inputs: BTreeSet<ProvenanceKey>,
    /// The declared construct names this node bottoms out in.
    pub constructs: BTreeSet<ConstructName>,
}

/// A planned object: its key, its container, its create-config, and the provenance
/// node that emitted it (design D6; `derive.qnt` `PlannedObject`).
///
/// Fields are private: the only constructors are the witness methods
/// [`Tenant::child_dprc`], [`Tenant::companion`], [`Tenant::dpni`], [`Port::terminate`]
/// and [`Fabric::dpsw`], so a companion cannot stand free of a tenant:
///
/// ```compile_fail
/// use dpaa2_api::compiled::{Attributes, Container, ObjectKey, PlannedObject, ProvenanceKey};
/// use dpaa2_api::{ConstructName, Family};
/// // A free-standing dpio: `PlannedObject` has no public constructor, so only
/// // `Tenant::companion` can emit a companion — never a bare literal.
/// let _ = PlannedObject {
///     key: ObjectKey::new("", Family::Dpio, 1),
///     container: Container::Root,
///     attributes: Attributes::Unsized,
///     provenance: ProvenanceKey::new("", "dpio", ""),
///     label: ConstructName::from(""),
/// };
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PlannedObject {
    key: ObjectKey,
    container: Container,
    attributes: Attributes,
    provenance: ProvenanceKey,
    /// The MC label written on create: the owning construct's name byte-for-byte,
    /// lossless by the ADR-0015 decision-13 name rule (≤ 15 chars, never `family.N`).
    /// An anchored object (a port's dpni, a fabric's dpsw) is stamped too, for
    /// debuggability with bare restool, but re-associated by its anchor — its dpmac
    /// edge — never by this label (ADR-0015 decision 9); the [`ObjectKey`] stays the
    /// plan identity.
    label: ConstructName,
}

impl PlannedObject {
    /// The object's identity.
    #[must_use]
    pub fn key(&self) -> &ObjectKey {
        &self.key
    }

    /// The MC label the object carries: the owning construct's name (ADR-0015
    /// decisions 9+13). The label the reconciler writes on create and repairs by
    /// `set-label`; never the plan identity, which is [`key`](Self::key).
    #[must_use]
    pub fn label(&self) -> &ConstructName {
        &self.label
    }

    /// Where the object lives.
    #[must_use]
    pub fn container(&self) -> &Container {
        &self.container
    }

    /// The object's create-config.
    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// The provenance node that emitted the object.
    #[must_use]
    pub fn provenance(&self) -> &ProvenanceKey {
        &self.provenance
    }
}

/// A connect endpoint (design D6; `derive.qnt` `AttachPoint`): an object's port surface, or
/// a bare dpmac. A dpmac end is legal only where a witness places it — a port-edge
/// ([`Port::terminate`]) or a fabric-edge ([`Fabric::edge`]) — never at a link end
/// (see [`Interface`]).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum AttachPoint {
    /// An object's port surface.
    Object {
        /// The object.
        key: ObjectKey,
        /// The endpoint port (0 for single-port objects).
        port: u32,
    },
    /// A bare dpmac.
    Mac(DpmacId),
}

impl AttachPoint {
    /// A dpmac endpoint.
    #[must_use]
    pub fn mac(dpmac: DpmacId) -> Self {
        Self::Mac(dpmac)
    }

    /// An object-port endpoint.
    #[must_use]
    pub fn object(key: ObjectKey, port: u32) -> Self {
        Self::Object { key, port }
    }
}

/// A tenant interface handle: a dpni surface a witness yields, and the only value a
/// [`Link`] or a port-edge may take as an end (design D6).
///
/// It is neither `Copy` nor `Clone` and is *consumed* when it is wired, so one
/// interface cannot be connected twice:
///
/// ```compile_fail
/// use dpaa2_api::{kernel_tenant, Link, TenantRef};
/// let k = kernel_tenant(1);
/// let l = Link { name: "w".into(), interface_a: TenantRef::Kernel, interface_b: TenantRef::Kernel, renamed: None };
/// let (_o1, ia) = k.dpni(1, 0, "w".into());
/// let (_o2, ib) = k.dpni(2, 0, "w".into());
/// let (_o3, ic) = k.dpni(3, 0, "w".into());
/// let _e1 = l.wire(ia, ib);
/// let _e2 = l.wire(ia, ic); // error: use of moved value `ia`
/// ```
#[derive(Debug)]
pub struct Interface {
    key: ObjectKey,
    port: u32,
}

impl Interface {
    /// The dpni key this interface belongs to.
    #[must_use]
    pub fn key(&self) -> &ObjectKey {
        &self.key
    }

    /// The connect endpoint this interface presents — a non-consuming view, re-minted
    /// from `&self` on each call. Only [`into_port_edge`](Self::into_port_edge) and
    /// [`Link::wire`]/[`Fabric::wire`] consume the handle, so the type lock stops one
    /// *handle* from being wired twice; plan-wide single connect stays the runtime
    /// invariant `INTENT_I2`, not a type guarantee.
    #[must_use]
    pub fn attach_point(&self) -> AttachPoint {
        AttachPoint::Object {
            key: self.key.clone(),
            port: self.port,
        }
    }

    /// Consumes the interface into a port-edge to a dpmac (design D6; the
    /// dpni↔dpmac edge of `object-model.md` §2, figure 6a).
    #[must_use]
    pub fn into_port_edge(self, dpmac: DpmacId) -> Edge {
        Edge {
            provenance: ProvenanceKey::new(self.key.tenant.clone(), "port-edge", ""),
            a: self.attach_point(),
            b: AttachPoint::Mac(dpmac),
        }
    }
}

/// A connection between two endpoints, with the provenance node that emitted it
/// (design D6; `derive.qnt` `Edge`).
///
/// Fields are private: the only constructors are the witness methods
/// [`Interface::into_port_edge`], [`Link::wire`], [`Port::terminate`] and
/// [`Fabric::edge`]. Nothing else can produce an edge.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Edge {
    a: AttachPoint,
    b: AttachPoint,
    provenance: ProvenanceKey,
}

impl Edge {
    /// One endpoint.
    #[must_use]
    pub fn a(&self) -> &AttachPoint {
        &self.a
    }

    /// The other endpoint.
    #[must_use]
    pub fn b(&self) -> &AttachPoint {
        &self.b
    }

    /// The provenance node that emitted the edge.
    #[must_use]
    pub fn provenance(&self) -> &ProvenanceKey {
        &self.provenance
    }

    /// The dpni key and dpmac a dpni↔dpmac port-edge connects, or `None` for any
    /// other edge — a dpni↔dpni link/fabric wire or a dpsw↔dpmac fabric-edge (design
    /// D10). This is the one discriminator for the port facet: the objects the port
    /// reconciler actuates are exactly these dpni ends, so both
    /// [`DesiredTopology::from_parts`](crate::DesiredTopology::from_parts) (which
    /// pairs on the dpmac) and [`CompiledPlan::plan_only_by_family`] (which excludes
    /// the dpni) read the same set from here.
    #[must_use]
    pub fn port_edge_dpni(&self) -> Option<(&ObjectKey, DpmacId)> {
        match (&self.a, &self.b) {
            (AttachPoint::Object { key, .. }, AttachPoint::Mac(dpmac))
            | (AttachPoint::Mac(dpmac), AttachPoint::Object { key, .. })
                if key.family == Family::Dpni =>
            {
                Some((key, *dpmac))
            }
            _ => None,
        }
    }
}

/// The compiled object plan (design D6; `derive.qnt` `Plan`): the objects, the
/// edges, the emission order, and the provenance DAG.
///
/// The collections are public, but they can only hold witness-built [`PlannedObject`]s
/// and [`Edge`]s, so the relationship locks hold however a plan is assembled.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CompiledPlan {
    /// The planned objects.
    pub objects: BTreeSet<PlannedObject>,
    /// The connection edges.
    pub edges: BTreeSet<Edge>,
    /// The emission order (`object-model.md` §5): companions before the objects
    /// that draw them, in the order the witnesses appended them.
    pub order: Vec<ObjectKey>,
    /// The provenance DAG, keyed by [`ProvenanceKey`].
    pub provenance: BTreeMap<ProvenanceKey, ProvenanceNode>,
}

impl CompiledPlan {
    /// Counts the objects the port facet has no executor for, grouped by family
    /// (design D10): every planned object except the dpni end of a dpni↔dpmac
    /// port-edge, which the port reconciler actuates. `reconcile` reports this so an
    /// operator sees every derived object the plan carries; it is never actuated and
    /// never drift. A port-only plan carries nothing but port-edge dpnis, so its
    /// summary is empty.
    #[must_use]
    pub fn plan_only_by_family(&self) -> BTreeMap<Family, usize> {
        let actuated: BTreeSet<&ObjectKey> = self
            .edges
            .iter()
            .filter_map(|e| e.port_edge_dpni().map(|(key, _)| key))
            .collect();
        let mut summary: BTreeMap<Family, usize> = BTreeMap::new();
        for obj in &self.objects {
            if !actuated.contains(obj.key()) {
                *summary.entry(obj.key().family).or_insert(0) += 1;
            }
        }
        summary
    }

    /// The compiled transmit-queue count of the dpni terminating the port anchored
    /// at `dpmac`, or `None` when the plan has no such port-edge. This is the one
    /// sizing authority `reconcile` carries into `Transition::Create` so `ensure`
    /// executes exactly what dry-run rendered (synthesis L2/B3).
    #[must_use]
    pub fn port_dpni_num_queues(&self, dpmac: DpmacId) -> Option<u32> {
        let key = self.edges.iter().find_map(|e| {
            e.port_edge_dpni()
                .filter(|&(_, m)| m == dpmac)
                .map(|(key, _)| key)
        })?;
        let obj = self.objects.iter().find(|o| o.key() == key)?;
        match obj.attributes() {
            Attributes::Dpni { num_queues } => Some(*num_queues),
            _ => None,
        }
    }
}

/// The child-DPRC option mask restool creates by default, verified on the reference
/// child (design D6; `dprc.md`; `derive.qnt` `DPRC_DEFAULT_OPTIONS`).
#[must_use]
pub fn dprc_default_options() -> BTreeSet<Permission> {
    BTreeSet::from([
        Permission::Spawn,
        Permission::Alloc,
        Permission::ObjCreate,
        Permission::IrqCfg,
    ])
}

// ---- witness constructors (design D6) -------------------------------------
//
// The plan's relationships live here, on the intent constructs, so nothing else
// can produce a companion, an interface, or an edge. The full sizing is the
// derivation's (`compile`, task 3.2); these build one object/edge from the
// construct that owns it.

impl Tenant {
    /// Where this tenant's objects live (design D6; `derive.qnt` `containerOf`):
    /// the reserved kernel and a restricted drawer pooling the kernel land in
    /// [`Container::Root`]; a restricted drawer pooling a named holder lands in the
    /// holder's [`Container::Child`]; every other tenant keeps its own child DPRC.
    #[must_use]
    pub fn container(&self) -> Container {
        if self.name.is_kernel() {
            Container::Root
        } else if let crate::intent::Isolation::Restricted { pool } = &self.isolation {
            if pool.is_kernel() {
                Container::Root
            } else {
                Container::Child(pool.clone())
            }
        } else {
            Container::Child(self.name.clone())
        }
    }

    /// The tenant's child-DPRC marker in [`Container::Root`] (design D6;
    /// `derive.qnt` `dprcObjs`). Emitted only for a tenant that owns a container
    /// (an isolated tenant or a public holder); the reserved kernel and a
    /// restricted drawer own none.
    #[must_use]
    pub fn child_dprc(&self) -> PlannedObject {
        PlannedObject {
            key: ObjectKey::new(self.name.clone(), Family::Dprc, 1),
            container: Container::Root,
            attributes: Attributes::Dprc {
                options: dprc_default_options(),
            },
            provenance: ProvenanceKey::new(self.name.clone(), "dprc", ""),
            label: ConstructName::from(&self.name),
        }
    }

    /// One companion object (design D6; `derive.qnt` `sizedObjs`): a dpio, dpbp,
    /// dpmcp or dpcon in the tenant's own container. The sole constructor of a
    /// companion — nothing draws one without a tenant witness.
    #[must_use]
    pub fn companion(&self, family: Family, ordinal: u32) -> PlannedObject {
        PlannedObject {
            key: ObjectKey::new(self.name.clone(), family, ordinal),
            container: self.container(),
            attributes: Attributes::Unsized,
            provenance: ProvenanceKey::new(self.name.clone(), family.as_str(), ""),
            label: ConstructName::from(&self.name),
        }
    }

    /// One dpni object and its interface handle (design D6; `derive.qnt` dpni
    /// origins). The [`Interface`] is the only value a [`Link`] or a port-edge accepts
    /// as an end. `label` is the construct the dpni serves — the port, link, or fabric
    /// name its origin resolves to — carried as the MC label (ADR-0015 decisions 9+13);
    /// the caller derives it from the origin, since one tenant's dpnis serve different
    /// constructs.
    #[must_use]
    pub fn dpni(
        &self,
        ordinal: u32,
        num_queues: u32,
        label: ConstructName,
    ) -> (PlannedObject, Interface) {
        let key = ObjectKey::new(self.name.clone(), Family::Dpni, ordinal);
        let obj = PlannedObject {
            key: key.clone(),
            container: self.container(),
            attributes: Attributes::Dpni { num_queues },
            provenance: ProvenanceKey::new(self.name.clone(), "dpnis", ""),
            label,
        };
        (obj, Interface { key, port: 0 })
    }

    /// One dpseci object for a crypto block (design D6; `derive.qnt` `dpseciObjs`):
    /// sized by the block's own `flows` with the `DPSECI_OPT_HAS_CG` safety bit, in
    /// the tenant's own container. The sole constructor of a dpseci — like a
    /// companion, it cannot stand free of a tenant, so it can never land in
    /// [`Container::Root`] for a non-kernel tenant. The block a dpseci came from is
    /// named by its `ordinal` (declaration order, task 2.6e).
    #[must_use]
    pub fn dpseci(&self, ordinal: u32, num_queues: u32) -> PlannedObject {
        PlannedObject {
            key: ObjectKey::new(self.name.clone(), Family::Dpseci, ordinal),
            container: self.container(),
            attributes: Attributes::Dpseci {
                num_queues,
                has_cg: true,
            },
            provenance: ProvenanceKey::new(self.name.clone(), "dpseci", ""),
            label: ConstructName::from(&self.name),
        }
    }
}

impl Port {
    /// The port witness (design D6): the dpni the port terminates and its
    /// dpni↔dpmac edge (`object-model.md` §2, figure 6a). Placement comes from the
    /// terminating [`Tenant`] witness, so a tenant's dpni can never be asked to
    /// live in the root dprc; ordinal and queue count are the derivation's outputs.
    #[must_use]
    pub fn terminate(
        &self,
        tenant: &Tenant,
        ordinal: u32,
        num_queues: u32,
    ) -> (PlannedObject, Edge) {
        let key = ObjectKey::new(tenant.name.clone(), Family::Dpni, ordinal);
        let obj = PlannedObject {
            key: key.clone(),
            container: tenant.container(),
            attributes: Attributes::Dpni { num_queues },
            provenance: ProvenanceKey::new(tenant.name.clone(), "dpnis", ""),
            // The port's own name: the dpni is anchored on the port's dpmac, but the
            // label is stamped for debuggability (ADR-0015 decision 9).
            label: ConstructName::from(&self.name),
        };
        let edge = Edge {
            a: AttachPoint::Object { key, port: 0 },
            b: AttachPoint::Mac(self.dpmac),
            provenance: ProvenanceKey::new(tenant.name.clone(), "port-edge", self.name.clone()),
        };
        (obj, edge)
    }
}

impl Link {
    /// The link witness (design D6): the dpni↔dpni pseudo-wire between two tenant
    /// interfaces (`object-model.md` §2, figure 6b, DPNI-I9). It takes two
    /// [`Interface`]s, so a dpmac end is not a link end:
    ///
    /// ```compile_fail
    /// use dpaa2_api::{kernel_tenant, DpmacId, Link, TenantRef};
    /// use dpaa2_api::compiled::AttachPoint;
    /// let k = kernel_tenant(1);
    /// let l = Link { name: "w".into(), interface_a: TenantRef::Kernel, interface_b: TenantRef::Kernel, renamed: None };
    /// let (_o, ia) = k.dpni(1, 0, "w".into());
    /// // `wire` takes `Interface`, so a bare dpmac end is a type error:
    /// let _e = l.wire(AttachPoint::mac(DpmacId::new(7)), ia);
    /// ```
    ///
    /// The link end `a` is the [`Link::interface_a`] side, so its dpni's tenant keys
    /// the edge's provenance (`derive.qnt` `linkEdges`); `construct` is the link's
    /// own name, so the edge points at the [`ProvenanceNode`] the compiler emits for it.
    // Both interfaces are taken by value on purpose: consuming them is the
    // double-connect lock (an `Interface` wired once cannot be wired again).
    #[allow(clippy::needless_pass_by_value)]
    #[must_use]
    pub fn wire(&self, a: Interface, b: Interface) -> Edge {
        Edge {
            provenance: ProvenanceKey::new(a.key.tenant.clone(), "link-edge", self.name.clone()),
            a: a.attach_point(),
            b: b.attach_point(),
        }
    }
}

impl Fabric {
    /// The container the fabric's dpsw lives in — its forwarder's (design D6): the
    /// kernel forwards a hardware fabric, so this is [`Container::Root`].
    fn forwarder_container(&self) -> Container {
        if self.forwarded_by.is_kernel() {
            Container::Root
        } else {
            Container::Child(self.forwarded_by.clone())
        }
    }

    /// The fabric witness (design D6): the dpsw for a hardware fabric (`dpsw.md`
    /// kernel-bindable predicate, read-not-verified), sized by its interface count.
    #[must_use]
    pub fn dpsw(&self, ordinal: u32, num_ifs: u32) -> PlannedObject {
        PlannedObject {
            key: ObjectKey::new(self.forwarded_by.clone(), Family::Dpsw, ordinal),
            container: self.forwarder_container(),
            attributes: Attributes::Dpsw {
                num_ifs,
                max_fdbs: num_ifs,
                per_fdb_flooding: true,
                per_fdb_broadcast: true,
                ctrl_if: true,
            },
            provenance: ProvenanceKey::new(self.forwarded_by.clone(), "dpsw", self.name.clone()),
            // The fabric's own name: the dpsw is anchored on its wired dpmac set, but the
            // label is stamped for debuggability (ADR-0015 decision 9).
            label: self.name.clone(),
        }
    }

    /// A fabric-edge (design D6; `object-model.md` §2, figure 6c): the dpsw
    /// interface `ifx` to one endpoint — a member port's dpmac or a member tenant's
    /// dpni. The one place besides a port-edge where a dpmac end is legal.
    #[must_use]
    pub fn edge(&self, dpsw: &ObjectKey, ifx: u32, endpoint: AttachPoint) -> Edge {
        Edge {
            a: AttachPoint::Object {
                key: dpsw.clone(),
                port: ifx,
            },
            b: endpoint,
            provenance: ProvenanceKey::new(
                self.forwarded_by.clone(),
                "fabric-edge",
                self.name.clone(),
            ),
        }
    }

    /// A software-fabric pseudo-wire (design D6; `object-model.md` §2, figure 6b;
    /// `derive.qnt` `wireEdges`): the forwarding tenant's dpni ↔ a member tenant's
    /// dpni, the boundary connector a software switch bridges without a dpsw. Like
    /// [`Link::wire`] it consumes both [`Interface`]s (the double-connect lock); the
    /// interface `a` is the forwarder's side, and `construct` is the fabric's own
    /// name.
    #[allow(clippy::needless_pass_by_value)]
    #[must_use]
    pub fn wire(&self, a: Interface, b: Interface) -> Edge {
        Edge {
            provenance: ProvenanceKey::new(
                self.forwarded_by.clone(),
                "fabric-wire",
                self.name.clone(),
            ),
            a: a.attach_point(),
            b: b.attach_point(),
        }
    }
}
