// The identity ladder as a pure relation, and the four laws it owes (openspec
// change `intent-layer`, task 6.4, bead gqf.52; ADR-0015 decisions 9-12). This
// is the hole-poker ADR-0015's revisit trigger names: bounded matching over a
// small alphabet, because identity bugs live at sizes 2-3 and the rename-swap
// flaw (decision 10) was found by hand. `matchRelation` re-associates an edited
// intent's compiled objects to the board a prior converge left, anchor-first
// then label-second, two passes on the label rung; `converge` applies the plan
// (intent/observed.qnt); the four named laws below pin what the ladder promises.
//
// THE TWO-PASS ALGORITHM (ADR-0015 decisions 9-11), in order, each stage
// binding only objects the earlier stages left free:
//   1. Anchor rung (decision 9, anchor-first). Every anchored compiled object
//      (anchor != empty) binds to the board object of the same family whose
//      anchor set is equal — hardware vouches for the identity, so no label and
//      no `from` is consulted. A rename of an anchored port is therefore free
//      (the anchor binds it whatever its name); moving a port to another dpmac
//      changes the anchor, so it binds NOTHING here and falls to create+remove —
//      a rewire, not a rename, and the anchor wins (decision 9, recorded so it
//      is not read later as a bug).
//   2. Label rung pass 1 (decision 10 rule (ii), the swap fix). Every still-free
//      unanchored compiled object binds to the unused unanchored board object of
//      the same family whose label equals its name — EXCEPT a CONTESTED rename
//      source: a board object whose label is named by some `from` clause AND
//      whose config disagrees with the exact claimant. Deferring the contested
//      source to pass 2 is what stops the swap `wan0<->e0` from binding new-e0 to
//      old-e0's object and "repairing" its attributes; gating on config (not a
//      flat exclusion) is what lets the swap SELF-NEUTRALIZE — once the labels
//      are swapped the configs agree, so pass 1 binds them directly and the
//      `from` matches nothing (see labelPass1 for the full argument).
//   3. Label rung pass 2 (decision 10, the rename widening). Every still-free
//      unanchored compiled object carrying `from` binds to the unused unanchored
//      board object whose label equals its `from`. These bindings are the
//      `renamed` set — self-neutralization watches it go empty once the board
//      carries the new labels.
//   4. Leftover rung (decision 11, indiscernibility). Among the unanchored
//      compiled and unused unanchored board objects a family still has: if two or
//      more board objects remain with a compiled object wanting one and their
//      configs are not all interchangeable, REFUSE Ambiguous(family) — a guess
//      could rewire hardware. Otherwise bind greedily (all-interchangeable ->
//      any bijection is sound; a lone candidate -> the single choice), leftover
//      compiled create, leftover board remove.
//
// Named invariants (the four families, each citing its decision):
//   frameLaw (decision 12) — an edit touching only construct c perturbs no
//     object outside c's cone above hitless; here, byte-identical outside the
//     cone, ordinal renumbering notwithstanding (decision 8).
//   renameSelfNeutralizes (decision 10) — after a converge under a rename, the
//     same intent re-matches with an identity binding and an empty `renamed` set:
//     the `from` clause is inert, a second converge is a no-op.
//   convergeIdempotent (decisions 9-11) — converge(intent, converge(intent, b))
//     == converge(intent, b): the board is a fixpoint.
//   swapCorrect (decision 10) — the wan0<->e0 swap over a board carrying both
//     labels yields exactly two hitless relabels, cross-bound, never a
//     create/destroy nor an attribute repair.
// Apalache-marked subset: none (simulator-only, corpus-wide seal — see
//   intent/alphabet.qnt compileLaws and intent/observed.qnt header).
//
// RED RUST TWINS THIS MODULE AWAITS (documented by name, not written — task 6.4
// is pure Quint; ADR-0015 "the Rust pairing/property twin runs the same relation
// on real strings"):
//   - dpaa2-api: the reconciler's matcher (this two-pass relation on real
//     construct names and dpmac sets) and a disruption-classed `Plan` whose
//     headline is `planClass` (task 6.5). Its cone is the derived-object closure,
//     not this layer's per-construct singleton (intent/observed.qnt outsideCone).
//   - dpaa2-verify: the pairing twin that runs `matchRelation` against the Rust
//     matcher the way NAME_RANK is linted against `str::cmp`, plus an MBT edit
//     replay that drives the edit alphabet (intent/edits.qnt) through the Rust
//     converge and diffs the board (task 6.7).
// These laws are model-only today; no failing Rust test is written here.
module intent_match {
  import types.* from "../core/types"            // Family (Dpni, Dpsw, ...)
  import intent_observed.* from "./observed"     // CompiledObject, Observed, ObservedBoard, Plan, Verdict, apply, planClass, outsideCone, ...

