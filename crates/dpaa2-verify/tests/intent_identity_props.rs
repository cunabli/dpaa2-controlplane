//! The identity-across-time laws of `models/intent/match.qnt` as PROPERTY tests over
//! generated real construct names (ADR-0015 decisions 8-13). The model's `edits.qnt`
//! sweep pins the four laws over an abstract 3-name pool with abstract `int` configs;
//! this suite is the red Rust twin ADR-0015 names ("the Rust pairing/property twin runs
//! the same relation on real strings"): it draws valid [`ConstructName`]s and real
//! [`ConfigFacet`]s and asserts `convergeIdempotent`, `frameLaw`, `renameSelfNeutralizes`,
//! and the `swapCorrect` class over them.
//!
//! Names are generated valid by construction (ADR-0015 decision 13): a lowercase letter
//! then 0..=14 of `[a-z0-9-]`, so lengths 1..=15 and — crucially — with this charset a
//! `.` is impossible, so the reserved `family.N` handle pattern is unmatchable and every
//! generated name is a legal interface name with no rejection path to model.

use std::collections::BTreeSet;

use dpaa2_api::{
    BoardObject, Class, ConfigFacet, ConstructName, DpmacId, Family, MatchObject, MatchVerdict,
    TenantName, converge_match, converge_match_class, match_board,
};
use proptest::prelude::*;

// ---- the model's cone helpers, reimplemented locally (matcher.rs test-local twins) ----

/// The board objects OUTSIDE the touched cone (`observed.qnt` `outsideCone`): those whose
/// label is not a name the edit touches. This is the LABEL-SINGLETON cone — a `None`
/// label (the model's `""`) is never in any cone, so a drifted object always stays
/// outside — NOT any derived-object closure (mirrors `matcher.rs`'s `outside_cone`).
fn outside_cone(
    board: &BTreeSet<BoardObject>,
    touched: &BTreeSet<ConstructName>,
) -> BTreeSet<BoardObject> {
    board
        .iter()
        .filter(|o| o.label.as_ref().is_none_or(|l| !touched.contains(l)))
        .cloned()
        .collect()
}

/// The frame law (`match.qnt` `frameLaw`, decision 12): the objects outside the touched
/// cone are byte-identical across a converge.
fn frame_law(
    pre: &BTreeSet<BoardObject>,
    post: &BTreeSet<BoardObject>,
    touched: &BTreeSet<ConstructName>,
) -> bool {
    outside_cone(pre, touched) == outside_cone(post, touched)
}

// ---- construct helpers ----

fn cname(s: &str) -> ConstructName {
    ConstructName::from(s)
}

/// A link facet with far-end tenant `cfgN` — the real projection of the model's abstract
/// unanchored config int (matcher.rs test idiom: two distinct ends stand in for 0/1).
fn link(far: &str) -> ConfigFacet {
    ConfigFacet::Link {
        ends: (TenantName::from("kernel"), TenantName::from(far)),
    }
}

fn unanchored(name: &str, far: &str, from: Option<&str>) -> MatchObject {
    MatchObject {
        family: Family::Dpni,
        name: cname(name),
        anchor: BTreeSet::new(),
        config: link(far),
        from: from.map(cname),
    }
}

/// The board a synced converge leaves from an empty board. A base converge cannot refuse
/// here (no drift ⇒ no ambiguity; `edits.qnt` lines 35-41 record ambiguity as
/// directed-only), so this is `converge_match`; the cheap never-fired guard that a refused
/// converge changes nothing lives at each call site.
fn synced(base: &BTreeSet<MatchObject>) -> BTreeSet<BoardObject> {
    converge_match(base, &BTreeSet::new())
}

/// Fresh-name generator: a valid interface name by construction (decision 13).
fn a_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,14}"
}

prop_compose! {
    /// A scene: 1..=3 unanchored links (distinct generated names, far end drawn from the
    /// tiny pool {cfg0, cfg1} so equal and unequal configs both occur, no `from`) plus
    /// 0..=2 anchored ports (distinct dpmac anchors, [`ConfigFacet::Anchored`]).
    fn scene()(
        names in prop::collection::hash_set(a_name(), 1..=5),
        n_un in 1usize..=3,
        cfgs in prop::collection::vec(any::<bool>(), 3),
    ) -> BTreeSet<MatchObject> {
        let mut names: Vec<String> = names.into_iter().collect();
        names.sort();
        let n_un = n_un.min(names.len());
        let mut out = BTreeSet::new();
        for (i, nm) in names.iter().take(n_un).enumerate() {
            let far = if cfgs[i % cfgs.len()] { "cfg1" } else { "cfg0" };
            out.insert(unanchored(nm, far, None));
        }
        for (j, nm) in names.iter().skip(n_un).take(2).enumerate() {
            out.insert(MatchObject {
                family: Family::Dpni,
                name: cname(nm),
                anchor: BTreeSet::from([DpmacId::new(7 + u32::try_from(j).unwrap())]),
                config: ConfigFacet::Anchored,
                from: None,
            });
        }
        out
    }
}

