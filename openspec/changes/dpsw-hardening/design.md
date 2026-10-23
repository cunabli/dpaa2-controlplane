# Design

## Context

See proposal.md — Why. The change executes the dpsw-hardening epic;
every child bead is self-contained (its description carries the
tasks.md group verbatim: sites, decided direction, acceptance,
verification), so this design records only the decisions already made
at review synthesis and the constraints that bound every parcel. The
one-task-at-a-time rule applies; group order is 1 → {2, 3, 4, 6} → 5 →
7, with 7 (the pre-archive sweep) gating `/opsx:archive` of
dpsw-typestate.

Standing constraints inherited from dpsw-typestate (all protected by
the review brief; a parcel that touches one fails review):

- **D2/D3 construction law**: the bindability predicate gates
  construction; flooding/broadcast/ctrl-if stay derivation outputs with
  no northbound knob. Group 2 adds a bound, never a knob.
- **D5 witness-free dpsw edges**: both dpsw edge kinds stay
  `DisconnectOnly` with no teardown witness; group 3 asserts that
  negative, never mints one.
- **D6 census discipline**: convergence stays census + read-back only;
  group 5 is preview composition, group 1 is consumption of the typed
  plan — neither adds a judgment outside dpaa2-api, and no detail-row
  field gates convergence.
- **Sans-io**: judgments live in dpaa2-api; adapters execute/observe.
- **Bounded lift**: dpmac machinery stays claimed, not retyped —
  `models/traces/families/dpmac/` and the 8 dpsw family traces stay
  byte-identical across the whole change; `fabricAcceptedTrace.itf.json`
  is the ONLY trace group 4 re-freezes.
- **Board facts are the oracle**: V-DPSW-2 rev 2 and V-DPSW-5 rev 6
  (structural) are recorded verdicts; nothing here re-runs a sitting.
  The reachability deferral and its #15 trigger are untouched.
- **ADR-0023/#15 scheduling**: no Quint convergence model of the
  ensure pipeline lands here — group 1 folds dispatch onto the
  existing typed plan, group 5 factors an already-mutation-free loop.

## Decisions (made at synthesis, recorded here)

- **D1 — fold direction for the dead plan surface (M1): the engine
  consumes the api plan.** `dispatch_switch_create` executes
  `plan_switched_domain`'s `SwitchStep`s; the interposition dispatch
  matches all `InterposeStep` arms with the proof bound. Alternative —
  delete `SwitchPlan` and amend tasks.md 6.4 — rejected: it leaves
  DPSW-I10's refusal carrier-less and rewrites sealed task text.
- **D2 — the membership bound lands on both sides together (M2).**
  Model (`dpswDerivationLawful`, `dpsw_attrs_well_formed`) and Rust
  (`refuse/fabric.rs`) move in one group so the ADR-0002 isomorphism
  never breaks mid-change.
- **D3 — the provenance anchor stays single-line data (M6).** The
  board-anchored wording names suite ids (V-DPSW-1, V-DPSW-4) as data;
  ADR ids stay out of the string (intent-compiler spec constraint).
  The anchor is an ADR-0002 twin frozen into a trace and a snapshot,
  so the group is regen-bearing by construction.
- **D4 — pre-archive spec-delta edits are legal (synthesis ruling).**
  "Amendable until sealed" seals the commit trail, not the change
  directory: group 7's edits to dpsw-typestate's spec deltas land as a
  NEW commit under the `Change: dpsw-typestate` trailer before
  `/opsx:archive` — never as fixups into sealed history.
- **D5 — rule outcomes (synthesis §5)**: R1 accepted reshaped — every
  sitting's pre-run record includes a named offline pre-check (render
  the suite + run its intents against the fake backend end to end) and
  any typed refusal kills the sitting before it starts. R2 — sealed
  task text naming an overturned design decision gets a one-line
  bracketed supersession note at the close-out doc bank; annotate,
  never rewrite.

## Risks / Trade-offs

- [Group 1 touches the witnessed ensure path] → the board-witnessed
  behavior is pinned by the existing engine tests (create→connect→plug
  order, plug-last, interposition sever-first) plus
  `vdpsw5_intents.rs`; all stay green unchanged — only the carrier of
  the ordering moves.
- [Group 4 regen churn] → the re-freeze is scoped to one intent trace
  and one snapshot; the byte-stability gate over family traces catches
  any spill.
- [Group 7 edits a sealed change's directory] → design D4 above; the
  commit-msg hook's close-then-commit still applies (the group's bead
  closes first).

## Migration Plan

Additive hardening; no persisted state, no backend change, no board
sitting. After group 7 lands, dpsw-typestate archives via
`/opsx:archive` with the review directory included.

## Open Questions

None — every group executes a synthesis decision.
