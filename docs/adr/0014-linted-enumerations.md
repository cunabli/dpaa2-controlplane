# ADR-0014: Hand-maintained enumerations of a checkable program are linted copies, never siblings

- **Status:** Accepted — 2026-09-02. Promotes design D9 of the archived
  OpenSpec change `verify-foundation` (its only prior home) to a standing
  law that binds every change, not just the one that first needed it.
- **Date:** 2026-09-02
- **Supersedes / relates to:** design D9 of the archived `verify-foundation`
  change (`openspec/changes/archive/2026-08-30-verify-foundation/design.md`),
  promoted here; ADR-0002 (formal methods — the model is the source of
  truth this law keeps prose honest against); ADR-0013 §Consequences (the
  first record to slate its own enumerations for the lint, bead
  `dpaa2-controlplane-gqf.34`).

## Context

A checkable program — a Quint model, a Rust type surface, a compiled
derivation — owns its rules in one place a machine reads. Prose repeats
those rules for a human reader: an invariant table, a refusal vocabulary, a
witness list, a coverage narrative, an enumeration in an ADR. The repeated
copy has no compiler, so it drifts, and a drifted copy is worse than none —
the reader trusts a restatement the program no longer honours.

Two proofs stand on record that the drift is real, not hypothetical.

- **The core D9 ledger lint.** The `verify-foundation` change kept several
  hand-maintained ledgers describing one invariant/scenario program from
  different angles — the `models/COVERAGE.md` disposition ledger, the
  `docs/baseline/dp*.md` family tables that are the invariant source of
  truth, the `models/board/` suite ledger, and the roadmap change series.
  Left as siblings they would disagree, so `crates/dpaa2-verify/src/ledger.rs`
  parses each into plain rows and applies cross-checks R1–R6 (id agreement
  both ways, tally recount, cited-suite directory, roadmap-row citation,
  owning-change on open cells, baseline-vs-ledger status agreement); a
  disagreement fails in CI rather than in review. This is D9 working as
  intended, and it is the model for every enumeration since.

- **The intent-layer `COVERAGE.md` drift.** While the intent vocabulary was
  being built, its coverage narrative in `models/COVERAGE.md` fell out of
  step with the model across tasks 2.6b and 2.6c — two commits that each
  passed the model gate, because no lint yet cross-checked that narrative.
  The drift survived both and was caught only incidentally in task 2.6d, by
  a reader who happened to notice. A gate that green-lights a drifted copy
  twice is not a gate; the miss is what this ADR exists to prevent.

D9 named this mechanism but lived only inside a change that is now archived,
so nothing carried it forward. This record promotes it to standing law.

## Decision

A hand-maintained table that restates rules owned by code or a model is a
**linted copy, never a free-standing sibling.** Concretely:

1. **What the law covers.** Any prose enumeration of rules whose authority
   lives in a checkable program — invariant tables, refusal vocabularies,
   witness/scenario lists, coverage narratives, and the enumerated sections
   of an ADR that restate a model or a type surface. If the reader could act
   on the copy as if it were the rule, the copy is in scope.

2. **The same-change obligation.** A change that adds or edits such a table
   discharges one of two duties in that same change — never in a follow-up:
   - it **extends the lint** — adds or updates the ledger-lint row (an R-rule
     in `crates/dpaa2-verify/src/ledger.rs`) that cross-checks the table
     against its source of truth; or
   - it **names the bead** that will add that lint, cited in the table's own
     text, so the gap is on record with an owner rather than forgotten.

3. **Until the lint lands, the program wins.** A table awaiting its lint
   states plainly that the checkable program is the source of truth and it
   is the reader's copy — exactly as ADR-0013 §Consequences already does.

The rule is deliberately blunt: a restatement either has a machine that
keeps it honest or has a named bead that will give it one. There is no third
disposition, because "a careful author will keep it in step" is precisely
the assumption the intent-layer drift falsified.

## Consequences

- Enumerations stop being a source of quiet rot. The 2.6b/2.6c class of
  drift becomes a red test instead of a lucky catch, and a reader may trust
  any linted table as far as its CI is green.
- The lint is a growing surface, not a fixed one. Each new enumeration adds
  its own R-rule or its own bead; the `dpaa2-verify` ledger lint is where
  they accrue, alongside the four ledgers R1–R6 already guard.
- ADR-0013 is the first record to comply: its §Consequences declares the 24
  refusal variants (§5), the INTENT_I1–I9 invariants (§6), and the five
  scenarios (§7) hand-maintained copies of `models/intent/*.qnt`, slated for
  the ledger lint in phase 3 under bead `dpaa2-controlplane-gqf.34` — the
  first application of this law.
- The cost is one obligation per table at authoring time. That is cheaper
  than the alternative this ADR replaces: a reader misled by a copy the
  program stopped honouring, discovered only when someone happens to look.

## References

- `openspec/changes/archive/2026-08-30-verify-foundation/design.md` §D9 —
  the coverage ledger as a first-class, honesty-bearing artifact (promoted
  here).
- `crates/dpaa2-verify/src/ledger.rs` — the core lint: four hand-maintained
  ledgers cross-checked by rules R1–R6.
- `models/COVERAGE.md` — the disposition ledger, and the narrative whose
  2.6b/2.6c drift (caught in 2.6d) is the second precedent.
- ADR-0013 §Consequences — the first record to slate its own enumerations
  for the lint; bead `dpaa2-controlplane-gqf.34` (the intent ledger rows).
- ADR-0002 — the formal-methods process that makes the model the source of
  truth this law keeps prose honest against.