/// The cheap, never-fired guard: were the base converge to refuse it would leave the
/// empty board untouched (`match.qnt` `converge`'s refused arm). Unreachable here.
fn assert_base_not_refused(base: &BTreeSet<MatchObject>, pre: &BTreeSet<BoardObject>) -> bool {
    !matches!(
        match_board(&BTreeSet::new(), base),
        MatchVerdict::Refused(_)
    ) || pre.is_empty()
}

fn names_of(base: &BTreeSet<MatchObject>) -> BTreeSet<ConstructName> {
    base.iter().map(|c| c.name.clone()).collect()
}

fn unanchored_names(base: &BTreeSet<MatchObject>) -> Vec<ConstructName> {
    base.iter()
        .filter(|c| c.anchor.is_empty())
        .map(|c| c.name.clone())
        .collect()
}

/// The far-end pool member of an unanchored object, for a config flip.
fn far_of(c: &MatchObject) -> Option<&TenantName> {
    match &c.config {
        ConfigFacet::Link { ends } => Some(&ends.1),
        _ => None,
    }
}

proptest! {
    /// convergeIdempotent (`match.qnt`, decisions 9-11): a second converge is a no-op.
    #[test]
    fn converge_is_idempotent(base in scene()) {
        let pre = synced(&base);
        prop_assert!(assert_base_not_refused(&base, &pre));
        prop_assert_eq!(converge_match(&base, &pre), pre);
    }

    /// frameLaw (`match.qnt`, decision 12) over each single edit of the alphabet: an edit
    /// touching only its cone perturbs no object outside it (byte-identical outside).
    #[test]
    fn frame_law_over_single_edits(
        base in scene(),
        fresh in a_name(),
        which in 0u8..5,
        sel in any::<prop::sample::Index>(),
    ) {
        let pre = synced(&base);
        prop_assert!(assert_base_not_refused(&base, &pre));
        let existing = names_of(&base);
        let fresh = cname(&fresh);

        let (post, touched) = match which {
            // Add a fresh unanchored construct — cone {name}.
            0 => {
                prop_assume!(!existing.contains(&fresh));
                let mut edited = base.clone();
                edited.insert(unanchored(fresh.as_str(), "cfg0", None));
                let touched = BTreeSet::from([fresh.clone()]);
                (converge_match(&edited, &pre), touched)
            }
            // Remove one construct — cone {name}.
            1 => {
                let victim = (*sel.get(&base.iter().collect::<Vec<_>>())).clone();
                let mut edited = base.clone();
                edited.remove(&victim);
                let touched = BTreeSet::from([victim.name.clone()]);
                (converge_match(&edited, &pre), touched)
            }
            // Flip one unanchored construct's config — cone {name}.
            2 => {
                let uns: Vec<&MatchObject> =
                    base.iter().filter(|c| c.anchor.is_empty()).collect();
                let victim = sel.get(&uns);
                let far = if far_of(victim).is_some_and(|f| f.as_str() == "cfg0") {
                    "cfg1"
                } else {
                    "cfg0"
                };
                let mut edited = base.clone();
                edited.remove(*victim);
                edited.insert(unanchored(victim.name.as_str(), far, None));
                let touched = BTreeSet::from([victim.name.clone()]);
                (converge_match(&edited, &pre), touched)
            }
            // Rename one construct to a fresh name, stamping `from` — cone {old, new}.
            3 => {
                prop_assume!(!existing.contains(&fresh));
                let victim = (*sel.get(&base.iter().collect::<Vec<_>>())).clone();
                let mut renamed = victim.clone();
                renamed.name = fresh.clone();
                renamed.from = Some(victim.name.clone());
                let mut edited = base.clone();
                edited.remove(&victim);
                edited.insert(renamed);
                let touched = BTreeSet::from([victim.name.clone(), fresh.clone()]);
                (converge_match(&edited, &pre), touched)
            }
            // Wipe one board label to None (a firmware reset) — cone {the wiped label}.
            _ => {
                let objs: Vec<&BoardObject> = pre.iter().collect();
                let victim = sel.get(&objs);
                let Some(label) = victim.label.clone() else {
                    // Every synced object carries a label, so this is unreachable.
                    return Ok(());
                };
                let drifted: BTreeSet<BoardObject> = pre
                    .iter()
                    .map(|o| {
                        if o.handle == victim.handle {
                            BoardObject { label: None, ..o.clone() }
                        } else {
                            o.clone()
                        }
                    })
                    .collect();
                let touched = BTreeSet::from([label]);
                // Intent unchanged; the drift is repaired by the converge (decision 9).
                (converge_match(&base, &drifted), touched)
            }
        };
        prop_assert!(
            frame_law(&pre, &post, &touched),
            "an edit touching {touched:?} perturbed an object outside its cone"
        );
    }

    /// renameSelfNeutralizes (`match.qnt`, decision 10) on real names: after one converge
    /// under a rename, the SAME intent re-matches with an empty `renamed` set.
    #[test]
    fn rename_self_neutralizes_on_real_names(
        base in scene(),
        fresh in a_name(),
        sel in any::<prop::sample::Index>(),
    ) {
        let pre = synced(&base);
        prop_assert!(assert_base_not_refused(&base, &pre));
        let fresh = cname(&fresh);
        prop_assume!(!names_of(&base).contains(&fresh));

        let uns = unanchored_names(&base);
        let old = sel.get(&uns).clone();
        let victim = base.iter().find(|c| c.name == old).unwrap().clone();
        let mut renamed = victim.clone();
        renamed.name = fresh.clone();
        renamed.from = Some(old);
        let mut edited = base.clone();
        edited.remove(&victim);
        edited.insert(renamed);

        let converged = converge_match(&edited, &pre);
        match match_board(&converged, &edited) {
            MatchVerdict::Ok(plan) => {
                prop_assert!(plan.renamed.is_empty(), "the widening did not self-neutralize");
            }
            MatchVerdict::Refused(rs) => prop_assert!(false, "re-match refused: {rs:?}"),
        }
    }

    /// swapCorrect (`match.qnt`, decision 10) generalized: two unanchored constructs with
    /// DIFFERENT configs, names swapped (each `from` = the other), yield exactly two pairs
    /// through the rename rung, cross-bound, configs travelling with their objects, headline
    /// Hitless, and idempotent after.
    #[test]
    fn swap_is_hitless_and_correct(
        names in prop::collection::hash_set(a_name(), 2..=2),
    ) {
        let names: Vec<String> = names.into_iter().collect();
        let (n1, n2) = (names[0].as_str(), names[1].as_str());
        let base = BTreeSet::from([
            unanchored(n1, "cfg0", None),
            unanchored(n2, "cfg1", None),
        ]);
        let pre = synced(&base);
        // Each object keeps its config; the names swap, each `from` the other's name.
        let swapped = BTreeSet::from([
            unanchored(n1, "cfg1", Some(n2)),
            unanchored(n2, "cfg0", Some(n1)),
        ]);

        let plan = match match_board(&pre, &swapped) {
            MatchVerdict::Ok(p) => p,
            MatchVerdict::Refused(rs) => return Err(TestCaseError::fail(format!("swap refused: {rs:?}"))),
        };
        prop_assert_eq!(plan.pairs.len(), 2);
        prop_assert_eq!(&plan.renamed, &plan.pairs, "both bound through the rename rung");
        prop_assert!(plan.created.is_empty() && plan.removed.is_empty());
        prop_assert_eq!(converge_match_class(&swapped, &pre), Class::Hitless);

        let handle_of = |board: &BTreeSet<BoardObject>, lbl: &str| {
            board.iter().find(|o| o.label.as_ref() == Some(&cname(lbl))).unwrap().handle
        };
        let (h1, h2) = (handle_of(&pre, n1), handle_of(&pre, n2));

        let post = converge_match(&swapped, &pre);
        let p1 = post.iter().find(|o| o.label.as_ref() == Some(&cname(n1))).unwrap();
        let p2 = post.iter().find(|o| o.label.as_ref() == Some(&cname(n2))).unwrap();
        // Cross-bound: n1 now rides old n2's handle and vice versa.
        prop_assert_eq!(p1.handle, h2, "new {} <- old {}'s object", n1, n2);
        prop_assert_eq!(p2.handle, h1, "new {} <- old {}'s object", n2, n1);
        // Configs travel WITH their objects (no attribute repair).
        prop_assert_eq!(&p1.config, &link("cfg1"));
        prop_assert_eq!(&p2.config, &link("cfg0"));
        // A fixpoint after the swap.
        prop_assert_eq!(converge_match(&swapped, &post), post);
    }
}
