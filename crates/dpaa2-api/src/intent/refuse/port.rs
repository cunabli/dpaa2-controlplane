//! Per-port refusal validators: the dpmac anchor (existence, reservation,
//! foreign ownership, rate) and the double-claim of a dpmac or a member port
//! (ADR-0018 "Refusals split by construct").

use std::collections::BTreeSet;

use crate::core::family::Family;
use crate::core::inventory::{Availability, Inventory};
use crate::core::model::DpmacId;
use crate::core::types::ConstructName;
use crate::intent::derive::port_by_name;
use crate::intent::{Intent, Member};

use super::Refusal;

pub(super) fn anchor_refusals(intent: &Intent, inv: &Inventory, out: &mut BTreeSet<Refusal>) {
    // Ownership is judged against the declared-name recognition set (ADR-0010 §4 as
    // refined by ADR-0015): a dpmac held by an object wearing one of these names is
    // ours, not foreign.
    let declared = intent.declared_names();
    for p in &intent.ports {
        if !inv.dpmacs.contains_key(&p.dpmac) {
            out.insert(Refusal::Unanchored {
                port: p.name.clone(),
                dpmac: p.dpmac,
            });
            continue;
        }
        match inv.availability_of(Family::Dpmac, p.dpmac.into_inner(), &declared) {
            Availability::Reserved(why) => {
                out.insert(Refusal::Reserved {
                    port: p.name.clone(),
                    dpmac: p.dpmac,
                    why,
                });
            }
            Availability::Foreign(owner) => {
                out.insert(Refusal::Foreign {
                    port: p.name.clone(),
                    dpmac: p.dpmac,
                    owner,
                });
            }
            Availability::Free => {
                let max_rate = inv.dpmacs[&p.dpmac].max_rate;
                if p.rate > max_rate {
                    out.insert(Refusal::OverRate {
                        port: p.name.clone(),
                        rate: p.rate,
                        max_rate,
                    });
                }
            }
        }
    }
}

fn ports_on_dpmac(intent: &Intent, d: DpmacId) -> BTreeSet<ConstructName> {
    intent
        .ports
        .iter()
        .filter(|p| p.dpmac == d)
        .map(|p| p.name.clone())
        .collect()
}

fn fabrics_naming_port(intent: &Intent, pn: &ConstructName) -> BTreeSet<ConstructName> {
    intent
        .fabrics
        .iter()
        .filter(|f| {
            f.members
                .iter()
                .any(|m| matches!(m, Member::Port(p) if p == pn))
        })
        .map(|f| f.name.clone())
        .collect()
}

fn member_port_names(intent: &Intent) -> BTreeSet<ConstructName> {
    let mut s = BTreeSet::new();
    for f in &intent.fabrics {
        for m in &f.members {
            if let Member::Port(p) = m {
                s.insert(p.clone());
            }
        }
    }
    s
}

pub(super) fn double_claimed_refusals(intent: &Intent, out: &mut BTreeSet<Refusal>) {
    let claimed: BTreeSet<DpmacId> = intent.ports.iter().map(|p| p.dpmac).collect();
    for d in claimed {
        let ports = ports_on_dpmac(intent, d);
        if ports.len() >= 2 {
            out.insert(Refusal::DoubleClaimed {
                dpmac: d,
                constructs: ports.into_iter().collect(),
            });
        }
    }
    for pn in member_port_names(intent) {
        let fabrics = fabrics_naming_port(intent, &pn);
        if fabrics.len() >= 2 {
            let dpmac = port_by_name(intent, &pn).map_or(DpmacId::new(0), |p| p.dpmac);
            out.insert(Refusal::DoubleClaimed {
                dpmac,
                constructs: fabrics.into_iter().collect(),
            });
        }
    }
}
