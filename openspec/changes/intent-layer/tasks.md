# Tasks

Entry point only (design D13): granular tasks, dependencies, and
acceptance criteria live as bd issues under the `intent-layer` epic;
one bead at a time through acceptance. Phase 1 is Quint only and ends
at the gate (2.6); nothing in phase 3 is ready until the gate closes
(design D7). The single board task (4.2) is a read-only sitting and
the change's only other operator sync point (design D12).

## 0. Tracking and housekeeping

- [x] 0.1 Create the `intent-layer` beads epic and one issue per task
      below, wiring dependencies (0 → 1 → 2 → gate → 3 → 4 → 5) and
      instantiating the six-point DoD per issue; mark ROADMAP row 3 in
      flight
- [x] 0.2 Align the prose to what #2 settled: `object-model.md` §4 takes
      ADR-0012's one-dpmcp-per-process and vendor-neutral wording; the
      dpmcp/dpbp/dpio intent sections cite ADR-0012 for numbers only;
      ADR-0010's "intent layer (#5)" becomes #3; `COVERAGE.md`
      re-anchors DPDCEI-I1 to this change's tenant-absence rule and
      the deferred online-driver backlog rows owned by #3 are listed
- [x] 0.3 Amend ADR-0002: Alloy recorded as a per-model escalation
      trigger beside TLA+, for a relational property that proves awkward
      in Quint; not taken up front (design D7)

## 1. Intent model (Quint, `models/intent/`)

- [x] 1.1 Vocabulary as types: tenant (name, dataplane, `max_cores`,
      `crypto_flows`), port (dpmac, rate, tenant), link, fabric, crypto;
      the reserved `kernel` tenant; the inventory type (dpmacs with
      `max_rate`/`eth_if`/`link_type` and availability free/reserved/
      foreign from the ADR-0003 matrix; three-valued ceilings
      counted/observed/unknown with ADR-0011 provenance) loaded from the
      reference snapshot (design D1, D2)
- [x] 1.2 Derivation as pure defs: the per-class workers-per-port table
      (10G ⇒ 2 seeded, 25G ⇒ 5 declared, `unmeasured`), T = 1 main + Σ
      workers, `max_cores` bound, companion draws by
      reference to `companions.qnt`, dpcon per polled queue, dpseci and
      dpsw predicates, one child DPRC per non-kernel tenant, dprtc.0
      pinned; derived objects keyed (tenant, family, ordinal) with
      labels rendered from the key; provenance as a tree from each
      derived value to its rule, its inputs, the construct and the
      anchor (design D3, D4, D6)
- [x] 1.3 Refusals as the total function's other half, returned as the
      complete list: tenant absence, unanchored, reserved, foreign,
      double-claimed, over-rate, fabric not kernel-forwarded, core budget
      exceeded, unknown rate class, extra on a non-companion family or
      with a non-positive count, crypto without flows, cross-tenant
      infeasibility naming
      family/needed/available against counted/observed ceilings and
      warning on unknown (design D5)
- [x] 1.4 Plan relationships as invariants: one container per object
      and root never a tenant, typed connect ends and no double
      connect, companions only as tenant derivations, emission order
      per `object-model.md` §5 — each a named invariant citing its
      anchor; `companions.qnt` gains its named invariants
- [x] 1.5 Typecheck + simulator green; Apalache marks set on the
      feasibility and companion-count invariants over the finite intent
      alphabet; ladder scripts extended to the intent corpus (marks are
      simulator-only: Apalache front-end heap wall accepted 2026-08-30,
      follow-up bead gqf.26)

## 2. Scenarios, simulation, fit check, gate

- [x] 2.1 Scenario (1): hardware-switched fabric over dpmac.7/8 at 10G,
      kernel-forwarded, with a userspace-poll router member terminating
      dpmac.9/10 under `max_cores` = M — `fabric.qnt` beside
      `fabric.toml`; twin: M below the derived T refused as
      CoreBudgetExceeded (design D8)
- [x] 2.2 Scenario (2): virtual fabric between two userspace-poll child
      dprcs, joined by a link, no dpmac — `vfabric.qnt`/`.toml`; twin: a
      third child dprc overdraws the dpbp ceiling, refused Infeasible
- [x] 2.3a Taxonomy decisions from the taxonomy review (artifact,
      nine threads, 2026-08-31): consumer → tenant, regime → dataplane
      (`kernel|userspace-poll|userspace-interrupt`, the third refused
      as `UnpricedDataplane` until priced), fabric owner →
      `forwarded_by`, port/crypto owner → tenant; the rate table
      becomes per-class workers-per-port with T = 1 + Σ workers and
      the `UnmeasuredCombination` warning; design/proposal/spec deltas
      amended in lockstep (discovered detour, bead gqf.27)
