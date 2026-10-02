//! The dpmac boot-born-offer surface as observation-judged types — the Rust twin of the
//! Quint model `models/families/dpmac.qnt` (`module dpmac_lifecycle`; ADR-0019 pattern
//! P4, the boot-born offer).
//!
//! Filed under `families/` as the per-family vocabulary tile whose `families/dpmac.rs`
//! path mirrors that `models/families/dpmac.qnt` quint twin one-to-one (ADR-0018).
//!
//! Authored model-first (quint-is-the-spec): the sums, payloads, and pure judgments here
//! are structurally isomorphic to the Quint sums and predicates, and the ADR-0002 §3 law
//! binds them — same cases, same payloads, same semantics; names converge on readable
//! English on both surfaces (the qnt's `camelCase` reads as Rust `snake_case`, no other
//! rename). The baseline anchor is `docs/baseline/dpmac.md` ("Attribute mutability", "MC
//! API notes", "Kernel-side behavior", "Lifecycle ordering and dependencies",
//! "Silent-failure notes"; DPMAC-I2/I3/I4/I6/I7).
//!
//! # The dpmac is P4: no constructor for the port object (ADR-0019 §P4, DPMAC-I1)
//!
//! The board births the dpmac from the DPC at boot and pins it in the root: `creatable:
//! false`, so there is no create path and no "set state" verb. This tile therefore mints
//! no board-object constructor — it carries the observation vocabulary (attributes, link
//! channels, counters, peer evidence) and the pure judgments the control plane runs over
//! a read-back. Every value enters as an observation parameter; the one typestate axis,
//! driver arbitration, is *judged from* that evidence ([`judge_arbitration`]), never
//! commanded (ADR-0019 §P1 phase-marker idiom; `docs/baseline/dpmac.md` "Kernel-side
//! behavior").
//!
//! # The MAC is immutable by construct (DPMAC-I2, ADR-0001 C2)
//!
//! The port MAC is the crate's runtime [`MacAddr`] slot, programmed pre-Linux and read
//! back through `dpmac_get_mac_addr` only. There is no setter anywhere on this surface —
//! immutability is a property of the absent write path, not a runtime check
//! (`docs/baseline/dpmac.md` "Attribute mutability"; the connected dpni inherits this
//! address, ADR-0001 C2).
//!
//! # The teardown sums land here; the edge law lands in dpmac-typestate task 2.2 (ADR-0008 §8)
//!
//! [`KernelFace`] and [`SeveredWitness`] are the data the dpni↔dpmac sever-then-unbind
//! law consumes. They are mirrored here so the vocabulary is complete, but the law itself
//! — the per-edge-kind `sever`/`unbind` verbs and the severed-witness demand — lives on
//! the connection surface and is dpmac-typestate task 2.2's parcel (ADR-0019 edge facet,
//! ADR-0008 §8). This tile mints no sever/unbind verb and no connect-surface change.

use crate::core::model::MacAddr;

// ---- the port MAC (DPMAC-I2) ----

/// The boot-programmed reference MAC the model anchors on (`dpmac.qnt` `BOOT_MAC`;
/// `docs/baseline/dpmac.md` "Attribute mutability"). The stable, globally-unique address
/// the connected dpni inherits (ADR-0001 C2); reused as the crate's [`MacAddr`], never a
/// duplicate slot.
pub const BOOT_MAC: MacAddr = MacAddr::new([2, 0, 0, 0, 0, 7]);

// ---- the DPC-born attribute surface (DPMAC-I3) ----

/// `link_type` — the DPC `board_info` port descriptor (`dpmac.qnt` `type LinkType`;
/// `docs/baseline/dpmac.md` "Option inventory": only PHY/FIXED/BACKPLANE appear in the
/// corpus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LinkType {
    /// A PHY-managed port.
    PhyManaged,
    /// A fixed-link port.
    Fixed,
    /// A backplane port.
    Backplane,
}

/// `fec_mode` — the forward-error-correction mode (`dpmac.qnt` `type FecMode`;
/// `docs/baseline/dpmac.md` "Option inventory": `none` is the only value in the entire
/// DPC corpus, so the sum has one constructor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FecMode {
    /// `none` — the only FEC mode in the corpus.
    FecNone,
}

