## Why

Provisioning DPAA2 correctly means carrying numbers no operator should
have to know by default: a userspace-poll tenant with T threads needs 2·T
dpios, two dpbps, T transmit queues and one dpmcp per process; a kernel
tenant draws one per CPU and one per object; a dpsw is only kernel-bindable
under a fixed option predicate; a dpseci needs one queue per crypto
flow. Change #2 (`verify-foundation`) verified those rules on the board
(ADR-0011, ADR-0012) and left them as prose and one Quint function
(`models/core/companions.qnt`) — nothing turns an operator's intent into
them, and today's `topology.toml` still speaks the object language it was
meant to replace (a port is a dpmac with a name and a MAC; every other
object is somebody's shell script). ADR-0005 decided the fix on
2026-08-22: operators declare network constructs, a pure compiler derives
the object plan, every derived number carries its provenance. This is
that change, proposed now because its two dependencies are archived and
every later change (#4 onward) executes a plan this compiler emits.

## What Changes

- **The intent vocabulary** — the frontend-neutral intermediate
  representation in `dpaa2-api`: a *tenant* (name, dataplane
  `kernel-netlink|userspace-poll|userspace-event`, `max_cores` budget), a *port*
  (dpmac anchor, rate, owning tenant), a *link* (point-to-point
  dpni↔dpni pseudo-wire between two tenants), a *fabric*
  (a forwarding domain over members — ports, tenants, other fabrics —
  switched in hardware by a dpsw or in software by its forwarding tenant), and *crypto*
  (per-tenant accelerator carrying its own `flows` demand). `kernel` is the reserved root-dataplane
  tenant; dprtc.0 is pinned and never derived; dpdbg is not a
  construct. The operator states capacity and who consumes it — never a
  dpio, dpbp, dpcon, dpmcp, queue or worker count.
- **The derivation compiler** — a pure, total
  `compile(intent, inventory) -> Result<plan, refusal>` in `dpaa2-api`.
  The inventory is the observed hardware offer — each dpmac with
  `max_rate`, `eth_if`, `link_type` and its availability (free, reserved
  with a reason, or foreign with its owner — the ADR-0003 safety matrix
  and DPL-owned objects become inventory facts); each pool ceiling as a
  three-valued quantity (counted, observed-with-provenance, unknown —
  ADR-0011's own distinction) — read from the board in `ensure` and from
  change #2's reference snapshot in tests, never hand-written. The
  compiler emits the complete object set, each derived object keyed by
  (tenant, family, ordinal) with its label rendered from the key
  (ADR-0010: names are not identities), and each derived value carrying
  a provenance *tree* — rule, inputs, and their provenance down to the
  declared construct and the evidence anchor. It refuses with the
  complete list of violations, not the first — tenant absence
  (DPDCEI-I1, deferred here from #2), an unanchored, reserved, foreign or
  double-claimed dpmac, an over-rate port, a userspace-poll tenant forwarding
  a hardware fabric, a thread count over the core budget, an extra on a
  non-companion family or with a non-positive count, a crypto block with no
  flows, an unseeded rate class, a dataplane with no companion pricing
  (`userspace-event` today), and cross-tenant infeasibility against a counted or
  observed ceiling (an unknown ceiling warns, never refuses).
- **The plan's relationships are locked by construction**: containment
  (one container per object, root never a tenant), typed connect edges
  (no double connect, no dpmac at a link end), companions obtainable
  only as a tenant's derivation, and lifecycle ordering
  (`object-model.md` §5) as a property of how the plan is built, not a
  sort applied afterwards. Per-object lifecycle typestates stay with
  changes #4–#8.
- **Derived counts are requests; extras add on top** — every derived
  count is a request; a per-(tenant, family) `[[extra]]` adds its `count`,
  so the effective count is request + count, raise-only by construction.
  Only the four companion families dpio/dpbp/dpmcp/dpcon accept an extra;
  any other family, or a count below 1, refuses; provenance prints both
  the request and the extra. A policy-expression language for extras (CEL,
  as Kubernetes uses for validation rules) is recorded as a revisit
  trigger, not built.
- **The thread-count rule is declared, not measured**: T derives from a
  per-class workers-per-port table seeded from the reference board (10G ⇒
  2 workers; two 10G ports in poll-mode ran T = 1 + 2·2 = 5);
  provenance marks it *unmeasured*, and ADR-0005 is
  amended to record the capacity-model gap and the trigger that closes
  it (an NXP-citeable per-worker figure or a board measurement of our
  own). A tenant's `max_cores` bounds it — a plan that needs more is
  refused, so "prove it impossible" is an answer the tool gives.
- **Model first, then a hard gate, then Rust** — `models/intent/`
  carries the vocabulary as types, the derivation as pure definitions,
  every rule as a named invariant with its evidence anchor, and three
  scenarios written as intent (a hardware-switched fabric over three 10G
  ports under a core budget; a virtual fabric between two child dprcs
  with no dpmac; a userspace router over N×10G + 1×25G with at most N
  crypto flows), each with a negative twin that is refused and each
  `<name>.qnt` beside the `<name>.toml` an operator would type. Random
  simulation over the intent space turns counterexamples into rules or
  recorded unknowns; a fit check writes the reference board's real
  provisioning as intent and diffs the compiled plan object-for-object
  against the #2 snapshot. The gate artefact is the user manual:
  `docs/intent.md` (constructs, inputs, derived quantities, refusals,
  worked examples from the scenarios) and the README example are written
  before any Rust and are what the operator reviews; a decision bead —
  *intent vocabulary accepted* — closed after that review gates every
  Rust task, and the reviewed document ships as the reference.
- **The pipeline** becomes `topology.toml → Intent (dpaa2-config) →
  compile (dpaa2-api) → DesiredTopology → reconcile`. `DesiredTopology`
  grows from a port list into the full derived object set with
  provenance; `reconcile`'s signature is unchanged and it executes what
  it has executors for (dpni↔dpmac today), carrying the rest as
  plan-only. `dpaa2ctl dry-run` prints the plan with provenance.
  `ConfigSource` stays the seam a later frontend (YANG/gNMI data tree →
  Intent) plugs into.
- **Layering is preserved**: the intent layer is additive, above
  `dpaa2-api`'s plan and reconciler and above `dpaa2-mc`/`dpaa2-hal`.
  The plan's constructors are public, so a library user can build a
  `DesiredTopology` programmatically — through the same witness-taking
  constructors, so the relationship locks still hold — and feed
  `reconcile` without `Intent` or TOML; `McControl` and the HAL remain
  directly usable to drive the MC by hand. `reconcile` does not depend
  on `Intent`. Raw object-level escape hatches (ADR-0005 §5's last
  resort) are therefore addable later at the plan layer without
  touching the compiler; none is built here.
- **BREAKING**: the port-only `topology.toml` schema is replaced, not
  extended; the file gains an `[intent]` table carrying a mandatory
  `schema` version key — the anchor for document-level properties — so
  the next break has a hook; a file with no `[[tenant]]` still parses
  (ports default to the kernel tenant in the root container — today's
  behavior), but the neutral `DesiredTopology` type changes shape and
  nothing preserves the old one. `Refusal` and `Dataplane` are
  `#[non_exhaustive]`: a live-census shortfall variant and a passthrough
  dataplane (a VFIO child whose guest dataplane the host cannot see, #4) are
  already known to be coming.
- **Board milestone is read-only**: one sitting re-runs the fit check
  against a live census; nothing mutates. The compiler is board-free by
  construction (ADR-0005); the first execution of a compiled plan is
  change #4's.
- Housekeeping folded in: `object-model.md` §4 takes ADR-0012's settled
  dpmcp count (one per process, not three) and vendor-neutral wording;
  ADR-0010's "intent layer (#5)" becomes #3; `models/COVERAGE.md`
  re-anchors DPDCEI-I1 to this change's tenant-absence rule; dpdmux
  (shared uplink) stays out of the vocabulary until its regime law
  settles (ADR-0009, change #12); Alloy is recorded as a per-model
  escalation trigger beside TLA+ (ADR-0002) for relational properties
  only if Quint proves awkward for one.

## Capabilities

### New Capabilities

- `intent-compiler`: the frontend-neutral intent vocabulary, the
  observed hardware inventory (availability and three-valued ceilings),
  the pure derivation from intent to the complete object plan with keyed
  identities, provenance trees and additive extras, the complete
  named refusals, the by-construction plan relationships, the additive
  layering over api/mc/hal, and the executable Quint model
  (`models/intent/`) with its paired scenarios, the user manual as the
  gate artefact, and the vocabulary-acceptance gate.

### Modified Capabilities

- `topology-config`: the on-disk schema speaks constructs (tenant,
  port with rate and tenant, link, fabric, crypto) and converts into the
  neutral `Intent`, not a port list; ports without a tenant default to
  the kernel tenant; validation covers construct references and the
  reserved `kernel` name.
- `reconciler`: the desired side is a compiled plan carrying every
  derived object with provenance; reconciliation executes the objects it
  has executors for and reports the rest as plan-only, never as drift.
- `provisioning-cli`: `dry-run` prints the derived plan with per-object
  provenance (rule, source construct, evidence anchor, extra marks)
  and prints a refusal with its named rule instead of a plan; `ensure`
  reads the inventory before compiling.
- `formal-models`: the model corpus gains `models/intent/` under the
  same CI ladder; every intent scenario is paired with the TOML that
  expresses it, and a Rust test holds the two equivalent.

## Impact

- Crates: `dpaa2-api` (new `intent`, `inventory`, `compile` modules;
  `DesiredTopology` reshaped; `Refusal` error type), `dpaa2-config`
  (schema rewritten to constructs; converts to `Intent`), `dpaa2-tools`
  (dry-run provenance output; inventory read in `ensure`), `dpaa2-mc`
  (dpmac attribute read for the inventory), `dpaa2-verify` (ITF replay
  through `compile`; scenario TOML↔trace equivalence test;
  `quint-connect` evaluated against the existing replayer, adopted only
  if it retires code). Dev-dependencies `proptest` (the three laws cheap
  to state in Rust) and `insta` (dry-run text snapshots).
- Models: new `models/intent/` (vocabulary, derivation, invariants,
  scenarios with paired TOML); `models/core/companions.qnt` gains named
  invariants; `models/COVERAGE.md` and the ladder scripts extended.
- Docs: new `docs/intent.md` (the gate artefact, shipped as the
  reference manual); ADR-0005 amended (vocabulary as accepted at the
  gate; capacity model gap and trigger; Alloy escalation; external
  anchors RFC 9315/9316 and the ONOS intent framework as the
  compile/installer precedent; CEL as the extra-policy revisit
  trigger); ADR-0010 reference fixed;
  `docs/baseline/object-model.md` §4 and the dpmcp/dpbp/dpio intent
  sections aligned to ADR-0012; `docs/ROADMAP.md` row 3 marked in
  flight; README example rewritten in the new schema.
- Layering: no change to `McControl`, `KernelControl`, or the HAL
  surfaces; `reconcile(desired, observed)` keeps its signature and gains
  no dependency on `Intent`; direct use of `dpaa2-api`/`dpaa2-mc` as
  libraries stays a supported path.
- Process: beads epic `intent-layer`, one bead at a time through
  acceptance; follow-ups as child beads; one operator sitting
  (read-only) as the board sync point; spec deltas promoted at archive.
