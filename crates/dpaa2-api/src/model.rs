//! Backend- and frontend-neutral topology model.
//!
//! The types here describe *what MC objects exist and how they connect*, with no
//! dependency on `restool`, ioctl, `serde`, or any concrete transport. Two graphs
//! are modelled: the operator's [`DesiredTopology`] (intent) and the
//! [`ObservedTopology`] read back from the Management Complex every pass.
//!
//! Identity is anchored on the stable **DPMAC** (design D1): the operator keys a
//! port by `dpmac.N`, and a managed DPNI's identity is derived from its connection
//! edge to that DPMAC, never from its MC-assigned index.

use core::fmt;
use core::str::FromStr;
use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::compiled::{AttachPoint, CompiledPlan, Edge};
use crate::family::Family;
use crate::intent::kernel_tenant;

/// A 48-bit Ethernet MAC address.
///
/// `Ord` is derived (lexicographic over the octets) so a [`MacAddr`] can ride on the
/// fully-ordered [`crate::intent::Port`] the plan keys and sets order (task 3.3); the
/// ordering is incidental, never semantic.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacAddr([u8; 6]);

impl MacAddr {
    /// The all-zero address carried by unprovisioned `macN` placeholders (E6).
    pub const ZERO: MacAddr = MacAddr([0; 6]);

    /// Builds a MAC from its six octets. `const` so it can back test fixtures and
    /// named constants.
    #[must_use]
    pub const fn new(octets: [u8; 6]) -> Self {
        Self(octets)
    }

    /// The six octets, most-significant first.
    #[must_use]
    pub const fn octets(&self) -> [u8; 6] {
        self.0
    }

    /// Returns `true` for the all-zero placeholder address.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }
}

impl From<[u8; 6]> for MacAddr {
    fn from(octets: [u8; 6]) -> Self {
        Self(octets)
    }
}

/// Error returned when a MAC address string is malformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacParseError;

impl fmt::Display for MacParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("malformed MAC address")
    }
}

impl std::error::Error for MacParseError {}

impl FromStr for MacAddr {
    type Err = MacParseError;

    /// Parses a colon- or hyphen-separated 6-octet MAC address.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let sep = if s.contains(':') { ':' } else { '-' };
        let mut octets = [0u8; 6];
        let mut count = 0usize;
        for part in s.split(sep) {
            if count == 6 || part.len() != 2 {
                return Err(MacParseError);
            }
            octets[count] = u8::from_str_radix(part, 16).map_err(|_| MacParseError)?;
            count += 1;
        }
        if count == 6 {
            Ok(MacAddr(octets))
        } else {
            Err(MacParseError)
        }
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let o = &self.0;
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            o[0], o[1], o[2], o[3], o[4], o[5]
        )
    }
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacAddr({self})")
    }
}

/// Stable anchor for a DPMAC object, e.g. `dpmac.3`.
///
/// DPMAC indices are fixed by the board's DPC and never renumber, which is why the
/// whole model keys on them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DpmacId(u32);

impl DpmacId {
    /// Wraps a raw MC index.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// The raw MC index. Prefer [`Display`](fmt::Display) (`dpmac.N`) for output;
    /// this is the last-resort accessor for arithmetic or map keys.
    #[must_use]
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

impl From<u32> for DpmacId {
    fn from(index: u32) -> Self {
        Self(index)
    }
}

impl fmt::Display for DpmacId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dpmac.{}", self.0)
    }
}

impl fmt::Debug for DpmacId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// Identity of an observed DPNI object, e.g. `dpni.7`.
///
/// The index is MC-assigned at creation and may change across reboots; it is used
/// only to *address* an already-observed object, never to match intent.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DpniId(u32);

impl DpniId {
    /// Wraps a raw MC index.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// The raw MC index. Prefer [`Display`](fmt::Display) (`dpni.N`) for output;
    /// this is the last-resort accessor for arithmetic or map keys.
    #[must_use]
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

impl From<u32> for DpniId {
    fn from(index: u32) -> Self {
        Self(index)
    }
}

impl fmt::Display for DpniId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dpni.{}", self.0)
    }
}

impl fmt::Debug for DpniId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// Kinds of MC object the model can carry.
///
/// The set is deliberately open to extension (e.g. `Dpsw`) so that switch
/// topologies can be added later without redefining the model (spec: "general
/// enough to admit additional object kinds").
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[non_exhaustive]
pub enum ObjectKind {
    /// A network interface object (`dpni.N`).
    Dpni,
    /// A MAC / `SerDes` lane object (`dpmac.N`).
    Dpmac,
}

