# ADR-0022: Reification policy is a property of the edge kind

- **Status:** Accepted — scoping grilling 2026-10-03 (OpenSpec change
  `cross-dprc-links`, design D2–D5/D8; bead dpaa2-controlplane-kux.6)
- **Date:** 2026-10-04
- **Supersedes / relates to:** ADR-0019 (edge facet — endpoint rules are
  typed at the connection surface, not inside a family; this record is
  that surface's own law); ADR-0017 (decision 3's population order and
  the deferred-visibility obligation it files forward — the healing row
  below is where that obligation is represented); ADR-0008 §8 (the
  driver-handback hazard the severed witness types away); ADR-0009 (the
  dpdmux uplink is un-disconnectable on the pinned firmware); ADR-0015
  (the consent machinery the Disruptive rebind rides); ADR-0013 §link
  (the intent construct this surface reifies); ADR-0021 (connection
  verbs stay 1:1 with whitelisted MC commands); ADR-0014 (the table
  below is a linted copy, bead dpaa2-controlplane-yfa)

## Context

The MC's connection model is one verb pair — `dprc connect` /
`dprc disconnect` — issued on a common ancestor of both endpoints,
type-agnostic in restool, validated for pair legality by the firmware,
with edge arity fixed at two (a multi-port endpoint is address syntax,
`<type>.<id>.<port>`, never a wider edge). The corpus records seven
legal edge kinds (`docs/baseline/object-model.md` §2), and the model
types them in one place: `models/core/connect.qnt` owns pair legality
(`legalPair`), port restrictions (`legalPorts`), and the
teardown-ordering guard (`edgeDemandsSeveredWitness`).

What a kind demands *beyond* connect/disconnect differs. A dpni↔dpmac
edge carries a driver-handback hazard: unbinding the dpni while the
edge stands strands the port driverless, so teardown demands a severed
witness (ADR-0008 §8). A dpni↔dpni edge has no handback mechanism by
construction — there is nothing to sever, only a connection to drop. A
dpdmux uplink cannot be disconnected at all on the pinned firmware
(ADR-0009). Without one record owning these differences, each edge kind
re-derives where connect is issued, what teardown demands, and how
kernel visibility heals — per-kind copies of one decision, and a
mid-pipeline fork that compiled intent (one `Edge` type) and the model
(one connection surface) do not have.

The healing question is the same decision seen from the container face.
ADR-0017 establishes that an MC-accepted create into a bound container
is kernel-invisible until a scan, and files the deferred-visibility
obligation forward to the change that makes post-bind faces
representable. A connection surface with post-bind connect faces is
that change: the obligation must be constructible exactly where the
face is.

## Decision

There is **one connection surface, and what varies by kind is a
declared reification policy — never a parallel code path.**

1. **Container-agnostic construct, surface-otherwise.** root↔root,
   root↔child, and child↔child links are all representable; connect is
   issued at a common ancestor of both endpoints (`CONNECT_ANCESTOR`;
   the root satisfies every pair). Patterns the MC refuses surface as
   typed refusals — nothing is pre-forbidden in the plan surface except
   what intent already refuses (`LinkSelfLoop`, a programmatic-parity
   refusal). Cardinality one and disconnect-before-reconnect are laws
   of the surface itself (DPRC-I5; the LINK-I\* invariants in
   `models/families/link_lifecycle.qnt`).

2. **Reification policy is a table row.** `dpaa2-api::plan` carries the
   edge-kind table mirroring `core/connect.qnt`. A kind's row declares
   its connect authority and its teardown law; machinery already
   delivered for a kind is claimed by the surface, not retyped — the
   dpmac transitions, executor, and `SeveredProof` minting stay where
   they are, keyed as they are, consumed the way `DPMAC_SeverOrder`
   consumes the shared predicate in the model. The structural proof
   that the lift is bounded: the dpmac family's frozen ITF traces
   replay green, unchanged.

3. **Lifecycle partial order.** Fresh convergence orders
   **populate → connect → bind**: connect faces live on the unplugged
   container typestate beside create/assign, extending ADR-0017
   decision 3. Post-bind connect/disconnect of **already-visible**
   endpoints is additionally legal — connect manufactures no bus
   device, so nothing in ADR-0017's evidence forbids it; the kernel-end
   `ENDPOINT_CHANGED` discard law is recorded at its board face, not
   assumed.

4. **The healing policy is a reification row, not a separate regime.**
   Post-bind *create* and *destroy* are governed by the last row of the
   table below, uniformly across kinds.

### The edge-kind table