/// `eth_if` — the ethernet interface protocol (`dpmac.qnt` `type EthIf`;
/// `docs/baseline/dpmac.md` "Option inventory"). Only `USXGMII` appears in the corpus;
/// every other mode rides the RCW `SerDes` protocol, outside it. A runtime exception:
/// `dpmac_set_protocol` (MC ≥ 10.32) moves it, so it is projected out of [`StableAttributes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EthIf {
    /// `USXGMII`.
    Usxgmii,
    /// Any other RCW `SerDes` protocol (outside the corpus).
    OtherSerdesProtocol,
}

/// `dpmac_attr` as the model observes it (`dpmac.qnt` `type DpmacAttributes`;
/// `docs/baseline/dpmac.md` "Attribute mutability"): four DPC-born read-only fields plus
/// the two runtime-mutable exceptions, [`eth_if`](Self::eth_if) (`dpmac_set_protocol`) and
/// [`ipg_length`](Self::ipg_length) (`dpmac_set_params`). Observation data — there is no
/// setter on this surface (DPMAC-I3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DpmacAttributes {
    /// `max_rate` — DPC-born, read-only.
    pub max_rate: u32,
    /// `link_type` — DPC-born, read-only.
    pub link_type: LinkType,
    /// `fec_mode` — DPC-born, read-only.
    pub fec_mode: FecMode,
    /// `serdes_cfg` — DPC-born, read-only (provenance unresolved, `docs/baseline/dpmac.md`).
    pub serdes_cfg: u32,
    /// `eth_if` — the runtime exception moved by `dpmac_set_protocol`.
    pub eth_if: EthIf,
    /// `ipg_length` — the runtime exception moved by `dpmac_set_params`.
    pub ipg_length: u16,
}

/// The boot read-back the model anchors on (`dpmac.qnt` `BOOT_ATTRS`; the reference board
/// environment).
pub const BOOT_ATTRS: DpmacAttributes = DpmacAttributes {
    max_rate: 25000,
    link_type: LinkType::PhyManaged,
    fec_mode: FecMode::FecNone,
    serdes_cfg: 0,
    eth_if: EthIf::Usxgmii,
    ipg_length: 12,
};

/// The stable attribute surface — the DPC-born fields only (`dpmac.qnt` `type
/// StableAttributes`). The two runtime exceptions (`eth_if`, `ipg_length`) are projected
/// out, so drift can never be claimed on them (`docs/baseline/dpmac.md` "Attribute
/// mutability"; DPMAC-I3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableAttributes {
    /// `max_rate`.
    pub max_rate: u32,
    /// `link_type`.
    pub link_type: LinkType,
    /// `fec_mode`.
    pub fec_mode: FecMode,
    /// `serdes_cfg`.
    pub serdes_cfg: u32,
}

/// Projects the live attributes to their stable surface — the `dpmac.qnt` `stableAttributes`
/// (drops `eth_if` and `ipg_length`, the two runtime exceptions; DPMAC-I3).
#[must_use]
pub const fn stable_attributes(a: &DpmacAttributes) -> StableAttributes {
    StableAttributes {
        max_rate: a.max_rate,
        link_type: a.link_type,
        fec_mode: a.fec_mode,
        serdes_cfg: a.serdes_cfg,
    }
}

// ---- the two directional link channels (DPMAC-I4) ----

/// The state-up channel (MC→peer view): the PHY-observed reality the MAC driver pushes
/// into MC via `dpmac_set_link_state`, surfaced to the peer as `dpni_get_link_state`
/// (`dpmac.qnt` `type LinkStateUp`; `docs/baseline/dpmac.md` "MC API notes"). Readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkStateUp {
    /// Whether the PHY link is up.
    pub up: bool,
    /// The negotiated rate.
    pub rate: u32,
    /// The duplex/pause option bits.
    pub pause_options: u16,
}

/// The boot link state (`dpmac.qnt` `BOOT_LINK_STATE`): down, no rate.
pub const BOOT_LINK_STATE: LinkStateUp = LinkStateUp {
    up: false,
    rate: 0,
    pause_options: 0,
};

