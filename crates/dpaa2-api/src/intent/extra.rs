use crate::core::family::Family;
use crate::core::types::TenantName;

/// An additive extra (design D5; ADR-0003; `types.qnt` `Extra`): every derived count is a
/// request, a per-`(tenant, family)` extra adds its `count` on top, so the
/// effective count is `request + count` — raise-only by construction. Only the four
/// companion families dpio/dpbp/dpmcp/dpcon accept an extra; any other family is
/// refused (`ExtraNotCompanion`), and `count` must be ≥ 1 (`ExtraNotPositive`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Extra {
    /// The tenant the extra raises a count for.
    pub tenant: TenantName,
    /// The companion family raised.
    pub family: Family,
    /// The count added on top of the request (≥ 1).
    pub count: i64,
}
