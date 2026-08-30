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
- [x] 4.3 Build the batch-suite generator: simulator traces →
      reviewable scripts with inline expectations, reference-pair
      assertion, result files for offline diff; mutating-suite gate on
      the recovery guarantee
- [x] 4.4 Build the online driver: operator-launched, step/pause/abort,
      transcript, per-step confirmation in learning mode

## 5. Board program (operator-run, serial; design D7)

- [x] 5.1 Generate the recovery-guarantee suite (scratch mutation set,
      reboot, diff vs DPL baseline); operator runs it; guarantee
      verified or the program stops (board milestone)
- [x] 5.2 Generate the full object-lifecycle sweep (scratch DPRCs,
      unwired dpmacs, unconditional teardown); operator runs; diff,
      amend models/baseline, freeze divergence traces
- [x] 5.3 Generate link-signaling V-LINK-1..5 (dpmac.7/9, per-run
      flags); operator runs; diff and fold back
- [x] 5.4 Root-container dpdbg/dprtc probes under the online driver,
      per-step confirmation; dprtc.0 destroy only after 5.1; UART
      reroute excluded; freeze findings
- [x] 5.5 Generate traffic-bearing reachability scenarios (V-TRAF
      patterns, dpmac.7/9 flagged, no rate targets); operator runs;
      diff and fold back
- [x] 5.6 Ledger pass: fold all board results into COVERAGE.md;
      board-pending rows settled or re-anchored
- [x] 5.7 Read-back sweep (board sitting): one scratch-container suite
      creating a bare dpni, bare dpdmai, NO_CHANNEL dpio, dpbp and
      dpdcei with a hook reading each back via `info`; settles the
      defaults the lifecycle suites never read (DPNI-I7, DPDMAI-I5,
      DPIO-I3, DPBP-I5, DPDCEI-I1's version half); inventory row added
- [x] 5.8 Harness: every step's stderr and MC status land beside its
      exit file, the teardown trap saves the sitting's kernel-log
      window, and the operator instruction archives results outside
      the checkout — refusal evidence and ADR-0008 markers become files,
      not prose, before the refusal sitting
- [x] 5.9 Refusal probes (board sitting): default-built dpsw at kernel
      probe, dpdmux uplink→dpni connect, move of a plugged object,
      dpaiop create, dpseci and dpni create-validation and dead-option
      exit shapes — hooks where an object must stand, probe plans
      otherwise; settles DPSW-I1/I2, DPDMUX-I8, DPRC-I3, DPAIOP-I1/I2,
      DPSECI-I2, DPNI-I6
- [x] 5.10 Read-only observations (board sitting): `dpmac info` on the
      unwired ports, `mc.global`/`dump-mem`, child-container rescan
      visibility through sysfs, lock/unlock round-trips, the option-bit
      permission matrix, `generate-dpl` emit-and-diff, and the stray
      unknowns no invariant row carries (dpci 3–5, dprtc 6, dprc 2/6/11);
      settles DPMAC-I7, DPRC-I6/I11, DPCI-I3, DPAIOP-I3/DPDCEI-I2/DPDMAI-I4
- [x] 5.11 Riskier set, one design note each before generation (board
      sitting, sequenced last): pool exhaustion in a scratch container
      without starving boot residents (V-POOL-1..3), netdev runtime
      state across unbind/rebind (V-DPNI-3), the peer-request channel
      through the dpmac.7 netdev (V-LINK-4), the dpdmai reboot cycle
      (V-DPDMAI-2); each settled or re-anchored with its reason

- [x] 5.12 Companion draw, measured (board sitting): V-POOL-4 creates
      dpios and dpnis one at a time in a scratch child and reads the
      firmware's MC-portal pool after each, so "one dpmcp per
      portal-consuming object, including each dpio" is board-settled
      and ADR-0012's open question 1 gets its number; design note
      before generation, as in 5.11

## 6. Evidence automation (self-standing; gates close-out)

- [x] 6.1 Ledger lint: a cargo test cross-checks COVERAGE.md against
      the baseline invariant tables, the suite ledger and the roadmap
      (ids both ways, tally, cited suites exist, owning changes named,
      baseline status cells level); legend carries the restool-rendering
      and single-board caveats
- [x] 6.2 Machine-readable verdicts: `diff`/`drive` write verdict.json
      per run and append to a committed VERDICTS.json index; existing
      results back-filled; the lint resolves every "verified" cell to a
      passed verdict
- [x] 6.3 Board snapshot and diff: full tree, per-object info, driver
      links and API versions as redacted JSON; one clean-boot reference
      committed; replaces the head-count census, structures the
      recovery diff, and is the #10 differential gate's format
- [x] 6.4 MC status register: refusal codes as a lint-checked table
      seeded with the five seen so far; plans carry expected refusals
      that `diff` scores from captured stderr
- [x] 6.5 Ioctl command-policy coverage: the reference kernel's fsl-mc
      uapi whitelist as a committed table; a test checks every command
      the models and adapter emit against it — rows outside it are the
      exact scope of a kernel patch or of a VFIO userspace transport

- [x] 6.6 Regenerate the committed ITF traces from the current model so
      every trace carries the verb channel (`lastVerbs`) and the ioctl
      policy is checked live by `cargo test`, not only by Apalache
- [x] 6.7 Four committed plans (V-DPNI-1, V-DPRC-1, V-LIFE-DPNI-1,
      V-RECOVERY-1) name a `trace_file` under `models/traces/` that no
      longer exists — the traces moved beside their suites before the
      field was ever read back. Point them, and the matching script
      headers, at the trace beside the suite, and add a test that every
      committed plan's `trace_file` exists so a moved trace fails in
      `cargo test`, not in a sitting

## 7. Close-out

- [x] 7.1 Verify every spec scenario of `formal-models` and
      `mbt-harness` against the produced artifacts
- [x] 7.2 Cross-link models ↔ baseline docs ↔ ledger ↔ ADRs (baseline
      status cells brought level with the ledger); append
      upstream findings from divergences; quality floor green (cargo
      build/fmt/clippy/doc/test + model ladder)
- [ ] 7.3 Close beads, validate the change, ready for archive
