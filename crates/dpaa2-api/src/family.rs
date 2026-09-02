//! The MC object families and DPRC permission bits (object-model.md §3).
//!
//! Transcribed from `models/core/types.qnt` (`Family`, `Permission`): the derived
//! object plan keys every object by `(tenant, family, ordinal)` (design D6), so
//! the family is part of an object's identity, and a child DPRC's create options
//! are a set of these permission bits (`dprc.md`).

use core::fmt;

/// One of the sixteen MC object families (object-model.md §3).
///
/// The full set is transcribed even though the intent derivation emits only
/// [`DERIVED_FAMILIES`]: refusals and provenance name families outside that set
/// (`Dpmac` as a port anchor, `Dprtc` pinned in root), and the plan's
/// containment invariant is stated over the whole space (ADR-0013 §6 `INTENT_I1`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Family {
    /// Resource container (`dprc.N`).
    Dprc,
    /// Network interface (`dpni.N`).
    Dpni,
    /// MAC / `SerDes` lane (`dpmac.N`).
    Dpmac,
    /// Buffer pool (`dpbp.N`).
    Dpbp,
    /// I/O portal (`dpio.N`).
    Dpio,
    /// Concentrator (`dpcon.N`).
    Dpcon,
    /// MC command portal (`dpmcp.N`).
    Dpmcp,
    /// Security/CAAM interface (`dpseci.N`).
    Dpseci,
    /// Ethernet switch (`dpsw.N`).
    Dpsw,
    /// Demux (`dpdmux.N`).
    Dpdmux,
    /// AIOP (`dpaiop.N`).
    Dpaiop,
    /// Communication interface (`dpci.N`).
    Dpci,
    /// Compression/decompression engine (`dpdcei.N`).
    Dpdcei,
    /// DMA interface (`dpdmai.N`).
    Dpdmai,
    /// Real-time clock (`dprtc.N`).
    Dprtc,
    /// Debug object (`dpdbg.N`).
    Dpdbg,
}

impl Family {
    /// The restool type name (`dpni`, `dprc`, …), the prefix a rendered label
    /// carries (ADR-0010: the label is a projection of the key).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dprc => "dprc",
            Self::Dpni => "dpni",
            Self::Dpmac => "dpmac",
            Self::Dpbp => "dpbp",
            Self::Dpio => "dpio",
            Self::Dpcon => "dpcon",
            Self::Dpmcp => "dpmcp",
            Self::Dpseci => "dpseci",
            Self::Dpsw => "dpsw",
            Self::Dpdmux => "dpdmux",
            Self::Dpaiop => "dpaiop",
            Self::Dpci => "dpci",
            Self::Dpdcei => "dpdcei",
            Self::Dpdmai => "dpdmai",
            Self::Dprtc => "dprtc",
            Self::Dpdbg => "dpdbg",
        }
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The families the intent derivation emits (design D4; `types.qnt`
/// `DERIVED_FAMILIES`): the ceilings map's domain and the request/extra
/// families. `Dpmac`, `Dprtc`, `Dpdbg` are outside it — a dpmac is a port
/// anchor, dprtc.0 is pinned rather than sized, dpdbg is never derived.
pub const DERIVED_FAMILIES: [Family; 8] = [
    Family::Dprc,
    Family::Dpni,
    Family::Dpbp,
    Family::Dpio,
    Family::Dpcon,
    Family::Dpmcp,
    Family::Dpseci,
    Family::Dpsw,
];

/// A DPRC permission bit (`models/core/types.qnt` `Permission`; `dprc.md` §1).
///
/// A child DPRC is created with a set of these; which bit gates which operation
/// is an open matrix, so the model carries the whole set as one value.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Permission {
    /// Spawn child containers.
    Spawn,
    /// Allocate pool objects.
    Alloc,
    /// Create and destroy objects.
    ObjCreate,
    /// Configure interrupts.
    IrqCfg,
    /// Change topology (connect/disconnect).
    TopologyChanges,
}
