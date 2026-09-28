# pool-hardening — design

## Context

The synthesis ledger (archive/2026-09-27-pool-objects/review/synthesis.md §1) already carries the per-finding dispositions; this design records only the cross-cutting choices. All work is parcel-shaped: the main loop writes parcel specs from the bead acceptance lines, dispatches to the pinned Opus 4.8 developer agent, gates, and commits per bead (one task at a time, per repo rules 3–4).

## Goals / Non-Goals

**Goals:**
- Close the typed-surface caveat: child-scope pool refusals typed on the path the real backend actually exercises.
- Make every asserting surface bind the production guard's named accessor; earn the `itf-replay` marks with per-state asserts.
- Cover the DpdkSeat regime with a frozen trace.
- Retire text residue by deletion where possible (user terseness directive).

**Non-Goals:**
- No retro-model re-freeze (Pass 2 OQ3 ruling: classifier granularity hides the pre-D9 expansion; re-freeze is reserved for classifier-visible changes).
- No reversal of D9/D10/D12/ADR-0020 or the no-cfg-facet judgment; no DPL edit; no CHANGELOG hand-edits.
- No `qnt container()` helper fold (PASS2-F8: fold only when a third machine appears).

## Decisions

1. **Model-first ordering (user directive).** Bead C leads; B's dpio.qnt child-seat-surplus question is answered model-side before its Rust wiring; G's retro-qnt reframe lands early. Influence flows model→Rust; within C, qnt edits land before their Rust twins.
2. **Strengthen, not soften (L8).** The `itf-replay` marks are kept and `check_state` gains the per-state asserts (drawn disjoint unplugged; born-present) — ~3 lines — rather than softening COVERAGE. The marks then state what the replay executes.
3. **Rule amendment adopted.** Repo rule 12 lands in `.claude/agents/{rust-developer,opus48-developer}.md`: an asserting surface binds the same named accessor the production guard consumes, cited by name; an `itf-replay` mark requires a per-state assert of the named law. One rule fences both recorded classes (V-POOL-6 rev 1–3, 3.14 audit) and both in-epic recurrences (PASS2-F5/F6). Adopting it in this change makes C its first compliant instance.
4. **Both discovery paths in one bead (A).** `converge_population` and the probe `-EBUSY` discovered-draw path type together — on the restool backend the probe path is the only real surface, so typing one alone types only the fake path (Pass 3 OQ1 merge).
5. **DpdkSeat coverage is a new directed freeze (L25).** The dpio freeze harness gains a directed DpdkSeat run; the trace freezes and replays offline. Per Pass 2 OQ1: offline-cheap → new freeze work here, not bead 5y7. Remaining unreplayed arms (root shrink suppression, probe-skipped refusal, D12 simulate-only) are marked honestly in COVERAGE (bead D), not force-frozen.
6. **Type-only relocation (H).** `PoolFamilyDrift`/`PoolDrift`/`PlannedChildDpni`/`ChildPlan` move to `dpaa2-api::plan` beside `ContainerPlan`; observing/judging fns stay adapter-side. Render loses its dpaa2_mc+engine imports for pure data; the seat-deficit arithmetic duplicated engine/populate folds into the moved types.
7. **Freeze idempotence is the gate, not an assumption.** `pnpm model:freeze-pool && pnpm model:freeze-dpio && git diff --exit-code models/traces` re-runs after the new freeze (it ran clean at review close, HEAD 9abb7fa).

## Risks / Trade-offs

- [New per-state asserts could fail on existing frozen traces] → that is the point; a failure is a real twin-fidelity defect, triaged before softening anything (strengthen-not-soften holds).
- [Directed DpdkSeat run may be nondeterministic across freezes] → the idempotence gate (`git diff --exit-code`) catches it; the run seeds/dirs like the 15 existing traces.
- [H's type move churns imports across three crates] → mechanical, gated by `scripts/checks/quality-floor.sh`; no behavior change intended, replay+unit suites must stay green.
- [Rule 12 written too broadly could misfire on unit tests that legitimately probe internals] → scoped to suites/replays/COVERAGE marks that claim a production law, mirroring the synthesis wording.

## Open Questions

None blocking. B's dpio.qnt child-surplus semantics are resolved inside bead B's model-side half (that resolution is the task, sequenced before its Rust wiring).
