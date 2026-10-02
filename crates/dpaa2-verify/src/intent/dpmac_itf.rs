//! Reader for frozen dpmac-lifecycle ITF traces (`models/traces/families/dpmac/*.itf.json`,
//! emitted by `models/families/dpmac.qnt` module `dpmac_lifecycle` via `pnpm model:freeze-dpmac`).
//!
//! The boot-born-offer twin of the dpni/pool readers ([`crate::intent::dpni_itf`],
//! [`crate::intent::pool_itf`]): the P4 surface walks ONE state var — `world: World` — whose
//! record fields are the observation vocabulary the Rust [`dpaa2_api::families::dpmac`] tile
//! reasons at, so this reader decodes each frozen `world` into a `DpmacWorld` built from the
//! real family sums (ADR-0002 §3, no tag flattening) and `tests/dpmac_replay.rs` drives the
//! dpmac-typestate task-2.1 judgments and the task-2.2 edge law through the same states. The
//! transition between two states is not named in the ITF; the replayer infers it from the
//! field delta, mirroring `dpmac_lifecycle`'s action set.
//!
//! A `.fail()` directed step (the hazard-order refusal) froze no variable values (only
//! `#meta`); it decodes to `DpmacStep::Refused`, the disabled-guard sentinel the replayer
//! witnesses against the prior world.
//!
//! Mapping (the only place the two encodings are reconciled): every sum tag ⇒ the family
//! constructor of the same name (`Arbitration`/`PeerObservation`/`FirmwareVersion`/
//! `KernelFace`/`SeveredWitness` and the attribute sums); the `macAddr` `List[int]` ⇒ the
//! runtime `MacAddr`; the `attributes` record ⇒ `DpmacAttributes`. The two directional
//! link channels (`linkStateUp`/`requestsDown`) carry no judgment on this parcel's surface, so
//! they are not decoded (DPMAC-I4 is structural, covered in the Rust tile's own tests).

use serde_json::Value;

use dpaa2_api::core::model::MacAddr;
use dpaa2_api::families::dpmac::{
    Arbitration, DpmacAttributes, EthIf, FecMode, FirmwareVersion, KernelFace, LinkType,
    PeerObservation, SeveredWitness,
};

use crate::itf::{field, int64, state_var, tag};

/// One frozen `world` state as the dpmac observation vocabulary — the record fields the
/// task-2.1 judgments and the task-2.2 edge law read back (`dpmac_lifecycle` var `world`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DpmacWorld {
    /// The judged driver-arbitration phase (`world.arbitration`; DPMAC-I6).
    pub arbitration: Arbitration,
    /// The last endpoint-query evidence the phase is judged from (`world.peer`).
    pub peer: PeerObservation,
    /// The standalone-driver read-back (`world.standaloneBound`).
    pub standalone_bound: bool,
    /// The `macN` netdev presence (`world.macNetdev`).
    pub mac_netdev: bool,
    /// The boot-programmed port MAC (`world.macAddr`; DPMAC-I2).
    pub mac_addr: MacAddr,
    /// The observe-only attribute surface (`world.attributes`; DPMAC-I3).
    pub attributes: DpmacAttributes,
    /// The counter-vocabulary firmware index (`world.firmware`; DPMAC-I7).
    pub firmware: FirmwareVersion,
    /// The facing dpni's kernel-face across teardown (`world.kernelFace`; ADR-0008 §8).
    pub kernel_face: KernelFace,
    /// The edge-sever witness (`world.severed`; ADR-0008 §8).
    pub severed: SeveredWitness,
}

/// One frozen step of a directed dpmac run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DpmacStep {
    /// A state with the `world` var decoded.
    World(DpmacWorld),
    /// A `.fail()` step: the guard was disabled, so quint froze no variable values. The
    /// replayer witnesses the refused action against the prior world (the §8 hazard order).
    Refused,
}

fn arbitration(v: &Value) -> Result<Arbitration, String> {
    match tag(v)? {
        "Offered" => Ok(Arbitration::Offered),
        "KernelOwned" => Ok(Arbitration::KernelOwned),
        "RemoteOwned" => Ok(Arbitration::RemoteOwned),
        other => Err(format!("unknown Arbitration tag `{other}`")),
    }
}

fn peer(v: &Value) -> Result<PeerObservation, String> {
    match tag(v)? {
        "NoPeer" => Ok(PeerObservation::NoPeer),
        "SameContainerKernelPeer" => Ok(PeerObservation::SameContainerKernelPeer),
        "CrossContainerPeer" => Ok(PeerObservation::CrossContainerPeer),
        other => Err(format!("unknown PeerObservation tag `{other}`")),
    }
}

fn firmware(v: &Value) -> Result<FirmwareVersion, String> {
    match tag(v)? {
        "Mc1039" => Ok(FirmwareVersion::Mc1039),
        "Mc1040" => Ok(FirmwareVersion::Mc1040),
        other => Err(format!("unknown FirmwareVersion tag `{other}`")),
    }
}

