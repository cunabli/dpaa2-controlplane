// The board as the matcher sees it, and the pure converge step that rewrites
// it (openspec change `intent-layer`, task 6.4, bead gqf.52; ADR-0015 the
// identity-across-time contract, decisions 8-13). Until this module the intent
// corpus stopped at `compile(intent, inventory) -> Plan`: a clean-boot
// derivation with no memory of what the last converge left on the board.
// ADR-0015 decisions 9-13 add the missing half — how an EDITED intent
// re-associates to the objects a prior converge created, when the operator has
// renamed, rewired, or reconfigured a construct. This module carries the
// value types that half reasons over (the `ObservedBoard` and the compiled
// construct-object the matcher consumes), the pure `apply` that rewrites the
// board after a converge, and the ADR-0015 decision-12 disruption classing.
// The matching relation itself lives in intent/match.qnt; the edit alphabet and
// random machine in intent/edits.qnt.
//
// DELIBERATELY NOT MODELLED (what `ObservedBoard` abstracts away — the honesty
// block the corpus voice owes; ADR-0015 revisit "bounded model checking is the
// hole-poker, not a totality proof"):
//   - The full MC object tree. An `Observed` is ONE construct-level object (the
//     dpni a port bears, the dpsw a hardware fabric bears, the tenant's child
//     dprc), never its derived companions (dpio/dpbp/dpmcp/dpcon) or edges. The
//     ladder's laws (decisions 9-12) are about a construct's cross-time identity,
//     and companions cascade WITH their tenant — so modelling one object per
//     construct keeps the matcher's state space at the sizes 2-3 where identity
//     bugs live, and the derived-object closure is the Rust twin's concern
//     (task 6.5, red-twin block in intent/match.qnt).
//   - The config facet is an abstract `int`, not the compiled object's real
//     attribute tuple: record-equality (== on int) stands in for byte-identical
//     config (decision 11). A `ConfigEdit` bumps it; the matcher never reads its
//     structure, only compares it, so an int is faithful to "indiscernible iff
//     record-equal" and keeps the alphabet small (ponytail: an int abstracts the
//     whole attribute tuple; widen to a record only if a per-field law appears).
//   - The opaque handle `id` stands for the MC's `family.N` (ADR-0010): it is
//     NEVER identity across time (decision 8) — the matcher binds by anchor and
//     label, never by id, and `apply` mints fresh ids on create by max+1 upward,
//     NOT the MC's lowest-free reuse. The scheme is deliberately unlike the
//     firmware because no law reads it; if a law ever must, this is the seam to
//     revisit. `set-label` atomicity and id reuse are firmware behaviour the
//     model assumes and cannot prove (ADR-0015 revisit trigger).
//   - Ordinals are absent entirely. Decision 8: an ordinal is not identity across
//     an edit; the matcher leans on anchor and label, so the sibling-renumbering
//     an add/rename triggers in the real compile is INVISIBLE here — which is
//     exactly what the frame law (intent/match.qnt) then proves inert.
//
// Named invariants: none here — this module is pure value machinery. The four
//   invariant families live in intent/match.qnt (frameLaw, renameSelfNeutralizes,
//   convergeIdempotent, swapCorrect) and are wired into the intent/edits.qnt
//   machine. Apalache-marked subset: none (simulator-only, like every intent
//   rung — the compileLaws seal in intent/alphabet.qnt applies corpus-wide).
module intent_observed {
  import types.* from "../core/types"            // Family (Dpni, Dpsw, Dprc, ...)

  // ---- the compiled construct-object the matcher consumes ----

  // One derived object of the intent, at the grain the ladder matches: its
  // family, the construct NAME that is its cross-time identity and its intended
  // label (ADR-0015 decisions 1 and 13 — the name IS the label, byte-for-byte),
  // the hardware anchor it derives (decision 9: the wired dpmac set, empty for an
  // unanchored construct such as a tenant or a link), the abstract config facet
  // (decision 11), and the `renamed = { from }` clause the operator declared
  // (decision 10; "" when absent — the raw layer's `from` convention,
  // intent/intent_raw.qnt, carried here per construct rather than as a side map).
  type CompiledObject = {
    family: Family,
    name: str,
    anchor: Set[int],
    config: int,
    from: str,
  }