/// Provisioning lifecycle of a managed object.
///
/// States are ordered by progress; `reconcile` drives an object from left to right.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Lifecycle {
    /// No object exists for this port yet.
    Absent,
    /// The DPNI object exists but is not connected to its DPMAC.
    Created,
    /// The DPNI is connected to its DPMAC but no netdev has appeared.
    Connected,
    /// `dpaa2-eth` has bound the DPNI and a netdev exists.
    Bound,
}

/// Physical link type of a DPMAC (design E1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum LinkType {
    /// PHY-backed lane: `dpaa2-eth` binds and a netdev appears.
    Phy,
    /// Fixed link: no netdev appears; "provisioned" means merely connected.
    Fixed,
}

/// How a port's MAC address is treated (design D9, config spec).
///
/// `Ord`/`Hash` are derived so this rides on the fully-ordered, hashable
/// [`crate::intent::Port`] (task 3.3); the ordering is incidental, never semantic.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum MacMode {
    /// Verify the observed MAC against the declared one; never write (default).
    #[default]
    Assert,
    /// Set the DPNI primary MAC to the declared value.
    Actuate,
}

/// Whether the operator wants this port present or torn down (design D7).
///
/// Config produces [`Presence::Present`] in phase 1. [`Presence::Absent`] combined
/// with `--prune` opts a port into teardown; without prune, a removed port is left
/// in place.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Presence {
    /// The port should exist and be connected.
    #[default]
    Present,
    /// The port should be torn down (only actuated under `--prune`).
    Absent,
}

/// One desired port, keyed by its stable DPMAC anchor.
///
/// Actuatable fields: existence, the connection edge, and (when [`MacMode::Actuate`])
/// the primary MAC. Assert-only fields (e.g. link speed) are verified, never written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DesiredPort {
    /// The stable DPMAC this port is anchored to.
    pub dpmac: DpmacId,
    /// The stable interface name the resulting netdev should be renamed to.
    pub name: String,
    /// The port's known/declared MAC, if any.
    pub mac: Option<MacAddr>,
    /// Whether `mac` is asserted or actuated.
    pub mac_mode: MacMode,
    /// Present or (with prune) torn down.
    pub presence: Presence,
    /// Required create-time-only attributes, keyed by attribute name.
    ///
    /// A mismatch against the live object is reported as drift and refused rather
    /// than repaired by destroy-and-recreate (design D8). Phase-1 TOML leaves this
    /// empty; the machinery is exercised directly against the neutral model.
    pub immutable: BTreeMap<String, String>,
}

impl DesiredPort {
    /// Creates an assert-mode, present port with the given anchor and name.
    #[must_use]
    pub fn new(dpmac: DpmacId, name: impl Into<String>) -> Self {
        Self {
            dpmac,
            name: name.into(),
            mac: None,
            mac_mode: MacMode::Assert,
            presence: Presence::Present,
            immutable: BTreeMap::new(),
        }
    }
}

/// The reconciler's input: the compiled object plan plus the port-family
/// actuation projection that `reconcile` drives (design D10).
///
/// This is the reshaped desired value the compiler produces (design D6): a
/// [`CompiledPlan`] of objects keyed `(tenant, family, ordinal)`, edges, emission
/// order and provenance. `reconcile` executes the one family it has an executor for
/// — the dpni↔dpmac port subset — and (from task 3.6) reports the rest as
/// plan-only, so the actuation attributes it needs per port (name, MAC, mode,
/// presence, immutable) ride on the [`DesiredPort`] projection, which the abstract
/// plan cannot express. The two facets cannot drift on any public path:
/// [`from_ports`](Self::from_ports) and [`push`](Self::push) build the plan facet
/// from the ports by construction, and [`from_parts`](Self::from_parts) refuses a
/// pairing whose plan port-edges and ports disagree ([`FacetMismatch`]).
///
/// It carries no serialization derives (config spec); the northbound
/// [`crate::ConfigSource`] parses into it. The plan's witness-taking constructors
/// are public, so a library user builds one programmatically without an
/// [`crate::intent::Intent`] and reconciles it (design D11).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DesiredTopology {
    plan: CompiledPlan,
    ports: Vec<DesiredPort>,
}

