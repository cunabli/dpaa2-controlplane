//! Per-crypto-block refusal validators: a block's `flows` out of the one-device
//! range (ADR-0018 "Refusals split by construct").

use std::collections::BTreeSet;

use crate::intent::Intent;

use super::Refusal;

/// The queue-pair ceiling of one dpseci device (`models/families/dpseci.qnt`
/// `DPSECI_MAX_QUEUE_NUM`; verified `.build/src/linux/drivers/crypto/caam/dpseci.h:25`
/// and `.build/src/dpdk/.../fsl_dpseci.h:25`): a single object carries at most 16
/// Tx/Rx queue pairs, so a crypto block demanding more `flows` than this cannot be
/// realized by one device (`CryptoFlowsOverDevice`).
const DPSECI_MAX_QUEUE_NUM: i64 = 16;

pub(super) fn crypto_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for (i, k) in intent.crypto.iter().enumerate() {
        let ordinal = u32::try_from(i + 1).unwrap_or(0);
        if k.flows < 1 {
            out.insert(Refusal::CryptoFlowsNotPositive {
                tenant: k.tenant.clone(),
                ordinal,
                flows: k.flows,
            });
        } else if k.flows > DPSECI_MAX_QUEUE_NUM {
            out.insert(Refusal::CryptoFlowsOverDevice {
                tenant: k.tenant.clone(),
                ordinal,
                flows: k.flows,
                max_flows: DPSECI_MAX_QUEUE_NUM,
            });
        }
    }
}
