//! Property tests for `compile` and the witness-built plan surface (design D9;
//! task 3.2). Quint's random simulation over `models/intent/alphabet.qnt` is the
//! oracle; three laws are cheap enough to restate in Rust so the transcription
//! stays honest — `compile` is deterministic, an extra only ever raises a count,
//! every companion precedes the consumer that draws it — plus totality (a total
//! function never panics) and the nine `INTENT_I1..I9` invariants
//! (`models/intent/invariants.qnt`; ADR-0013 §6) as runtime predicates.
//!
//! # Two rungs, mirroring the model
//!
//! The predicates run in two places (requirement 4):
//!
//! - **(a) over every `Ok(compile)` output** — all nine, the model's
//!   `planInvariants` (I1–I6) plus `compileInvariants` (I7–I9).
//! - **(b) over arbitrary WITNESS-BUILT plans** ([`witness_plan`]) — the D11
//!   hand-built surface the ITF replay never sees, guarded by types alone today.
//!   Only the *structural* invariants run here: `intent_i1_containment_by_tenant`,
//!   `intent_i2_edges_typed_and_single`, `intent_i5_keys_are_identities`. The
//!   others (I3/I4/I6) are derivation-sizing / provenance-closure / emission-order
//!   relations only `compile` can guarantee — a hand-built plan does not populate
//!   the provenance DAG or the emission order the way the compiler does, so they
//!   are asserted on rung (a) only (noted at each predicate). If a structural
//!   invariant ever fails on a witness-built plan that is a real finding: it is
//!   left in place, not weakened to pass.
//!
//! # ADR-0014 hygiene
//!
//! The nine predicates are semantic transcriptions of `invariants.qnt`, named
//! `intent_i1_*`..`intent_i9_*`. They are not a new prose/table enumeration of the
//! invariant list: the single enumeration pair the ledger ties is the model ⇄
//! ADR-0013 §6 one (lint R12).

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use dpaa2_api::compiled::{
    AttachPoint, Attributes, CompiledPlan, Container, Edge, Measurement, ObjectKey, ProvenanceKey,
};
use dpaa2_api::inventory::{Availability, Ceiling, DpmacLinkType, DpmacOffer, EthInterface};
use dpaa2_api::{
    Compiled, Crypto, Dataplane, DpmacId, Extra, Fabric, Family, Intent, Inventory, Isolation,
    KERNEL, Link, MacMode, Member, Port, Switching, Tenant, TenantName, compile, kernel_tenant,
};

// ===========================================================================
// helpers copied from models/intent/invariants.qnt
// ===========================================================================

/// The four companion families I3/I4 range over (`invariants.qnt` `COMPANION_FAMS`).
const COMPANION_FAMS: [Family; 4] = [Family::Dpio, Family::Dpbp, Family::Dpmcp, Family::Dpcon];

/// The provenance rule name a companion family keys its node by (`invariants.qnt`
/// `companionRule`): the lowercase family name.
fn companion_rule(f: Family) -> &'static str {
    f.as_str()
}

fn obj_keys(p: &CompiledPlan) -> BTreeSet<ObjectKey> {
    p.objects.iter().map(|o| o.key().clone()).collect()
}

