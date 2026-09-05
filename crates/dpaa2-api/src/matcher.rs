//! The identity-across-time matcher: the Rust twin of `models/intent/match.qnt`
//! (`matchRelation`) and the decision-12 disruption classing of
//! `models/intent/observed.qnt` (ADR-0015 decisions 8-13; openspec change
//! `intent-layer`, task 6.5, bead gqf.54).
//!
//! When the operator edits the intent and the board already carries objects a prior
//! converge created, the compiler's clean-boot derivation is not enough: the
//! reconciler must *re-associate* each edited construct to the standing object that
//! carries it, so a rename relabels rather than destroys and a rewire is not mistaken
//! for a rename. [`match_board`] is that re-association as a pure, total verdict
//! producer — no I/O, no serde (design D10) — running the same four-stage relation the
//! model checks, on real [`ConstructName`]s and [`DpmacId`] anchor sets:
//!
//! 1. **Anchor rung** (decision 9, anchor-first). Every anchored [`MatchObject`] binds
//!    the same-family [`BoardObject`] whose anchor set is equal; label and `renamed`
//!    are ignored, because the hardware vouches for the identity. Renaming an anchored
//!    port is therefore free; moving it to another dpmac changes the anchor set, so it
//!    binds nothing here and falls to create+remove — a rewire, not a rename, and the
//!    anchor wins.
//! 2. **Label pass 1** (decision 10 rule ii, the swap fix). Every still-free
//!    unanchored [`MatchObject`] binds the unused same-family board object whose label
//!    equals its name — EXCEPT a *contested* rename source: a board object labelled by
//!    some `renamed.from` whose config disagrees with that exact claimant. The
//!    exclusion is gated on config (not flat) so the widening self-neutralizes: once a
//!    converge has carried the labels the exact bind is sound (decision 11
//!    indiscernibility) and the `from` matches nothing, whereas a flat exclusion would
//!    oscillate forever.
//! 3. **Label pass 2** (decision 10, the rename widening). Every still-free unanchored
//!    [`MatchObject`] carrying `renamed.from` binds the unused object labelled by its
//!    `from`; these are the rename set (each lowers to a [`crate::Transition::SetLabel`]).
//! 4. **Leftover rung** (decision 11, indiscernibility). Among the unanchored leftovers
//!    a family still has: two or more board objects with a compiled object wanting one
//!    and configs not all interchangeable REFUSES [`Ambiguity`] — a guess could rewire
//!    hardware; all-equal admits any bijection; a lone candidate binds; the rest
//!    create/remove.
//!
//! The verdict is `{ pairs, renamed, created, removed }` in the construct-name world.
//! Lowering a pair into actuation — a relabel to [`crate::Transition::SetLabel`] —
//! happens at the plan seam, not here (the parcel review's ratified decision 4); the
//! disruption class of every relabel and of the plan as a whole is
//! [`MatchPlan::headline`], transcribed from the model's `planClass`/`pairClass`. The
//! class vocabulary itself is [`crate::plan::Class`], shared with the reconciler's
//! [`crate::Transition`] classing so one `--allow` gate covers the merged headline.

use std::collections::BTreeSet;

use crate::family::Family;
use crate::intent::{Dataplane, Isolation};
use crate::model::DpmacId;
use crate::plan::Class;
use crate::types::{ConstructName, TenantName};

/// The opaque MC handle addressing one board object — `family.N` (ADR-0010),
/// abstracted from any one family so a dpsw or dprc board object bolts on later
/// without a rewrite. It is NEVER identity across time (ADR-0015 decision 8): the
/// matcher binds by anchor and label, never by handle; the handle is only the address
/// a relabel targets and the deterministic min tie-break (`observed.qnt` `id`,
/// `pickMinId`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Handle(u32);

impl Handle {
    /// Wraps a raw MC index.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// The raw MC index — the last-resort accessor for the fresh-handle tie-break.
    #[must_use]
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "N={}", self.0)
    }
}

