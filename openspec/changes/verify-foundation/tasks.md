# Tasks

Entry point only (design D10): granular tasks, dependencies, and
acceptance criteria live as bd issues under the `verify-foundation`
epic; one bead at a time through acceptance. Board suites (phase 5) are
strictly serial — the bead graph encodes design D7's order — and each
board task is an operator sitting sync point.

## 0. Tracking and toolchain scaffold

- [x] 0.1 Create the `verify-foundation` beads epic and one issue per
      task below, wiring dependencies (0 → 1 → 2 → 3 → 4 → 5 serial →
      6) and instantiating the six-point DoD per issue
- [x] 0.2 Add the Quint toolchain via pnpm and pin Apalache; create the
      `models/` skeleton (core/, families/, retro/, traces/,
      COVERAGE.md stub) and the `dpaa2-verify` crate skeleton in the
      workspace
- [x] 0.3 Amend ADR-0003 §8: the Mellanox decision point fires in this
      change; record flagged dpmac.7/9 for reachability-level traffic
      now, revert question re-decided at change #9 (design D8)

## 1. Core lifecycle model

- [x] 1.1 Encode the shared object machine: state, containment tree,
      create-vs-allocate, pools, connect edges, lifecycle ordering
      (object-model.md §§1–5), with per-transition evidence status
      (design D2)
- [x] 1.2 Add per-family parameter modules for the touched families
      (dprc, dpni, dpmac, dpbp, dpio, dpcon, dpmcp, dpseci)
- [x] 1.3 Add Tier B (dpsw, dpdmux) and Tier C (dpaiop, dpci, dpdcei,
      dpdmai, dprtc, dpdbg) parameter modules
- [x] 1.4 Typecheck + simulator green on canonical forward-order and
      teardown traces; forbidden transitions refused

## 2. Invariant encoding and coverage ledger

- [x] 2.1 Encode the six cross-cutting laws (object-model.md §6) and
      the touched families' candidates under their baseline ids,
      Breaking ones as prohibited-assumption encodings (design D3)
- [x] 2.2 Encode Tier B, then Tier C candidates; fallback checkpoint —
      if intractable, truncate here and ledger the tail as deferred
      with owning changes named
- [x] 2.3 Mark the Apalache subset per model header; Apalache green on
      marked invariants; record any TLA+ escalation taken
- [x] 2.4 Fill `models/COVERAGE.md`: every candidate modeled /
      deferred-to-named-change / board-pending-with-scenario-id

## 3. Retro-model and ITF replay

- [x] 3.1 Retro-model the reconciler's dpni↔dpmac association flow as
      a core-model instantiation; freeze its traces
- [x] 3.2 Build the ITF replayer in `dpaa2-verify`; retro traces green
      against the existing reconciler in `cargo test`
- [x] 3.3 Wire the full CI ladder (typecheck → simulate → ITF replay →
      marked Apalache) as pnpm scripts and a CI job

## 4. MBT harness

- [x] 4.1 Implement the shared adapter: model action ↔ restool command
      ↔ read-back observation (never exit status)
- [x] 4.2 Encode the port safety matrix and traffic classes as data;
      enforce at generation and in the execution wrapper (dpmac.3,
      dpmac.17, dpni.0 unreferenceable)
- [ ] 4.3 Build the batch-suite generator: simulator traces →
      reviewable scripts with inline expectations, reference-pair
      assertion, result files for offline diff; mutating-suite gate on
      the recovery guarantee
- [ ] 4.4 Build the online driver: operator-launched, step/pause/abort,
      transcript, per-step confirmation in learning mode

## 5. Board program (operator-run, serial; design D7)

- [ ] 5.1 Generate the recovery-guarantee suite (scratch mutation set,
      reboot, diff vs DPL baseline); operator runs it; guarantee
      verified or the program stops (board milestone)
- [ ] 5.2 Generate the full object-lifecycle sweep (scratch DPRCs,
      unwired dpmacs, unconditional teardown); operator runs; diff,
      amend models/baseline, freeze divergence traces
- [ ] 5.3 Generate link-signaling V-LINK-1..5 (dpmac.7/9, per-run
      flags); operator runs; diff and fold back
- [ ] 5.4 Root-container dpdbg/dprtc probes under the online driver,
      per-step confirmation; dprtc.0 destroy only after 5.1; UART
      reroute excluded; freeze findings
- [ ] 5.5 Generate traffic-bearing reachability scenarios (V-TRAF
      patterns, dpmac.7/9 flagged, no rate targets); operator runs;
      diff and fold back
- [ ] 5.6 Ledger pass: fold all board results into COVERAGE.md;
      board-pending rows settled or re-anchored

## 6. Close-out

- [ ] 6.1 Verify every spec scenario of `formal-models` and
      `mbt-harness` against the produced artifacts
- [ ] 6.2 Cross-link models ↔ baseline docs ↔ ledger ↔ ADRs; append
      upstream findings from divergences; quality floor green (cargo
      build/fmt/clippy/doc/test + model ladder)
- [ ] 6.3 Close beads, validate the change, ready for archive
