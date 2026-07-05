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
use std::collections::BTreeMap;

/// A 48-bit Ethernet MAC address.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
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

/// The operator's declared intent: a set of ports keyed by DPMAC anchor.
///
/// This is the neutral model produced by any northbound config source (TOML now,
/// gNMI later) and consumed by [`crate::reconcile()`]. It carries no serialization
/// derives (config spec).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DesiredTopology {
    ports: Vec<DesiredPort>,
}

impl DesiredTopology {
    /// Creates an empty desired topology.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a desired topology from an iterator of ports.
    #[must_use]
    pub fn from_ports(ports: impl IntoIterator<Item = DesiredPort>) -> Self {
        Self {
            ports: ports.into_iter().collect(),
        }
    }

    /// Appends a port.
    pub fn push(&mut self, port: DesiredPort) {
        self.ports.push(port);
    }

    /// All declared ports.
    #[must_use]
    pub fn ports(&self) -> &[DesiredPort] {
        &self.ports
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
