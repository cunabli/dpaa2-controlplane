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
- [x] 3.5 Design amendment — single provider (discovered at 4.3's
      opener: the per-port provisioning chain and the pool construct
      double-feed one functional pool; the kernel allocator ignores
      labels): the pool construct is the sole provider of pool-family
      objects at root; per-port dpmcp/dpbp/dpcon creation ceases and a
      departing port leaves surplus for free-only shrink; ADR-0015
      companion custody narrows to child-container population
      (bead 960.17).
- [x] 3.6 Shim: create_dpni sheds the provisioning chain
      (ensure_dpio + provision_chain/dpni_dep_steps) and becomes
      create+stamp; port teardown stops destroying companions; the
      disconnect --endpoint flag fix rides along (design D9;
      bead 960.18).
- [x] 3.7 Derivation: per-declared-port draws fold into the pool
      requirement (+1 dpmcp, +1 dpbp, +num_queues dpcon per port;
      dpio seats unchanged per design D4); vpool6_intents re-pins the
      operands and V-POOL-6 expectations follow (design D9;
      bead 960.19).
- [x] 3.8 Model: pool_lifecycle gains an external-consumer
      environment (external draw, drawn DPL-born, labeled-other
      cohabitant) with directed runs replayed as re-frozen twins, so
      census divergences fail offline first (design D9;
      bead 960.20).
- [x] 3.9 Model — the plug facet (discovered at 4.3's authoring audit,
      2026-09-24: the plugged⇒drawn census proxy makes managed surplus
      unreclaimable and the divergence lived in the observation
      mapping, below the model's state space): pool_lifecycle splits
      allocatable (plugged, DPBP-I2) from drawn; reclaim is the
      unplug-probe law (unplug of drawn refused — the board's in-use
      refusal is the drawn signal; plugged-free unplugs then destroys);
      dpio residue is a typed reboot-required disposition; teardown
      walk (consumers before pool shrink) as a directed run; twins
      re-frozen with plugged explicit in the mapping (design D10;
      bead 960.22). Model first — the oracle the Rust twin validates
      against.
- [x] 3.10 Custody twin: census/select/dispatch conform to 3.9 —
      plugged distinct from drawn, unplug-probe shrink/prune verbs,
      the two label judges (judge_label vs PoolMembership) reconciled
      into one law (board rev1/rev3: the out-of-band empty-label dpbp
      was never a prune candidate); 960.21's fold-direction finding
      re-judged over the widened census (design D10; bead 960.23).
- [x] 3.11 Routing: ports actuate in their planned container — a
      root-only projection (keyed by the compiled plan's dpni
      container) feeds the port loop and link::apply; child port-edges
      leave the transitions for the population plan; render snapshots
      re-frozen (design D11; bead 960.24).
- [x] 3.12 MC seam for child ports: connect issued from the common
      ancestor without the root plug step (DPNI-I9 form) + a dpni
      endpoint read for idempotence; stateful fake. Board-witness
      marker: dpni(child)↔dpmac(root) is DPNI-I9-allowed but
      unverified — 4.3 witnesses it (design D11; bead 960.25).
- [x] 3.13 Population pass: converge_population after
      converge_containers, planned FROM the compiled plan (per-port
      child dpnis — arity from the verified derivation, never a
      constant), vfio_handoff guarded by a bound_driver read, dry-run
      block + child census in status so converged renders empty;
      drift inside a bound child is a typed refusal (ADR-0017; the
      healing policy is roadmap #9's, bead w01) (design D11;
      bead 960.26).
- [x] 3.14 Teardown to baseline: undeclared managed-labelled root
      dpnis prune under the double gate; ensure reorders consumer
      teardown before pool shrink; container prune gains vfio_unbind
      + child-dpni disconnect; dpio residue renders as the typed
      disposition; V-POOL-6.sh shrink/prune legs revised to the
      amended laws (design D10/D11; bead 960.27).

- [x] 3.15 Loop-breaker, model first (discovered at the 4.3 sitting,
      2026-09-25: the cfg-drift branch churned a same-run-created dpni
      into a kernel crash): dpni.qnt gains the same-run rebuild-refusal
      law with a directed run; DpniObservation gains the field-level
      diff; reconcile takes the run-created set and emits a typed
      refusal (fields named) instead of the destroy sequence; teardown
      emissions reorder to disconnect → unbind → destroy (ADR-0008
      §8/§9; design D12; bead 960.29).
- [x] 3.16 Real unbind + refusal surfaced: KernelControl gains the
      sysfs dpni unbind (mirror of the child VFIO path), the engine's
      Unbind arm drives it, ensure carries the run-created set across
      passes and exits rendering the 3.15 refusal with its field diff;
      fake + unit tests (ADR-0008 §8/§9; design D12; bead 960.30).

## 4. Suites

- [x] 4.1 Offline: suite generation for the pool walks; frozen ITF
      traces replay in cargo test as conformance twins (mbt-harness
      req 1).
- [x] 4.2 Board: census/ceiling and convergence walks at root scope;
      free/drain observed through the kernel face; record COVERAGE
      row advances (pre-run record commit per board protocol).
- [ ] 4.3 Board: the two MVP scenarios — live kernel interface in
      dprc.1; populated VFIO-bound dprc.N; drift-heal and teardown
      re-runs (system-integration req 1). Waits on 3.9–3.16 (the
      2026-09-24 audit: Scenario B unwired, teardown unreachable,
      shrink/prune assert laws the shipped census cannot execute);
      V-MVP-1 intent + offline pins landed at the first authoring
      pass, the suite script follows 3.14.
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