  // ---- stage helpers ----

  // The rename SOURCES: every non-empty `from` in the compiled set. Pass 1
  // excludes board objects labelled by one of these (decision 10 rule (ii)).
  pure def exclusionSet(compiled: Set[CompiledObject]): Set[str] =
    compiled.fold(Set(), (acc, c) => if (c.from != "") acc.union(Set(c.from)) else acc)

  pure def isPaired(pairs: Set[MatchPair], c: CompiledObject): bool =
    pairs.exists(p => p.compiled == c)

  pure def usedIds(pairs: Set[MatchPair]): Set[int] = pairs.map(p => p.id)

  // ---- the accumulator threaded through the stages ----
  // `pairs` every binding so far, `renamed` the pass-2 subset, `used` the bound
  // board ids (a board object binds at most once).

  type MatchAcc = { pairs: Set[MatchPair], renamed: Set[MatchPair], used: Set[int] }

  pure val EMPTY_ACC: MatchAcc = { pairs: Set(), renamed: Set(), used: Set() }

  // Stage 1 — anchor rung. Bind each anchored compiled object to the same-family
  // board object with an equal anchor set (anchors are unique, so the min-id
  // tie-break never actually chooses between two).
  pure def anchorRung(board: ObservedBoard, compiled: Set[CompiledObject], acc0: MatchAcc): MatchAcc =
    compiled.filter(c => c.anchor != Set()).fold(acc0, (acc, c) =>
      val cand = board.filter(o =>
        o.family == c.family and o.anchor == c.anchor and not(acc.used.contains(o.id)))
      val i = pickMinId(cand)
      if (i == -1) acc
      else { pairs: acc.pairs.union(Set({ compiled: c, id: i })),
             renamed: acc.renamed, used: acc.used.union(Set(i)) })

  // Stage 2 — label rung pass 1. Bind each still-free unanchored compiled object
  // to the unused unanchored same-family board object whose label equals its
  // name — EXCEPT a CONTESTED rename source: a board object whose label is named
  // by some `from` clause AND whose config disagrees with the exact claimant.
  //
  // The exclusion is conditional on config for a reason ADR-0015 decisions 10 and
  // 11 share. A flat exclusion of every rename source (the first reading of rule
  // (ii)) fixes the swap's FIRST converge but breaks self-neutralization: once the
  // board carries the swapped labels, the `from` clauses still name existing
  // objects, so a flat exclusion re-defers them to pass 2 and the swap oscillates
  // forever. Gating on config mismatch resolves both: on the first converge the
  // exact match is a config REPAIR (the object the operator moved a name away from
  // holds the wrong config), so it is deferred to the rename rung; on the second
  // converge the object already holds the right config, so the exact match is a
  // no-op and binds directly, the `from` matches nothing (self-neutralized). When
  // the two configs are EQUAL the objects are indiscernible at that position
  // (decision 11), so the exact bind is sound whether it is a "swap" or not.
  pure def labelPass1(board: ObservedBoard, compiled: Set[CompiledObject], exclusions: Set[str], acc0: MatchAcc): MatchAcc =
    compiled.filter(c => c.anchor == Set() and not(isPaired(acc0.pairs, c))).fold(acc0, (acc, c) =>
      val cand = board.filter(o =>
        o.family == c.family and o.anchor == Set()
          and o.label == c.name
          and not(exclusions.contains(o.label) and o.config != c.config)
          and not(acc.used.contains(o.id)))
      val i = pickMinId(cand)
      if (i == -1) acc
      else { pairs: acc.pairs.union(Set({ compiled: c, id: i })),
             renamed: acc.renamed, used: acc.used.union(Set(i)) })

