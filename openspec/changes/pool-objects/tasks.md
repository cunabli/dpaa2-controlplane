# pool-objects — tasks

Phases run strictly 1→5 (design D8); inside each phase the family
traversal is dpmcp → dpbp → dpcon → dpio. One task at a time through
its acceptance criteria; each implementation task is an
opus48-developer parcel.

## 1. Models

- [x] 1.1 Stand up `models/families/pool_lifecycle.qnt` (design D7;
      ADR-0019 Quint module architecture): the pattern-owned machine
      with the allocator-custody cycle for the trio — free-pool
      membership, draw, return with DPBP-I3 dirty return, ceiling
      refusal at the census; trio family modules gain only family
      types + invariant index pointing at it (formal-models req 1);
      Apalache-mark the invariants.
- [x] 1.2 dpio joins pool_lifecycle: seat arithmetic and the
      probe-time dpmcp draw (DPIO-I1/DPMCP-I1) as machine
      transitions, seat-regime vocabulary and the DPCON-I4
      notification-edge type as family types,
      NO_CHANNEL/priority surface for DPIO-I3; a divergence earning a
      dpio_lifecycle module of its own is recorded as D4 facet
      evidence.
- [x] 1.3 Encode the convergence and prune laws in pool_lifecycle
      with its directed runs, baseline-id runs in main.qnt
      (formal-models req 2): idempotence, free-only shrink,
      ShrinkBelowDraw refusal, DPL-born exemption; resolve the open
      question of invariant id placement against COVERAGE
      conventions; model gate green.

## 2. dpaa2-api (P3 implementation)

- [x] 2.1 The generic P3 shape for the trio: census/sizing types over
      FamilyParams, three instantiations, isomorphic to the phase-1
      sums; ADR-0019 gains its P3 reference-implementation line when
      this lands (reconciler req 1).
- [x] 2.2 The count-drift disposition: grow/shrink/refuse + prune
      deltas at the count level, dispatch-edge boundary documented
      (design D2); unit tests from the phase-1 laws (reconciler
      req 2).
- [x] 2.3 dpio seat variant — carries the ADR-0019 judgment marker:
      acceptance includes judging the cfg hazard class against a
      concrete refusal or probe and amending ADR-0019 with a cfg
      facet only if earned (design D4; marker bead).

## 3. dpaa2-mc (adapter)

- [x] 3.1 Create/destroy verbs for the four families; delta→id
      resolution with free-only victim selection; read-back as the
      only observation (mc-backend req 1).
- [x] 3.2 Kernel root-bind face: dpaa2-eth bind, per-target probe
      read-back, typed -EPROBE_DEFER observation (mc-backend req 2).
- [x] 3.3 Child population + VFIO handoff over the dprc-encapsulation
      typestates (mc-backend req 3).
- [x] 3.4 Root-scope pool convergence joins the operator surfaces
      (discovered at 4.1: V-POOL-6 found engine::ensure container-only,
      the engine's own tile-#6 deferral): ensure/dry-run/status census
      the root per family against the compiled plan's derived counts
      and drive drift_disposition→dispatch_pool_deltas — grow,
      free-only shrink, prune, ShrinkBelowDraw surfaced as a refusal
      (design D3); prerequisite for the 4.2 convergence walk and 4.3
      (bead 960.16).

## 4. Suites

- [x] 4.1 Offline: suite generation for the pool walks; frozen ITF
      traces replay in cargo test as conformance twins (mbt-harness
      req 1).
- [x] 4.2 Board: census/ceiling and convergence walks at root scope;
      free/drain observed through the kernel face; record COVERAGE
      row advances (pre-run record commit per board protocol).
- [ ] 4.3 Board: the two MVP scenarios — live kernel interface in
      dprc.1; populated VFIO-bound dprc.N; drift-heal and teardown
      re-runs (system-integration req 1).
- [ ] 4.4 GATED — DPL-defined-child escape: only if 4.1–4.3 leave a
      routed invariant unreachable at root scope; names the
      invariant, operator-approved before any boot-config write
      (design D5; bead 5y7).

## 5. Close-out

- [ ] 5.1 COVERAGE.md rows routed to #6 updated with dispositions;
      ROADMAP row #6 marked; ADR records that fired during the change
      verified in place (design D6).
- [ ] 5.2 Quality floor green (scripts/checks/quality-floor.sh);
      epic-review; beads closed; archive readiness.
