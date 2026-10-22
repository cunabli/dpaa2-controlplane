# Tasks

## 1. Model gate (ADR-0002: Quint first)

- [x] 1.1 Grow `models/families/dpsw.qnt` to the P1 phase-machine shape
      with the reset facet (design D1): immutable twelve-field create
      block, the DPSW-I1 bindability predicate as type law, DPSW-I2
      vendor-recipe refusal, census on the observable cfg signature
      (design D6). Verify: ladder typecheck + simulate green on the
      module and its instantiations.
- [x] 1.2 Fill the two ADR-0022 placeholder rows in `core/connect.qnt`
      (design D5: reification policy + port-bound connects, no teardown
      witness) without touching any other kind's law. Verify: existing
      frozen connection-surface traces replay unchanged; new edge
      actions simulate.
- [x] 1.3 Mark and check the Apalache invariants (DPSW-I1, DPSW-I2,
      DPSW-I7 census judge, DPSW-I9 probe-consumer, DPSW-I10 port
      bound; DPSW-I4 reset facet parameterized conservative) and freeze
      the ITF traces for the new behaviors; record pure operators
      hand-twin-sufficient (dpseci-hardening convention). Verify:
      marked-Apalache green within ladder bounds; traces committed
      beside the model.
- [x] 1.4 Sync `models/COVERAGE.md` DPSW-I1…I10 dispositions with
      anchors per the named-disposition requirement (consumer-owned
      rows → ADR-0004 §8; board rows → the suite ids of group 7).
      Verify: ledger lint passes; no implicit row.

## 2. Intent layer (vocabulary + derivation)

- [x] 2.1 Add the switched-domain construct to the accepted vocabulary
      and the pure derivation compiler with per-rule provenance
      (intent-compiler delta; design D3 forced fields). Verify: unit
      tests per derivation rule; dry-run names each rule.
- [x] 2.2 Add the typed regime refusal (design D4: non-kernel steering
      refused at compile) and the derivation invariants run. Verify:
      refusal unit test + invariant suite green.
- [x] 2.3 Parse the construct in `dpaa2-config` (topology-config delta:
      name-keyed, linted enums, forced fields absent from the schema).
      Verify: round-trip parse test; forced-field refusal test.

## 3. Reconciler core (isomorphic Rust)

- [x] 3.1 Add `crates/dpaa2-api/src/families/dpsw.rs` structurally
      isomorphic to the model: constructor-gated create block (design
      D2), lifecycle typestates, census judgment. Verify: unit tests
      mirror the model laws; an unbindable cfg does not compile or is
      constructor-refused.
- [x] 3.2 Add the plan surface: companion placement → create → port
      connects → plug ordering, destroy-and-recreate disruption
      classing, the two edge-kind rows in `dpaa2-api::plan` (reconciler
      delta). Verify: plan unit tests incl. companion-starvation and
      port-overflow refusals.
- [x] 3.3 Wire read-back-only convergence (design D6): typed
      honest-unknown observation faces feed the census; exit codes
      never judge. Verify: silent-destroy-failure and
      blanked-connection-line unit tests.

## 4. Southbound shim (restool verbs)

- [x] 4.1 Drive create/destroy/info through the shim (taildrop update
      out of scope — observation-only, proposal Out-of-scope),
      rendering the compiled block verbatim with create-time labeling
      (mc-backend delta). Verify: render unit tests field-for-field;
      ioctl-policy resolution test covers the new verbs.
- [x] 4.2 Parse `dpsw info` into the typed observation (per-if block,
      raw-escape options, absence ≠ zero counters, `Unobservable`
      connection face). Verify: parse unit tests over captured fixture
      output incl. truncation and missing-line cases.

## 5. CLI surface

- [x] 5.1 Render the read-only switch detail row in
      `dpaa2ctl status --detail` (provisioning-cli delta). Verify:
      snapshot test incl. an `Unobservable` port face.

## 6. Verify harness (twins, suites, capture tap)

- [x] 6.1 Add MBT twins: property twins over the pure operators and
      ITF frozen-trace replay for the dpsw lifecycle and edge kinds.
      Verify: `cargo test` replay green; `lastVerbs` resolve against
      the ioctl-policy module.
- [x] 6.2 Render the board program: one discovery suite (DPSW-I4 reset
      scope, DPSW-I5 bpid law, FDB budget) and one witness suite
      (end-to-end switched-domain), traffic class per ADR-0003,
      reference pair asserted. Verify: generated suites pass the
      harness lint; V-DPSW-1/4 verdicts untouched.
- [x] 6.3 Add the capture tap to the suite driver preamble (design D7:
      strace-wrapped spawns, offline payload parse, loud preamble fail)
      and the corpus parse round-trip check. Verify: offline test —
      tapped and untapped renders drive identical steps; dead-tap test
      fails the preamble.

- [x] 6.4 Wire the switched-domain pass into `dpaa2ctl ensure`
      (system-integration delta's typed path; found missing by the 7.2
      offline pre-check, bead dbf.22): dispatch the task-3.2 plan after
      the links pass (create via the shim verbs, interposition sever
      first, read-back judgment only), tear down an undeclared root
      dpsw under the double gate in the baseline recipe order (port
      disconnects → kernel unbind → destroy; allocator-claimed root
      companions get `driver/unbind` before any `--plugged=0`), and
      refuse typed any compiled construct family without an executor
      (exhaustive-match completeness guard). Companion placement is
      co-residency in the dpsw's container (root), not a child dprc —
      the 3.2 plan shape is amended in its own seal. Verify: engine
      unit tests over the fake backend (create, interposition,
      teardown, guard refusal); offline pins and quality floor green.

## 7. Board milestone (operator in loop)

- [x] 7.1 Pre-run record commit after offline gates (board-suite
      protocol), then the discovery sitting; feed divergences back to
      model/baseline before proceeding. Verify: suite verdicts
      recorded; model ladder green after any amendment.
- [x] 7.2 Witness sitting: the switched-domain suite records the
      STRUCTURAL end-to-end verdict (converge, interposition, kernel
      bind, idempotence, typed teardown) with the capture tap banking
      the #10 fixture corpus. The reachability-traffic leg is deferred
      with a named revisit trigger (the change #15 driver-occupancy
      ordering guard plus the kernel refresh; adjudication on bead
      dpaa2-controlplane-dbf.19). Verify: structural verdict recorded in
      the ledger; corpus committed beside results with parse round-trip
      green.

## 8. Docs and gates

- [x] 8.1 Amend `docs/baseline/dpsw.md` (unknowns answered by group 7;
      DPSW-I9/I10 added to the invariant-candidate table),
      ADR-0013 (vocabulary), ADR-0022 (table rows), ADR-0019
      (P1-instance note), ROADMAP row #11 status. Verify: anchored-refs
      hook clean; docs cite suite verdicts.
- [ ] 8.2 Quality floor green across the workspace
      (`scripts/checks/quality-floor.sh`) and spec deltas validated
      (`openspec validate --change dpsw-typestate`). Verify: both
      commands exit 0.

## Workflow follow-up

- Beads epic creation and per-task claiming (deferred — no beads work
  in the proposing session).
- Epic review (`/epic-review dpsw-typestate`) before archive; archive
  via `/opsx:archive` after acceptance.
- The banked fixture corpus hands off to `mc-portal-backend` (#10) at
  its scoping, alongside the bead dpaa2-controlplane-94k dossier.
