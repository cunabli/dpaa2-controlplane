# ADR-0019: Four state patterns cover the sixteen families

- **Status:** Accepted — grilling session 2026-09-19 (bead
  dpaa2-controlplane-chi; no openspec change — a modeling-vocabulary
  agreement, it prescribes shape and changes no behavior)
- **Date:** 2026-09-19
- **Supersedes / relates to:** ADR-0018 (the module tree this catalog
  fills: 0018 names where a family's pieces live, this record names what
  shape they take — together they define the software structure and the
  change management of state structure); ADR-0002 §3 (structural
  isomorphism and what a typestate can prove — the laws every pattern
  instance obeys); ADR-0014 (the family→pattern table below is a linted
  copy, bead dpaa2-controlplane-qtk); ADR-0012 (the companion sizing law
  the counted-companion pattern encodes); ADR-0009 (the dpdmux uplink
  refusal, an edge facet); ADR-0005 (the intent layer this catalog is
  orthogonal to)

## Context

The MC exposes sixteen object families, and every one of them is a kind
of state the control plane must represent: containment state (dprc),
create-configured dataplane state (dpni, dpsw), counted capacity (dpio,
dpbp), boot-born hardware offer (dpmac). The machine-checked
classification of that state already exists: every
`models/families/<f>.qnt` instantiates one shared `FamilyParams` record
(`creatable`, `placement`, `singleton`, `pooled`, `resetOnBind`,
`hasKernelDriver`, `hotBindAvailable`, `endpointPorts`,
`createTriggersRescan`, `draw`), and `models/families/params.qnt` holds
the corpus-wide table. The models therefore answer *what each family
is*; what they do not prescribe is *what Rust shape represents it*.

Without a catalog, shape is a per-change decision transmitted as
precedent citations in doc comments — `families/dpni.rs` anchors its
observation split to "the sibling of dprc's" and its refusals to "the
vocabulary-v2 parity precedent". A convention carried by citation is
real but unenforceable: each new family re-derives it, and sixteen
derivations of a handful of ideas is how one pattern per object — and
sixteen ad-hoc rule sets — accretes. A library must be prescriptive
about its own vocabulary: a handful of named patterns, each the best
representation of the hazard it types away, selected by classification
rather than invented by analogy.

## Decision

State shape is selected from a catalog of **four patterns, keyed by the
hazard class the type system must make unrepresentable**. `FamilyParams`
decides the pattern; a family implements exactly one primary pattern and
may carry named facets of another. A family whose hazard fits no pattern
amends this record with a new one — it never ships a bespoke shape.

The catalog argues from hazard, not taste: a pattern is "best" exactly
when the mistake class it removes is the mistake class the MC punishes.

### P1 — Phase machine (the hazard is ordering)

For families whose verbs the MC punishes *out of sequence*: assign only
while unplugged, lock strips verbs from a hierarchy, destroy only when
emptied. Wrong sequences become unrepresentable.

- **Reference implementation:** `families/dprc.rs` —
  `Container<S: Phase>` with one marker type per model state, capability
  traits (`Active`, `UnpluggedFace`, `LiveFace`, `UnlockedHolder`) as
  compile-time verb gates, `#[must_use]` consuming transitions.
- **Idioms:** phantom-type typestate; sealed capability traits; consuming
  moves; parse-don't-validate promotion on observed data (ADR-0002 §3).
  A phase never outlives re-observation: the observed snapshot is plain
  data, never a type parameter (ADR-0002 §3).

### P2 — Configured object (the hazard is configuration)

For families created from a configuration block whose invalid values,
flags, or field combinations the firmware accepts silently or refuses
late. Wrong *values* become unrepresentable; phase machinery is
deliberately absent, because these objects' verbs are not
order-hazardous — carrying phantom states here would type away a hazard
that does not exist and stiffen a surface that must stay data.

- **Reference implementation:** `families/dpni.rs`.
- **Idioms:** newtype refined ranges with fallible constructors (the
  refusal is a type error, not a board rejection); a closed flag enum
  over the verified vocabulary plus a provenance-carrying raw-escape
  enum (no unattributed bit rides a mask); the cfg block immutable by
  privacy — shared-reference access only, witnessed by a `compile_fail`
  doctest; runtime surface as a state slot *inside* the type
  (`RuntimeState`), so new setters land without reshaping the family;
  an observation projection type that excludes write-only fields by
  construct, so drift can never be claimed on what cannot be read back;
  a pure disposition function deciding converge/mutate/destroy+create;
  parity refusals naming every unrepresentable knob (ADR-0014).
- **The type boundary is the verified envelope, not the profile.** Field
  ranges and vocabulary are typed; membership in a board-verified option
  profile is intent-layer policy (ADR-0005), not a construction rule.
  This is load-bearing: the MBT probes must express in-envelope,
  off-profile configurations by design, so sealing construction to the
  profiles would falsify the harness. Any crate can build an
  in-envelope block; only the intent compiler derives one in production.