  // Stage 3 — label rung pass 2, the rename widening. Bind each still-free
  // unanchored compiled object carrying `from` to the unused unanchored board
  // object whose label equals its `from`; record the binding in `renamed`.
  pure def labelPass2(board: ObservedBoard, compiled: Set[CompiledObject], acc0: MatchAcc): MatchAcc =
    compiled.filter(c => c.anchor == Set() and c.from != "" and not(isPaired(acc0.pairs, c))).fold(acc0, (acc, c) =>
      val cand = board.filter(o =>
        o.family == c.family and o.anchor == Set()
          and o.label == c.from and not(acc.used.contains(o.id)))
      val i = pickMinId(cand)
      if (i == -1) acc
      else val pr = { compiled: c, id: i }
        { pairs: acc.pairs.union(Set(pr)),
          renamed: acc.renamed.union(Set(pr)), used: acc.used.union(Set(i)) })

  // ---- stage 4: indiscernibility (ADR-0015 decision 11) ----

  // The unanchored compiled objects and unused unanchored board objects a family
  // still has after the label rungs.
  pure def leftoverCompiled(compiled: Set[CompiledObject], acc: MatchAcc, f: Family): Set[CompiledObject] =
    compiled.filter(c => c.family == f and c.anchor == Set() and not(isPaired(acc.pairs, c)))
  pure def leftoverObserved(board: ObservedBoard, acc: MatchAcc, f: Family): ObservedBoard =
    board.filter(o => o.family == f and o.anchor == Set() and not(acc.used.contains(o.id)))

  // The families with any unanchored compiled leftover — the only ones a refusal
  // or a leftover binding can concern.
  pure def leftoverFamilies(compiled: Set[CompiledObject], acc: MatchAcc): Set[Family] =
    compiled.filter(c => c.anchor == Set() and not(isPaired(acc.pairs, c))).map(c => c.family)

  // Ambiguous (decision 11): a family whose leftover cannot be resolved without a
  // guess that could rewire hardware — two or more leftover board objects with a
  // compiled object wanting one, whose configs are not all interchangeable. When
  // every leftover config (board AND compiled) is equal, any bijection is
  // observably equivalent, so it is sound, not a guess; when at most one board
  // object and one compiled object remain, there is a single choice, no guess.
  pure def ambiguousFamily(board: ObservedBoard, compiled: Set[CompiledObject], acc: MatchAcc, f: Family): bool =
    val lc = leftoverCompiled(compiled, acc, f)
    val lo = leftoverObserved(board, acc, f)
    val cfgs = lc.map(c => c.config).union(lo.map(o => o.config))
    and {
      lc != Set(),
      lo != Set(),
      not(lc.size() <= 1 and lo.size() <= 1),
      cfgs.size() > 1,
    }

  pure def ambiguityRefusals(board: ObservedBoard, compiled: Set[CompiledObject], acc: MatchAcc): Set[MatchRefusal] =
    leftoverFamilies(compiled, acc)
      .filter(f => ambiguousFamily(board, compiled, acc, f))
      .map(f => Ambiguous({ family: f }))

  // Stage 4 binding (only when no family is ambiguous): bind each leftover
  // compiled object greedily to the min-id unused unanchored same-family board
  // object. Sound because the family is not ambiguous (all-interchangeable or a
  // single candidate), so any surviving choice is observably equivalent.
  pure def leftoverRung(board: ObservedBoard, compiled: Set[CompiledObject], acc0: MatchAcc): MatchAcc =
    compiled.filter(c => c.anchor == Set() and not(isPaired(acc0.pairs, c))).fold(acc0, (acc, c) =>
      val cand = board.filter(o =>
        o.family == c.family and o.anchor == Set() and not(acc.used.contains(o.id)))
      val i = pickMinId(cand)
      if (i == -1) acc
      else { pairs: acc.pairs.union(Set({ compiled: c, id: i })),
             renamed: acc.renamed, used: acc.used.union(Set(i)) })

  // ---- the matching relation ----

