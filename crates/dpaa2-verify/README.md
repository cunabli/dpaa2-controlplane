# dpaa2-verify

The model-based-testing harness of the control plane: the crate that
binds the Quint model corpus under `models/` to the Rust code and (in
later phases) to the board. Requirements live in
`openspec/changes/verify-foundation` (`mbt-harness` spec); the design
is D6 of that change's `design.md`.

## What it does today

The **ITF trace replayer** (phase 3): frozen model traces in
`models/traces/*.itf.json` replay against the pure reconciler
(`dpaa2_api::reconcile`) on every `cargo test`, no board attached.
This is rung 3 of the model validation (`pnpm model:replay`); the
full run is `pnpm model:validation` — see `models/README.md` for the
other rungs.

The **MBT harness** (phase 4), four components over one seam:

- `adapter` — the shared mapping binding the three faces: each core
  machine action to the restool/sysfs command that drives it (or an
  explicit *await* for steps the board takes by itself), to the
  read-back probes observing its outcome, and to the expectation
  those probes must confirm. Conformance is judged on read-back
  alone; exit status is auxiliary evidence, never an observation
  (DPNI-I6, DPMAC-I8). Input is `quint run --mbt` traces, which name
  each action taken and its parameters.
- `safety` — the ADR-0003 port matrix and traffic classes as data,
  enforced independently at generation (over the model trace) and at
  execution (over every rendered command): dpmac.3 / dpmac.17 /
  dpni.0 unreferenceable anywhere, class ceilings on the wired pair,
  link/traffic runs refused unflagged.
- `generate` — model trace → reviewable board suite: a shell script
  (every command visible, expected post-state beside each step,
  reference-pair assertion first, results to files, unconditional
  teardown trap, embedded total-deny self-check) plus a plan file the
  harness diffs offline against the results. Mutating suites are
  refused until the recovery guarantee is verified (marker file
  committed by task 5.1); the recovery-verification suite itself must
  mutate only the scratch set it creates.
- `driver` — the operator-launched online walk: step/pause/abort,
  per-step confirmation in learning mode (always, for root-container
  scenarios), free-run with stop-on-divergence once promoted, and a
  JSONL transcript of every action, expectation, and observation.

```sh
cargo run -p dpaa2-verify -- generate --trace t.itf.json --id V-X-1 --out suites/
cargo run -p dpaa2-verify -- diff --plan suites/V-X-1.plan.json --results results/
cargo run -p dpaa2-verify -- drive --trace t.itf.json --transcript run.jsonl   # on the board
```

## How the replay works

A frozen trace is a sequence of MC-legal states produced by the core
machine's guards. The reconciler is level-triggered — observe, plan,
apply the whole plan, re-observe — so each trace is segmented into
*epochs* at the reconciler's observation points. At each observation
state the replayer:

1. reduces the ITF state to what an observer can see (`itf.rs`:
   DPNIs, DPMACs, connection edges, kernel-bind as the netdev proxy),
2. projects that to an `ObservedTopology` and runs `reconcile_with`,
3. diffs the plan against the steps the model actually took next
   (`replay.rs`), including the wait-to-observe `Bind` the plan
   promises before the kernel delivers it.

One reconciler plan step spans several machine actions (`Create` is a
whole companion-provisioning chain); the classifier in `replay.rs` is
the single place that mapping lives. Sub-steps an observer cannot see
— companion creates, plug flips, bus rescans, pool draws — never
surface as plan steps.

Out of replay scope, covered by `dpaa2-api`'s own unit tests: MAC
assert/actuate (the model carries no MAC values), fixed-link ports,
foreign-object preservation, immutable-attribute drift.

## Running

```sh
cargo test -p dpaa2-verify     # or: pnpm model:replay
```

`tests/retro_replay.rs` replays every committed trace and keeps one
deliberate mis-replay (teardown without prune) proving the diff
catches wrong decisions.

## Adding or regenerating a trace

1. Add/extend a directed run in `models/retro/reconciler.qnt`,
   documenting its observation points in the run's comment.
2. `pnpm model:freeze` — rewrites `models/traces/{test}.itf.json`.
3. Transcribe the observation points into the `TRACES` table in
   `tests/retro_replay.rs` (file, port anchor, presence, prune,
   observation state indices).

The epoch table and the trace must move together: a regenerated trace
whose shape changed fails the replay loudly rather than drifting. The
last observation index must be the trace's final state, where the
plan must be converged.

Board-discovered divergences (phase 5) follow the same path: amend the
model and the owning baseline document in the same change, freeze the
reproducing trace, and the replay guards it thereafter (ADR-0002 §2).