fn count_of(p: &CompiledPlan, tenant: &TenantName, fam: Family) -> i64 {
    let n = p
        .objects
        .iter()
        .filter(|o| &o.key().tenant == tenant && o.key().family == fam)
        .count();
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn ordinals_of(p: &CompiledPlan, tenant: &TenantName, fam: Family) -> BTreeSet<i64> {
    p.objects
        .iter()
        .filter(|o| &o.key().tenant == tenant && o.key().family == fam)
        .map(|o| i64::from(o.key().ordinal))
        .collect()
}

/// 1-based-run set `1.to(value)` (`invariants.qnt`): empty when `value < 1`.
fn one_to(value: i64) -> BTreeSet<i64> {
    (1..=value).collect()
}

/// Position of a key in the emission order, or `-1` if absent (`invariants.qnt`
/// `keyIndex`).
fn key_index(order: &[ObjectKey], k: &ObjectKey) -> i64 {
    order
        .iter()
        .position(|x| x == k)
        .map_or(-1, |i| i64::try_from(i).unwrap_or(-1))
}

fn is_mac(en: &AttachPoint) -> bool {
    matches!(en, AttachPoint::Mac(_))
}

fn is_fam(en: &AttachPoint, f: Family) -> bool {
    matches!(en, AttachPoint::Object { key, .. } if key.family == f)
}

/// An edge matches an unordered pair of end predicates (`invariants.qnt`
/// `edgeMatches`; §2 edges are ↔).
fn edge_matches(
    e: &Edge,
    pa: impl Fn(&AttachPoint) -> bool,
    pb: impl Fn(&AttachPoint) -> bool,
) -> bool {
    (pa(e.a()) && pb(e.b())) || (pa(e.b()) && pb(e.a()))
}

fn all_attach_points(p: &CompiledPlan) -> Vec<AttachPoint> {
    p.edges
        .iter()
        .flat_map(|e| [e.a().clone(), e.b().clone()])
        .collect()
}

fn attach_point_count(p: &CompiledPlan, en: &AttachPoint) -> usize {
    all_attach_points(p).iter().filter(|x| *x == en).count()
}

fn attach_points_set(p: &CompiledPlan) -> BTreeSet<AttachPoint> {
    all_attach_points(p).into_iter().collect()
}

/// An `Object` end is well-formed: its key exists and its port surface is a dpni
/// (port 0) or a dpsw interface within `num_ifs` (`invariants.qnt` `attachPointWellFormed`).
fn attach_point_well_formed(p: &CompiledPlan, en: &AttachPoint) -> bool {
    match en {
        AttachPoint::Mac(_) => true,
        AttachPoint::Object { key, port } => {
            if !obj_keys(p).contains(key) {
                return false;
            }
            match key.family {
                Family::Dpni => *port == 0,
                Family::Dpsw => p.objects.iter().any(|o| {
                    o.key() == key
                        && matches!(o.attributes(), Attributes::Dpsw { num_ifs, .. } if *port < *num_ifs)
                }),
                _ => false,
            }
        }
    }
}

/// The kernel-owned dprtc.0 key, first in the emission order (`derive.qnt`
/// `dprtcKey`).
fn dprtc_key() -> ObjectKey {
    ObjectKey::new(KERNEL, Family::Dprtc, 1)
}

// ===========================================================================
// INTENT_I1..I9 as runtime predicates (invariants.qnt; ADR-0013 §6)
// ===========================================================================

/// `INTENT_I1` containmentByTenant: every object sits in a real container
/// (`invariants.qnt` `containmentByTenant`; object-model.md §1). Structural — runs
/// on rungs (a) and (b).
fn intent_i1_containment_by_tenant(p: &CompiledPlan) -> bool {
    let ks = obj_keys(p);
    p.objects.iter().all(|o| {
        let k = o.key();
        let non_kernel_object = !k.tenant.is_kernel() && k.family != Family::Dprc;
        let contained = if non_kernel_object {
            match o.container() {
                Container::Child(x) => ks.contains(&ObjectKey::new(x.clone(), Family::Dprc, 1)),
                Container::Root => false,
            }
        } else {
            true
        };
        let kernel_in_root = !k.tenant.is_kernel() || *o.container() == Container::Root;
        let dprc_in_root =
            k.family != Family::Dprc || (*o.container() == Container::Root && k.ordinal == 1);
        contained
            && kernel_in_root
            && dprc_in_root
            && k.family != Family::Dpmac
            && k.family != Family::Dpdbg
    })
}

/// `INTENT_I2` edgesTypedAndSingle: typed connect ends, no double connect
/// (`invariants.qnt` `edgesTypedAndSingle`; object-model.md §2). Structural — runs
/// on rungs (a) and (b): this is the D6 lock the witness types enforce.
fn intent_i2_edges_typed_and_single(p: &CompiledPlan) -> bool {
    let single = attach_points_set(p)
        .iter()
        .all(|en| attach_point_count(p, en) <= 1);
    let typed = p.edges.iter().all(|e| {
        let not_mac_mac = !(is_mac(e.a()) && is_mac(e.b()));
        let well_formed = attach_point_well_formed(p, e.a()) && attach_point_well_formed(p, e.b());
        let rule = e.provenance().rule.as_str();
        let link_or_wire = rule != "link-edge" && rule != "fabric-wire"
            || (is_fam(e.a(), Family::Dpni) && is_fam(e.b(), Family::Dpni));
        let port = rule != "port-edge" || edge_matches(e, |en| is_fam(en, Family::Dpni), is_mac);
        let fabric = rule != "fabric-edge"
            || edge_matches(
                e,
                |en| is_fam(en, Family::Dpsw),
                |en| is_mac(en) || is_fam(en, Family::Dpni),
            );
        not_mac_mac && well_formed && link_or_wire && port && fabric
    });
    single && typed
}

/// `INTENT_I3` companionsOnlyDerived: a companion exists only as its tenant's count
/// node, sized to that node's value (`invariants.qnt` `companionsOnlyDerived`;
/// ADR-0012). Rung (a) ONLY: it is a derivation-sizing relation — it demands the
/// provenance DAG carry a node whose value equals the companion count, which the
/// witnesses set on the object's `provenance` pointer but do not populate in
/// `plan.provenance`; only `compile` builds that map.
fn intent_i3_companions_only_derived(p: &CompiledPlan) -> bool {
    p.objects.iter().all(|o| {
        let k = o.key();
        if !COMPANION_FAMS.contains(&k.family) {
            return true;
        }
        let pk = ProvenanceKey::new(k.tenant.clone(), companion_rule(k.family), "");
        let Some(node) = p.provenance.get(&pk) else {
            return false;
        };
        *o.provenance() == pk
            && count_of(p, &k.tenant, k.family) == node.value
            && ordinals_of(p, &k.tenant, k.family) == one_to(node.value)
    })
}

/// `INTENT_I4` emissionOrderLawful: pool companions before the objects that draw
/// them, dprtc.0 first, dpio before dpmcp in the kernel (`invariants.qnt`
/// `emissionOrderLawful`; object-model.md §5). Rung (a) ONLY: emission order is a
/// property of the compiler's constructor sequence — a hand-built plan need not
/// carry dprtc, the full key set, or the draw ordering.
fn intent_i4_emission_order_lawful(p: &CompiledPlan) -> bool {
    let ks = obj_keys(p);
    let order_is_keys =
        p.order.len() == p.objects.len() && p.order.iter().cloned().collect::<BTreeSet<_>>() == ks;
    let starts_dprtc = p.order.first() == Some(&dprtc_key());
    let dprc_first = ks.iter().all(|k1| {
        k1.family != Family::Dprc
            || ks.iter().all(|k2| {
                k2.tenant != k1.tenant
                    || k2 == k1
                    || key_index(&p.order, k1) < key_index(&p.order, k2)
            })
    });
    let consumer_fams = [Family::Dpni, Family::Dpseci, Family::Dpsw];
    let companions_first = ks.iter().all(|k1| {
        !COMPANION_FAMS.contains(&k1.family)
            || ks.iter().all(|k2| {
                !(consumer_fams.contains(&k2.family) && k2.tenant == k1.tenant)
                    || key_index(&p.order, k1) < key_index(&p.order, k2)
            })
    });
    let dpio_before_dpmcp = ks.iter().all(|k1| {
        !(k1.tenant.is_kernel() && k1.family == Family::Dpio)
            || ks.iter().all(|k2| {
                !(k2.tenant.is_kernel() && k2.family == Family::Dpmcp)
                    || key_index(&p.order, k1) < key_index(&p.order, k2)
            })
    });
    order_is_keys && starts_dprtc && dprc_first && companions_first && dpio_before_dpmcp
}

/// `INTENT_I5` keysAreIdentities: no two objects share a key, every ordinal 1-based
/// (`invariants.qnt` `keysAreIdentities`; ADR-0010). Structural — runs on rungs (a)
/// and (b).
fn intent_i5_keys_are_identities(p: &CompiledPlan) -> bool {
    p.objects.len() == obj_keys(p).len() && p.objects.iter().all(|o| o.key().ordinal >= 1)
}

/// `INTENT_I6` provenanceClosed: every provenance reference resolves, every node carries
/// an anchor, only rule "T" is unmeasured (`invariants.qnt` `provenanceClosed`;
/// design D6). Rung (a) ONLY: a hand-built plan does not populate the provenance
/// DAG the witnesses only point into.
fn intent_i6_provenance_closed(p: &CompiledPlan) -> bool {
    let pk: BTreeSet<&ProvenanceKey> = p.provenance.keys().collect();
    let objs_point = p.objects.iter().all(|o| pk.contains(o.provenance()));
    let edges_point = p.edges.iter().all(|e| pk.contains(e.provenance()));
    let inputs_closed = p
        .provenance
        .values()
        .all(|n| n.inputs.iter().all(|i| pk.contains(i)));
    let anchored = p.provenance.values().all(|n| !n.anchor.is_empty());
    let unmeasured_is_t = p
        .provenance
        .values()
        .all(|n| n.mark != Measurement::Unmeasured || n.rule.as_str() == "T");
    objs_point && edges_point && inputs_closed && anchored && unmeasured_is_t
}

/// `INTENT_I7` feasibleAgainstCeilings: an accepted plan is within every checkable
/// ceiling, warning on Unknown (`invariants.qnt` `feasibleAgainstCeilings`;
/// ADR-0011). The refusal-non-empty half is the totality property; here on Ok only.
fn intent_i7_feasible_against_ceilings(c: &Compiled, inv: &Inventory) -> bool {
    dpaa2_api::DERIVED_FAMILIES.into_iter().all(|fam| {
        let count = i64::try_from(
            c.plan
                .objects
                .iter()
                .filter(|o| o.key().family == fam)
                .count(),
        )
        .unwrap_or(i64::MAX);
        match inv.ceilings.get(&fam) {
            Some(Ceiling::Counted(n) | Ceiling::Observed { n, .. }) => count <= *n,
            _ => {
                count <= 0
                    || c.warnings.contains(&dpaa2_api::Warning::UnknownCeiling {
                        family: fam,
                        needed: count,
                    })
            }
        }
    })
}

/// The additive extra declared for `(tenant, family)`, 0 when none (`derive.qnt`
/// `extraOf`).
fn extra_of(intent: &Intent, tenant: &TenantName, fam: Family) -> i64 {
    intent
        .extras
        .iter()
        .filter(|e| &e.tenant == tenant && e.family == fam)
        .map(|e| e.count)
        .sum()
}

/// `INTENT_I8` companionCountsByRegime: every companion count is its effective
/// value (request + declared extra), the dpio request is companionDraw's, dpnis
/// carry `cpus`/`>= T` queues, and objects sit in the tenant's container
/// (`invariants.qnt` `companionCountsByRegime`; ADR-0012). Rung (a) only.
///
/// The `request == companionDraw` leg the model asserts over `sizeTenant`'s private
/// output is transcribed here for the dpio family — the one leg `invariants.qnt`
/// itself singles out (`s.effectiveDpio.request == reqDraw.dpio`) — against the public
/// `T`/`cpus` provenance nodes. The dpbp/dpmcp/dpcon *request formulas* (their
/// dpsw/dpseci census rows) are the derivation's internal sizing, pinned directly by
/// the crate's own reference-board unit tests (`refuse.rs`); this predicate checks
/// their count/value/extra consistency, not their re-derivation.
fn intent_i8_companion_counts_by_regime(c: &Compiled, intent: &Intent) -> bool {
    let p = &c.plan;
    intent.tenants.iter().all(|t| {
        let is_kernel = t.dataplane == Dataplane::KernelNetlink;
        let feeder = if is_kernel { "cpus" } else { "T" };
        let Some(feeder_node) = p
            .provenance
            .get(&ProvenanceKey::new(t.name.clone(), feeder, ""))
        else {
            return false;
        };
        let feeder_val = feeder_node.value;

        // every companion count is exactly request + extra (raise-only, design D5)
        let companions_ok = COMPANION_FAMS.into_iter().all(|fam| {
            let Some(node) =
                p.provenance
                    .get(&ProvenanceKey::new(t.name.clone(), fam.as_str(), ""))
            else {
                return false;
            };
            let ev = extra_of(intent, &t.name, fam);
            count_of(p, &t.name, fam) == node.value
                && node.value == node.request + ev
                && node.extra == (if ev == 0 { None } else { Some(ev) })
        });

        // the dpio request is companionDraw's draw (invariants.qnt reqDraw.dpio)
        let dpio_req = p
            .provenance
            .get(&ProvenanceKey::new(t.name.clone(), "dpio", ""))
            .map(|n| n.request);
        let dpio_ok = dpio_req.is_some_and(|req| {
            if t.name.is_kernel() {
                req == feeder_val // root kernel: one dpio per online CPU
            } else if is_kernel {
                req == 0 // child-resident namespace: zero extra dpio
            } else {
                req == 2 * feeder_val // poll-mode: 2·T
            }
        });

        // dpni queues, and every object of the tenant in the tenant's container
        let container = t.container();
        let placement_ok = p.objects.iter().all(|o| {
            if o.key().tenant != t.name {
                return true;
            }
            let queues_ok = o.key().family != Family::Dpni
                || match o.attributes() {
                    Attributes::Dpni { num_queues } => {
                        let q = i64::from(*num_queues);
                        if is_kernel {
                            q == feeder_val
                        } else {
                            q >= feeder_val
                        }
                    }
                    _ => false,
                };
            let container_ok = o.key().family == Family::Dprc || *o.container() == container;
            queues_ok && container_ok
        });

        companions_ok && dpio_ok && placement_ok
    })
}

/// `INTENT_I9` isolatedContainerPrivate: an Isolated tenant's container is
/// sole-tenant (`invariants.qnt` `isolatedContainerPrivate`; task 2.6c). Rung (a)
/// only.
fn intent_i9_isolated_container_private(c: &Compiled, intent: &Intent) -> bool {
    let p = &c.plan;
    intent.tenants.iter().all(|t| {
        if t.isolation != Isolation::Isolated {
            return true;
        }
        let child = Container::Child(t.name.clone());
        let own_objects_here = p.objects.iter().all(|o| {
            o.key().tenant != t.name || o.key().family == Family::Dprc || *o.container() == child
        });
        let no_foreign_here = p
            .objects
            .iter()
            .all(|o| *o.container() != child || o.key().tenant == t.name);
        own_objects_here && no_foreign_here
    })
}

/// Runs every plan-only invariant (I1–I6) over an `Ok(compile)` plan.
fn all_plan_invariants(p: &CompiledPlan) -> bool {
    intent_i1_containment_by_tenant(p)
        && intent_i2_edges_typed_and_single(p)
        && intent_i3_companions_only_derived(p)
        && intent_i4_emission_order_lawful(p)
        && intent_i5_keys_are_identities(p)
        && intent_i6_provenance_closed(p)
}

// ===========================================================================
// the reference inventory (invariants.qnt REF_INVENTORY / refuse.rs ref_inv)
// ===========================================================================

const RESERVED_3: &str = "ADR-0003 §3: wired to a peer that must never see traffic (total-deny)";

fn offer(id: u32, rate: i64, avail: Availability) -> (DpmacId, DpmacOffer) {
    let d = DpmacId::new(id);
    (
        d,
        DpmacOffer {
            id: d,
            max_rate: rate,
            eth_if: EthInterface::Xfi,
            link_type: DpmacLinkType::Phy,
            avail,
        },
    )
}

/// The reference board inventory with a variable online-CPU count (the one axis the
/// alphabet's `REF_INVENTORY` fixes; varied here within the alphabet's `CORES`
/// bounds to exercise the kernel per-CPU draw).
fn ref_inventory(cpus: u32) -> Inventory {
    let dpmacs = BTreeMap::from([
        offer(3, 25_000, Availability::Reserved(RESERVED_3.to_owned())),
        offer(4, 25_000, Availability::Free),
        offer(5, 25_000, Availability::Free),
        offer(6, 25_000, Availability::Free),
        offer(7, 10_000, Availability::Free),
        offer(8, 10_000, Availability::Free),
        offer(9, 10_000, Availability::Free),
        offer(10, 10_000, Availability::Free),
        offer(
            17,
            1_000,
            Availability::Reserved("ADR-0003 §3: management plane (dpni.0)".to_owned()),
        ),
    ]);
    let ceilings = BTreeMap::from([
        (Family::Dprc, Ceiling::Unknown),
        (
            Family::Dpni,
            Ceiling::Observed {
                n: 18,
                provenance: "ADR-0011 decision 2".to_owned(),
            },
        ),
        (Family::Dpbp, Ceiling::Counted(63)),
        (Family::Dpio, Ceiling::Unknown),
        (Family::Dpcon, Ceiling::Unknown),
        (
            Family::Dpmcp,
            Ceiling::Observed {
                n: 203,
                provenance: "ADR-0011 decision 3".to_owned(),
            },
        ),
        (Family::Dpseci, Ceiling::Unknown),
        (Family::Dpsw, Ceiling::Unknown),
    ]);
    Inventory {
        cpus,
        dpmacs,
        foreign: BTreeMap::from([((Family::Dpni, 0), "dpl".to_owned())]),
        ceilings,
    }
}

// ===========================================================================
// strategies over the finite intent alphabet (models/intent/alphabet.qnt)
// ===========================================================================

/// The name pool a construct's owner is drawn from: the reserved kernel and the two
/// non-kernel tenant names (`alphabet.qnt` `TENANT_NAMES` + the kernel). A reference
/// to `c1`/`c2` when the tenant was not generated is a legal `TenantAbsent` draw.
fn name() -> impl Strategy<Value = TenantName> {
    prop::sample::select(vec![KERNEL, "c1", "c2"]).prop_map(TenantName::from)
}

/// The pool a tenant may name (`alphabet.qnt` `POOL_NAMES`).
fn pool_name() -> impl Strategy<Value = TenantName> {
    prop::sample::select(vec!["", KERNEL, "c1", "c2"]).prop_map(TenantName::from)
}

/// The finite member lists (`alphabet.qnt` `MEMBER_LISTS`).
fn member_list() -> impl Strategy<Value = Vec<Member>> {
    let lists = vec![
        vec![Member::Port("p1".into())],
        vec![Member::Port("p1".into()), Member::Port("p2".into())],
        vec![Member::Port("p1".into()), Member::Tenant("c1".into())],
        vec![Member::Tenant("c1".into()), Member::Tenant("c2".into())],
        vec![Member::Port("p2".into()), Member::Fabric("f1".into())],
        vec![Member::Tenant(KERNEL.into())],
        vec![Member::Port("p3".into()), Member::Tenant(KERNEL.into())],
        vec![],
    ];
    prop::sample::select(lists)
}

/// A userspace tenant (`alphabet.qnt` `addTenant`): dataplane from
/// `USERSPACE_DATAPLANES`, isolation from `ISOLATIONS`, pool from `POOL_NAMES`,
/// `max_cores` from `CORES`. Named `c1`/`c2` by position at assembly.
fn userspace_tenant() -> impl Strategy<Value = (Dataplane, Isolation, i64, TenantName)> {
    (
        prop::sample::select(vec![Dataplane::UserspacePoll, Dataplane::UserspaceEvent]),
        prop::sample::select(vec![
            Isolation::Public,
            Isolation::Restricted,
            Isolation::Isolated,
        ]),
        prop::sample::select(vec![1i64, 4, 5, 8, 16]),
        pool_name(),
    )
}

fn dpmac() -> impl Strategy<Value = DpmacId> {
    prop::sample::select(vec![3u32, 4, 7, 8, 9, 17, 99]).prop_map(DpmacId::new)
}

fn rate() -> impl Strategy<Value = i64> {
    prop::sample::select(vec![10_000i64, 25_000, 40_000])
}

/// `(Intent, Inventory)` drawn from the alphabet's bounds: the kernel is always
/// declared (`alphabet.qnt` `init`), 0–2 userspace tenants, ≤3 ports, ≤1 link, ≤1
/// fabric, ≤2 crypto blocks, ≤2 extras, and the reference inventory with a CPU count
/// from `CORES`.
fn intent_and_inventory() -> impl Strategy<Value = (Intent, Inventory)> {
    let tenants = prop::collection::vec(userspace_tenant(), 0..=2);
    let ports = prop::collection::vec((dpmac(), rate(), name()), 0..=3);
    let links = prop::collection::vec((name(), name()), 0..=1);
    let fabrics = prop::collection::vec(
        (
            prop::sample::select(vec![Switching::Hardware, Switching::Software]),
            name(),
            member_list(),
        ),
        0..=1,
    );
    let crypto = prop::collection::vec((name(), prop::sample::select(vec![0i64, 1, 4, 17])), 0..=2);
    let extras = prop::collection::vec(
        (
            name(),
            prop::sample::select(vec![
                Family::Dpio,
                Family::Dpbp,
                Family::Dpmcp,
                Family::Dpcon,
                Family::Dpni,
            ]),
            prop::sample::select(vec![0i64, 1, 2, 4]),
        ),
        0..=2,
    );
    let cpus = prop::sample::select(vec![1u32, 4, 8, 16]);

    (tenants, ports, links, fabrics, crypto, extras, cpus).prop_map(
        |(tenants, ports, links, fabrics, crypto, extras, cpus)| {
            let names = ["c1", "c2"];
            let mut ts = vec![kernel_tenant(16)];
            for (i, (dataplane, isolation, max_cores, pool)) in tenants.into_iter().enumerate() {
                ts.push(Tenant {
                    name: names[i].into(),
                    dataplane,
                    max_cores,
                    isolation,
                    pool,
                });
            }
            let ports = ports
                .into_iter()
                .enumerate()
                .map(|(i, (dpmac, rate, tenant))| Port {
                    name: format!("p{}", i + 1).into(),
                    dpmac,
                    rate,
                    tenant,
                    mac: None,
                    mac_mode: MacMode::default(),
                })
                .collect();
            let links = links
                .into_iter()
                .map(|(a, b)| Link {
                    name: "l1".into(),
                    interface_a: a,
                    interface_b: b,
                })
                .collect();
            let fabrics = fabrics
                .into_iter()
                .map(|(switching, forwarded_by, members)| Fabric {
                    name: "f1".into(),
                    switching,
                    forwarded_by,
                    members,
                })
                .collect();
            let crypto = crypto
                .into_iter()
                .map(|(tenant, flows)| Crypto { tenant, flows })
                .collect();
            let extras = extras
                .into_iter()
                .map(|(tenant, family, count)| Extra {
                    tenant,
                    family,
                    count,
                })
                .collect();
            let intent = Intent {
                tenants: ts,
                ports,
                links,
                fabrics,
                crypto,
                extras,
            };
            (intent, ref_inventory(cpus))
        },
    )
}

// ===========================================================================
// the witness-built plan strategy (design D11: the hand-built surface)
// ===========================================================================

/// Per-tenant object counts a witness-built plan draws: companions (dpio, dpbp,
/// dpmcp, dpcon), port-terminated dpnis, unconnected dpnis, dpsecis.
type Counts = (u32, u32, u32, u32, u32, u32, u32);

fn counts() -> impl Strategy<Value = Counts> {
    (
        0u32..=3,
        0u32..=3,
        0u32..=3,
        0u32..=3,
        0u32..=2,
        0u32..=2,
        0u32..=2,
    )
}

/// A named tenant spec for the witness plan: kernel-netlink or poll, public or
/// isolated (pool omitted — a restricted drawer's container is its holder's, out of
/// this coherent-builder's scope; noted). Kernel is always present separately.
fn named_tenant() -> impl Strategy<Value = (bool, bool, Counts)> {
    (any::<bool>(), any::<bool>(), counts())
}

/// Assembles a coherent witness-built [`CompiledPlan`] from public constructors only
/// (`Tenant::child_dprc`/`companion`/`dpni`/`dpseci`, `Port::terminate`,
/// `Link::wire`) — the design-D11 surface a library user drives without an `Intent`.
/// It emits a child dprc for every non-kernel tenant, gives each dpni a single edge
/// role (a port-edge to a unique dpmac, one wire end, or none) and each object a
/// distinct key, so the structural invariants I1/I2/I5 are the type-level guarantees
/// under test. dprtc.0 leads, mirroring the compiler.
#[allow(clippy::too_many_lines)]
fn build_witness_plan(
    kernel_counts: Counts,
    named: Vec<(bool, bool, Counts)>,
    wires: Vec<(usize, usize)>,
) -> CompiledPlan {
    // The tenant set: kernel first, then the named tenants c-w1, c-w2, ...
    let mut tenants = vec![kernel_tenant(16)];
    let mut specs = vec![kernel_counts];
    for (i, (is_kernel_netlink, is_public, c)) in named.into_iter().enumerate() {
        tenants.push(Tenant {
            name: format!("w{}", i + 1).into(),
            dataplane: if is_kernel_netlink {
                Dataplane::KernelNetlink
            } else {
                Dataplane::UserspacePoll
            },
            max_cores: 16,
            isolation: if is_public {
                Isolation::Public
            } else {
                Isolation::Isolated
            },
            pool: "".into(),
        });
        specs.push(c);
    }

    let mut plan = CompiledPlan::default();
    let mut next_dpni: BTreeMap<TenantName, u32> = BTreeMap::new();
    let mut next_dpmac: u32 = 100;

    let push = |plan: &mut CompiledPlan, obj: dpaa2_api::PlannedObject| {
        plan.order.push(obj.key().clone());
        plan.objects.insert(obj);
    };

    // dprtc.0 leads (kernel-owned, Root).
    push(&mut plan, tenants[0].companion(Family::Dprtc, 1));

    // A child dprc marker for every non-kernel tenant, so I1's containment holds.
    for t in tenants.iter().skip(1) {
        push(&mut plan, t.child_dprc());
    }

    for (t, &(dpio, dpbp, dpmcp, dpcon, n_port, n_free, n_dpseci)) in tenants.iter().zip(&specs) {
        for (fam, n) in [
            (Family::Dpio, dpio),
            (Family::Dpbp, dpbp),
            (Family::Dpmcp, dpmcp),
            (Family::Dpcon, dpcon),
        ] {
            for ord in 1..=n {
                push(&mut plan, t.companion(fam, ord));
            }
        }
        // port-terminated dpnis: each a unique dpni ordinal and a unique dpmac.
        for _ in 0..n_port {
            let ord = *next_dpni.entry(t.name.clone()).or_insert(1);
            next_dpni.insert(t.name.clone(), ord + 1);
            let port = Port {
                name: format!("{}-p{ord}", t.name).into(),
                dpmac: DpmacId::new(next_dpmac),
                rate: 10_000,
                tenant: t.name.clone(),
                mac: None,
                mac_mode: MacMode::default(),
            };
            next_dpmac += 1;
            let (obj, edge) = port.terminate(t, ord, 1);
            push(&mut plan, obj);
            plan.edges.insert(edge);
        }
        // unconnected dpnis.
        for _ in 0..n_free {
            let ord = *next_dpni.entry(t.name.clone()).or_insert(1);
            next_dpni.insert(t.name.clone(), ord + 1);
            let (obj, _iface) = t.dpni(ord, 1);
            push(&mut plan, obj);
        }
        for ord in 1..=n_dpseci {
            push(&mut plan, t.dpseci(ord, 1));
        }
    }

    // link wires: each end is a fresh dpni, so no interface is ever double-connected.
    for (a, b) in wires {
        let ia = a % tenants.len();
        let ib = b % tenants.len();
        let ta = &tenants[ia];
        let tb = &tenants[ib];
        let ord_left = *next_dpni.entry(ta.name.clone()).or_insert(1);
        next_dpni.insert(ta.name.clone(), ord_left + 1);
        let ord_right = *next_dpni.entry(tb.name.clone()).or_insert(1);
        next_dpni.insert(tb.name.clone(), ord_right + 1);
        let (oa, iface_a) = ta.dpni(ord_left, 1);
        let (ob, iface_b) = tb.dpni(ord_right, 1);
        push(&mut plan, oa);
        push(&mut plan, ob);
        let link = Link {
            name: "w".into(),
            interface_a: ta.name.clone(),
            interface_b: tb.name.clone(),
        };
        plan.edges.insert(link.wire(iface_a, iface_b));
    }

    plan
}

fn witness_plan() -> impl Strategy<Value = CompiledPlan> {
    (
        counts(),
        prop::collection::vec(named_tenant(), 0..=2),
        prop::collection::vec((0usize..3, 0usize..3), 0..=3),
    )
        .prop_map(|(k, named, wires)| build_witness_plan(k, named, wires))
}

// ===========================================================================
// the properties (design D9)
// ===========================================================================

proptest! {
    // D9 law 1 — `compile` is deterministic: the same inputs give the same output,
    // structurally. `extras` is a set, so it carries no order to be sensitive to;
    // every other field is a Vec whose order IS the ordinal source (design D6), so
    // determinism is the pure function returning equal results across calls.
    #[test]
    fn compile_is_deterministic((intent, inv) in intent_and_inventory()) {
        prop_assert_eq!(compile(&intent, &inv), compile(&intent, &inv));
    }

    // D9 law 4 — `compile` is total: for EVERY generated intent it returns Ok or a
    // NON-EMPTY refusal set, never a panic (proptest catches the panic). On Ok, all
    // nine INTENT_I* invariants hold (rung (a)).
    #[test]
    fn compile_is_total_and_invariants_hold((intent, inv) in intent_and_inventory()) {
        match compile(&intent, &inv) {
            Ok(c) => {
                prop_assert!(all_plan_invariants(&c.plan), "I1-I6 on {:?}", intent);
                prop_assert!(intent_i7_feasible_against_ceilings(&c, &inv), "I7 on {:?}", intent);
                prop_assert!(intent_i8_companion_counts_by_regime(&c, &intent), "I8 on {:?}", intent);
                prop_assert!(intent_i9_isolated_container_private(&c, &intent), "I9 on {:?}", intent);
            }
            Err(refusals) => prop_assert!(!refusals.is_empty(), "empty refusal on {:?}", intent),
        }
    }

    // D9 law 3 — companion-before-tenant: in the emission order every companion of a
    // tenant precedes that tenant's consumer objects (dpni/dpseci/dpsw). This is the
    // draw-ordering leg of I4, restated as its own law (object-model.md §5).
    #[test]
    fn companions_precede_consumers((intent, inv) in intent_and_inventory()) {
        let Ok(c) = compile(&intent, &inv) else { return Ok(()); };
        let p = &c.plan;
        let consumer_fams = [Family::Dpni, Family::Dpseci, Family::Dpsw];
        for comp in p.objects.iter().filter(|o| COMPANION_FAMS.contains(&o.key().family)) {
            for cons in p.objects.iter().filter(|o| {
                consumer_fams.contains(&o.key().family) && o.key().tenant == comp.key().tenant
            }) {
                prop_assert!(
                    key_index(&p.order, comp.key()) < key_index(&p.order, cons.key()),
                    "{:?} must precede {:?}", comp.key(), cons.key()
                );
            }
        }
    }

    // D9 law 2 — an extra only ever raises a count: adding a legal `(kernel, Dpbp)`
    // extra to a compiling intent yields the SAME plan except that one family's
    // count is raised by exactly `count`, its provenance value updated, and
    // everything else identical. Dpbp is a companion (a legal extra family) with a
    // Counted ceiling, so — unlike an Unknown-ceiling family, whose `UnknownCeiling`
    // warning carries the very count being raised — raising it perturbs no warning.
    // A raise that crosses the Dpbp ceiling refuses (Infeasible) rather than raising;
    // that is a legal outcome, so the sample is skipped, not asserted.
    #[test]
    fn an_extra_only_raises_a_count(
        (intent, inv) in intent_and_inventory(),
        count in 1i64..=4,
    ) {
        let Ok(base) = compile(&intent, &inv) else { return Ok(()); };

        let mut raised_intent = intent.clone();
        raised_intent.extras.insert(Extra {
            tenant: KERNEL.into(),
            family: Family::Dpbp,
            count,
        });
        // A raise past the Dpbp ceiling is a legal refusal, not a plan — skip it.
        let Ok(raised) = compile(&raised_intent, &inv) else { return Ok(()); };

        // exactly `count` more (kernel, Dpbp) objects; nothing else moved.
        let base_dpbp = count_of(&base.plan, &TenantName::from(KERNEL), Family::Dpbp);
        let raised_dpbp = count_of(&raised.plan, &TenantName::from(KERNEL), Family::Dpbp);
        prop_assert_eq!(raised_dpbp, base_dpbp + count);

        let strip = |p: &CompiledPlan| -> BTreeSet<ObjectKey> {
            p.objects
                .iter()
                .map(dpaa2_api::PlannedObject::key)
                .filter(|k| !(k.tenant.is_kernel() && k.family == Family::Dpbp))
                .cloned()
                .collect()
        };
        prop_assert_eq!(strip(&base.plan), strip(&raised.plan), "only Dpbp keys change");
        prop_assert_eq!(&base.plan.edges, &raised.plan.edges, "edges unchanged");
        prop_assert_eq!(&base.warnings, &raised.warnings, "warnings unchanged");

        // provenance identical except the one (kernel, dpbp, "") node's value/extra.
        let dpbp_key = ProvenanceKey::new(KERNEL, "dpbp", "");
        for (k, v) in &base.plan.provenance {
            if *k == dpbp_key {
                continue;
            }
            prop_assert_eq!(Some(v), raised.plan.provenance.get(k), "node {:?} changed", k);
        }
        let base_node = &base.plan.provenance[&dpbp_key];
        let raised_node = &raised.plan.provenance[&dpbp_key];
        prop_assert_eq!(raised_node.value, base_node.value + count, "dpbp value raised by count");
        prop_assert_eq!(raised_node.request, base_node.request, "request unchanged");
    }

    // Rung (b) — the structural invariants over arbitrary WITNESS-BUILT plans (design
    // D11), the hand-built surface the ITF replay never sees. I1/I2/I5 are the
    // type-level guarantees the witness constructors make; I3/I4/I6 are compile-only
    // (see their doc comments) and are not asserted here.
    #[test]
    fn witness_built_plans_hold_structural_invariants(plan in witness_plan()) {
        prop_assert!(intent_i1_containment_by_tenant(&plan), "I1 on witness plan");
        prop_assert!(intent_i2_edges_typed_and_single(&plan), "I2 on witness plan");
        prop_assert!(intent_i5_keys_are_identities(&plan), "I5 on witness plan");
    }
}