  // Total: MatchOk(plan) or MatchRefused(non-empty set). Runs the four stages;
  // if any family is ambiguous it refuses before binding leftovers, so a refusing
  // converge computes no plan and (via converge) mutates nothing.
  pure def matchRelation(board: ObservedBoard, compiled: Set[CompiledObject]): Verdict =
    val exclusions = exclusionSet(compiled)
    val a1 = anchorRung(board, compiled, EMPTY_ACC)
    val a2 = labelPass1(board, compiled, exclusions, a1)
    val a3 = labelPass2(board, compiled, a2)
    val refs = ambiguityRefusals(board, compiled, a3)
    if (refs != Set()) MatchRefused(refs)
    else
      val a4 = leftoverRung(board, compiled, a3)
      val created = compiled.filter(c => not(isPaired(a4.pairs, c)))
      val removed = boardIds(board).exclude(a4.used)
      MatchOk({ pairs: a4.pairs, renamed: a4.renamed, created: created, removed: removed })

  // ---- converge: match then apply (a refused converge mutates nothing) ----

  pure def converge(compiled: Set[CompiledObject], board: ObservedBoard): ObservedBoard =
    match matchRelation(board, compiled) {
      | MatchOk(plan) => apply(board, plan)
      | MatchRefused(_) => board
    }

  // The headline disruption class of a converge (decision 12); a refused converge
  // is hitless (it changes nothing).
  pure def convergeClass(compiled: Set[CompiledObject], board: ObservedBoard): Class =
    match matchRelation(board, compiled) {
      | MatchOk(plan) => planClass(board, plan)
      | MatchRefused(_) => Hitless
    }

  // ---- the four laws (pure; the machine and the directed runs share them) ----

  // frameLaw (decision 12): the objects outside the touched cone are byte-identical
  // across a converge — no transition above hitless, ordinal renumbering
  // notwithstanding (decision 8). `pre` is the board before the perturbation,
  // `post` the board after the converge that followed it.
  pure def frameLaw(pre: ObservedBoard, post: ObservedBoard, touched: Set[str]): bool =
    outsideCone(pre, touched) == outsideCone(post, touched)

  // convergeIdempotent (decisions 9-11): the board is a fixpoint. Stated over
  // converge (re-deriving the plan) rather than over a fixed plan re-applied:
  // a fixed plan with a non-empty `created` re-applied would mint a SECOND fresh
  // object, so apply-of-apply is not the fixpoint — re-matching is (the created
  // object now matches by anchor/label). This is the law ADR-0015's decisions
  // 9-11 promise; the parcel's apply(plan, apply(plan, b)) form holds only for a
  // create-free plan, noted here.
  pure def convergeIdempotent(compiled: Set[CompiledObject], board: ObservedBoard): bool =
    converge(compiled, converge(compiled, board)) == converge(compiled, board)

  // renameSelfNeutralizes (decision 10): after one converge under a rename, the
  // SAME intent re-matches with no rename widening consumed — the `renamed` set is
  // empty because the board now carries the new labels, so pass 1 binds them
  // directly. A second converge is therefore a no-op (see convergeIdempotent).
  pure def renameSelfNeutralizes(compiled: Set[CompiledObject], board: ObservedBoard): bool =
    match matchRelation(converge(compiled, board), compiled) {
      | MatchOk(plan) => plan.renamed == Set()
      | MatchRefused(_) => false
    }

  // ---- directed runs (pnpm model:test; task 6.4 witnesses) ----

  // A board object and a compiled construct, at the grain the ladder matches.
  pure def observedObj(family: Family, id: int, label: str, anchor: Set[int], config: int): Observed =
    { family: family, id: id, label: label, anchor: anchor, config: config }
  pure def compiledObj(family: Family, name: str, anchor: Set[int], config: int, fromName: str): CompiledObject =
    { family: family, name: name, anchor: anchor, config: config, from: fromName }

  // The plan of a successful match, or a sentinel empty plan on refusal (only the
  // ambiguity run reads the refused arm, through matchRelation directly).
  pure def planOf(board: ObservedBoard, compiled: Set[CompiledObject]): Plan =
    match matchRelation(board, compiled) {
      | MatchOk(plan) => plan
      | MatchRefused(_) => { pairs: Set(), renamed: Set(), created: Set(), removed: Set() }
    }