/// The config-bearing projection the matcher compares (ADR-0015 decision 11).
///
/// Two same-family unanchored constructs are *indiscernible* exactly when their facets
/// are `Eq`; the leftover rung then admits any bijection between them, and pass 1's
/// exclusion self-neutralizes once configs agree. The facet MUST carry only the
/// attributes that distinguish two such constructs — never the name, an ordinal, or
/// the `renamed`/`from` clause, whose inclusion would make two indiscernible objects
/// compare unequal and so break decision-11 soundness and rename self-neutralization.
/// The model abstracts the whole tuple to an `int`; this is the real projection the
/// twin compares (ADR-0015: "the Rust twin runs the same relation on real strings").
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ConfigFacet {
    /// An anchored construct's config is never compared — the anchor decides its
    /// identity (decision 9), so this placeholder stands in and two anchored objects
    /// of a family are trivially interchangeable.
    Anchored,
    /// A link, projected to its two tenant-interface ends: parallel links between the
    /// same tenant pair are told apart by where they land, never by name or `renamed`.
    Link {
        /// The tenants whose interfaces terminate the wire, `a` then `b`.
        ends: (TenantName, TenantName),
    },
    /// A tenant, projected to its dataplane, core budget, isolation, and pool
    /// (decision 11) — the attributes two same-owner tenants can differ on, and
    /// nothing else (never the name, an ordinal, or the `renamed` clause).
    Tenant {
        /// Where the tenant's dataplane runs.
        dataplane: Dataplane,
        /// The core budget the derived thread count fits under.
        max_cores: i64,
        /// The tenant's place in the container tree.
        isolation: Isolation,
        /// The public holder a restricted tenant draws inside (empty when absent).
        pool: TenantName,
    },
}

/// One compiled construct the matcher re-associates, at the grain the ladder matches
/// (`match.qnt` `CompiledObject`): its family, the construct name that is its
/// cross-time identity and its intended label (decisions 1 and 13 — the name IS the
/// label), the hardware anchor it derives (decision 9: the wired dpmac set, empty for
/// an unanchored construct such as a link), its config facet (decision 11), and the
/// `renamed = { from }` clause the operator declared (decision 10; `None` when absent).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MatchObject {
    /// The object family.
    pub family: Family,
    /// The construct name — its cross-time identity and its label byte-for-byte.
    pub name: ConstructName,
    /// The wired dpmac anchor set (empty ⇒ unanchored).
    pub anchor: BTreeSet<DpmacId>,
    /// The config-bearing projection (decision 11).
    pub config: ConfigFacet,
    /// The declared `renamed.from`, or `None` — the temporary widening of decision 10.
    pub from: Option<ConstructName>,
}

/// One object a prior converge left on the board, as read back (`match.qnt`
/// `Observed`). `handle` is the opaque MC handle — never identity across time
/// (decision 8), only a deterministic tie-break and the address a relabel targets;
/// `label` is the set-label seam (`None` ⇒ drift, never-written, or wiped by a
/// firmware reset); `anchor` is hardware-vouched; `config` the facet the matcher
/// compares.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BoardObject {
    /// The object family.
    pub family: Family,
    /// The MC handle addressing the object (never identity).
    pub handle: Handle,
    /// The set-label seam, `None` when drifted or never written.
    pub label: Option<ConstructName>,
    /// The hardware-vouched dpmac anchor set (empty ⇒ unanchored).
    pub anchor: BTreeSet<DpmacId>,
    /// The config-bearing projection (decision 11).
    pub config: ConfigFacet,
}

/// A binding of one compiled construct to one standing board handle (`match.qnt`
/// `MatchPair`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MatchPair {
    /// The compiled construct the object now carries.
    pub compiled: MatchObject,
    /// The board handle it bound.
    pub handle: Handle,
}

/// The only ambiguity a match can hit (ADR-0015 decision 11): two or more unanchored
/// candidates of one family with no label discriminator and configs that are not all
/// interchangeable — a guess that could rewire hardware, so the converge refuses by
/// name rather than guess (`match.qnt` `MatchRefusal`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Ambiguity {
    /// The family that refused.
    pub family: Family,
}

/// The plan a successful match yields (`match.qnt` `Plan`): the bindings, the subset
/// of them that consumed a rename widening (decision 10 pass 2 — self-neutralization
/// watches this go empty on a converged board), the compiled constructs that matched
/// nothing (to create) and the board handles that matched nothing (to remove).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MatchPlan {
    /// Every binding.
    pub pairs: BTreeSet<MatchPair>,
    /// The bindings that consumed a rename widening (a subset of `pairs`).
    pub renamed: BTreeSet<MatchPair>,
    /// Compiled constructs that matched nothing — create fresh.
    pub created: BTreeSet<MatchObject>,
    /// Board handles that matched nothing — remove.
    pub removed: BTreeSet<Handle>,
}

