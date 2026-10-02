# dpmac-typestate — tasks

Six dependency-ordered phases (design D1–D7); one kind of work per
phase; one bead at a time through acceptance. Model gate before Rust
(roadmap DoD #2); board sitting is the only operator-critical step.

## 1. Model

- [x] 1.1 Grow `models/families/dpmac.qnt` into the P4 reference shape:
  arbitration phase sum with observation-judged transitions, the two
  directional link channels (requests-down typed Unreadable), the
  firmware-version-indexed counter vocabulary (10.39's 28; 10.40 named
  unread), MAC immutability; invariants named under their baseline ids;
  typecheck + simulate green.
- [x] 1.2 Type the dpni–dpmac edge teardown law at the connection
  surface (sever consumes KernelOwned → Offered + severed witness;
  unbind demands the witness for dpmac-facing edges only); Apalache
  green on the marked invariants; frozen traces committed; existing
  edge-kind traces replay unchanged.
- [x] 1.3 COVERAGE sync: DPMAC-I2/I3/I4(reachable)/I6/I7 rows
  deferred→modeled with rungs named; out-of-scope re-anchors recorded
  loudly (requests-down channel, MC-view link read, bulk statistics →
  #10 restool-absence ledger rows; DPRTC-I4 off #7); ledger lint green.

## 2. Rust core

- [x] 2.1 `crates/dpaa2-api/src/families/dpmac.rs`: isomorphic
  typestates, the MAC relation judgment
  (Inherited/Overridden/Pending/Mismatched, Pending never drift), the
  `Known | NotInVocabulary` counter types; TDD unit tests; quality
  floor green.
- [x] 2.2 The severed-witness edge type on the connection surface, with
  the negative face asserted (the unbind-before-sever order does not
  typecheck) and the planner consuming the types; existing plan tests
  for non-dpmac edges unchanged.
- [x] 2.3 MBT conformance twins in `dpaa2-verify`: frozen-trace replay
  of the arbitration transitions and edge law; property twins for the
  vocabulary and MAC-relation judgments; ITF replay green.

## 3. Adapters

- [x] 3.1 `dpaa2-mc` shim reads for dpmac attributes/MAC/counters: one
  spawn per dpmac, vocabulary-checked parse, deviating row count as a
  typed version-signal, dead-spawn (`assert(false)` hazard) as a typed
  observation failure; shim tests against captured fixtures.
- [x] 3.2 `dpaa2-hal` sysfs carrier primitive: per-arbitration netdev
  resolution (dpni netdev / macN), NoObservable for the driverless
  case; policy-free; reference-pair property assertion hook for suites.

## 4. Product

- [x] 4.1 `dpaa2ctl` read-only port-detail view (arbitration state, MAC
  relation, carrier, counters) wired into status; display-only —
  no field gates convergence; integration tests.

## 5. Board

- [x] 5.1 Generate Suite A (e2e typestate suite: intent on dpmac.7,
  hooks for arbitration/MAC/attr-constancy/counters/carrier, typed
  sever-then-unbind teardown, RemoteOwned leg) inside the safety
  envelope; offline gates green; pre-run record commit.
- [x] 5.2 Author Suite B (V-DPMAC-2 phantom create, scratch-child
  contained, own teardown + census, root face recorded untaken);
  offline gates green; pre-run record commit.
- [x] 5.3 Operator sitting: Suites A + B, one sitting, Suite B late;
  verdicts into VERDICTS.json and the suite ledger; divergences triaged
  implementation-first; banked verdicts cited, never re-run.

## 6. Close-out

- [ ] 6.1 Baseline and docs sync: `docs/baseline/dpmac.md` amendments
  (unknown #1's answer, carrier observability), ADR-0019 amendment (P4
  reference landed; phase-marker promotion trigger fired; edge
  teardown-law facet), ROADMAP row #7, CHANGELOG via commits; full
  quality floor; epic review per standing practice.