- **Degenerate members are still this pattern.** A family whose create
  block is small or empty (dpci, dpdcei, dpdmai, dprtc, dpdbg) uses P2
  with a degenerate cfg plus its refusal surface (singleton refusal,
  placement refusal); a trivial create never mints a new pattern.

### P3 — Counted companion (the hazard is capacity)

For families that exist as *capacity consumed by other families*, not as
individuals: the reconciler reasons about counts and draw arithmetic
(ADR-0012), never about companion #7. A per-object identity type here
would lie — these objects carry no intent-side identity (ADR-0015 keys
identity by name; companions wear their consumer's name).

- **Members:** dpio, dpbp, dpmcp, dpcon.
- **Idioms:** plain counts and sizing functions; pool custody as
  membership for the allocator trio (dpbp/dpmcp/dpcon, `pooled: true`);
  regime-typed per-CPU seats for dpio (`pooled: false` — DPIO-I1/I2:
  regime-typed, never pooled). Capacity is judged against
  `Ceiling`/`Inventory` (ADR-0011), not per-object state.

### P4 — Boot-born offer (there is no create path)

For families the platform births (`creatable: false`): the board *offers*
them; the control plane observes, judges availability, and connects. A
constructor would lie, so none exists — the Rust surface is
observation-only attribute types feeding the inventory offer
(read-never-written, ADR-0003 safety matrix for custody).

- **Members:** dpmac (reference shape: observed attributes + offer
  types), dpaiop.

### Facets and promotion

A family may carry a named facet of another pattern without changing its
primary:

- **Ordering facet on a P2 family.** Order-sensitive create-adjacent
  sequencing (dpni's set-MAC-before-plug) lives procedurally in the
  adapter while the family's own surface carries no order-hazardous
  verbs. The promotion trigger is explicit: when a family's *typed
  surface* gains order-sensitive verbs (e.g. setters valid only in one
  plug state), that family adds phase markers in the P1 idiom — the
  runtime-state slot is the seam they attach to, so promotion reshapes
  nothing.
- **Reset facet.** `resetOnBind: true` (dpni, dpsw, dpdmux — each marked
  *breaking* in its model) is disposition knowledge for the planner, not
  a phase: it changes what convergence must re-apply, not what sequences
  are representable.
- **Edge facet.** Endpoint rules (`endpointPorts`, the dpci↔dpci
  same-family edge, the dpdmux uplink refusal of ADR-0009) are typed at
  the connection surface, not inside the family.

### The family→pattern table

The pattern-deciding fields restate `models/families/*.qnt` — the models
are the source of truth and this table is the reader's copy until its
lint lands (ADR-0014; bead dpaa2-controlplane-qtk adds the ledger R-rule
cross-checking every row against `FamilyParams`).

| Family | Pattern | Deciding `FamilyParams` signal | Facets / notes |
|---|---|---|---|
| dprc   | P1 phase machine | `createTriggersRescan: true`; lock/assign verb surface | reference implementation |
| dpni   | P2 configured object | `creatable`, rich cfg (`CreateCfg` in `dpni.qnt`) | reference implementation; reset facet; ordering facet procedural in adapter |
| dpsw   | P2 configured object | `creatable`, cfg gates driver bind (V-DPSW-1) | reset facet (DPSW-I4, breaking); edge facet (ports) |
| dpdmux | P2 configured object | `creatable`, cfg + port shape | reset facet (DPDMUX-I3, breaking); edge facet (ADR-0009 uplink) |
| dpseci | P2 configured object | `creatable`, sized cfg (queues, HAS_CG) | companion draw `dpmcp: 1` |
| dpci   | P2 configured object (degenerate) | `creatable`, `endpointPorts: 1` | edge facet: dpci↔dpci, the only same-family-only edge |
| dpdcei | P2 configured object (degenerate) | `creatable`, driver-less | |
| dpdmai | P2 configured object (degenerate) | `creatable`, `draw.dpmcp: 1` | |
| dprtc  | P2 configured object (degenerate) | `creatable`, `singleton: true` (DPRTC-I1) | singleton refusal is part of its P2 refusal surface |
| dpdbg  | P2 configured object (degenerate) | `creatable`, `singleton: true`, `placement: RootOnly` (DPDBG-I1) | singleton + placement refusals |
| dpio   | P3 counted companion | `pooled: false`, regime-typed (DPIO-I1/I2), per-CPU seats | seat arithmetic, not pool custody |
| dpbp   | P3 counted companion | `pooled: true`, allocator custody | pool free is no reset (DPBP-I3) |
| dpmcp  | P3 counted companion | `pooled: true`, allocator custody | |
| dpcon  | P3 counted companion | `pooled: true`, allocator custody | |
| dpmac  | P4 boot-born offer | `creatable: false` (DPMAC-I1), `placement: RootOnly` | offer feeds inventory; custody per ADR-0003 matrix |
| dpaiop | P4 boot-born offer | `creatable: false` (platform-refused, DPAIOP-I1) | driver-less |

### Library posture: prescriptive, orthogonal, open

- **Orthogonal to intent.** The catalog governs how MC state is
  *represented*; the intent layer governs how values are *chosen*
  (ADR-0005, ADR-0013). Neither cites the other's internals: a pattern
  never names a `Dataplane`, and an intent rule never names a phase
  marker. This is what keeps the library usable below any northbound
  surface, not only the declarative one.
- **Named extension points, so deviation is an amendment, not a
  rewrite.** A new consumer profile is a documented intent-layer
  amendment; a new setter surface is a field in the family's runtime
  state slot; a new order-hazard is a facet promotion under the stated
  trigger; a new family picks a pattern from this table's logic. A
  genuinely new hazard class adds a pattern *to this record* — the
  catalog grows by ADR amendment, and a Rust surface that deviates
  without one is a defect in review.

### Rust idiom stance

Every pattern is assembled exclusively from well-known Rust idioms —
typestate via phantom types, newtype refinement, sealed/capability
traits, `#[must_use]` consuming transitions, `compile_fail` doctests as
negative witnesses, exhaustive-`match` linted enum copies (ADR-0014) —
and future families use the same idiom vocabulary. No bespoke
mechanisms: a reader who knows the standard idioms reads any family; a
macro or trait system invented for one family is the per-object style
this record exists to refuse.

## HAL projection (forward-looking; revisit trigger: mc-portal-backend, roadmap #10)

This section is directional until the ioctl portal consumes it; the
revisit trigger is that tile's design.

The catalog projects onto the crate boundaries of ADR-0018 without
bending them:

- **dpaa2-api** carries the pattern types; nothing in a pattern names a
  transport.
- **Adapters own wire encoding.** A configured object's flag enum maps
  to raw bits in the adapter (the restool shim's option-mask bit map is
  the shape of it); the same typed block is the transport-neutral
  payload an ioctl portal encodes differently behind the unchanged
  `contract/` seam — the differential gate of ADR-0018's tile-#10
  trigger holds both encodings to one surface.
- **dpaa2-hal stays policy-free primitives** (ADR-0018): a phase
  machine's consuming transition dispatches as a *sequence* of HAL
  verbs; the ordering knowledge lives in the pattern type and the
  adapter, never in the HAL. When the portal transport lands, the P2
  runtime-state slots gain setters without reshaping any family — that
  is the promotion trigger above, and the moment this section is
  rewritten from projection to record.

Together, ADR-0018 and this record define the software structure: 0018
fixes *where* the pieces live (core/contract/families/plan/intent, and
the adapter crates' triggers), this record fixes *what shape state
takes* and how that shape changes (facet promotion, catalog amendment).

## Reference-implementation deltas (recorded, not applied)

`families/dprc.rs` and `families/dpni.rs` are the canonical exemplars of
P1 and P2. Two alignments would make them fully exemplary; each lands
only when a change already touches the surface, never as churn:

- **Observation naming.** The observation projections spell one idea
  three ways: `ObservedContainer` (plan-side dprc), `DpniObservation`
  (family-side dpni), `ObservedDpni` (core model). The catalog's rule:
  the family-side drift projection is `<Family>Observation`; adapter-fed
  observed records are `Observed<Noun>`; a rename to conform rides the
  next change touching the type.
- **Canonical homes.** Refusal enums and disposition functions live in
  `families/<f>.rs` (the ADR-0018 landing convention); any that sit
  elsewhere move when their surface is next edited.

## Consequences

- A new family's shape is a table lookup plus a checklist, not a design
  debate; review rejects bespoke shapes by citing this record.
- The `FamilyParams` taxonomy becomes load-bearing on the Rust side: the
  mapping table is linted against the models (bead
  dpaa2-controlplane-qtk), so a model reclassification surfaces as a red
  lint, not a stale ADR.
- Pattern facets and promotion triggers are named, so a family gaining
  order-sensitive verbs (or the portal's setter surface) extends its
  type in place — the cost of being wrong about a family's primary
  pattern is an amendment, not a rewrite.
- The idiom stance bounds the learning surface: four patterns, one idiom
  vocabulary, sixteen families.

## References

- `models/families/params.qnt` — the corpus-wide `FamilyParams` table;
  per-family records in `models/families/<f>.qnt` (source of truth for
  the mapping table).
- `crates/dpaa2-api/src/families/dprc.rs` — P1 reference
  implementation.
- `crates/dpaa2-api/src/families/dpni.rs` — P2 reference
  implementation.
- ADR-0018 — module tree and crate boundaries; the companion record.
- ADR-0002 §3 — structural isomorphism; what a typestate can and cannot
  prove.
- ADR-0014 — linted enumerations; bead dpaa2-controlplane-qtk (this
  record's table lint).
- ADR-0012 — companion sizing by consumer regime and thread count (P3).
- ADR-0011 — ceilings and pool-bound capacity (P3 judgment surface).
- ADR-0009 — the dpdmux uplink refusal (edge facet).
- ADR-0005 / ADR-0013 — the intent layer this catalog is orthogonal to.