impl MatchPlan {
    /// The plan's headline disruption class (ADR-0015 decision 12; `observed.qnt`
    /// `planClass`): the maximum over its parts, the class converge gates on and
    /// dry-run reports. Every create and every remove is [`Class::Disruptive`]; each
    /// matched pair contributes its [`pair_class`], which needs the board object as it
    /// stood before the converge (`pre_board`) to read the prior label. Shares
    /// [`Class`] with [`crate::Plan::headline`] so `--allow` gates the merged headline.
    ///
    /// # Panics
    /// Panics only on a corrupt plan — a pair whose handle names no object in
    /// `pre_board`. [`match_board`] never produces one (every pair binds a standing
    /// board object), so the invariant holds for any plan it returns.
    #[must_use]
    pub fn headline(&self, pre_board: &BTreeSet<BoardObject>) -> Class {
        let new_labels: BTreeSet<ConstructName> = self
            .pairs
            .iter()
            .map(|p| p.compiled.name.clone())
            .chain(self.created.iter().map(|c| c.name.clone()))
            .collect();
        let pair_classes = self.pairs.iter().map(|p| {
            let old = pre_board
                .iter()
                .find(|o| o.handle == p.handle)
                .expect("a matched pair addresses a standing board object");
            pair_class(old, &p.compiled, &new_labels)
        });
        let churn = [
            (!self.created.is_empty()).then_some(Class::Disruptive),
            (!self.removed.is_empty()).then_some(Class::Disruptive),
        ]
        .into_iter()
        .flatten();
        pair_classes.chain(churn).max().unwrap_or(Class::Hitless)
    }
}

/// A total verdict (`match.qnt` `Verdict`): a plan, or a non-empty refusal set
/// (ADR-0013 §5 posture — never an empty refusal).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MatchVerdict {
    /// The re-association succeeded; here is the plan.
    Ok(MatchPlan),
    /// The re-association refused; here is why, by family.
    Refused(BTreeSet<Ambiguity>),
}

// ---- the four-stage matching relation (match.qnt matchRelation) --------------

/// The accumulator threaded through the stages: `pairs` every binding so far,
/// `renamed` the pass-2 subset, `used` the bound board handles (each binds at most once).
#[derive(Clone, Default)]
struct MatchAcc {
    pairs: BTreeSet<MatchPair>,
    renamed: BTreeSet<MatchPair>,
    used: BTreeSet<Handle>,
}

/// The rename SOURCES: every declared `from` in the compiled set. Pass 1 excludes
/// board objects labelled by one of these when their config still disagrees
/// (`match.qnt` `exclusionSet`, decision 10 rule ii).
fn exclusion_set(compiled: &BTreeSet<MatchObject>) -> BTreeSet<ConstructName> {
    compiled.iter().filter_map(|c| c.from.clone()).collect()
}

fn is_paired(pairs: &BTreeSet<MatchPair>, c: &MatchObject) -> bool {
    pairs.iter().any(|p| &p.compiled == c)
}

/// The smallest handle among the candidates, or `None` — the deterministic tie-break
/// so a pure match never nondeterministically picks a candidate (`observed.qnt`
/// `pickMinId`; the parcel's min-observed-handle rule).
fn pick_min_handle<'a>(candidates: impl IntoIterator<Item = &'a BoardObject>) -> Option<Handle> {
    candidates.into_iter().map(|o| o.handle).min()
}

/// Stage 1 — anchor rung (decision 9). Bind each anchored compiled object to the
/// same-family board object with an equal anchor set (anchors are unique, so the
/// min-handle tie-break never actually chooses between two).
fn anchor_rung(
    board: &BTreeSet<BoardObject>,
    compiled: &BTreeSet<MatchObject>,
    acc: &mut MatchAcc,
) {
    for c in compiled.iter().filter(|c| !c.anchor.is_empty()) {
        let candidate = pick_min_handle(board.iter().filter(|o| {
            o.family == c.family && o.anchor == c.anchor && !acc.used.contains(&o.handle)
        }));
        if let Some(handle) = candidate {
            acc.pairs.insert(MatchPair {
                compiled: c.clone(),
                handle,
            });
            acc.used.insert(handle);
        }
    }
}

/// Stage 2 — label rung pass 1 (decision 10 rule ii). Bind each still-free unanchored
/// compiled object to the unused unanchored same-family board object whose label
/// equals its name — EXCEPT a contested rename source (labelled by some `from` AND
/// config unequal to this claimant). Gating on config, not a flat exclusion, is what
/// lets the swap self-neutralize (see the module docs and ADR-0015 decision 10).
fn label_pass1(
    board: &BTreeSet<BoardObject>,
    compiled: &BTreeSet<MatchObject>,
    exclusions: &BTreeSet<ConstructName>,
    acc: &mut MatchAcc,
) {
    // The model filters against the pass-entry accumulator, then the fold threads the
    // running one (`match.qnt` labelPass1); collecting the targets first snapshots
    // that entry state and frees `acc` for the binding mutation below.
    let targets: Vec<&MatchObject> = compiled
        .iter()
        .filter(|c| c.anchor.is_empty() && !is_paired(&acc.pairs, c))
        .collect();
    for c in targets {
        let candidate = pick_min_handle(board.iter().filter(|o| {
            o.family == c.family
                && o.anchor.is_empty()
                && o.label.as_ref() == Some(&c.name)
                && !(o.label.as_ref().is_some_and(|l| exclusions.contains(l))
                    && o.config != c.config)
                && !acc.used.contains(&o.handle)
        }));
        if let Some(handle) = candidate {
            acc.pairs.insert(MatchPair {
                compiled: c.clone(),
                handle,
            });
            acc.used.insert(handle);
        }
    }
}