/// The requests-down channel (peer→MC view): peer requests flowing down via
/// `dpmac_get_link_cfg` (`dpmac.qnt` `type RequestsDownChannel`; `docs/baseline/dpmac.md`
/// "MC API notes", DPMAC-I4). On the restool transport this read is outside the
/// `/dev/dprc.N` whitelist and has no kernel-side observable, so the one constructor
/// carries **no payload**: no expression can read a requests-down value — unreadability is
/// a type property, not a comment (DPMAC-I4, structural).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestsDownChannel {
    /// The unreadable marker — carries nothing, so nothing can be read from it.
    Unreadable,
}

// ---- the firmware-indexed counter vocabulary (DPMAC-I7, dpmac-typestate design D4) ----

/// A representative slice of the counter space across the two firmware tiers (`dpmac.qnt`
/// `type Counter`; `docs/baseline/dpmac.md` "MC API notes", V-DPMAC-1). Our 10.39 firmware
/// defines the first rows (size/pause/byte/frame classes); the egress and per-priority-PFC
/// rows exist only from 10.40. Named here, but unread at 10.39.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Counter {
    /// Ingress byte count — 10.39 readable.
    IngressByteCount,
    /// Ingress frame count — 10.39 readable.
    IngressFrameCount,
    /// Ingress pause frames — 10.39 readable.
    IngressPauseFrames,
    /// Egress byte count — 10.40 only (egress size buckets).
    EgressByteCount,
    /// Per-priority-0 PFC received — 10.40 only.
    PfcPriority0Received,
}

impl Counter {
    /// The whole representative counter slice — the Rust copy of the `dpmac.qnt`
    /// `ALL_COUNTERS` set.
    pub const ALL_COUNTERS: [Self; 5] = [
        Self::IngressByteCount,
        Self::IngressFrameCount,
        Self::IngressPauseFrames,
        Self::EgressByteCount,
        Self::PfcPriority0Received,
    ];

    /// The 10.39 vocabulary — the Rust copy of the `dpmac.qnt` `COUNTERS_1039` set.
    pub const COUNTERS_1039: [Self; 3] = [
        Self::IngressByteCount,
        Self::IngressFrameCount,
        Self::IngressPauseFrames,
    ];

    /// The 10.40 extension — the Rust copy of the `dpmac.qnt` `COUNTERS_1040_EXT` set
    /// (10.40 reads `COUNTERS_1039 ∪ COUNTERS_1040_EXT`, i.e. the whole slice).
    pub const COUNTERS_1040_EXT: [Self; 2] = [Self::EgressByteCount, Self::PfcPriority0Received];
}

/// The MC firmware version the counter vocabulary is indexed by (`dpmac.qnt` `type
/// FirmwareVersion`). The reference board runs 10.39.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FirmwareVersion {
    /// MC 10.39 — the reference board firmware.
    Mc1039,
    /// MC 10.40 — the extended vocabulary.
    Mc1040,
}

/// The readable counter vocabulary for a firmware version — the `dpmac.qnt`
/// `counterVocabulary` (never defaulted to the full set; `docs/baseline/dpmac.md` "MC API
/// notes"). 10.40 reads the whole slice (`COUNTERS_1039 ∪ COUNTERS_1040_EXT`), so it
/// returns [`Counter::ALL_COUNTERS`] directly.
#[must_use]
pub const fn counter_vocabulary(fw: FirmwareVersion) -> &'static [Counter] {
    match fw {
        FirmwareVersion::Mc1039 => &Counter::COUNTERS_1039,
        FirmwareVersion::Mc1040 => &Counter::ALL_COUNTERS,
    }
}

/// A counter read — `Known(value)` or `NotInVocabulary` (`dpmac.qnt` `type CounterRead`).
/// These are distinct constructors: a present-but-zero counter is `Known(0)`, an absent
/// counter is `NotInVocabulary`, so **absence ≠ zero is unrepresentable** (DPMAC-I7;
/// `docs/baseline/dpmac.md` "Silent-failure notes": restool prints whatever succeeds and
/// silently skips refusals, so the vocabulary — not a zero-fill — is the only honest
/// shape). Counters never enter reconcile; they are read-only observability (dpmac-typestate design D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CounterRead {
    /// The counter is in the firmware's vocabulary and read back this value.
    Known(u64),
    /// The counter is outside the firmware's vocabulary — distinct from `Known(0)`.
    NotInVocabulary,
}

