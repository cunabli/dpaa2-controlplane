use crate::core::types::TenantName;

/// An accelerator for one tenant (design D1; restool-baseline; `dpseci.md`; `types.qnt` `Crypto`).
///
/// Its dpseci `num_queues` derives from this block's own `flows` — a
/// tenant-visible demand, never an object count. A tenant may declare several
/// blocks; declaration order numbers its dpseci ordinals (task 2.6e), and no
/// ceiling folds a tenant's blocks together.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Crypto {
    /// The owning tenant.
    pub tenant: TenantName,
    /// The flow demand this block sizes its own dpseci to.
    pub flows: i64,
}
