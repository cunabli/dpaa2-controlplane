# dprc-hardening

## Why

The dprc-encapsulation epic review (four passes + synthesis,
`openspec/changes/dprc-encapsulation/review/synthesis.md`) returned YES on
both epic-level claims — the lifecycle sum is ADR-0002-isomorphic and the
pure core carries the change (D2) — but found the prune-dispatch surface
unsound on exactly the two states an undeclared container will present
(Locked, plugged residents), with the gap invisible to `cargo test`
because every fixture seeds `plugged:false`. It also found three sans-io
leaks in the shim, a tautological ITF assertion, unwitnessed model arms,
and archive-gating spec drift (a prune delta that contradicts the base
capability's SHALL NOT without a MODIFIED block). This change lands those
findings as fixes; dprc-encapsulation archives after its gate task here.

## What Changes

- **Archive gate (docs on the reviewed change)**: the reconciler delta
  gains the MODIFIED block restating the ownership fence with the
  container carve-out (M3); the formal-models delta states the
  Apalache/simulate split honestly (M4); the DPRC-I8 deferral pointers
  converge on `pool-objects` (#6) across all artifacts (M5, decision D2
  below); the V-DPDBG-2 ride-along gets its task row (OI-4).
- **Prune soundness (cluster M1/M2)**: the resident census is keyed by
  family-qualified `ObjectRef` (the bare-`ResidentId` collision at
  `restool.rs:451` is reachable today); `plan_teardown` honors `Locked`
  (gap, no doomed steps); prune refusals are typed per-candidate
  (`PruneOutcome::Refused`, MC 0x10 → `Teardown::ResidentPlugged`) and
  never abort the pass or skip re-observation; the shim stops judging
  lifecycle (`ContainerState::classify` moves to the core; resident
  origin reported `Option<ResidentKind>`); the fake refuses destroy with
  0x10 while a resident is plugged, with a fixture that fails first.
- **Attribution carries the verb (M6/M7)**: `attribute_mc` takes the
  refused verb (lock-strip vs topology-gap discrimination);
  `attribute_refusal` relocates from the tools shell into
  `dpaa2_api::dprc_plan`; the ITF replay asserts per-arm expected
  statuses instead of a tautology.
- **Model witnesses (M9)**: frozen traces for accepted move-out/unplug, a
  lock-strip sweep over the five untested `Container<Locked>` refusals,
  and the three task-1.4 runs; `Unlocked::Empty` witnessed. Existing runs
  only — the D8 fingerprint and the recorded empty-label escape are
  untouched.
- **Shim hygiene (M10/M11)**: one classified error exit for every
  `McControl` verb; signal death is `Error::Backend`, not a client-guard
  refusal; four mechanical duplicate folds.
- **Board sitting (OI-1 + OI-3)**: one scratch, serial, self-cleaning
  probe pair — duplicate-id-under-lock ordering and the per-boot dpmcp
  budget — filed early for operator scheduling; it blocks no code task.
- **Doc polish (M8/M13)**: `--prune` help names container teardown;
  ADR-0001 §4 amendment; ADR-0017 consequence corrected + crate citation
  anchor; baseline "rev 2" antecedent; bind/unbind idempotent-refinement
  sentence; proposal Impact of the reviewed change corrected
  (`dpaa2-config` → `dpaa2-api`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `reconciler`: prune planning honors observed state (Locked ⇒ typed gap,
  no doomed steps); per-candidate typed prune outcomes with unconditional
  re-observation; census keyed by family-qualified object references;
  attribution is verb-aware.
- `mc-backend`: the shim reports observations raw (no lifecycle
  judgment); one classified error exit per verb; signal death is a
  backend error.
- `formal-models`: witness coverage — frozen traces for the accepted
  move-out/unplug arms, the lock-strip refusal sweep, and the DPRC-I12
  runs; the replay asserts expected refusal statuses per attribution arm.

## Impact

- Crates: `dpaa2-api` (dprc_plan re-keying, verb-aware attribution,
  ContainerState::classify, fake 0x10), `dpaa2-mc` (observation producer,
  exit hygiene, folds), `dpaa2-tools` (prune outcomes, help text, fixture),
  `dpaa2-verify` (replay assertions, new traces).
- Models: `models/families/dprc.qnt` directed runs (witnesses only),
  `models/COVERAGE.md` rows, frozen traces +3.
- Docs: ADR-0001 §4 amendment, ADR-0017 correction, `docs/baseline/dprc.md`
  antecedent fix, the reviewed change's delta/prose amends (task 1),
  ROADMAP row #4 hardening note at delivery.
- Deferred (recorded, not built): `observe_container(id)` seam + dpmcp
  budget disposition → tile #5 (long-term bead); DPRC-I8 board probe →
  first DPL-defined-child window under tile #6 (long-term bead); the
  nominated rule amendment (a delta overriding a base SHALL must carry a
  MODIFIED block) recorded with the process checks.