/// Reads a counter against a firmware version's vocabulary — the `dpmac.qnt` `readCounter`,
/// with the honest value carried in (dpmac-typestate design D4: `Known(value) | NotInVocabulary`). A
/// counter in the vocabulary reads [`CounterRead::Known`] of the observed `raw` value; one
/// outside reads [`CounterRead::NotInVocabulary`], never `Known(0)` (the model stubs the
/// value as `0`; this surface carries the real read-back, the one deliberate shape
/// difference). Pure and sans-io — the caller supplies the raw value.
#[must_use]
pub fn read_counter(fw: FirmwareVersion, counter: Counter, raw: u64) -> CounterRead {
    if counter_vocabulary(fw).contains(&counter) {
        CounterRead::Known(raw)
    } else {
        CounterRead::NotInVocabulary
    }
}

// ---- the witnessable observation surface (dpmac-typestate design D4; DPMAC-I7) ----

/// The witnessable dpmac observation surface — exactly what one `restool dpmac info
/// dpmac.N` spawn renders, and no more (`docs/baseline/dpmac.md` "Command surface": eth
/// interface, link type, MAC address, max rate; dpmac-typestate spec "The restool shim
/// reads the dpmac observation surface"). The unwitnessable attributes (`fec_mode`,
/// `serdes_cfg`, `ipg_length`) are deliberately absent: restool never prints them, so a
/// read-back would be invention, not observation (dpmac-typestate design D4). `eth_if`
/// rides along for completeness but drift is judged on [`StableAttributes`], which
/// projects it out (DPMAC-I3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpmacObservation {
    /// `DPMAC ethernet interface:` — `USXGMII` vs any other RCW `SerDes` protocol.
    pub eth_if: EthIf,
    /// `DPMAC link type:` — PHY / FIXED / BACKPLANE (DPMAC-I3).
    pub link_type: LinkType,
    /// `MAC address:` — the burned-in port MAC the connected dpni inherits (DPMAC-I2, ADR-0001 C2).
    pub mac: MacAddr,
    /// `maximum supported rate N Mbps` — DPC-born, read-only (DPMAC-I3).
    pub max_rate: u32,
    /// The counter read-back, vocabulary-checked (DPMAC-I7, dpmac-typestate design D4).
    pub counters: CounterReadout,
}

/// The outcome of reading a port's counters against the pinned firmware's vocabulary
/// (DPMAC-I7; dpmac-typestate design D4; `docs/baseline/dpmac.md` "Counter skew",
/// "Silent-failure notes"). restool prints whatever the firmware answers and silently
/// skips refusals, so the rendered row set — never a zero-fill — is the only honest
/// signal: an exact vocabulary match reads the counters as [`CounterRead::Known`] values
/// (a present-but-zero counter is `Known(0)`, never absent), and any deviation is a typed
/// firmware-version signal, not a parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CounterReadout {
    /// The rendered rows matched the firmware vocabulary exactly: each counter a `Known`
    /// value in the firmware's render order (DPMAC-I7: absence ≠ zero, so every entry is
    /// `Known`, a present reading — never a defaulted zero).
    Vocabulary(Vec<CounterRead>),
    /// The rendered row set deviated from the vocabulary (short, over, reordered, or an
    /// unknown name) — a firmware-version signal carrying the expected and observed row
    /// counts, never a parse error and never a zero-fill (dpmac-typestate design D4).
    VersionSignal {
        /// The pinned firmware's vocabulary row count.
        expected: usize,
        /// The rendered row count actually observed.
        got: usize,
    },
}

// ---- the driver-arbitration phase (DPMAC-I6, dpmac-typestate design D2) ----

/// The dpmac typestate — who owns the port, judged from observation (`dpmac.qnt` `type
/// Arbitration`; `docs/baseline/dpmac.md` "Kernel-side behavior"). The ADR-0019 §P1
/// phase-marker idiom, carried as data and never commanded: there is no create and no
/// "set state" verb, so the phase only follows a re-observation (DPMAC-I6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arbitration {
    /// Unconnected: the standalone `fsl_dpaa2_mac` driver binds the PHY.
    Offered,
    /// A same-container kernel dpni bound `fsl_dpaa2_eth`, which evicted the standalone driver.
    KernelOwned,
    /// A cross-container consumer: the standalone driver keeps the PHY, the datapath is remote.
    RemoteOwned,
}