The model is the source of truth and this table is the reader's copy
until its lint lands (ADR-0014; bead dpaa2-controlplane-yfa adds the
ledger R-rule cross-checking every row against `core/connect.qnt`).

| Edge kind | Corpus meaning | Reification policy |
|---|---|---|
| dpni ↔ dpmac | physical port; MAC inherited dpmac→dpni (DPNI-I3) | connect + kernel bind; **severed-witness teardown**: the edge is severed (dpmac handed back KernelOwned→Offered, minting `SeveredProof`) before the dpni's kernel face unbinds (`edgeDemandsSeveredWitness`; ADR-0008 §8) |
| dpni ↔ dpni (incl. loopback) | point-to-point pair, incl. cross-container (DPNI-I9) | root-issued connect at `CONNECT_ANCESTOR`; **disconnect-only teardown** — no driver handback exists by construction (`edgeDemandsSeveredWitness` is false for the kind) |
| dpni ↔ dpsw.N.M | switch port membership | awaits its own change; no recorded teardown hazard |
| dpni ↔ dpdmux.N.M | demux downlink | awaits its own change; no recorded teardown hazard |
| dpsw.N.M ↔ dpmac | switch uplink | awaits its own change; no recorded teardown hazard |
| dpdmux.N.0 ↔ dpmac | demux uplink; port 0 only, MC ≥ 10.37 refuses the rest (`legalPorts`) | un-disconnectable on the pinned firmware — the model refuses the teardown (ADR-0009) |
| dpci ↔ dpci | inter-partition link; the only same-family-only edge (DPCI-I1) | awaits its own change; no recorded teardown hazard |
| **healing — post-bind container face, every kind** | an MC-accepted create into a bound container is kernel-invisible until a scan (ADR-0017) | post-bind create is representable **only** with an eager `DeferredVisibility` obligation attached — unconstructible without a planned discharge. The only discharge is a **consented rebind cycle** (unbind → bind → re-observe), a `Disruptive`-class plan transition on the ADR-0015 consent machinery; declined consent is a typed standing residue. Destroy is the lazy mirror: a stale node blocks nothing and may stand indefinitely as residue. Ordering law: **disconnect-before-destroy** — destroy is reachable only from a disconnected endpoint; an engine typestate policy, stricter than the MC (V-LINK-6, 2026-10-05: the firmware accepts the still-connected destroy and drops the edge atomically — LINK-I1 state face). No reboot residue exists on this surface (reboot stays exclusive to ADR-0020 pool shrink). If a gentler scan-bearing event is found at the raw command path (#10), it lands there as a second discharge route; ADR-0017's open question stays open |

### Adapter posture

McControl connection verbs are named 1:1 with whitelisted MC commands,
ancestor-explicit, with typed returns; restool text parsing stays an
implementation detail behind the trait, and the surface adds no portal
read slices (ADR-0021 — the portal backend of roadmap #10 drops in
under the differential gate).

## Consequences

- A new edge kind is a table row plus a model row, not a design debate;
  a parallel per-kind connection path is a review defect, cited against
  this record.
- ADR-0017's forward-filed obligation has its representation: post-bind
  create carries `DeferredVisibility` explicitly, and the 0017
  amendment points here instead of restating it.
- Convergence judgment is unchanged by healing: a bound consumer
  container is converged by re-observation after a scan (ADR-0017
  decision 2); the obligation types *when the plan may claim it*, not
  what observation means.
- The teardown laws are typed once at the surface: severed witness for
  dpni↔dpmac, disconnect-only for dpni↔dpni, refusal for the dpdmux
  uplink — a family never carries edge law internally (ADR-0019 edge
  facet).

## References

- `models/core/connect.qnt` — `legalPair`, `legalPorts`,
  `edgeDemandsSeveredWitness` (source of truth for the table).
- `models/families/link_lifecycle.qnt` — wire typestates, the LINK-I\*
  invariants, both healing obligations and the consented discharge.
- `docs/baseline/object-model.md` §2 — the corpus edge inventory and
  per-edge semantics the model encodes.
- `docs/baseline/dprc.md` — connect/disconnect rows; the V-DPCI-1
  banked refusal witness (child-issued connect, `No privilege (0x4)`).
- `openspec/changes/cross-dprc-links/design.md` — cross-dprc-links D2
  (container-agnostic surface), cross-dprc-links D3 (bounded lift),
  cross-dprc-links D4 (partial order), cross-dprc-links D5 (healing
  policy), cross-dprc-links D8 (this record's mandate).
- ADR-0008 §8 — the driverless-interval hazard behind the severed
  witness.
- ADR-0014 — bead dpaa2-controlplane-yfa, the table's lint obligation.