/// Stage 3 — label rung pass 2, the rename widening (decision 10). Bind each
/// still-free unanchored compiled object carrying `from` to the unused unanchored
/// board object whose label equals its `from`; record the binding in `renamed`.
fn label_pass2(
    board: &BTreeSet<BoardObject>,
    compiled: &BTreeSet<MatchObject>,
    acc: &mut MatchAcc,
) {
    let targets: Vec<&MatchObject> = compiled
        .iter()
        .filter(|c| c.anchor.is_empty() && c.from.is_some() && !is_paired(&acc.pairs, c))
        .collect();
    for c in targets {
        let candidate = pick_min_handle(board.iter().filter(|o| {
            o.family == c.family
                && o.anchor.is_empty()
                && o.label == c.from
                && !acc.used.contains(&o.handle)
        }));
        if let Some(handle) = candidate {
            let pair = MatchPair {
                compiled: c.clone(),
                handle,
            };
            acc.pairs.insert(pair.clone());
            acc.renamed.insert(pair);
            acc.used.insert(handle);
        }
    }
}

// ---- stage 4: indiscernibility (decision 11) --------------------------------

fn leftover_compiled<'a>(
    compiled: &'a BTreeSet<MatchObject>,
    acc: &'a MatchAcc,
    family: Family,
) -> impl Iterator<Item = &'a MatchObject> {
    compiled
        .iter()
        .filter(move |c| c.family == family && c.anchor.is_empty() && !is_paired(&acc.pairs, c))
}

fn leftover_observed<'a>(
    board: &'a BTreeSet<BoardObject>,
    acc: &'a MatchAcc,
    family: Family,
) -> impl Iterator<Item = &'a BoardObject> {
    board
        .iter()
        .filter(move |o| o.family == family && o.anchor.is_empty() && !acc.used.contains(&o.handle))
}

/// The families with any unanchored compiled leftover — the only ones a refusal or a
/// leftover binding can concern (`match.qnt` `leftoverFamilies`).
fn leftover_families(compiled: &BTreeSet<MatchObject>, acc: &MatchAcc) -> BTreeSet<Family> {
    compiled
        .iter()
        .filter(|c| c.anchor.is_empty() && !is_paired(&acc.pairs, c))
        .map(|c| c.family)
        .collect()
}

/// Ambiguous (decision 11): two or more leftover board objects with a compiled object
/// wanting one whose configs are not all interchangeable — a guess could rewire
/// hardware. All-interchangeable admits any bijection; at most one of each leaves a
/// single choice (`match.qnt` `ambiguousFamily`).
fn ambiguous_family(
    board: &BTreeSet<BoardObject>,
    compiled: &BTreeSet<MatchObject>,
    acc: &MatchAcc,
    family: Family,
) -> bool {
    let leftover_compiled: Vec<_> = leftover_compiled(compiled, acc, family).collect();
    let leftover_observed: Vec<_> = leftover_observed(board, acc, family).collect();
    let configs: BTreeSet<&ConfigFacet> = leftover_compiled
        .iter()
        .map(|c| &c.config)
        .chain(leftover_observed.iter().map(|o| &o.config))
        .collect();
    // A family is ambiguous only when a compiled construct wants a board object, two
    // or more of each remain, and their configs are not all interchangeable.
    if leftover_compiled.is_empty() || leftover_observed.is_empty() {
        return false;
    }
    if leftover_compiled.len() <= 1 && leftover_observed.len() <= 1 {
        return false;
    }
    configs.len() > 1
}

fn ambiguity_refusals(
    board: &BTreeSet<BoardObject>,
    compiled: &BTreeSet<MatchObject>,
    acc: &MatchAcc,
) -> BTreeSet<Ambiguity> {
    leftover_families(compiled, acc)
        .into_iter()
        .filter(|&f| ambiguous_family(board, compiled, acc, f))
        .map(|family| Ambiguity { family })
        .collect()
}

