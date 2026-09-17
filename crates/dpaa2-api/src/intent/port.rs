use crate::core::model::{DpmacId, MacAddr, MacMode};
use crate::core::types::ConstructName;
use crate::intent::TenantRef;

/// A port: the dpmac anchor, the rate it must deliver in Mbps (the unit `dpmac
/// info` reports), and the tenant that terminates it (design D1; `types.qnt`
/// `Port`). A port named by a fabric is terminated by the fabric's forwarder — a
/// differing tenant is refused (`PortTenantMismatch`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Port {
    /// The port's name (its construct identity in refusals and provenance).
    pub name: ConstructName,
    /// The stable dpmac anchor.
    pub dpmac: DpmacId,
    /// The rate the port must deliver, in Mbps.
    pub rate: i64,
    /// The tenant reference that terminates the port — the reserved kernel or a
    /// declared tenant ([`TenantRef`], vocabulary-v2 D2). A port that named no
    /// tenant in the TOML is the [`TenantRef::Kernel`] case (the parser's default);
    /// there is no `""` sentinel.
    pub tenant: TenantRef,
    /// The port's known/declared MAC, if any — an actuation-only fact the
    /// derivation never reads (design D9). It rides on the port so
    /// [`compile`](crate::intent::refuse::compile)'s
    /// [`DesiredPort`](crate::core::model::DesiredPort) projection keeps the operator's
    /// MAC intent, but the sizing rules ignore it; the Quint model omits it
    /// deliberately, which is why the model-copy lint does not bind it (ADR-0013 §11).
    pub mac: Option<MacAddr>,
    /// Whether [`mac`](Self::mac) is asserted (verified) or actuated (written) —
    /// likewise an actuation-only fact the derivation never reads (design D9),
    /// carried for the projection and omitted from the model.
    pub mac_mode: MacMode,
    /// An accepted `renamed = { from }` clause — the port's prior name, or `None`
    /// when absent (ADR-0015 decision 10 / task 6.5). For the rename matcher only;
    /// [`compile`](crate::intent::refuse::compile) ignores it.
    pub renamed: Option<ConstructName>,
}
