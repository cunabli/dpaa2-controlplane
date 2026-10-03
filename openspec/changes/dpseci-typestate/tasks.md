# dpseci-typestate — tasks

Six dependency-ordered phases (design D1–D8); one kind of work per
phase; one bead at a time through acceptance. Model gate before Rust
(roadmap DoD #2); the board sitting is the only operator-critical step.
Parcels: model and suite generation → opus48-developer; crate work →
rust-developer; every parcel spec carries the settled-decisions block.

## 1. Model

- [x] 1.1 Grow `models/families/dpseci.qnt` into the P2 cfg shape
  (design D1): queue count 1..16, length-coupled priorities each 1..8,
  closed options vocabulary with raw escape, restool-layer refusal
  surface replaying banked V-DPSECI-1, DPSECI-I1 by construction,
  DPSECI-I4 as birth capability; consumer-facing consequences only;
  typecheck + simulate green.
- [x] 1.2 COVERAGE sync (design D7): DPSECI-I1/I4 deferred→modeled with
  rungs named; I3 → implemented-at-adapter; I2 MC layer, I5 board face,
  and I9 re-anchored loud with each fence stated (#10); ledger lint
  green.

## 2. Rust core

- [x] 2.1 `crates/dpaa2-api/src/families/dpseci.rs`: isomorphic
  typestates (design D1/D5), refined newtypes, options vocabulary with
  escape, congestion capability from `HAS_CG`, `compile_fail`
  immutability doctest; TDD unit tests; quality floor green.
- [ ] 2.2 Derivation completes the create surface (design D3/D4):
  compiled dpseci carries priorities `[2; num_queues]` and options
  `{HAS_CG}` with rule provenance; immutable-cfg repair plans as
  destroy+create; existing derivation tests unchanged.
- [ ] 2.3 MBT conformance twins in `dpaa2-verify`: frozen-trace replay
  of create/refuse transitions, property twins for length-coupling and
  range judgments; ITF replay green.

## 3. Adapters

- [ ] 3.1 `dpaa2-hal` MC-ioctl read primitive (design D2): closed
  command sum (OPEN / GET_ATTR / GET_API_VERSION / DPSECI_GET_TX_QUEUE
  / CLOSE), confined unsafe with layouts asserted against the pinned
  sources, three typed outcomes, fixture tests; policy-free.
- [ ] 3.2 `dpaa2-mc` dpseci paths: restool create/destroy dispatch
  (mandatory pair, computed options mask, presence read-back on
  destroy), `info` parse with no options field, GET_ATTR read path over
  the hal primitive with typed unobservable outcome; shim tests against
  captured fixtures.

## 4. Product

- [ ] 4.1 `dpaa2ctl status --detail` dpseci row (queues, priorities,
  observed options, API version, binding state; unknown rendered
  honestly when the read path is unavailable); display-only;
  integration tests.

## 5. Board

- [ ] 5.1 Generate Suite A inside the safety envelope (design D6): e2e
  `[[crypto]]` converge on a scratch tenant, dual-transport read-backs,
  VFIO RemoteOwned leg, typed teardown + census, read-only boot-object
  hooks (unknowns #2/#3, driver link); offline gates green; pre-run
  record commit.
- [ ] 5.2 Operator sitting: Suite A, one sitting; verdicts
  V-DPSECI-2 rev 1 and V-DPSECI-3 rev 1 into VERDICTS.json and the
  suite ledger; divergences triaged implementation-first; banked
  verdicts cited, never re-run.

## 6. Close-out

- [ ] 6.1 Docs sync: `docs/baseline/dpseci.md` amendments (unknowns
  #2/#3 answers, read-slice observability note), the new ADR for the
  ioctl read slice (design D2), ADR-0019 note (P2 member landed),
  ROADMAP row #8, CHANGELOG via commits; full quality floor; epic
  review per standing practice.