  // One object a prior converge left on the board, as read back. `id` is the
  // opaque MC handle (never identity — see the honesty block); `label` is the
  // set-label seam ("" = drift, never-written, or wiped by a firmware reset);
  // `anchor` is hardware-vouched (the dpmac set the dpni is wired to, empty for
  // an unanchored object); `config` the abstract facet the matcher compares.
  type Observed = {
    family: Family,
    id: int,
    label: str,
    anchor: Set[int],
    config: int,
  }

  type ObservedBoard = Set[Observed]

  // ---- the match verdict (the shape intent/match.qnt returns) ----

  // A binding of one compiled object to one standing board id.
  type MatchPair = { compiled: CompiledObject, id: int }

  // The only ambiguity a match can hit (ADR-0015 decision 11): two or more
  // unanchored candidates of one family with no label discriminator and configs
  // that are not all interchangeable — a guess that could rewire hardware, so the
  // converge refuses by name rather than guess.
  type MatchRefusal = Ambiguous({ family: Family })

  // The plan a successful match yields: the bindings, the subset of them that
  // consumed a rename widening (decision 10 pass 2 — the self-neutralization law
  // watches this go empty on a converged board), the compiled objects that
  // matched nothing (to create fresh) and the board ids that matched nothing (to
  // remove).
  type Plan = {
    pairs: Set[MatchPair],
    renamed: Set[MatchPair],
    created: Set[CompiledObject],
    removed: Set[int],
  }

  // Total verdict: a plan, or a non-empty refusal set (ADR-0013 §5 posture —
  // never an empty refusal, mirroring compile()'s Refused).
  type Verdict = MatchOk(Plan) | MatchRefused(Set[MatchRefusal])

  // ---- board lookups ----

  pure val NO_OBJ: Observed = { family: Dpni, id: -1, label: "", anchor: Set(), config: -1 }

  // The object bearing an id, or NO_OBJ when absent (every matched id exists, so
  // the sentinel is only a totality guard).
  pure def observedById(board: ObservedBoard, i: int): Observed =
    board.fold(NO_OBJ, (acc, o) => if (o.id == i) o else acc)

  // The smallest board id, or -1 for the empty set — the deterministic tie-break
  // the matcher uses so a pure match never nondeterministically picks a candidate.
  pure def pickMinId(cands: ObservedBoard): int =
    cands.fold(-1, (acc, o) => if (acc == -1 or o.id < acc) o.id else acc)

  pure def boardIds(board: ObservedBoard): Set[int] = board.map(o => o.id)

  pure def maxId(board: ObservedBoard): int =
    board.fold(0, (acc, o) => if (o.id > acc) o.id else acc)

  // ---- apply: the board after a converge (ADR-0015 decisions 9-10) ----

  // A matched object relabelled to its construct name and its config asserted;
  // an unmatched compiled object created fresh; an unmatched board object
  // removed. Pure and total. Fresh ids are minted max+1 upward over BOTH the
  // surviving board and the ids already handed out this converge (folding the
  // created set with a running counter), so two creates never collide — the id
  // scheme is irrelevant to identity (decision 8), only distinctness matters.
  pure def apply(board: ObservedBoard, plan: Plan): ObservedBoard =
    // id -> intended (name, config) for every matched pair.
    val relabelName: int -> str =
      plan.pairs.fold(Map(), (m, p) => m.put(p.id, p.compiled.name))
    val relabelCfg: int -> int =
      plan.pairs.fold(Map(), (m, p) => m.put(p.id, p.compiled.config))
    val matchedIds = plan.pairs.map(p => p.id)
    // Survivors: matched objects relabelled + config-asserted; removed dropped;
    // an unmatched-but-unremoved object cannot exist (removed == all unmatched).
    val survivors = board
      .filter(o => not(plan.removed.contains(o.id)))
      .map(o =>
        if (matchedIds.contains(o.id))
          o.with("label", relabelName.get(o.id)).with("config", relabelCfg.get(o.id))
        else o)
    // Fresh objects for the created set, ids max+1 upward.
    val base = maxId(board) + 1
    val fresh = plan.created.fold({ next: base, set: Set() }, (acc, c) =>
      { next: acc.next + 1,
        set: acc.set.union(Set({
          family: c.family, id: acc.next, label: c.name,
          anchor: c.anchor, config: c.config })) })
    survivors.union(fresh.set)

