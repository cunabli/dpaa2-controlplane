# Tasks

Entry point only (design D13): granular tasks, dependencies, and
acceptance criteria live as bd issues under the `intent-layer` epic;
one bead at a time through acceptance. Phase 1 is Quint only and ends
at the gate (2.6); nothing in phase 3 is ready until the operator closes
it (design D7). The single board task (4.2) is a read-only sitting and
the change's only other operator sync point (design D12).

## 0. Tracking and housekeeping

- [x] 0.1 Create the `intent-layer` beads epic and one issue per task
      below, wiring dependencies (0 → 1 → 2 → gate → 3 → 4 → 5) and
      instantiating the six-point DoD per issue; mark ROADMAP row 3 in
      flight
- [ ] 0.2 Align the prose to what #2 settled: `object-model.md` §4 takes
      ADR-0012's one-dpmcp-per-process and vendor-neutral wording; the
      dpmcp/dpbp/dpio intent sections cite ADR-0012 for numbers only;
      ADR-0010's "intent layer (#5)" becomes #3; `COVERAGE.md`
      re-anchors DPDCEI-I1 to this change's consumer-absence rule and
      the deferred online-driver backlog rows owned by #3 are listed
- [ ] 0.3 Amend ADR-0002: Alloy recorded as a per-model escalation
      trigger beside TLA+, for a relational property that proves awkward
      in Quint; not taken up front (design D7)

## 1. Intent model (Quint, `models/intent/`)

- [ ] 1.1 Vocabulary as types: consumer (name, regime, `max_cores`,
      `crypto_flows`), port (dpmac, rate, owner), link, fabric, crypto;
      the reserved `kernel` consumer; the inventory type (dpmacs with
      `max_rate`/`eth_if`/`link_type` and availability free/reserved/
      foreign from the ADR-0003 matrix; three-valued ceilings
      counted/observed/unknown with ADR-0011 provenance) loaded from the
      reference snapshot (design D1, D2)
- [ ] 1.2 Derivation as pure defs: the rate-class → T table (one seeded
      row, `unmeasured`), `max_cores` bound, companion draws by
      reference to `companions.qnt`, dpcon per polled queue, dpseci and
      dpsw predicates, one child DPRC per non-kernel consumer, dprtc.0
      pinned; derived objects keyed (consumer, family, ordinal) with
      labels rendered from the key; provenance as a tree from each
      derived value to its rule, its inputs, the construct and the
      anchor (design D3, D4, D6)
- [ ] 1.3 Refusals as the total function's other half, returned as the
      complete list: consumer absence, unanchored, reserved, foreign,
      double-claimed, over-rate, fabric not kernel-steered, core budget
      exceeded, limit below request, cross-consumer infeasibility naming
      family/needed/available against counted/observed ceilings and
      warning on unknown (design D5)
- [ ] 1.4 Plan relationships as invariants: one container per object
      and root never a consumer, typed connect ends and no double
      connect, companions only as consumer derivations, emission order
      per `object-model.md` §5 — each a named invariant citing its
      anchor; `companions.qnt` gains its named invariants
- [ ] 1.5 Typecheck + simulator green; Apalache marks set on the
      feasibility and companion-count invariants over the finite intent
      alphabet; ladder scripts extended to the intent corpus

## 2. Scenarios, simulation, fit check, gate

- [ ] 2.1 Scenario (1): hardware-switched fabric over dpmac.7/8/9 at 10G
      under `max_cores` = M — `fabric.qnt` beside `fabric.toml`; twin:
      M below T refused (design D8)
- [ ] 2.2 Scenario (2): virtual fabric between two poll-mode containers,
      no dpmac — `vfabric.qnt`/`.toml`; twin: a third container
      overdraws the dpbp ceiling
- [ ] 2.3 Scenario (3): userspace router over N×10G + 1×25G with
      `crypto_flows` ≤ N — `router.qnt`/`.toml`; twin: a port claimed by
      two consumers
- [ ] 2.4 Random simulation over the finite intent alphabet with every
      invariant on; each counterexample becomes a rule (new invariant +
      scenario twin) or a recorded unknown in the model header; coverage
      of the alphabet counted