/// Stage 4 binding (only when no family is ambiguous): bind each leftover compiled
/// object greedily to the min-handle unused unanchored same-family board object. Sound
/// because the family is not ambiguous, so any surviving choice is observably
/// equivalent (`match.qnt` `leftoverRung`).
fn leftover_rung(
    board: &BTreeSet<BoardObject>,
    compiled: &BTreeSet<MatchObject>,
    acc: &mut MatchAcc,
) {
    let targets: Vec<&MatchObject> = compiled
        .iter()
        .filter(|c| c.anchor.is_empty() && !is_paired(&acc.pairs, c))
        .collect();
    for c in targets {
        let candidate = pick_min_handle(board.iter().filter(|o| {
            o.family == c.family && o.anchor.is_empty() && !acc.used.contains(&o.handle)
        }));
        if let Some(handle) = candidate {
            acc.pairs.insert(MatchPair {
                compiled: c.clone(),
                handle,
            });
            acc.used.insert(handle);
        }
    }
}

/// The matching relation (ADR-0015 decisions 9-11; `match.qnt` `matchRelation`).
/// Total: [`MatchVerdict::Ok`] with a plan, or [`MatchVerdict::Refused`] with a
/// non-empty set. Runs the four stages in order; if any family is ambiguous it refuses
/// before binding leftovers, so a refusing converge computes no plan and mutates nothing.
#[must_use]
pub fn match_board(
    board: &BTreeSet<BoardObject>,
    compiled: &BTreeSet<MatchObject>,
) -> MatchVerdict {
    let exclusions = exclusion_set(compiled);
    let mut acc = MatchAcc::default();
    anchor_rung(board, compiled, &mut acc);
    label_pass1(board, compiled, &exclusions, &mut acc);
    label_pass2(board, compiled, &mut acc);

    let refusals = ambiguity_refusals(board, compiled, &acc);
    if !refusals.is_empty() {
        return MatchVerdict::Refused(refusals);
    }

    leftover_rung(board, compiled, &mut acc);
    let created: BTreeSet<MatchObject> = compiled
        .iter()
        .filter(|c| !is_paired(&acc.pairs, c))
        .cloned()
        .collect();
    let used = acc.used.clone();
    let removed: BTreeSet<Handle> = board
        .iter()
        .map(|o| o.handle)
        .filter(|h| !used.contains(h))
        .collect();
    MatchVerdict::Ok(MatchPlan {
        pairs: acc.pairs,
        renamed: acc.renamed,
        created,
        removed,
    })
}

// ---- apply + converge (observed.qnt apply/converge) -------------------------

fn max_handle(board: &BTreeSet<BoardObject>) -> u32 {
    board
        .iter()
        .map(|o| o.handle.into_inner())
        .max()
        .unwrap_or(0)
}

/// The board after a converge (ADR-0015 decisions 9-10; `observed.qnt` `apply`): a
/// matched object relabelled to its construct name and its config asserted, an
/// unmatched compiled object created fresh, an unmatched board object removed. Pure and
/// total; fresh handles are minted max+1 upward so two creates never collide — the
/// handle scheme is irrelevant to identity (decision 8), only distinctness matters.
#[must_use]
pub fn apply(board: &BTreeSet<BoardObject>, plan: &MatchPlan) -> BTreeSet<BoardObject> {
    let mut out: BTreeSet<BoardObject> = BTreeSet::new();
    for o in board {
        if plan.removed.contains(&o.handle) {
            continue;
        }
        if let Some(pair) = plan.pairs.iter().find(|p| p.handle == o.handle) {
            out.insert(BoardObject {
                label: Some(pair.compiled.name.clone()),
                config: pair.compiled.config.clone(),
                ..o.clone()
            });
        } else {
            out.insert(o.clone());
        }
    }
    for (next, c) in (max_handle(board) + 1..).zip(&plan.created) {
        out.insert(BoardObject {
            family: c.family,
            handle: Handle::new(next),
            label: Some(c.name.clone()),
            anchor: c.anchor.clone(),
            config: c.config.clone(),
        });
    }
    out
}

/// Match then apply — a refused converge mutates nothing (`match.qnt` `converge`).
#[must_use]
pub fn converge(
    compiled: &BTreeSet<MatchObject>,
    board: &BTreeSet<BoardObject>,
) -> BTreeSet<BoardObject> {
    match match_board(board, compiled) {
        MatchVerdict::Ok(plan) => apply(board, &plan),
        MatchVerdict::Refused(_) => board.clone(),
    }
}

/// The headline disruption class of a converge (decision 12); a refused converge is
/// hitless (it changes nothing) (`match.qnt` `convergeClass`).
#[must_use]
pub fn converge_class(compiled: &BTreeSet<MatchObject>, board: &BTreeSet<BoardObject>) -> Class {
    match match_board(board, compiled) {
        MatchVerdict::Ok(plan) => plan.headline(board),
        MatchVerdict::Refused(_) => Class::Hitless,
    }
}

// ---- disruption classing (decision 12; observed.qnt pairClass) --------------