/// The endpoint-query evidence a phase is judged from (`dpmac.qnt` `type PeerObservation`;
/// `fsl_mc_get_endpoint`, `docs/baseline/dpmac.md` "Kernel-side behavior").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PeerObservation {
    /// No connection: the standalone driver binds.
    NoPeer,
    /// A dpni/dpsw in the same container: the eth driver takes the MAC over.
    SameContainerKernelPeer,
    /// A cross-container peer (`fsl_mc_get_endpoint` → `-EPERM`): treated as unconnected.
    CrossContainerPeer,
}

impl PeerObservation {
    /// The whole observation alphabet — the Rust copy of the `dpmac.qnt`
    /// `PEER_OBSERVATIONS` set (all three draws, no more).
    pub const PEER_OBSERVATIONS: [Self; 3] = [
        Self::NoPeer,
        Self::SameContainerKernelPeer,
        Self::CrossContainerPeer,
    ];
}

/// Whether the standalone driver is bound, given the peer evidence — the `dpmac.qnt`
/// `standaloneBoundFor`. The eth driver force-unbinds the standalone driver on a
/// same-container connect and hands it back on disconnect deterministically, so the
/// binding is a function of the peer, not an independent degree of freedom
/// (`docs/baseline/dpmac.md` "Kernel-side behavior").
#[must_use]
pub const fn standalone_bound_for(peer: PeerObservation) -> bool {
    match peer {
        PeerObservation::NoPeer | PeerObservation::CrossContainerPeer => true,
        PeerObservation::SameContainerKernelPeer => false,
    }
}

/// Judges the arbitration phase from the peer evidence — the `dpmac.qnt` `judgeArbitration`
/// (`docs/baseline/dpmac.md` "Lifecycle ordering and dependencies" steps 1–3). The phase
/// follows the endpoint query; it is never commanded (DPMAC-I6).
#[must_use]
pub const fn judge_arbitration(peer: PeerObservation) -> Arbitration {
    match peer {
        PeerObservation::NoPeer => Arbitration::Offered,
        PeerObservation::CrossContainerPeer => Arbitration::RemoteOwned,
        PeerObservation::SameContainerKernelPeer => Arbitration::KernelOwned,
    }
}

// ---- the dpni↔dpmac teardown sums (ADR-0008 §8; the law lands in dpmac-typestate task 2.2) ----

/// The facing dpni's kernel-face across a teardown (`dpmac.qnt` `type KernelFace`;
/// `docs/baseline/dpmac.md` "Kernel-side behavior", ADR-0008 §8). Mirrored here as the
/// data the sever-then-unbind law consumes; the law itself — the verbs and the
/// severed-witness demand — is dpmac-typestate task 2.2's connection-surface parcel, not this tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KernelFace {
    /// Boot state, and the clean state after a full teardown cycle: no kernel dpni holds the port.
    NoKernelDpni,
    /// A same-container dpni bound `fsl_dpaa2_eth` and took the port's datapath.
    KernelFaceBound,
    /// The kernel-face was released — legal only after the edge was severed (ADR-0008 §8).
    KernelFaceReleased,
}

/// The observation vocabulary for whether the dpni↔dpmac edge was torn (`dpmac.qnt` `type
/// SeveredWitness`; ADR-0008 §8). This is a freely-constructible read-back judgment, not a
/// proof token: the plan-surface law that *demands* a witness before the kernel-face unbinds
/// is [`crate::plan::SeveredProof`] (dpmac-typestate task 2.2 — `Transition::sever` mints it,
/// `Transition::unbind` consumes it). The two are deliberately distinct: this enum describes
/// an observed state, `SeveredProof` is the unforgeable compile-time token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SeveredWitness {
    /// The edge is not yet severed.
    NotSevered,
    /// The edge was severed — the standalone driver re-attached at once.
    Severed,
}

// ---- the port MAC relation judgment (dpmac-typestate design D5; DPNI-I3 value semantics) ----

