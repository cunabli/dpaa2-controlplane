// The bounded edit alphabet and the random machine that steps (intent, board)
// through it (openspec change `intent-layer`, task 6.4, bead gqf.52; ADR-0015
// decisions 8-12). This is the hole-poker: a finite alphabet of edits over a
// 3-construct name pool, converges interleaved, and the four identity laws on at
// every converged state. Bounded on purpose (ADR-0015 revisit trigger: identity
// bugs live at sizes 2-3, and the rename-swap flaw was found by hand), so the
// sweep finishes in seconds.
//
// The machine strictly alternates two phases so the frame law has an honest
// before/after (decision 12): from a converged ("synced") state it PERTURBS once
// — an intent edit (Add/Remove/Rename/Swap/ConfigEdit) or a board drift — records
// the touched construct names (the edit's cone) and snapshots the pre-perturb
// board, then CONVERGES back to synced. Every law is checked in the synced state,
// where the board is a fixpoint of the current intent:
//   convergeIdempotent  — the synced board re-converges to itself.
//   frameLaw            — the synced board differs from the pre-perturb board
//                         only inside the touched cone (siblings byte-identical,
//                         ordinal renumbering notwithstanding — decision 8).
//   renameSelfNeutralizes — a re-match of the current intent consumes no rename.
//   swapCorrect         — reached whenever the swap action fires (its correctness
//                         is the same convergeIdempotent + hitless class the
//                         directed swapCorrectTest pins, intent/match.qnt).
//
// Rename edits keep the input rule-(i)-clean (ADR-0015 decision 10, parsed at
// task 6.3): a plain Rename replaces old with new, so the old name is gone (no
// live construct is claimed twice); a Swap renames each of a pair to the other,
// so every `from`-target is itself renamed away — exactly the two shapes rule (i)
// admits. match may therefore assume its input is clean.
//
// Named invariants: identityLaws (convergeIdempotent, frameLaw,
//   renameSelfNeutralizes over every synced state; swapCorrect rides the sweep as
//   the swap action's reachable shape). Apalache-marked subset: none
//   (simulator-only, corpus-wide seal — intent/observed.qnt header).
//
// DELIBERATELY DIRECTED-ONLY (documented, never a silent gap — intent/alphabet.qnt
// coverage idiom): the ambiguity refusal (decision 11) needs TWO unanchored
// same-family objects drifted with differing configs before one converge; strict
// alternation drifts at most one object per converge (a single drift is always a
// lone candidate, repaired hitless), so ambiguity is unreachable in the sweep and
// is covered by ambiguityRefusesTest (intent/match.qnt) — a recorded unknown,
// exactly as intent/alphabet.qnt records wForeignAnchor.
module intent_edits {
  import types.* from "../core/types"            // Family (Dpni)
  import intent_observed.* from "./observed"     // CompiledObject, Observed, ObservedBoard, ...
  import intent_match.* from "./match"           // matchRelation, converge, the four laws

  var intent: Set[CompiledObject]
  var board: ObservedBoard
  var phase: str            // "perturb" (synced) | "converge"
  var touched: Set[str]     // the cone of the last perturbation (decision 12)
  var preBoard: ObservedBoard  // the board before the last perturbation (frame law)

  // ---- the finite alphabet ----

  // Three construct names plus one spare rename target — enough for a swap
  // (2 names) with a third construct standing by as the frame law's untouched
  // sibling, and a spare so a lone Rename has a fresh name to move to. All are
  // Dpni: the ladder matches construct-objects, and one family keeps the sweep
  // small (the anchor/label rungs are family-agnostic — anchorRenameFreeTest and
  // the directed runs exercise the family split).
  pure val NAMES: Set[str] = Set("wan0", "e0", "lan0")
  pure val SPARE: Set[str] = Set("wan1")
  pure val ALL_NAMES: Set[str] = NAMES.union(SPARE)
  // Two real dpmac anchors and the unanchored case (empty set): anchored objects
  // match by dpmac (decision 9), unanchored by label/rename (decisions 10-11).
  pure val ANCHORS: Set[Set[int]] = Set(Set(), Set(7), Set(8))
  pure val CONFIGS: Set[int] = Set(0, 1)

  // ---- helpers over the intent set ----

  pure def intentNames(i: Set[CompiledObject]): Set[str] = i.map(c => c.name)
  pure def compiledByName(i: Set[CompiledObject], nm: str): CompiledObject =
    i.fold({ family: Dpni, name: "", anchor: Set(), config: -1, from: "" },
      (acc, c) => if (c.name == nm) c else acc)
  pure def dropName(i: Set[CompiledObject], nm: str): Set[CompiledObject] = i.filter(c => c.name != nm)

  // ---- init: empty intent, empty board, synced ----

  action init = all {
    intent' = Set(),
    board' = Set(),
    phase' = "perturb",
    touched' = Set(),
    preBoard' = Set(),
  }

  // ---- perturbations (guard phase == "perturb"); each records the cone and
  //      snapshots the pre-perturb board, then hands off to convergeStep ----

  // Add a construct (a fresh name), anchored or not, at some config. A non-empty
  // anchor must be free: two constructs sharing a dpmac is the DoubleClaimed the
  // compiler refuses upstream (intent/refuse.qnt), and the matcher's anchor rung
  // assumes anchors are unique (decision 9) — so the alphabet never mints one.
  action addConstruct = {
    nondet nm = ALL_NAMES.oneOf()
    nondet an = ANCHORS.oneOf()
    nondet cfg = CONFIGS.oneOf()
    all {
      phase == "perturb",
      not(intentNames(intent).contains(nm)),
      an == Set() or not(intent.exists(c => c.anchor == an)),
      intent.size() < 3,
      intent' = intent.union(Set({ family: Dpni, name: nm, anchor: an, config: cfg, from: "" })),
      touched' = Set(nm), preBoard' = board, phase' = "converge", board' = board,
    }
  }