  // ---- disruption classing (ADR-0015 decision 12) ----

  // The three classes, low to high. hitless — a set-label relabel or an
  // attribute assert, no traffic effect. boundary — no traffic-path change, but
  // an externally-held name (a netdev name, a VPP allowlist token, decision 6)
  // leaves the system. disruptive — a create, a destroy, a rewire.
  type Class = Hitless | Boundary | Disruptive

  pure def classRank(c: Class): int =
    match c { | Hitless => 0 | Boundary => 1 | Disruptive => 2 }

  pure def maxClass(cs: Set[Class]): Class =
    cs.fold(Hitless, (acc, c) => if (classRank(c) > classRank(acc)) c else acc)

  // The class of one matched relabel/assert, given the object as it stood before
  // (`old`), the compiled object it now carries, and every name the plan writes
  // (`newLabels`). Decision 12:
  //   - name unchanged  -> hitless (a config-only change is an attribute assert);
  //   - old label ""     -> hitless (a drift repair, decision 9: no external name
  //                         was ever held, so nothing externally-held changes);
  //   - old label reclaimed elsewhere in the plan (newLabels contains it) ->
  //                         hitless: the externally-held name SET is unchanged,
  //                         the name only moved to another object (the swap and
  //                         any name-cycle, decision 10);
  //   - otherwise        -> boundary: the old name leaves the externally-held set.
  // A relabel is never disruptive: it is `set-label`, decision 9's repair verb.
  pure def pairClass(old: Observed, c: CompiledObject, newLabels: Set[str]): Class =
    if (old.label == c.name) Hitless
    else if (old.label == "") Hitless
    else if (newLabels.contains(old.label)) Hitless
    else Boundary

  // The plan's headline class (decision 12: dry-run reports the maximum, converge
  // gates on an explicit allow of it, disruptive is never implied). Every create
  // and every remove is disruptive; each matched pair contributes its pairClass.
  pure def planClass(preBoard: ObservedBoard, plan: Plan): Class =
    val newLabels = plan.pairs.map(p => p.compiled.name).union(plan.created.map(c => c.name))
    val pairClasses = plan.pairs.map(p => pairClass(observedById(preBoard, p.id), p.compiled, newLabels))
    val churn = (if (plan.created != Set()) Set(Disruptive) else Set())
      .union(if (plan.removed != Set()) Set(Disruptive) else Set())
    maxClass(pairClasses.union(churn))

  // ---- the cone of an edit (ADR-0015 decision 12, the frame law) ----

  // The board objects OUTSIDE a set of touched construct names — the frame law's
  // subject. cone(c) in this abstract layer is "the objects whose label is a
  // name the edit touches": a leaf construct is one object, so an edit touching
  // construct names `touched` reaches exactly the objects those names label.
  // The honest ceiling: the Rust twin's cone (task 6.5) is the derived-object
  // CLOSURE — a tenant edit reaches its companions and edges — because that layer
  // models the full plan; here one object per construct makes the cone the
  // touched names' objects, and the sibling renumbering an edit triggers in the
  // real compile is invisible (decision 8), which is precisely what the frame law
  // then shows leaves the outside-cone objects byte-identical.
  pure def outsideCone(board: ObservedBoard, touched: Set[str]): ObservedBoard =
    board.filter(o => not(touched.contains(o.label)))
}