/// The class of one matched relabel/assert, given the object as it stood before
/// (`old`), the construct it now carries, and every name the plan writes (`new_labels`)
/// (ADR-0015 decision 12; `observed.qnt` `pairClass`):
///   - name unchanged ⇒ hitless (a config-only change is an attribute assert);
///   - old label `None` ⇒ hitless (a drift repair, decision 9 — no external name was
///     ever held);
///   - old label reclaimed elsewhere in the plan ⇒ hitless (the externally-held name
///     SET is unchanged, the name only moved — the swap and any name-cycle);
///   - otherwise ⇒ boundary (the old name leaves the externally-held set).
///
/// A relabel is never disruptive: it is `set-label`, decision 9's repair verb.
#[must_use]
pub fn pair_class(
    old: &BoardObject,
    compiled: &MatchObject,
    new_labels: &BTreeSet<ConstructName>,
) -> Class {
    match &old.label {
        Some(label) if *label == compiled.name => Class::Hitless,
        None => Class::Hitless,
        Some(label) if new_labels.contains(label) => Class::Hitless,
        Some(_) => Class::Boundary,
    }
}

#[cfg(test)]
mod tests {
    //! The directed runs of `models/intent/match.qnt`, transcribed by name onto real
    //! [`ConstructName`]s and [`DpmacId`] anchor sets (ADR-0015 decisions 9-12). The
    //! model's abstract unanchored `Dpni` objects become links, so their config facet
    //! is [`ConfigFacet::Link`] — two distinct ends stand in for the model's config
    //! `0`/`1`; anchored ports carry [`ConfigFacet::Anchored`], never compared.

    use super::*;

    fn name(s: &str) -> ConstructName {
        ConstructName::from(s)
    }

    fn anchor(dpmac: u32) -> BTreeSet<DpmacId> {
        BTreeSet::from([DpmacId::new(dpmac)])
    }

    /// A link config facet with a distinguishing far end (the near end is always the
    /// kernel); `variant` stands in for the model's abstract config int.
    fn link_config(variant: &str) -> ConfigFacet {
        ConfigFacet::Link {
            ends: (TenantName::from("kernel"), TenantName::from(variant)),
        }
    }

    fn observed(
        handle: u32,
        label: Option<&str>,
        anchor: BTreeSet<DpmacId>,
        config: ConfigFacet,
    ) -> BoardObject {
        BoardObject {
            family: Family::Dpni,
            handle: Handle::new(handle),
            label: label.map(name),
            anchor,
            config,
        }
    }

    fn compiled(
        name_: &str,
        anchor: BTreeSet<DpmacId>,
        config: ConfigFacet,
        from: Option<&str>,
    ) -> MatchObject {
        MatchObject {
            family: Family::Dpni,
            name: name(name_),
            anchor,
            config,
            from: from.map(name),
        }
    }

    fn plan_of(board: &BTreeSet<BoardObject>, compiled: &BTreeSet<MatchObject>) -> MatchPlan {
        match match_board(board, compiled) {
            MatchVerdict::Ok(plan) => plan,
            MatchVerdict::Refused(rs) => panic!("expected a plan, refused: {rs:?}"),
        }
    }

    /// swapCorrect (decision 10): `wan0` and `e0` renamed to each other over a board
    /// carrying both labels yields exactly two cross-bound hitless relabels, never a
    /// create/destroy nor an attribute repair — each config travels with its object.
    #[test]
    fn swap_correct() {
        let board = BTreeSet::from([
            observed(10, Some("wan0"), BTreeSet::new(), link_config("a")),
            observed(20, Some("e0"), BTreeSet::new(), link_config("b")),
        ]);
        let compiled = BTreeSet::from([
            compiled("wan0", BTreeSet::new(), link_config("b"), Some("e0")),
            compiled("e0", BTreeSet::new(), link_config("a"), Some("wan0")),
        ]);
        let plan = plan_of(&board, &compiled);
        assert_eq!(plan.pairs.len(), 2);
        assert_eq!(
            plan.renamed, plan.pairs,
            "both bound through the rename rung"
        );
        assert!(plan.created.is_empty());
        assert!(plan.removed.is_empty());
        let wan0 = plan
            .pairs
            .iter()
            .find(|p| p.compiled.name == name("wan0"))
            .unwrap();
        let e0 = plan
            .pairs
            .iter()
            .find(|p| p.compiled.name == name("e0"))
            .unwrap();
        assert_eq!(wan0.handle, Handle::new(20), "new wan0 <- old e0's object");
        assert_eq!(e0.handle, Handle::new(10), "new e0 <- old wan0's object");
        assert_eq!(converge_class(&compiled, &board), Class::Hitless);
        assert_eq!(
            converge(&compiled, &board),
            BTreeSet::from([
                observed(20, Some("wan0"), BTreeSet::new(), link_config("b")),
                observed(10, Some("e0"), BTreeSet::new(), link_config("a")),
            ])
        );
    }