- [x] 2.3 Scenario (3): userspace router over N×10G + 1×25G with
      `crypto_flows` ≤ N — `router.qnt`/`.toml`; twin: a port claimed by
      two tenants
- [x] 2.4 Random simulation over the finite intent alphabet with every
      invariant on; each counterexample becomes a rule (new invariant +
      scenario twin) or a recorded unknown in the model header; coverage
      of the alphabet counted
- [x] 2.5 Fit check: the reference board's provisioning (kernel root
      with dpmac.7/9, the userspace-poll child dprc) as `reference.qnt`/`.toml`;
      compiled plan diffed object-for-object against
      `baselines/reference.json`; the 3-vs-1 dpmcp finding and any other
      difference dispositioned (divergence vs extra — open question 1)
- [x] 2.5a The TOML document-level properties anchor in an `[intent]`
      table, `schema` the first of them (operator decision 2026-08-31):
      scenario TOMLs and the proposal/design/spec-delta wording amended
      in lockstep (discovered detour, bead gqf.28)
- [x] 2.6 GATE — decision bead *intent vocabulary accepted*: the gate
      artefact is `docs/adr/0013-accepted-intent-vocabulary.md`
      (constructs, inputs, derived quantities, the 24 refusals, INTENT_I1–I9,
      the five scenarios as worked witnesses, the open questions presented,
      the honest relaxations and the revisit shapes) plus the rewritten README
      example and ADR-0005 shrunk to a pointer at ADR-0013 — written before
      any Rust. The gate decision on the accepted vocabulary, the
      capacity-model gap and its trigger, the fit check's disposition, the
      external anchors (RFC 9315/9316, ONOS intents) and the CEL revisit
      trigger is carried by ADR-0013 (§§8–10). ADR-0013, ADR-0005, README, and
      the ROADMAP row-3 pointer are landed; gate review closes
      the bead (design D3, D5, D7)