  // -- swapCorrect (decision 10): the counterexample that motivated pass 1's
  //    exclusion. Board carries two unanchored dpnis labelled "wan0" (config 0)
  //    and "e0" (config 1); the intent swaps their names, each config travelling
  //    WITH its object. The naive one-pass matcher would bind new-"wan0"
  //    (wanting config 1) to old-"wan0" (config 0) and repair its config
  //    disruptively; the two-pass matcher cross-binds and emits two hitless
  //    relabels. Pins: exactly two pairs, both consumed a rename, cross-bound,
  //    nothing created or removed, no config change, headline hitless.
  run swapCorrectTest =
    val board = Set(
      observedObj(Dpni, 10, "wan0", Set(), 0),
      observedObj(Dpni, 20, "e0", Set(), 1))
    val compiled = Set(
      compiledObj(Dpni, "wan0", Set(), 1, "e0"),    // the port formerly called e0, now wan0
      compiledObj(Dpni, "e0", Set(), 0, "wan0"))    // the port formerly called wan0, now e0
    val plan = planOf(board, compiled)
    val wan0Pair = plan.pairs.filter(p => p.compiled.name == "wan0")
    val e0Pair = plan.pairs.filter(p => p.compiled.name == "e0")
    all {
      plan.pairs.size() == 2,
      plan.renamed == plan.pairs,               // both bound through the rename rung
      plan.created == Set(),
      plan.removed == Set(),
      wan0Pair.exists(p => p.id == 20),          // new wan0 <- old e0's object
      e0Pair.exists(p => p.id == 10),            // new e0   <- old wan0's object
      // each surviving object keeps its own config (no attribute repair)
      converge(compiled, board) == Set(
        observedObj(Dpni, 20, "wan0", Set(), 1),
        observedObj(Dpni, 10, "e0", Set(), 0)),
      convergeClass(compiled, board) == Hitless,
    }

  // -- a lone rename is boundary, not hitless: the old name leaves the
  //    externally-held set (decision 12). Board "wan0"; intent "e0" from "wan0".
  run plainRenameBoundaryTest =
    val board = Set(observedObj(Dpni, 10, "wan0", Set(), 0))
    val compiled = Set(compiledObj(Dpni, "e0", Set(), 0, "wan0"))
    val plan = planOf(board, compiled)
    all {
      plan.renamed.size() == 1,
      plan.created == Set(),
      plan.removed == Set(),
      convergeClass(compiled, board) == Boundary,
      converge(compiled, board) == Set(observedObj(Dpni, 10, "e0", Set(), 0)),
    }

  // -- anchor wins: renaming an anchored port binds by dpmac regardless of name,
  //    and consumes NO rename widening (decision 9). Board dpni on dpmac.7
  //    labelled "wan0"; intent "e0" on dpmac.7 (no `from` needed).
  run anchorRenameFreeTest =
    val board = Set(observedObj(Dpni, 10, "wan0", Set(7), 0))
    val compiled = Set(compiledObj(Dpni, "e0", Set(7), 0, ""))
    val plan = planOf(board, compiled)
    all {
      plan.pairs == Set({ compiled: compiledObj(Dpni, "e0", Set(7), 0, ""), id: 10 }),
      plan.renamed == Set(),
      converge(compiled, board) == Set(observedObj(Dpni, 10, "e0", Set(7), 0)),
    }

  // -- rewire, not rename (decision 9): moving the port to dpmac.8 changes the
  //    anchor, so the old dpni is removed and a fresh one created — disruptive.
  run rewireIsDisruptiveTest =
    val board = Set(observedObj(Dpni, 10, "wan0", Set(7), 0))
    val compiled = Set(compiledObj(Dpni, "wan0", Set(8), 0, ""))
    val plan = planOf(board, compiled)
    all {
      plan.pairs == Set(),
      plan.removed == Set(10),
      plan.created == compiled,
      convergeClass(compiled, board) == Disruptive,
    }

  // -- a single drifted unanchored label repairs hitlessly (decision 9): board
  //    object labelled "" (wiped), intent still names it — one candidate, no
  //    ambiguity, a set-label repair.
  run driftRepairTest =
    val board = Set(observedObj(Dpni, 10, "", Set(), 0))
    val compiled = Set(compiledObj(Dpni, "wan0", Set(), 0, ""))
    all {
      converge(compiled, board) == Set(observedObj(Dpni, 10, "wan0", Set(), 0)),
      convergeClass(compiled, board) == Hitless,
    }

