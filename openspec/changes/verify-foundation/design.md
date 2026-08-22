## Context

Change #1 (`restool-baseline`, archived) produced the documented ground
truth: 16 family baselines, `object-model.md`'s five relationship views,
105 invariant candidates (34 marked **Breaking:**), the traffic
inventory, and the pinned reference pair (MC 10.39.0 + Linux 6.6.52 +
restool v2.4). ADR-0002 declared the formal-methods process and ADR-0003
the board protocol — both accepted, neither yet exercised. The repo
already ships one board-validated slice of Rust: the reconciler's
dpni↔dpmac association (ADR-0001). The recovery guarantee — "a reboot
restores the DPL baseline" — is still an explicit assumption
(ADR-0003 §7) that must be verified before any mutating suite runs.

This change is where the process meets the machine: models become
executable, invariants become checkable, and the board confirms or
corrects them — before any new Rust is written. The decisions below were
settled in the 2026-08-22 proposal session and are recorded here so
tasks transcribe, not re-litigate, them.

## Goals / Non-Goals

**Goals:**

- A verified foundation across the **entire** project surface: Quint
  models of the object lifecycle for all 16 families, validated against
  the board through operator-run, scripted, serial suites.
- The recovery guarantee verified first, unblocking every mutating suite
  in this change and the series.
- The `dpaa2-verify` harness (adapter, batch generator, online driver,
  ITF replayer) with the port safety envelope coded in.
- The CI ladder every later change's model gate runs on.
- An honest coverage ledger: every invariant candidate modeled,
  deferred-to-named-change, or board-pending — no silent drops.

**Non-Goals:**