- [x] 2.6a Vocabulary renames decided at the 2.6 gate review: dataplane
      values `kernel` → `kernel-netlink` and `userspace-interrupt` →
      `userspace-event` (the value names the ownership mechanism, leaving
      room for kernel XDP/BPF); link ends `a`/`b` → `interface_a`/
      `interface_b` (an end names a tenant's interface, not a port); crypto
      owns its `flows` (moved off `[[tenant]]` onto `[[crypto]]`, each block
      self-contained). Model, scenario TOMLs, and the proposal/design/spec
      deltas amended in lockstep (discovered detour, bead gqf.29)
- [x] 2.6b `[[limit]]` deleted as a construct: the request/limit override
      channel becomes the additive `[[extra]]` channel (design D5 revisited)
- [x] 2.6c Tenant isolation and pool: the tenant construct carries its
      isolation boundary (public/restricted/isolated, default isolated) and
      pool membership; declared kernel-netlink namespaces are child-resident
      (dpio 0), a restricted drawer co-resides in its holder's dprc, the
      reserved kernel is nameable at a link end, pool legality refuses by
      name, and INTENT_I9 preserves the isolated container's sole-tenancy
      (design D6a). Crypto per-block dpseci sizing (design item 7) is
      deferred to a follow-up parcel — the 2.6a max-fold stands until then.
- [x] 2.6d Scenario (4): kernel-namespace pseudo-wires — one intent over all
      three link shapes (ns↔ns veth pair, ns↔reserved-kernel uplink, ns↔
      userspace app wire), the FIRST scenario to witness the child-resident
      kernel draw (a declared kernel-netlink namespace contributes dpio 0, dpbp/
      dpmcp one per dpni, dpcon dpnis·cpus, numQueues = cpus, in its own
      kernel-bound child dprc), the undeclared reserved kernel materialised in
      Root with the full per-CPU draw, and the portless poll-mode tenant at
      T = 1. Paired `scenarios/vwire.qnt` + `vwire.toml`; the twin (vpp pooling
      the reserved kernel) refused with exactly { PoolDataplaneMismatch }. Ladder
      green; coverage witnesses unchanged (scenarios feed model:test, not the
      alphabet-only coverage run). (bead gqf.32)
- [x] 2.6e Crypto identity — ordered blocks, each dpseci sized by its own
      flows: `Intent.crypto` becomes a `List[Crypto]`, so a `[[crypto]]`
      array-of-tables' declaration order numbers a tenant's dpseci ordinals
      (the ports/links/fabric-members convention). The 2.6a/2.6c max-fold
      ceiling and its `ponytail:` comment are retired — a tenant with two
      blocks of distinct flows gets two dpsecis, each `num_queues` its own
      block's, the block traced to a dpseci by ordinal (declaration order).
      `CryptoFlowsNotPositive` gains the block ordinal so two same-flows bad
      blocks stay distinct in the refusal set, and a new `CryptoFlowsOverDevice`
      refuses a block whose flows exceed one dpseci's 16 queue pairs
      (`DPSECI_MAX_QUEUE_NUM`, cited to the kernel + DPDK trees) — one block is
      one device, so the demand is refused not clamped, the remedy being to
      split across blocks. Provenance stays a single
      per-tenant dpseci node (value = accelerator count): Quint has no
      int→string, so a nameless block cannot key a distinct provenance node, and a
      fourth `ordinal` ProvenanceKey field would pollute ~105 unrelated construct
      keys — considered and rejected in design D6. Directed
      `twoCryptoBlocksTest` asserts the two-dpseci shape directionally; router
      (single block) unchanged. Ladder green; coverage narrative refreshed to
      the post-2.6b/2.6c witness list. (bead gqf.33)

## 3. Rust (gated on 2.6)

- [x] 3.0 `dpaa2-verify`: the intent enumerations become linted copies
      (ADR-0014) — ledger rules R11 (refuse.qnt's 24 `Refusal` variants ⇄
      alphabet.qnt witnesses bijectively, and named by ADR-0013 §5 and
      COVERAGE.md's intent section), R12 (INTENT_I1–I9 ⇄ ADR-0013 §6),
      R13 (scenarios/*.qnt ⇄ *.toml stems ⇄ ADR-0013 §7), plus
      deliberate-drift negative tests over in-memory mutations. Runs
      first in phase 3 so model amendments found while transcribing
      (3.1/3.2) cannot drift the copies unguarded; the semantic
      toml→plan pairing stays in 3.4. COVERAGE.md's INTENT rows land at
      5.1 (invariants.qnt records this), so R12's coverage leg waits
      there. (bead gqf.34)
- [x] 3.1 `dpaa2-api`: `Intent`, `Inventory`, `Refusal`, and the plan
      types transcribed from the model — constructors take the deriving
      construct as witness; a free-standing companion, a dpmac at a link
      end, a double connect, a tenant in root do not compile (design
      D6); `DesiredTopology` reshaped to the plan with provenance; the
      #2 retro-model adapter updated and the ladder re-run in the same
      bead; objects keyed (tenant, family, ordinal), provenance tree
      type, `Ceiling` and availability types, `Refusal`/`Dataplane`
      `#[non_exhaustive]`; constructors public so a plan is buildable
      without `Intent` (design D11), covered by one test that builds and
      reconciles a plan by hand
- [x] 3.1a `dpaa2-verify`: the Rust `Family`/`Refusal` copies tied to the
      model — fold `adapter.rs`'s sibling `Family` onto `dpaa2_api::Family`
      (serde/tag impls stay local to verify; api stays serde-free, design
      D10) and/or extend `intent_lint` with a Rust-copy leg, so neither
      enumeration can drift unlinted (ADR-0014; architecture review of
      3.1, bead gqf.36)
- [x] 3.1b `dpaa2-api`: `from_parts` coherence — the D11 seam either
      checks the plan/ports facets agree (every `DesiredPort` has its
      port-edge and vice versa) or stops doc-claiming non-drift for that
      path; lands before 3.6's plan-only reporting (architecture review
      of 3.1, bead gqf.37)
- [x] 3.2 `dpaa2-api`: `compile(intent, inventory)` — derivation,
      additive extras, provenance trees, the complete refusal
      list; unit tests one per refusal variant and per companion rule;
      `proptest` for determinism, extras only raise a count, companion-before-
      tenant (design D9); ITF replay of the intent traces through
      `compile` in `dpaa2-verify`, with `quint-connect` evaluated
      against the existing replayer and adopted only if it retires code
      (evaluated and NOT adopted, audit upheld: step-driver paradigm
      fits neither the pure single-state intent leg nor the retro leg's
      coarser-than-action epochs, and its serde mapping would force an
      ADR-0014 mirror vocabulary; revisit triggers in `intent_itf.rs`)
- [ ] 3.3 `dpaa2-config`: schema rewritten to the constructs with the
      `[intent]` table's mandatory `schema` key; validation (references resolve,
      `kernel` reserved, link ends distinct, fabric members exist, count
      fields rejected, unknown schema version refused); converts to
      `Intent`; `ConfigSource::load` returns `Intent`; README example and
      ADR-0013's construct examples re-checked against the shipped parser
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
- [ ] 5.4 Disposition of the accepted Apalache gap on `compileLaws`
      (bead gqf.26): wire `model:verify` if a quint/Apalache upgrade or a
      compile-once restructure makes it affordable, else seal the gap in
      ADR-0002's escalation note with the measured costs
