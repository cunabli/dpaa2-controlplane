use std::collections::BTreeSet;

use crate::core::types::ConstructName;
use crate::intent::{Crypto, Extra, Fabric, Link, Port, Tenant};

/// The complete intent an operator states (design D1; restool-baseline; `types.qnt` `Intent`).
///
/// Names are identities (ADR-0015 decision 1): design D6 keys every derived object by
/// `(tenant, family, ordinal)`, and as of task 3.3d the ordinal is minted by NAME
/// order, never by a construct's position in the document (ADR-0015 decision 5, the
/// position-independence law — reordering cosmetic blocks never rewires hardware).
/// tenants/ports/links/fabrics stay `Vec`s for a minimal shape, but the derivation
/// sorts them by name (the `derive` module; `derive.qnt`) for ordinal minting and
/// emission order alike, and no longer consumes their document position. `crypto`
/// is the sole exception (ADR-0015 decision 4 / task 2.6e): a `[[crypto]]` block is
/// genuinely anonymous, so declaration order IS the dpseci ordinal and crypto is never
/// sorted. Only `extras` is a set — unordered, matched by `(tenant, family)`,
/// additive, never by position.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Intent {
    /// The declared tenants, in name order — order-free by construction (dprc identity is
    /// keyed by name, not position; the config keys them as `[tenant.<name>]` tables).
    pub tenants: Vec<Tenant>,
    /// The declared ports, in order.
    pub ports: Vec<Port>,
    /// The declared links, in order.
    pub links: Vec<Link>,
    /// The declared fabrics, in name order — order-free by construction (the config keys
    /// them as `[fabric.<name>]` tables).
    pub fabrics: Vec<Fabric>,
    /// The declared crypto blocks, in order.
    pub crypto: Vec<Crypto>,
    /// The additive extras, unordered.
    pub extras: BTreeSet<Extra>,
}

impl Intent {
    /// The ownership-recognition set of ADR-0010 §4 (`edits.qnt` `intentNames`): every
    /// name the control plane recognizes as its own. It is every declared construct
    /// name — tenants, ports, links, fabrics — plus every active `renamed = { from }`
    /// value on those constructs. The `from` inclusion is decision 10's rename
    /// widening: mid-rename a board object still wears the old name, and it must not be
    /// judged foreign while the rename converges. A label absent from this set (and
    /// non-empty) is somebody else's (ADR-0010 §4, refined by ADR-0015). Crypto blocks
    /// are anonymous (ADR-0015 decision 4) and contribute nothing.
    #[must_use]
    pub fn declared_names(&self) -> BTreeSet<ConstructName> {
        let mut names = BTreeSet::new();
        for t in &self.tenants {
            names.insert(ConstructName::from(&t.name));
            if let Some(from) = &t.renamed {
                names.insert(ConstructName::from(from));
            }
        }
        for p in &self.ports {
            names.insert(p.name.clone());
            if let Some(from) = &p.renamed {
                names.insert(from.clone());
            }
        }
        for l in &self.links {
            names.insert(l.name.clone());
            if let Some(from) = &l.renamed {
                names.insert(from.clone());
            }
        }
        for f in &self.fabrics {
            names.insert(f.name.clone());
            if let Some(from) = &f.renamed {
                names.insert(from.clone());
            }
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::{DpmacId, MacMode};
    use crate::intent::{TenantRef, kernel_tenant};

    #[test]
    fn declared_names_include_current_names_and_active_renames() {
        // A small intent with one renamed port: the recognition set carries every
        // declared name and the port's old name (decision 10's widening), so a board
        // object still wearing the old name is not judged foreign mid-rename.
        let intent = Intent {
            tenants: vec![kernel_tenant(4)],
            ports: vec![Port {
                name: "wan1".into(),
                dpmac: DpmacId::new(7),
                rate: 10_000,
                tenant: TenantRef::Kernel,
                mac: None,
                mac_mode: MacMode::default(),
                renamed: Some("wan0".into()),
            }],
            links: vec![Link {
                name: "l0".into(),
                interface_a: TenantRef::Kernel,
                interface_b: TenantRef::Kernel,
                renamed: None,
            }],
            ..Intent::default()
        };
        let names = intent.declared_names();
        assert!(names.contains(&ConstructName::from("kernel")));
        assert!(names.contains(&ConstructName::from("wan1")));
        assert!(names.contains(&ConstructName::from("wan0")));
        assert!(names.contains(&ConstructName::from("l0")));
        assert_eq!(names.len(), 4);
    }
}
