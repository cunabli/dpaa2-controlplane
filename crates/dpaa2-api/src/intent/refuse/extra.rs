//! Per-extra refusal validators: an additive extra on a non-companion family or
//! with a non-positive count (ADR-0018 "Refusals split by construct").

use std::collections::BTreeSet;

use crate::core::family::Family;
use crate::intent::Intent;

use super::Refusal;

pub(super) fn extra_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    let companions = [Family::Dpio, Family::Dpbp, Family::Dpmcp, Family::Dpcon];
    for e in &intent.extras {
        if !companions.contains(&e.family) {
            out.insert(Refusal::ExtraNotCompanion {
                tenant: e.tenant.clone(),
                family: e.family,
            });
        }
        if e.count < 1 {
            out.insert(Refusal::ExtraNotPositive {
                tenant: e.tenant.clone(),
                family: e.family,
                count: e.count,
            });
        }
    }
}