  // -- ambiguity refuses by name (decision 11): two unanchored dpnis with drifted
  //    labels and DIFFERENT configs, the intent still names two — no label
  //    discriminator and configs not interchangeable, so a guess could rewire.
  //    The converge refuses and mutates nothing.
  run ambiguityRefusesTest =
    val board = Set(
      observedObj(Dpni, 10, "", Set(), 0),
      observedObj(Dpni, 20, "", Set(), 1))
    val compiled = Set(
      compiledObj(Dpni, "l1", Set(), 0, ""),
      compiledObj(Dpni, "l2", Set(), 1, ""))
    all {
      match matchRelation(board, compiled) {
        | MatchRefused(rs) => rs == Set(Ambiguous({ family: Dpni }))
        | MatchOk(_) => false
      },
      converge(compiled, board) == board,       // refusal mutates nothing
    }

  // -- indiscernible-but-equal matches arbitrarily and soundly (decision 11):
  //    two unanchored dpnis, drifted labels, IDENTICAL config; the intent names
  //    two with that config -> a bijection, no refusal.
  run indiscernibleArbitraryTest =
    val board = Set(
      observedObj(Dpni, 10, "", Set(), 0),
      observedObj(Dpni, 20, "", Set(), 0))
    val compiled = Set(
      compiledObj(Dpni, "l1", Set(), 0, ""),
      compiledObj(Dpni, "l2", Set(), 0, ""))
    val plan = planOf(board, compiled)
    all {
      plan.pairs.size() == 2,
      plan.created == Set(),
      plan.removed == Set(),
    }

  // -- convergeIdempotent and renameSelfNeutralizes on the swap (decision 10):
  //    the board is a fixpoint after one converge, and the second match consumes
  //    no rename widening.
  run selfNeutralizesTest =
    val board = Set(
      observedObj(Dpni, 10, "wan0", Set(), 0),
      observedObj(Dpni, 20, "e0", Set(), 1))
    val compiled = Set(
      compiledObj(Dpni, "wan0", Set(), 1, "e0"),
      compiledObj(Dpni, "e0", Set(), 0, "wan0"))
    all {
      renameSelfNeutralizes(compiled, board),
      convergeIdempotent(compiled, board),
    }

  // -- the frame law directly (decision 12): a ConfigEdit to "e0" over a
  //    two-object board leaves "wan0" byte-identical. `pre` is the converged
  //    board, `post` the board after the edited converge.
  run frameLawTest =
    val base = Set(
      compiledObj(Dpni, "wan0", Set(7), 0, ""),
      compiledObj(Dpni, "e0", Set(8), 0, ""))
    val pre = converge(base, Set())
    val edited = Set(
      compiledObj(Dpni, "wan0", Set(7), 0, ""),
      compiledObj(Dpni, "e0", Set(8), 1, ""))              // config bumped on e0 only
    val post = converge(edited, pre)
    all {
      frameLaw(pre, post, Set("e0")),               // wan0 untouched
      not(frameLaw(pre, post, Set())),              // e0 genuinely changed (cone non-empty)
      convergeClass(edited, pre) == Hitless,        // an attribute assert
    }

  // -- an additive edit does not renumber siblings (decision 8, the payoff of an
  //    ordinal-free matcher): adding "lan0" leaves wan0 and e0 byte-identical.
  run addFrameTest =
    val base = Set(
      compiledObj(Dpni, "wan0", Set(7), 0, ""),
      compiledObj(Dpni, "e0", Set(8), 0, ""))
    val pre = converge(base, Set())
    val added = base.union(Set(compiledObj(Dpni, "lan0", Set(), 0, "")))
    val post = converge(added, pre)
    all {
      frameLaw(pre, post, Set("lan0")),
      convergeClass(added, pre) == Disruptive,      // the create itself is disruptive
    }

  // A value use for every sum-type constructor in this module's imported unions
  // that is only value-constructed here (QNT404 guard; see intent/types.qnt
  // ETH_IFS and intent/refuse.qnt REFUSALS_USE).
  pure val MATCH_USE: Set[Verdict] = Set(
    MatchOk({ pairs: Set(), renamed: Set(), created: Set(), removed: Set() }),
    MatchRefused(Set(Ambiguous({ family: Dpni }))))
  pure val CLASS_USE: Set[Class] = Set(Hitless, Boundary, Disruptive)
}
