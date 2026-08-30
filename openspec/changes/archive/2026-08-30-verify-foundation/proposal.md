## Why

The baseline (change #1) documented the entire restool/MC object surface
and distilled 105 invariant candidates across the 16 families — but they
are prose propositions: nothing executes them, nothing can catch a wrong
belief before it becomes Rust, and the recovery guarantee that underwrites
every future mutating board suite is still an unverified assumption
(ADR-0003 §7). This change stands up the formal-methods foundation
ADR-0002 declared — Quint primary, TLA+ escalation, dual-mode model-based
testing — and validates it against the board across the entire surface,
so the intended typestate contracts are machine-checked and
board-confirmed before any new Rust is written.

## What Changes

- Add `models/` — the Quint model corpus: a core object-lifecycle model
  that makes `docs/baseline/object-model.md`'s five views executable
  (containment/DPRC tree, connect edges, create-vs-allocate, allocation
  pools, lifecycle ordering), plus a retro-model of the one flow already
  encoded in Rust — the reconciler's dpni↔dpmac association — built on
  the core model as its first instantiation.
- Encode the baseline's invariant candidates as named Quint invariants
  and temporal properties. All 105 are attempted, the 34 **Breaking:**
  ones faithfully preserved (write-only state, exit-status weakness,
  tri-state visibility, two id spaces, …). Documented fallback if the
  full set proves intractable: narrow to the already-touched families;
  encoding is ordered touched-families-first so the fallback truncates
  rather than reworks.
- Add the model CI ladder (ADR-0002 §6): typecheck → simulate → ITF
  replay against the Rust core → Apalache on invariants explicitly
  marked for symbolic checking; wired as pnpm scripts and a CI job.
- Add the `dpaa2-verify` crate — the dual-mode MBT harness over one
  shared adapter (model action ↔ restool command ↔ observed state):
  batch-suite generator emitting reviewable scripts with expectations
  inline, the operator-launched online driver (step/pause/abort,
  transcript, per-step→per-block promotion), the ITF replayer running
  frozen traces in `cargo test`, and the port safety envelope enforced
  at generation and execution (ADR-0003 §4–5).
- Board milestones, operator-run, scripted, serial (ADR-0003 §1–3):
  first the **recovery-guarantee verification** (reboot restores the DPL
  baseline) gating everything else; then full-surface batch suites — the
  complete object-lifecycle sweep in scratch DPRCs, link-signaling
  V-LINK-1..5 on flagged dpmac.7/9, the root-container dpdbg/dprtc
  probes under per-step confirmation (dprtc.0 destroy sequenced after
  the recovery check that unblocks it; the dpdbg UART reroute stays
  excluded as an open safety question), and traffic-bearing V-TRAF
  scenarios at reachability level on flagged dpmac.7/9. Divergences
  amend the model and the baseline documents in the same change.
- Add the coverage ledger: every invariant candidate accounted for as
  modeled, deferred-to-named-change, or board-pending — with board
  results folded in as suites complete.
- **BREAKING** (process, not code): ADR-0003 §8 is amended — pulling
  traffic-bearing scenarios into this change makes it the first
  traffic-bearing phase, so the Mellanox decision point fires early;
  the recorded outcome is careful flagged use of dpmac.7/9 for
  reachability-level traffic now, with the device-tree revert question
  still open for re-decision at change #9.

## Capabilities

### New Capabilities

- `formal-models`: the executable Quint model corpus — core object
  lifecycle, retro-model instantiations, named invariants and temporal
  properties, the coverage ledger, and the CI ladder every later change's
  model gate runs on.
- `mbt-harness`: the `dpaa2-verify` crate — shared adapter, batch-suite
  generator, online driver, ITF replayer, and the coded port-safety
  envelope; the machinery every later change's conformance gate and
  board milestone run on.

### Modified Capabilities

- (none — `object-baseline` documents get content amendments where board
  evidence diverges, per their own amend-in-place rule; no requirement
  of an existing capability changes)

## Impact

- New workspace crate `crates/dpaa2-verify`; new top-level `models/`;
  `package.json` gains the Quint toolchain; CI gains the model-ladder job
  (Apalache runs under the JVM already present in CI images).
- The existing reconciler crates are consumed read-only as the
  retro-model's subject and the ITF replayer's target; no behavior
  change to shipped code.
- Board: the first mutating suites of the series. All mutations confined
  to scratch child DPRCs except the declared root-container probes and
  one reboot for the recovery check; every script asserts the pinned
  reference pair (MC 10.39.0 + Linux 6.6.52) before acting; dpmac.3 and
  dpmac.17/dpni.0 remain total-deny everywhere.
- Docs: ADR-0003 §8 amendment; baseline family documents and
  `object-model.md` amended in place as board evidence lands; upstream
  findings appended when divergences implicate restool/MC/kernel.