/// How a dpmac-connected dpni's observed primary MAC relates to the port (dpmac-typestate design D5;
/// `docs/baseline/dpmac.md` "Attribute mutability", ADR-0001 C2). A judgment result, not a
/// model-vocabulary sum — so it carries no linted-copy apparatus, the dpni
/// [`DpniDisposition`](crate::families::dpni::DpniDisposition) precedent.
///
/// [`Pending`](Self::Pending) is checked **first** and is explicitly not drift: a judgment
/// that reads a bind-timing all-zeros transient as drift builds a churn loop (the V-MVP-1
/// rev 1 lesson; DPNI-I3 value semantics). [`Mismatched`](Self::Mismatched) plans a
/// mutation only when intent declared a MAC — otherwise it is a bare observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MacRelation {
    /// The observed primary MAC is all-zeros — the consumer has not bound yet. Not drift.
    Pending,
    /// Intent declared a MAC and the observed primary equals it (intent-declared wins the tie).
    Overridden,
    /// The observed primary equals the dpmac's burned-in address — the inherit default.
    Inherited,
    /// None of the above. Drift is planned only when intent declared a value
    /// (`intent_declared: true`); otherwise it surfaces as an observation.
    Mismatched {
        /// Whether intent declared a MAC for this port (true ⇒ plan a mutation; false ⇒ observe).
        intent_declared: bool,
    },
}