- [ ] 2.5 Fit check: the reference board's provisioning (kernel root
      with dpmac.7/9, the poll-mode child) as `reference.qnt`/`.toml`;
      compiled plan diffed object-for-object against
      `baselines/reference.json`; the 3-vs-1 dpmcp finding and any other
      difference dispositioned (divergence vs override — open question 1)
- [ ] 2.6 GATE — decision bead *intent vocabulary accepted*: the gate
      artefact is `docs/intent.md` (constructs, inputs, derived
      quantities, refusals, the scenarios as worked examples, the open
      questions decided) plus the rewritten README example, written
      before any Rust; the operator reviews and closes the bead;
      ADR-0005 amended with the accepted vocabulary, the capacity-model
      gap and its trigger, the fit check's disposition, the external
      anchors (RFC 9315/9316, ONOS intents) and the CEL revisit trigger
      (design D3, D5, D7)

## 3. Rust (gated on 2.6)

- [ ] 3.1 `dpaa2-api`: `Intent`, `Inventory`, `Refusal`, and the plan
      types transcribed from the model — constructors take the deriving
      construct as witness; a free-standing companion, a dpmac at a link
      end, a double connect, a consumer in root do not compile (design
      D6); `DesiredTopology` reshaped to the plan with provenance; the
      #2 retro-model adapter updated and the ladder re-run in the same
      bead; objects keyed (consumer, family, ordinal), provenance tree
      type, `Ceiling` and availability types, `Refusal`/`Regime`
      `#[non_exhaustive]`; constructors public so a plan is buildable
      without `Intent` (design D11), covered by one test that builds and
      reconciles a plan by hand
- [ ] 3.2 `dpaa2-api`: `compile(intent, inventory)` — derivation,
      request/limit overrides, provenance trees, the complete refusal
      list; unit tests one per refusal variant and per companion rule;
      `proptest` for determinism, limit ≥ request, companion-before-
      consumer (design D9); ITF replay of the intent traces through
      `compile` in `dpaa2-verify`, with `quint-connect` evaluated
      against the existing replayer and adopted only if it retires code
- [ ] 3.3 `dpaa2-config`: schema rewritten to the constructs with the
      mandatory `schema = 1` key; validation (references resolve,
      `kernel` reserved, link ends distinct, fabric members exist, count
      fields rejected, unknown schema version refused); converts to
      `Intent`; `ConfigSource::load` returns `Intent`; README example and
      `docs/intent.md` re-checked against the shipped parser
- [ ] 3.4 `dpaa2-verify`: the pairing test — every `scenarios/<name>.toml`
      parses, compiles against the snapshot inventory, and equals its
      `<name>.itf.json` plan; the ladder fails on an unpaired scenario
- [ ] 3.5 `dpaa2-mc` + `dpaa2-tools`: inventory read (`dpmac info`
      attributes, `mc.global --resources`) behind the `McControl` seam;
      `ensure`/`dry-run` read → compile → reconcile; dry-run prints the
      plan with provenance trees and the plan-only report, `insta`
      snapshots of the text (design D9); refusals print the full list
      and exit non-zero
- [ ] 3.6 `dpaa2-api` reconciler: plan-only objects reported by family
      and count, never as drift; `is_converged` ignores them; retro
      traces still green

## 4. Board milestone (read-only, one sitting; design D12)

- [ ] 4.1 Generate the fit-check sitting: read the live census (`dprc
      show` tree, `mc.global --resources`, `dpmac info` on the
      lifecycle-safe ports), build the inventory from it, compile the
      reference intent through the shipped `dpaa2ctl dry-run`, diff
      against the census; no mutation anywhere in the script; reference
      pair asserted
- [ ] 4.2 Operator runs the sitting; diff dispositioned; divergences
      amend the model, the ADR-0005 amendment, or the baseline in the
      same bead; evidence archived outside the checkout per the board
      evidence rules

## 5. Close-out

- [ ] 5.1 Ledger pass: `COVERAGE.md` rows for the intent invariants and
      the DPDCEI-I1 re-anchor; ledger lint green
- [ ] 5.2 Docs: ROADMAP row 3 delivered; ADR-0005 amendment sealed;
      CHANGELOG flows from commits; spec deltas ready to promote
- [ ] 5.3 Quality floor: `cargo build | fmt | clippy | clippy --tests |
      doc | test` and the model ladder green; epic closed with every
      child bead through acceptance