    /// A lone rename is boundary, not hitless: the old name leaves the externally-held
    /// set (decision 12).
    #[test]
    fn plain_rename_boundary() {
        let board = BTreeSet::from([observed(
            10,
            Some("wan0"),
            BTreeSet::new(),
            link_config("a"),
        )]);
        let compiled = BTreeSet::from([compiled(
            "e0",
            BTreeSet::new(),
            link_config("a"),
            Some("wan0"),
        )]);
        let plan = plan_of(&board, &compiled);
        assert_eq!(plan.renamed.len(), 1);
        assert!(plan.created.is_empty() && plan.removed.is_empty());
        assert_eq!(converge_class(&compiled, &board), Class::Boundary);
        assert_eq!(
            converge(&compiled, &board),
            BTreeSet::from([observed(10, Some("e0"), BTreeSet::new(), link_config("a"))])
        );
    }

    /// Anchor wins (decision 9): renaming an anchored port binds by dpmac regardless of
    /// name and consumes NO rename widening.
    #[test]
    fn anchored_rename_free() {
        let board = BTreeSet::from([observed(10, Some("wan0"), anchor(7), ConfigFacet::Anchored)]);
        let compiled = BTreeSet::from([compiled("e0", anchor(7), ConfigFacet::Anchored, None)]);
        let plan = plan_of(&board, &compiled);
        assert_eq!(plan.pairs.len(), 1);
        assert!(
            plan.renamed.is_empty(),
            "the anchor bound it, no rename consumed"
        );
        assert_eq!(
            converge(&compiled, &board),
            BTreeSet::from([observed(10, Some("e0"), anchor(7), ConfigFacet::Anchored)])
        );
    }

    /// Rewire, not rename (decision 9): moving the port to another dpmac changes the
    /// anchor, so the old dpni is removed and a fresh one created — disruptive.
    #[test]
    fn rewire_is_disruptive() {
        let board = BTreeSet::from([observed(10, Some("wan0"), anchor(7), ConfigFacet::Anchored)]);
        let compiled = BTreeSet::from([compiled("wan0", anchor(8), ConfigFacet::Anchored, None)]);
        let plan = plan_of(&board, &compiled);
        assert!(plan.pairs.is_empty());
        assert_eq!(plan.removed, BTreeSet::from([Handle::new(10)]));
        assert_eq!(plan.created, compiled);
        assert_eq!(converge_class(&compiled, &board), Class::Disruptive);
    }

    /// A single drifted unanchored label repairs hitlessly (decision 9): one candidate,
    /// no ambiguity, a set-label repair.
    #[test]
    fn drift_repair_hitless() {
        let board = BTreeSet::from([observed(10, None, BTreeSet::new(), link_config("a"))]);
        let compiled = BTreeSet::from([compiled("wan0", BTreeSet::new(), link_config("a"), None)]);
        assert_eq!(
            converge(&compiled, &board),
            BTreeSet::from([observed(
                10,
                Some("wan0"),
                BTreeSet::new(),
                link_config("a")
            )])
        );
        assert_eq!(converge_class(&compiled, &board), Class::Hitless);
    }

    /// Ambiguity refuses by name (decision 11): two unanchored objects, drifted labels,
    /// DIFFERENT configs, the intent names two — a guess could rewire, so the converge
    /// refuses and mutates nothing.
    #[test]
    fn ambiguity_refuses() {
        let board = BTreeSet::from([
            observed(10, None, BTreeSet::new(), link_config("a")),
            observed(20, None, BTreeSet::new(), link_config("b")),
        ]);
        let compiled = BTreeSet::from([
            compiled("l1", BTreeSet::new(), link_config("a"), None),
            compiled("l2", BTreeSet::new(), link_config("b"), None),
        ]);
        assert_eq!(
            match_board(&board, &compiled),
            MatchVerdict::Refused(BTreeSet::from([Ambiguity {
                family: Family::Dpni
            }]))
        );
        assert_eq!(
            converge(&compiled, &board),
            board,
            "a refusal mutates nothing"
        );
    }

