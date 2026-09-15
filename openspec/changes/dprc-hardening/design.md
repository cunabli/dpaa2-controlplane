# dprc-hardening design

## Context

Proposed 2026-09-14 straight from the dprc-encapsulation epic-review
synthesis (`openspec/changes/dprc-encapsulation/review/synthesis.md` —
merged clusters M1–M13, beads B1–B9, open items OI-1..5). The review is
the design work: findings arrived verified, clustered, and with
acceptance-shaped dispositions. This design records only the decisions
the synthesis left open and the execution shape. Priority balance,
per the proposing session: efficiency – correctness – cost, in that
order of tension resolution (parcel context reuse for efficiency,
severity order for correctness, board time spent only where the desk
cannot answer).

## Decisions

### D1 — Tasks are fixer-context parcels, not finding numbers

The synthesis's B1–B9 group by severity cluster; a fixer would re-read
the same files across four beads. Tasks here regroup by
files-in-context: Parcel A (prune soundness: `dprc_plan.rs`,
`engine.rs`, `fake.rs`, convergence fixtures — B3→B4-core→B1+B2-api, TDD
order, the failing fixture lands first), Parcel B (southbound shim:
`restool.rs`, `parse.rs`, `runner.rs` — B2-producer+B6, after Parcel A
so the `ObservedContainer` re-keying lands before its producer follows),
Parcel C (model witnesses: `dprc.qnt`, traces, `dprc_replay.rs` —
B5+B4-replay, after the verb-aware attribution signature exists).
B-numbers survive as traceability inside each task's acceptance.

### D2 — DPRC-I8 is owned by pool-objects (#6)

OI-2 resolved at propose time: the COVERAGE.md row (written at task 1.2
with full board context) already says #6, the baseline hedges "#6 or
#10", and #6 is the earliest tile that can reach the face (a DPL-defined
child arrives with pool machinery; the raw command path waits for #10).
Earliest reachability wins; the five prose artifacts still saying #10
are the stale side and are repointed by task 1.1. Reversible: if #6
scoping finds the DPL-child route insufficient, the pointer moves to #10
with an ADR note, and the long-term probe bead moves with it.

### D3 — V-DPDBG-2 was a legitimate ride-along; record, don't re-attribute

OI-4 resolved at propose time: the sitting ran under task 6.1/bead
cd3.14 by the epic's own commits (1eefc75, 1051e71) on an open board
window. Task 1.1 adds the missing task row to the reviewed change's
tasks.md §6 so COVERAGE.md:180's attribution has an anchor; nothing is
re-attributed.

### D4 — The sitting is filed first but blocks nothing

OI-1 (duplicate-id-under-lock ordering) and OI-3 (dpmcp budget) are one
scratch-safe serial probe pair, filed as task 2.1 immediately because
operator scheduling has latency — but no code task depends on them:
B1's Locked handling rests on the settled lock⇒0x4 fact, not on the
open ordering question. The sitting task carries its own doc updates
(ADR-0002 note or model/doc amend for OI-1; COVERAGE/baseline row for
OI-3), so task 6.1 stays pure desk work and never waits on the board.

### D5 — Long-term board work becomes standing beads, not tasks

The DPRC-I8 probe is unreachable today (a restool-created child is
unplugged and restool refuses `--plugged` on a dprc — the very reason
the deferral row exists), and the `observe_container(id)` seam (B7) plus
the dpmcp-budget consequence belong to tile #5's shape. Both become
standalone P3 beads pinned to their tiles instead of tasks this change
must drag; not overcaution — reachability.

### D6 — Protected decisions stay protected

No task may widen the D8 prune fingerprint, re-key `dprc::ResidentId`
(the ADR-0014 twin of `dprc.qnt` — only the *observation* type
`dprc_plan::ObservedContainer.residents` re-keys to `ObjectRef`), move
the derivation out of `dpaa2-api`, or split the D3 one-sum machine. The
review's grounding list carries into every parcel prompt.

## Risks / Trade-offs

- [Parcel A re-keys a type Parcel B produces] → strict A→B ordering; the
  one-writer-per-crate check enforces no overlap in flight.
- [Sitting results may contradict the Rust lock-first ordering (OI-1)]
  → PASS2-F5 is already dispositioned as an ADR-0002 note either way;
  only prose moves, no plan logic depends on the ordering.
- [F12 test fold changes fixture semantics (`Counted(18)` →
  `Observed{18}` + a labels seed)] → fold is sealed only with
  `cargo test` green on both suites, else dropped as not-worth-it.

## Migration Plan

No deployed state; level-triggered reconciler. The reviewed change
archives after task 1.1 (its review directory travels with it); this
change's own deltas merge at its archive.

## Open Questions

- OI-1 (lock-strip vs enabling-precondition ordering on the real MC) —
  **resolved off-board 2026-09-15**: a duplicate id is not constructible
  through restool (no id-pinning create; ids mint lowest-free in one
  global namespace per family, ADR-0010), so the MC's duplicate check is
  unreachable and no sitting can discriminate the order. Quint directed
  evidence confirmed the model's precondition-first guard; recorded as a
  note on ADR-0002 (PASS2-F5). The divergence is deliberate and moot,
  revisited only if the #10 ioctl tile adds an id-carrying create.
- OI-3 outcome (dpmcp budget draw per restool spawn) — **pending** the
  V-DPRC-13 sitting (task 2.1); reprioritizes the tile-#5 seam bead if it
  is a leak.