/// Refusal from [`DesiredTopology::from_parts`] when the plan facet and the port
/// projection disagree — the pairing would let the two facets drift (design D11).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FacetMismatch {
    /// A [`DesiredPort`] whose dpmac has no dpni↔dpmac port-edge in the plan.
    #[error("port on {0} has no matching port-edge in the plan")]
    PortWithoutEdge(DpmacId),
    /// A dpni↔dpmac port-edge in the plan whose dpmac has no [`DesiredPort`].
    #[error("port-edge on {0} has no matching port in the projection")]
    EdgeWithoutPort(DpmacId),
}

/// The dpmac a dpni↔dpmac port-edge connects, or `None` for any other edge (a
/// dpni↔dpni link-edge, a dpsw↔dpmac fabric-edge): the port projection actuates
/// exactly these edges, so they are the ones [`DesiredTopology::from_parts`] matches.
fn port_edge_mac(edge: &Edge) -> Option<DpmacId> {
    match (edge.a(), edge.b()) {
        (AttachPoint::Object { key, .. }, AttachPoint::Mac(dpmac))
        | (AttachPoint::Mac(dpmac), AttachPoint::Object { key, .. })
            if key.family == Family::Dpni =>
        {
            Some(*dpmac)
        }
        _ => None,
    }
}

impl DesiredTopology {
    /// Creates an empty desired topology.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a desired topology from an iterator of ports.
    ///
    /// A port-only file defaults every port to the kernel tenant in root (design
    /// D10): each port terminates a kernel dpni whose dpni↔dpmac edge is emitted
    /// through the [`crate::intent::Port`] witness path, so the plan facet is built,
    /// not fabricated.
    #[must_use]
    pub fn from_ports(ports: impl IntoIterator<Item = DesiredPort>) -> Self {
        let mut topology = Self::new();
        for port in ports {
            topology.push(port);
        }
        topology
    }

    /// Builds a desired topology from an already-compiled plan and its port-family
    /// actuation projection (design D11): the seam a compiler or a library user
    /// fills through the plan's witness constructors.
    ///
    /// The two facets must agree, so this constructor is fallible where
    /// [`from_ports`](Self::from_ports) is not: it refuses a pairing in which a
    /// [`DesiredPort`] has no dpni↔dpmac port-edge in the plan, or a plan port-edge
    /// has no port in the projection ([`FacetMismatch`]). Without the check task
    /// 3.6's plan-only reporting could disagree with what `reconcile` actuates.
    ///
    /// # Errors
    ///
    /// Returns [`FacetMismatch`] when the plan's port-edges and the ports do not
    /// name the same set of dpmacs.
    pub fn from_parts(plan: CompiledPlan, ports: Vec<DesiredPort>) -> Result<Self, FacetMismatch> {
        let edge_macs: BTreeSet<DpmacId> = plan.edges.iter().filter_map(port_edge_mac).collect();
        if let Some(port) = ports.iter().find(|p| !edge_macs.contains(&p.dpmac)) {
            return Err(FacetMismatch::PortWithoutEdge(port.dpmac));
        }
        let port_macs: BTreeSet<DpmacId> = ports.iter().map(|p| p.dpmac).collect();
        if let Some(&dpmac) = edge_macs.iter().find(|m| !port_macs.contains(m)) {
            return Err(FacetMismatch::EdgeWithoutPort(dpmac));
        }
        Ok(Self { plan, ports })
    }

    /// Appends a port, extending the plan facet with the kernel dpni and port-edge
    /// it terminates (design D10; the port-only projection).
    pub fn push(&mut self, port: DesiredPort) {
        let ordinal = u32::try_from(self.ports.len() + 1).unwrap_or(u32::MAX);
        // ponytail: num_queues 0 in the port-only projection — real sizing is the
        // compiler's (task 3.2); reconcile reads it from `ports`, not the plan.
        let kernel = kernel_tenant(0);
        let (dpni, iface) = kernel.dpni(ordinal, 0);
        self.plan.order.push(dpni.key().clone());
        self.plan.objects.insert(dpni);
        self.plan.edges.insert(iface.into_port_edge(port.dpmac));
        self.ports.push(port);
    }

    /// All declared ports (the actuation projection `reconcile` drives).
    #[must_use]
    pub fn ports(&self) -> &[DesiredPort] {
        &self.ports
    }

    /// The compiled object plan (design D6): objects, edges, order, provenance.
    #[must_use]
    pub fn plan(&self) -> &CompiledPlan {
        &self.plan
    }