/// Classifies a dpmac-connected dpni's primary MAC against the port — the dpmac-typestate design-D5
/// judgment (not in the qnt module; `docs/baseline/dpmac.md` "Attribute mutability",
/// ADR-0001 C2, DPNI-I3). Pure and sans-io: the caller supplies the observed dpni primary
/// MAC, the dpmac's burned-in MAC, and the intent-declared value (`None` ⇒ inherit).
///
/// Precedence: all-zeros → [`MacRelation::Pending`] first (never drift). Then, when intent
/// declares a value the observed primary equals → [`MacRelation::Overridden`] — so when
/// intent declares exactly the burned-in address and the observed primary matches both,
/// the tie resolves to `Overridden` (intent-declared wins). Then equality with the
/// burned-in address → [`MacRelation::Inherited`]. Otherwise → [`MacRelation::Mismatched`],
/// carrying whether intent declared a value.
#[must_use]
pub fn judge_mac_relation(
    observed: MacAddr,
    burned_in: MacAddr,
    intent_declared: Option<MacAddr>,
) -> MacRelation {
    if observed.is_zero() {
        MacRelation::Pending
    } else if intent_declared == Some(observed) {
        MacRelation::Overridden
    } else if observed == burned_in {
        MacRelation::Inherited
    } else {
        MacRelation::Mismatched {
            intent_declared: intent_declared.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Parity of the observation surface with `models/families/dpmac.qnt`
    //! `dpmac_lifecycle`: the arbitration equation (`judgeArbitration`/`standaloneBoundFor`,
    //! DPMAC-I6), the firmware-indexed counter vocabulary (`counterVocabulary`/`readCounter`,
    //! DPMAC-I7), the stable attribute projection (`stableAttributes`, DPMAC-I3), and the
    //! dpmac-typestate design-D5 MAC relation judgment.

    use super::*;

    // ---- DPMAC-I6: arbitration follows the peer evidence (judgeArbitration/standaloneBoundFor) ----

    #[test]
    fn arbitration_follows_the_peer_observation_table() {
        // dpmac.qnt judgeArbitration / standaloneBoundFor tables, over all three draws.
        for peer in PeerObservation::PEER_OBSERVATIONS {
            let phase = judge_arbitration(peer);
            let bound = standalone_bound_for(peer);
            match peer {
                PeerObservation::NoPeer => {
                    assert_eq!(phase, Arbitration::Offered);
                    assert!(bound);
                }
                PeerObservation::CrossContainerPeer => {
                    assert_eq!(phase, Arbitration::RemoteOwned);
                    assert!(bound);
                }
                PeerObservation::SameContainerKernelPeer => {
                    assert_eq!(phase, Arbitration::KernelOwned);
                    assert!(!bound);
                }
            }
            // DPMAC_I6 law: the standalone driver is bound iff the phase is not KernelOwned.
            assert_eq!(bound, phase != Arbitration::KernelOwned);
        }
    }

    // ---- dpmac-typestate design D5: the MAC relation judgment, all four classes ----

    const PORT_MAC: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x07]);
    const OTHER_MAC: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x99]);
    const DECLARED_MAC: MacAddr = MacAddr::new([0x02, 0, 0, 0, 0, 0x42]);

    #[test]
    fn zeros_judge_pending_not_drift() {
        // spec "Bind-timing zeros do not churn": all-zeros ⇒ Pending, checked first, even
        // when intent declared a value — it is never drift.
        assert_eq!(
            judge_mac_relation(MacAddr::ZERO, PORT_MAC, None),
            MacRelation::Pending
        );
        assert_eq!(
            judge_mac_relation(MacAddr::ZERO, PORT_MAC, Some(DECLARED_MAC)),
            MacRelation::Pending
        );
    }

    #[test]
    fn pending_then_inherited_on_rebind() {
        // The later re-observation after bind judges Inherited (DPNI-I3 value semantics).
        assert_eq!(
            judge_mac_relation(MacAddr::ZERO, PORT_MAC, None),
            MacRelation::Pending
        );
        assert_eq!(
            judge_mac_relation(PORT_MAC, PORT_MAC, None),
            MacRelation::Inherited
        );
    }

    #[test]
    fn overridden_when_observed_equals_intent() {
        assert_eq!(
            judge_mac_relation(DECLARED_MAC, PORT_MAC, Some(DECLARED_MAC)),
            MacRelation::Overridden
        );
    }

    #[test]
    fn intent_tie_resolves_to_overridden() {
        // dpmac-typestate design D5 tie: intent declares exactly the burned-in value and observed matches
        // both ⇒ Overridden (intent-declared wins), not Inherited.
        assert_eq!(
            judge_mac_relation(PORT_MAC, PORT_MAC, Some(PORT_MAC)),
            MacRelation::Overridden
        );
    }

    #[test]
    fn mismatched_carries_the_declared_distinction() {
        // Drift is planned only when intent declared a value; otherwise it is an observation.
        assert_eq!(
            judge_mac_relation(OTHER_MAC, PORT_MAC, Some(DECLARED_MAC)),
            MacRelation::Mismatched {
                intent_declared: true
            }
        );
        assert_eq!(
            judge_mac_relation(OTHER_MAC, PORT_MAC, None),
            MacRelation::Mismatched {
                intent_declared: false
            }
        );
    }

    // ---- DPMAC-I7: the counter vocabulary is firmware-indexed, absence ≠ zero ----

    #[test]
    fn counter_vocabulary_is_firmware_indexed() {
        // dpmac.qnt counterVocabularyFirmwareIndexedTest: a 10.40-only counter reads
        // NotInVocabulary at 10.39 — distinct from Known(0), which a present 10.39 counter
        // reads; and the 10.40 extension is readable only at 10.40 (never defaulted on).
        assert_eq!(
            read_counter(FirmwareVersion::Mc1039, Counter::EgressByteCount, 0),
            CounterRead::NotInVocabulary
        );
        assert_ne!(
            read_counter(FirmwareVersion::Mc1039, Counter::EgressByteCount, 0),
            CounterRead::Known(0)
        );
        assert_eq!(
            read_counter(FirmwareVersion::Mc1039, Counter::IngressByteCount, 0),
            CounterRead::Known(0)
        );
        assert_eq!(
            read_counter(FirmwareVersion::Mc1039, Counter::IngressFrameCount, 42),
            CounterRead::Known(42)
        );

        // dpmac.qnt firmwareExtensionReadableAt1040Test: 10.40 extends, never defaults.
        assert_eq!(
            read_counter(FirmwareVersion::Mc1040, Counter::EgressByteCount, 7),
            CounterRead::Known(7)
        );
        assert!(!counter_vocabulary(FirmwareVersion::Mc1039).contains(&Counter::EgressByteCount));
        assert!(counter_vocabulary(FirmwareVersion::Mc1040).contains(&Counter::EgressByteCount));
    }

    // ---- DPMAC-I3: the stable projection drops the two runtime exceptions ----

    #[test]
    fn stable_projection_ignores_eth_if_and_ipg() {
        // dpmac.qnt DPMAC_I3 / attributeConstancyTest: varying eth_if and ipg_length leaves
        // the stable projection equal to the boot projection.
        let boot = stable_attributes(&BOOT_ATTRS);
        for eth_if in [EthIf::Usxgmii, EthIf::OtherSerdesProtocol] {
            for ipg_length in [0u16, 12, 96] {
                let varied = DpmacAttributes {
                    eth_if,
                    ipg_length,
                    ..BOOT_ATTRS
                };
                assert_eq!(stable_attributes(&varied), boot);
            }
        }
    }
}