fn kernel_face(v: &Value) -> Result<KernelFace, String> {
    match tag(v)? {
        "NoKernelDpni" => Ok(KernelFace::NoKernelDpni),
        "KernelFaceBound" => Ok(KernelFace::KernelFaceBound),
        "KernelFaceReleased" => Ok(KernelFace::KernelFaceReleased),
        other => Err(format!("unknown KernelFace tag `{other}`")),
    }
}

fn severed(v: &Value) -> Result<SeveredWitness, String> {
    match tag(v)? {
        "NotSevered" => Ok(SeveredWitness::NotSevered),
        "Severed" => Ok(SeveredWitness::Severed),
        other => Err(format!("unknown SeveredWitness tag `{other}`")),
    }
}

fn link_type(v: &Value) -> Result<LinkType, String> {
    match tag(v)? {
        "PhyManaged" => Ok(LinkType::PhyManaged),
        "Fixed" => Ok(LinkType::Fixed),
        "Backplane" => Ok(LinkType::Backplane),
        other => Err(format!("unknown LinkType tag `{other}`")),
    }
}

fn fec_mode(v: &Value) -> Result<FecMode, String> {
    match tag(v)? {
        "FecNone" => Ok(FecMode::FecNone),
        other => Err(format!("unknown FecMode tag `{other}`")),
    }
}

fn eth_if(v: &Value) -> Result<EthIf, String> {
    match tag(v)? {
        "Usxgmii" => Ok(EthIf::Usxgmii),
        "OtherSerdesProtocol" => Ok(EthIf::OtherSerdesProtocol),
        other => Err(format!("unknown EthIf tag `{other}`")),
    }
}

/// An `int` field as a `u32`, failing a negative or oversized frozen value.
fn u32_field(v: &Value, key: &str) -> Result<u32, String> {
    u32::try_from(int64(field(v, key)?)?).map_err(|e| format!("{key} out of u32 range: {e}"))
}

/// The `macAddr` `List[int]` as the runtime [`MacAddr`] — strict six-byte decode (DPMAC-I2).
fn mac_addr(v: &Value) -> Result<MacAddr, String> {
    let bytes = v.as_array().ok_or("macAddr is not a list")?;
    let octets: Vec<u8> = bytes
        .iter()
        .map(|b| u8::try_from(int64(b)?).map_err(|e| format!("mac byte out of range: {e}")))
        .collect::<Result<_, _>>()?;
    let octets: [u8; 6] = octets
        .try_into()
        .map_err(|v: Vec<u8>| format!("macAddr has {} bytes, not 6", v.len()))?;
    Ok(MacAddr::new(octets))
}

fn attributes(v: &Value) -> Result<DpmacAttributes, String> {
    Ok(DpmacAttributes {
        max_rate: u32_field(v, "maxRate")?,
        link_type: link_type(field(v, "linkType")?)?,
        fec_mode: fec_mode(field(v, "fecMode")?)?,
        serdes_cfg: u32_field(v, "serdesCfg")?,
        eth_if: eth_if(field(v, "ethIf")?)?,
        ipg_length: u16::try_from(int64(field(v, "ipgLength")?)?)
            .map_err(|e| format!("ipgLength out of u16 range: {e}"))?,
    })
}

/// One frozen step as a [`DpmacStep`].
fn dpmac_step(state: &Value) -> Result<DpmacStep, String> {
    // A `.fail()` step froze only `#meta`; state_var finds no `world` and this is the sentinel.
    let Ok(world) = state_var(state, "world") else {
        return Ok(DpmacStep::Refused);
    };
    Ok(DpmacStep::World(DpmacWorld {
        arbitration: arbitration(field(world, "arbitration")?)?,
        peer: peer(field(world, "peer")?)?,
        standalone_bound: field(world, "standaloneBound")?
            .as_bool()
            .ok_or("standaloneBound is not a bool")?,
        mac_netdev: field(world, "macNetdev")?
            .as_bool()
            .ok_or("macNetdev is not a bool")?,
        mac_addr: mac_addr(field(world, "macAddr")?)?,
        attributes: attributes(field(world, "attributes")?)?,
        firmware: firmware(field(world, "firmware")?)?,
        kernel_face: kernel_face(field(world, "kernelFace")?)?,
        severed: severed(field(world, "severed")?)?,
    }))
}

/// Parses a frozen dpmac-lifecycle trace into its per-state [`DpmacStep`]s.
///
/// # Errors
///
/// Returns a description of the first structural mismatch or an unknown sum tag — a trace not
/// produced by `dpmac_lifecycle` over the corpus vocabulary (a model↔core divergence).
pub fn parse_dpmac_trace(json: &str) -> Result<Vec<DpmacStep>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    field(&root, "states")?
        .as_array()
        .ok_or("trace has no states")?
        .iter()
        .map(dpmac_step)
        .collect()
}
