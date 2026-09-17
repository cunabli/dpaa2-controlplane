//! Per-link refusal validators: the self-loop parity twin (ADR-0018 "Refusals
//! split by construct").

use std::collections::BTreeSet;

use crate::intent::Intent;

use super::Refusal;

/// A link whose two ends resolve to the same tenant (`refuse.qnt`
/// `linkSelfLoopRefusals`). The compile-side twin of the parse self-loop check
/// (`crates/dpaa2-config/src/parse.rs` `convert_link`, "a link joins two distinct
/// tenants"); with [`crate::intent::TenantRef`] two [`crate::intent::TenantRef::Kernel`] ends compare equal too.
/// A refused link never reaches derivation — `compile` returns the refusal set and
/// never hands its plan out. Deliberate config→api duplication (design D11, 2026-09-06-intent-layer).
pub(super) fn link_self_loop_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    for l in &intent.links {
        if l.interface_a == l.interface_b {
            out.insert(Refusal::LinkSelfLoop {
                link: l.name.clone(),
            });
        }
    }
}