    /// Returns `true` if `dpmac` is a configured anchor (i.e. within our subgraph).
    #[must_use]
    pub fn owns(&self, dpmac: DpmacId) -> bool {
        self.ports.iter().any(|p| p.dpmac == dpmac)
    }
}

/// An observed DPNI object and everything we read about it in one pass.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ObservedDpni {
    /// The MC-assigned index of this DPNI.
    pub id: DpniId,
    /// The DPMAC this DPNI is connected to, if any.
    pub connected_to: Option<DpmacId>,
    /// The DPNI's primary MAC, if readable.
    pub mac: Option<MacAddr>,
    /// The Linux netdev name once `dpaa2-eth` has bound it.
    pub netdev: Option<String>,
    /// Create-time attributes read back from the MC, keyed by attribute name.
    ///
    /// Compared against [`DesiredPort::immutable`] for drift detection.
    pub attributes: BTreeMap<String, String>,
}

impl ObservedDpni {
    /// Derives the lifecycle state from what was observed.
    #[must_use]
    pub fn lifecycle(&self) -> Lifecycle {
        match (self.connected_to, &self.netdev) {
            (Some(_), Some(_)) => Lifecycle::Bound,
            (Some(_), None) => Lifecycle::Connected,
            (None, _) => Lifecycle::Created,
        }
    }
}

/// An observed DPMAC object.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ObservedDpmac {
    /// The stable DPMAC index.
    pub id: DpmacId,
    /// The DPMAC's physical link type.
    pub link_type: LinkType,
    /// The DPMAC's burned-in MAC, readable ahead of provisioning (design D3).
    pub mac: Option<MacAddr>,
}

/// The state of the MC as read back in a single observation pass.
///
/// Treated as authoritative every pass; nothing here is persisted between runs
/// (design D2, level-triggered).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ObservedTopology {
    /// All DPNI objects the observation surfaced.
    pub dpnis: Vec<ObservedDpni>,
    /// All DPMAC objects the observation surfaced.
    pub dpmacs: Vec<ObservedDpmac>,
}

impl ObservedTopology {
    /// Creates an empty observation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Finds the DPNI connected to `dpmac` by edge (index-independent matching).
    #[must_use]
    pub fn dpni_connected_to(&self, dpmac: DpmacId) -> Option<&ObservedDpni> {
        self.dpnis.iter().find(|d| d.connected_to == Some(dpmac))
    }

    /// Looks up a DPMAC by its stable id.
    #[must_use]
    pub fn dpmac(&self, dpmac: DpmacId) -> Option<&ObservedDpmac> {
        self.dpmacs.iter().find(|m| m.id == dpmac)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plan carrying a single kernel dpni terminating a port-edge to `dpmac`.
    fn plan_with_port_edge(dpmac: DpmacId) -> CompiledPlan {
        let kernel = kernel_tenant(1);
        let (dpni, iface) = kernel.dpni(1, 0);
        let mut plan = CompiledPlan::default();
        plan.order.push(dpni.key().clone());
        plan.objects.insert(dpni);
        plan.edges.insert(iface.into_port_edge(dpmac));
        plan
    }

    #[test]
    fn from_parts_accepts_agreeing_facets() {
        let plan = plan_with_port_edge(DpmacId::new(7));
        let ports = vec![DesiredPort::new(DpmacId::new(7), "lan0")];
        let topology = DesiredTopology::from_parts(plan, ports).expect("facets agree on dpmac.7");
        assert_eq!(topology.plan().edges.len(), 1);
        assert_eq!(topology.ports().len(), 1);
    }

    #[test]
    fn from_parts_refuses_a_port_without_its_edge() {
        // The plan facet has no port-edge, so the lone port cannot be actuated.
        let ports = vec![DesiredPort::new(DpmacId::new(7), "lan0")];
        let err = DesiredTopology::from_parts(CompiledPlan::default(), ports).unwrap_err();
        assert_eq!(err, FacetMismatch::PortWithoutEdge(DpmacId::new(7)));
    }

    #[test]
    fn from_parts_refuses_an_edge_without_its_port() {
        // The plan facet carries a port-edge no port in the projection names.
        let plan = plan_with_port_edge(DpmacId::new(7));
        let err = DesiredTopology::from_parts(plan, Vec::new()).unwrap_err();
        assert_eq!(err, FacetMismatch::EdgeWithoutPort(DpmacId::new(7)));
    }
}