    /// Indiscernible-but-equal matches arbitrarily and soundly (decision 11): two
    /// unanchored objects, drifted labels, IDENTICAL config; a bijection, no refusal.
    #[test]
    fn indiscernible_arbitrary() {
        let board = BTreeSet::from([
            observed(10, None, BTreeSet::new(), link_config("a")),
            observed(20, None, BTreeSet::new(), link_config("a")),
        ]);
        let compiled = BTreeSet::from([
            compiled("l1", BTreeSet::new(), link_config("a"), None),
            compiled("l2", BTreeSet::new(), link_config("a"), None),
        ]);
        let plan = plan_of(&board, &compiled);
        assert_eq!(plan.pairs.len(), 2);
        assert!(plan.created.is_empty() && plan.removed.is_empty());
    }

    /// convergeIdempotent (decisions 9-11): the board is a fixpoint after one converge —
    /// a second converge is a no-op.
    #[test]
    fn converge_idempotent() {
        let board = BTreeSet::from([
            observed(10, Some("wan0"), BTreeSet::new(), link_config("a")),
            observed(20, Some("e0"), BTreeSet::new(), link_config("b")),
        ]);
        let compiled = BTreeSet::from([
            compiled("wan0", BTreeSet::new(), link_config("b"), Some("e0")),
            compiled("e0", BTreeSet::new(), link_config("a"), Some("wan0")),
        ]);
        let once = converge(&compiled, &board);
        assert_eq!(converge(&compiled, &once), once);
    }

    /// renameSelfNeutralizes (decision 10): after one converge under a rename, the same
    /// intent re-matches with an EMPTY rename set — the `from` is inert.
    #[test]
    fn rename_self_neutralizes() {
        let board = BTreeSet::from([
            observed(10, Some("wan0"), BTreeSet::new(), link_config("a")),
            observed(20, Some("e0"), BTreeSet::new(), link_config("b")),
        ]);
        let compiled = BTreeSet::from([
            compiled("wan0", BTreeSet::new(), link_config("b"), Some("e0")),
            compiled("e0", BTreeSet::new(), link_config("a"), Some("wan0")),
        ]);
        let converged = converge(&compiled, &board);
        match match_board(&converged, &compiled) {
            MatchVerdict::Ok(plan) => {
                assert!(plan.renamed.is_empty(), "the widening self-neutralized");
            }
            MatchVerdict::Refused(rs) => panic!("second match refused: {rs:?}"),
        }
    }

    /// A restricted-tenant config facet; `pool` is the public holder it draws inside,
    /// the empty [`TenantName`] standing for "no pool" (an unrestricted tenant).
    fn tenant_config(max_cores: i64, pool: &str) -> ConfigFacet {
        ConfigFacet::Tenant {
            dataplane: Dataplane::UserspacePoll,
            max_cores,
            isolation: Isolation::Restricted,
            pool: TenantName::from(pool),
        }
    }

    fn tenant_board(handle: u32, label: Option<&str>, config: ConfigFacet) -> BoardObject {
        BoardObject {
            family: Family::Dprc,
            handle: Handle::new(handle),
            label: label.map(name),
            anchor: BTreeSet::new(),
            config,
        }
    }

    fn tenant_compiled(name_: &str, config: ConfigFacet) -> MatchObject {
        MatchObject {
            family: Family::Dprc,
            name: name(name_),
            anchor: BTreeSet::new(),
            config,
            from: None,
        }
    }

    /// Decision 11 on the tenant facet: two drifted tenants differing ONLY in a Tenant
    /// facet field (here `pool`) are discernible, so naming two refuses rather than
    /// guess which drawer pools where — a guess could rewire hardware. An all-equal
    /// facet (empty-pool sentinel included) instead admits a bijection and a lone
    /// drifted candidate repairs hitlessly, pinning the Tenant facet as `Eq`-compared.
    #[test]
    fn tenant_facet_gates_pairing() {
        // Differ only in the pool field: discernible ⇒ refuse.
        let board = BTreeSet::from([
            tenant_board(10, None, tenant_config(2, "pubA")),
            tenant_board(20, None, tenant_config(2, "pubB")),
        ]);
        let compiled = BTreeSet::from([
            tenant_compiled("t1", tenant_config(2, "pubA")),
            tenant_compiled("t2", tenant_config(2, "pubB")),
        ]);
        assert_eq!(
            match_board(&board, &compiled),
            MatchVerdict::Refused(BTreeSet::from([Ambiguity {
                family: Family::Dprc
            }]))
        );

        // Empty-pool sentinel, equal facet, one drifted candidate: a hitless drift
        // repair (no external name was ever held).
        let unpooled = tenant_config(2, "");
        let board = BTreeSet::from([tenant_board(10, None, unpooled.clone())]);
        let compiled = BTreeSet::from([tenant_compiled("t1", unpooled.clone())]);
        assert_eq!(converge_class(&compiled, &board), Class::Hitless);
        assert_eq!(
            converge(&compiled, &board),
            BTreeSet::from([tenant_board(10, Some("t1"), unpooled)])
        );
    }
}