- No new typestate Rust for object families (changes #3 onward); the
  existing reconciler is a modeling *subject*, not modified.
- No TLA+ up front — escalation is per-model, recorded when taken
  (ADR-0002 §1).
- No performance traffic: traffic-bearing scenarios prove reachability
  through configured object groups only. If a later scenario needs
  rate, it is flagged then and the cn10k set up accordingly.
- No dpdbg UART reroute (open ADR-0003 safety question) and no dpmac.3
  / dpmac.17 / dpni.0 interaction of any kind.

## Decisions

### D1. Full ROADMAP row 2, one change — no split

Models, invariant encoding, the `dpaa2-verify` crate, the CI ladder, the
recovery-guarantee verification, and the full-surface board suites all
land in this change, as the roadmap wrote it. Alternatives (model-only
now with the crate deferred; board-free crate with suites deferred) were
rejected in session: the foundation is only "verified" when the board
has answered, and the requirement is the entire surface with no
watering down.

### D2. One core lifecycle model; the reconciler retro-model is its first instantiation

The core Quint model makes `object-model.md`'s five views executable —
containment (the DPRC tree), connect edges, create-vs-allocate,
allocation pools, lifecycle ordering — as one generic object machine
with per-family parameters (placement gates, cardinality, pool
membership, reset-on-bind class). The dpni↔dpmac association flow
already encoded and board-validated in the reconciler is modeled *on*
the core as its first instantiation — retro-modeling known ground so
the first model↔board and model↔Rust comparisons run where reality is
already understood. The model carries each transition's evidence status
(board-exercised via prior work vs source/restool-read) so a failed
board step distinguishes "model wrong" from "claim was never verified".

### D3. All 105 invariant candidates attempted; touched-families-first; documented fallback

Every candidate from the 16 family docs is attempted as a named Quint
invariant or temporal property, the 34 **Breaking:** ones faithfully
(they are what the model must NOT assume — most encode as the *absence*
of a convenient axiom plus a property that would catch code relying on
it). Traceability rule: the model-side name is the baseline id
(`DPRC-I6`, `DPNI-I2`, …) so the ledger, the docs, and the model never
drift apart. Encoding order: families already touched by prior work
(dprc, dpni, dpmac, the pool quartet, dpseci) first, then Tier B, then
Tier C — so the recorded fallback (narrow to touched families if the
full set proves intractable) truncates the tail instead of reworking
the head.

### D4. Model repo layout and the marked-invariant convention

`models/` at top level: `core/` (one module per view plus the shared
state), `families/` (per-family parameter modules importing core),
`retro/` (the reconciler instantiation), `traces/` (committed frozen
ITF traces). Each model header lists its named invariants and marks the
subset for Apalache symbolic checking; unmarked invariants run in the
simulator only. Escalation-to-TLA+, if ever taken, is recorded in the
same header (ADR-0002 §1–2).

### D5. CI ladder wiring

Quint installs through pnpm beside the existing openspec dependency;
Apalache is version-pinned. Ladder per ADR-0002 §6, cheapest first:
`quint typecheck` → `quint run` (simulator, all invariants) →
`cargo test` ITF replay (frozen traces against the reconciler) →
Apalache on marked invariants. Exposed as pnpm scripts locally and one
new CI job; board replay is never CI (ADR-0003).

### D6. `dpaa2-verify` crate architecture

One adapter binds the three faces (ADR-0002 §4): model action ↔ restool
command ↔ observed board state, with observation = read-back (never
exit status — DPNI-I6, DPMAC-I8 made that a law). Four components over
it:

- **Batch-suite generator**: consumes Quint simulator traces, emits
  reviewable shell scripts — every restool command visible, the model's
  expected post-state as a comment beside each step, the reference-pair
  assertion first, results captured to files for offline diffing.
- **Port safety envelope**: the ADR-0003 §4 matrix and §5 traffic
  classes are data in the crate, enforced at generation *and* wrapped
  around execution; a trace whose class exceeds its named ports is
  refused, dpmac.3/dpmac.17/dpni.0 are unreferenceable.
- **ITF replayer**: replays committed traces against the Rust core in
  `cargo test`; the same trace that validated the model against the MC
  validates the code against the model, board-free, forever.
- **Online driver**: operator-launched, step/pause/abort, full
  transcript, per-step confirmation until a family's model survives a
  full batch suite (ADR-0003 §3). Built and exercised here on the
  root-container probes (mandatory per-step anyway); its first
  discovery-heavy target remains change #4.

### D7. The board program: recovery first, then the full surface, serial

All board work is files the operator reviews and runs, strictly
serially, results returned as files (ADR-0003 §1–2). Order:

1. **Recovery-guarantee verification** — capture state, apply a scratch
   mutation set, reboot, diff against the DPL baseline snapshot from
   `reference-environment.md`. Green here unblocks every mutating suite
   (ADR-0003 §7 stops being an assumption).
2. **Object-lifecycle sweep** — the full §1 inventory (V-DPRC-1..5,
   V-DPNI-1..3, V-DPMAC-1..2, and the per-family lifecycle scenarios)
   in scratch DPRCs with unconditional teardown, unwired dpmacs only.
3. **Link-signaling** — V-LINK-1..5 on dpmac.7/9, each run explicitly
   flagged.
4. **Root-container probes** — dpdbg/dprtc scenarios under the online
   driver's per-step confirmation (they cannot be scratch-contained);
   dprtc.0 destroy runs only after step 1 proved re-DPL-ability; the
   UART reroute stays out.
5. **Traffic-bearing reachability** — V-TRAF-pattern scenarios on
   flagged dpmac.7/9: frames traverse the configured object group,
   reachability asserted, no rate targets.

Divergences at any step amend the model and the baseline document in
the same change (ADR-0002 §2), and every board-discovered behavior is
frozen into a committed trace.

### D8. Mellanox decision point fires early: flagged dpmac.7/9, revert question stays open

Pulling traffic-bearing scenarios into this change makes it the first
traffic-bearing phase, so ADR-0003 §8's decision point applies now, not
at #9. Session outcome: option (a) — careful, per-run-flagged use of
dpmac.7/9 against the production peer — is exercised for
reachability-level traffic in this change; the device-tree revert
(option b) is *not* foreclosed and is re-decided at #9 when sustained
traffic suites arrive. ADR-0003 is amended in place to record this.

### D9. The coverage ledger is a first-class artifact

`models/COVERAGE.md`: one row per invariant candidate — baseline id,
model location (or deferred-to-named-change, or board-pending with the
settling scenario id), CI rung it runs at, board status after each
suite lands. The ledger is the honesty mechanism: a candidate absent
from the model is a decision on record, never an omission.

### D10. tasks.md is an entry point to beads (carried D11 pattern)

Granular tasks, dependencies, and DoD gates live as bd issues under a
`verify-foundation` epic; `tasks.md` lists phases and bead anchors. One
bead at a time through acceptance. Board sittings are sync points
marked on their beads; the serial suite order of D7 is encoded as bead
dependencies.

## Risks / Trade-offs

- [105 invariants may exceed what Quint/Apalache handle gracefully in
  one change] → touched-families-first ordering plus the recorded
  fallback: truncate to touched families, ledger the rest as deferred
  with owning changes named; simulator-only is an acceptable rung for
  unmarked invariants.
- [The operator is the critical path for a large serial board program]
  → suites are batched into sittings (beads mark the sync points);
  everything is reviewable files, so sittings can be split and resumed
  without the assistant in the loop.
- [Traffic on dpmac.7/9 faces a production peer] → reachability only,
  minimal frame counts, per-run flags, cn10k side limited to interfaces
  the operator already operates; nothing restarts or reconfigures the
  peer.
- [The recovery check itself mutates the board before the guarantee is
  proven] → its mutation set is the smallest scratch-DPRC set that a
  reboot must erase; it runs first precisely because everything after
  it leans on the answer; a red result stops the board program and the
  series' mutating work until understood.
- [Root-container probes cannot be scratch-contained] → online driver
  per-step confirmation is mandatory there regardless of promotion
  state; dprtc.0 destroy additionally gated on the recovery check.
- [Two artifacts (model + Rust) to keep honest] → ITF replay in CI is
  the standing guard; amend-in-place is the rule (ADR-0002 §2).

## Open Questions

- Which invariants Apalache can check symbolically vs simulator-only —
  answered empirically as encoding proceeds; the marks and any TLA+
  escalation are recorded per model header.
- Whether the DPL baseline snapshot captured in change #1 suffices as
  the recovery-diff reference, or the recovery script must re-capture
  its own pre-state each run — settled by the first recovery run.
- What, if anything, the cn10k side needs scripted for reachability
  checks (likely only reading counters on the two existing interfaces)
  — settled when the V-TRAF scripts are drafted; the peer stays
  operator-configured either way.