  // Remove a declared construct.
  action removeConstruct = {
    nondet nm = ALL_NAMES.oneOf()
    all {
      phase == "perturb",
      intentNames(intent).contains(nm),
      intent' = dropName(intent, nm),
      touched' = Set(nm), preBoard' = board, phase' = "converge", board' = board,
    }
  }

  // Flip a declared construct's config (an attribute edit; CONFIGS is {0,1}).
  action editConstructConfig = {
    nondet nm = ALL_NAMES.oneOf()
    all {
      phase == "perturb",
      intentNames(intent).contains(nm),
      val c = compiledByName(intent, nm)
      intent' = dropName(intent, nm).union(Set(c.with("config", if (c.config == 0) 1 else 0))),
      touched' = Set(nm), preBoard' = board, phase' = "converge", board' = board,
    }
  }

  // Rename a declared construct to a fresh name, carrying its anchor and config
  // and stamping `from` = the old name (decision 10). Old is dropped, so the input
  // stays rule-(i)-clean (no live construct is claimed twice).
  action renameConstruct = {
    nondet old = ALL_NAMES.oneOf()
    nondet new = ALL_NAMES.oneOf()
    all {
      phase == "perturb",
      intentNames(intent).contains(old),
      not(intentNames(intent).contains(new)),
      old != new,
      val c = compiledByName(intent, old)
      intent' = dropName(intent, old)
        .union(Set({ family: c.family, name: new, anchor: c.anchor, config: c.config, from: old })),
      touched' = Set(old, new), preBoard' = board, phase' = "converge", board' = board,
    }
  }

  // Swap two declared names in one edit (decision 10's counterexample), each
  // object keeping its own anchor and config. Every `from`-target is itself
  // renamed away, so the input stays rule-(i)-clean.
  action swapConstructs = {
    nondet n1 = ALL_NAMES.oneOf()
    nondet n2 = ALL_NAMES.oneOf()
    all {
      phase == "perturb",
      intentNames(intent).contains(n1),
      intentNames(intent).contains(n2),
      n1 != n2,
      val c1 = compiledByName(intent, n1)
      val c2 = compiledByName(intent, n2)
      intent' = dropName(dropName(intent, n1), n2).union(Set(
        { family: c1.family, name: n2, anchor: c1.anchor, config: c1.config, from: n1 },
        { family: c2.family, name: n1, anchor: c2.anchor, config: c2.config, from: n2 })),
      touched' = Set(n1, n2), preBoard' = board, phase' = "converge", board' = board,
    }
  }

  // Wipe one board object's label (a firmware reset / never-written label): the
  // drift decision 9 repairs by set-label. The cone is the object's OLD label, so
  // the repair lands inside the cone and the frame law holds.
  action driftLabel = {
    nondet nm = ALL_NAMES.oneOf()
    all {
      phase == "perturb",
      board.exists(o => o.label == nm),
      board' = board.map(o => if (o.label == nm) o.with("label", "") else o),
      touched' = Set(nm), preBoard' = board, phase' = "converge",
      intent' = intent,
    }
  }

  // ---- convergence (guard phase == "converge"): apply the plan, back to synced ----

  action convergeStep = all {
    phase == "converge",
    board' = converge(intent, board),
    phase' = "perturb",
    intent' = intent, touched' = touched, preBoard' = preBoard,
  }

  action step = any {
    addConstruct, removeConstruct, editConstructConfig, renameConstruct, swapConstructs, driftLabel, convergeStep,
  }

  // ---- the rung: the four identity laws over every synced state ----

  val identityLaws = (phase == "perturb") implies and {
    convergeIdempotent(intent, board),
    frameLaw(preBoard, board, touched),
    renameSelfNeutralizes(intent, board),
  }

  // ---- coverage witnesses (task 6.4) ----
  // One named predicate per interesting shape, counted by `pnpm model:coverage`.
  // A witness at 0 traces is a recorded unknown, never a silent gap
  // (intent/alphabet.qnt idiom). Law-shape witnesses stand for the laws being
  // exercised on a non-trivial state, since the laws themselves hold everywhere.

  val wSynced = phase == "perturb"
  val wIntentTwo = intent.size() >= 2
  val wIntentThree = intent.size() == 3
  val wBoardTwo = board.size() >= 2
  val wAnchoredPresent = intent.exists(c => c.anchor != Set())
  val wUnanchoredPresent = intent.exists(c => c.anchor == Set())
  val wRenamePresent = intent.exists(c => c.from != "")
  // A swap is in flight: two constructs each renamed from the other's name.
  val wSwapPresent =
    intent.exists(a => intent.exists(b =>
      a.name != b.name and a.from == b.name and b.from == a.name))
  val wDriftPresent = board.exists(o => o.label == "")
  // The board carries a name no intent construct claims yet, or vice versa — a
  // create/remove is pending (a disruptive converge is reachable).
  val wPendingCreate =
    intent.exists(c => c.anchor != Set() and not(board.exists(o => o.anchor == c.anchor)))
  val wPendingRemove =
    board.exists(o => not(intentNames(intent).contains(o.label)) and o.label != "")

  // A value use for every state var's element type reached only here.
  pure val EDIT_USE: Set[str] = ALL_NAMES
}
