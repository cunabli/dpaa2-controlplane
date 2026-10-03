# Design: cross-dprc-links

Decisions D1–D10 were settled in the scoping grilling of 2026-10-03;
this document is their durable record. Anchors: ADR-0013 §link,
ADR-0015, ADR-0017, ADR-0019, ADR-0021, `docs/baseline/object-model.md`
edge table, `docs/baseline/dprc.md` connect/disconnect rows,
`models/core/connect.qnt`.

## Context

Intent has derived dpni↔dpni edges since `intent-layer` (rule
`link-edge`, plus `fabric-wire`), but no path actuates them: child
populate's `planned_peer` resolves port-edges only and root reconcile
actuates dpni↔dpmac only, so link edges land as `plan_only`. The MC
side is small and known: `dprc connect/disconnect` at a common
ancestor, cardinality one, disconnect-before-reconnect, child-issued
connect refused `No privilege (0x4)` without `TOPOLOGY_CHANGES_ALLOWED`
(V-DPCI-1). `connect_in(ancestor, dpni, peer)` already renders the
cross-container form. No dpni↔dpni connect or frame has ever been
suite-witnessed (DPNI-I9 rests on production kdpni pairs); V-TRAF-1 is
a waiting placeholder with an empty port list. This change also owns
the ADR-0017 healing policy (bead dpaa2-controlplane-w01) and the
PASS4-F8 forward pointer.

## Goals / Non-Goals

**Goals:**

- Intent-declared dpni↔dpni links converge end to end and are
  board-witnessed carrying frames (root↔root) and connecting across
  containers (root↔child, child↔child).
- A shared connection surface whose per-kind reification keeps the
  model's `core/connect.qnt` structure isomorphic in Rust.
- The ADR-0017 drift obligations typed, planned, and heal-witnessed.
- McControl connection verbs portal-ready for #10.

**Non-Goals:**

- No VPP in suites, sittings, or writings (one directional context
  sentence in the proposal only); VPP use is post-archive product use.
- No sustained-rate targets; the saturation smoke judges monotone
  counters and zero discards, never a rate.
- No external ports: no dpmac appears in any face; the Mellanox §8
  decision point is dropped, not decided.
- No Rust netlink and no new dependency; no hal change.
- No portal read-slice additions (ADR-0021: #10 owns them).
- No link attributes (rates stay unexpressed: no NXP script ever used
  them; refusal-by-omission at the schema).

## Decisions

### D1 — Outcome and witness split (grilling Q1, amended twice)

Frame witness on a root↔root wire via netns separation in suite hook
scripts (the V-TRAF-0 pattern: exact-count dpni statistics oracle),
plus a saturation smoke face. Cross-container is witnessed at
connect/convergence level. Full teardown; nothing left standing.
*Alternative rejected:* VPP at the far end (couples the sitting to a
sibling effort; the capability, not board state, is the handoff).

### D2 — Container-agnostic construct, surface-otherwise (Q1)

The link construct never privileges cross-DPRC: root↔root, root↔child,
child↔child are all representable; connect is issued at a common
ancestor (root constant today, `CONNECT_ANCESTOR`). Patterns the MC
refuses surface as typed refusals; nothing is pre-forbidden in the
model except what intent already refuses (`LinkSelfLoop` stays — a
programmatic-parity refusal, vocabulary-v2).

### D3 — Shared connection surface, bounded lift (Q2, "a-bounded")

`dpaa2-api::plan` gains an edge-kind table mirroring
`core/connect.qnt`: legal pairs plus a per-kind **reification policy**
(dpni↔dpmac = connect + bind + severed-witness teardown; dpni↔dpni =
root-issued connect, disconnect-only teardown — no driver handback by
construction, `edgeDemandsSeveredWitness` stays false). **Bounded:**
the delivered dpmac transitions, executor, and `SeveredProof` minting
stay where they are, keyed as they are; the surface claims them the way
`DPMAC_SeverOrder` consumes the shared predicate in Quint. MC fixes
edge arity at two (multi-port endpoints are address syntax,
`<object>.<id>.<port>`), so the table's shape survives #11/#12.
*Acceptance proof:* the dpmac family's frozen ITF traces replay green,
unchanged. *Alternative rejected:* a parallel link-only path — it would
freeze a mid-pipeline fork that compiled intent (one `Edge` type) and
the model (one connection surface) do not have.

### D4 — Lifecycle partial order (Q3)

Fresh convergence: **populate → connect → bind** — connect faces live
on the unplugged container typestate beside create/assign, extending
ADR-0017 decision 3. **Post-bind connect/disconnect of already-visible
endpoints is additionally legal** (connect manufactures no bus device;
nothing in ADR-0017's evidence forbids it) and gets its own board face,
recording the kernel-end `ENDPOINT_CHANGED` → `-EPERM`-discarded dmesg
law (`dpaa2-eth.c:4839`).

### D5 — ADR-0017 healing policy (Q4; discharges PASS4-F8, closes w01)

Post-bind create becomes representable **only** with an eager
`DeferredVisibility` obligation attached — MC-accepted,
kernel-invisible, convergence still judged by re-observation after a
scan. The only modeled discharge is a **consented rebind cycle**
(unbind → bind → re-observe), a first-class `Disruptive`-class plan
transition riding the existing ADR-0015 consent machinery. Declined
consent → typed standing residue (the pool-objects honest-residue
idiom). The destroy side is the lazy mirror: a stale node blocks
nothing and may stand indefinitely as residue. Ordering law:
disconnect-before-destroy (finding 34 generalized; destroy reachable
only from a disconnected endpoint). No reboot exists on this surface —
reboot residue remains exclusive to ADR-0020 pool shrink. If #10's raw
path finds a gentler scan-bearing event, it lands there as a second
discharge route (ADR-0017's open question stays open).

### D6 — Netlink side is harness-only (Q5)

The netns/ping rig is test instrumentation, not control plane: it rides
operator-reviewed `--hook` shell files (ADR-0003: files are the only
board interface). Zero Rust netlink, zero deps, hal untouched — its
"netlink arrives with the change that consumes it" note stays true
because no control-plane consumer exists (netns/addressing is dataplane
config outside the intent vocabulary, like MTU and ethtool).
Forward-thinking lands on **McControl instead**: connection verbs named
1:1 with whitelisted MC commands, ancestor-explicit, typed returns
(`ObjectRef`, a `LinkState` type) — restool text parsing stays an
implementation detail behind the trait so #10's portal backend drops in
under the differential gate.

### D7 — Model gate shape (Q6)

New `models/families/link_lifecycle.qnt` consuming `core/connect.qnt`
operators: wire typestates (ends-exist → connected → disconnected →
end-destroy-legal), the D4 partial order, both D5 obligations with the
consented discharge. Invariants **LINK-I\***: disconnect-before-destroy;
no post-bind create without a planned discharge; cardinality-one /
disconnect-before-reconnect (**DPRC-I5 promoted from candidate**);
MC-refused patterns surface as refusals. Apalache-marked where the
state space permits. V-TRAF-1 scenario module renders the suite.

### D8 — One ADR, three amendments (Q6)

One new **connection-surface ADR**: the edge-kind table with the
healing policy as a reification row (Q2 and Q4 are one decision seen
from two sides). Amendments: ADR-0017 (PASS4-F8 discharged, pointer to
the new ADR), ADR-0019 (edge facet gains its second inhabitant and the
kind table), ADR-0003 §8 (Mellanox decision point dropped; revisit
trigger reworded to "a phase requiring sustained external traffic", no
tile attached).

### D9 — Deferral adoption rule and the #10 assignments (Q7)

A row parked at "traffic-bearing #9" rides this change **iff it needs
zero machinery beyond what this change builds**. In: **DPCON-I3**
(dpcon priority knob, same ping face) and **SINGLE_SENDER** (baseline
dpni unknown #7; one wire variant) — both need small additive intent
knobs (neither is expressible today). Assigned to #10, which they
structurally require (no restool verb for `set_notification`, no table
read-back, no multi-priority observation): **DPCON-I4**, **dpni
unknowns #4 and #11**. Mechanics: roadmap row #10's Delivers column
gains one line naming a single **dossier bead** that carries this
session's full analysis; COVERAGE/baseline markers re-point to the
tile; the bead reparents under #10's epic when scoped (the 5y7/#10
pattern). Root cause recorded: deferral-by-capability made #9 a magnet;
after this change the rig is a standing harness asset, so that queue
cannot re-form.

### D10 — Board suite V-TRAF-1 (Q7)

One sitting, pre-run record commit, full teardown. Faces: (1) root↔root
converge + frame witness + saturation smoke; (2) cross-container
populate→connect→bind + post-bind connect with dmesg law recorded,
judged via `dprc_get_connection` from root; (3) child↔child generality;
(4) create-side heal (obligation → consented rebind → resident in IOMMU
group — the V-DPRC-8 oracle at the right face); (5) destroy-mirror
unknown resolved (does a stale node linger?); (6) teardown laws
(disconnect-live, disconnect-before-destroy, DPRC-I5 double-connect
refusal); child-issued-connect refusal replays the banked V-DPCI-1
witness rather than re-running. No PHY dpmac anywhere (finding 49).

## Risks / Trade-offs

- [Lift destabilizes delivered dpmac behavior] → the frozen-trace
  replay gate is the acceptance criterion for parcel 3; any diff fails
  the parcel.
- [Destroy-mirror behavior unknown] → modeled as an explicit unknown
  with both outcomes typed; the face answers it and the baseline is
  amended either way.
- [Post-bind connect surprises on a bound-VFIO endpoint] → face judges
  from root via `dprc_get_connection`; dmesg recorded, not assumed;
  divergence feeds back to the model per the standing loop.
- [Saturation smoke flakes via kernel-stack variance] → oracle is
  monotone counters + zero discards, never timing or rate.
- [Knob creep from DPCON-I3/SINGLE_SENDER] → both are additive schema
  fields with derivation pass-through; no new construct, no new family.

## Migration Plan

Additive throughout; no persisted state exists to migrate. Intent files
without links or new knobs converge exactly as before (defaults
preserve current derivation). Rollback is reverting the change commits.

## Open Questions

None at scoping. Board-answerable unknowns are deliberately carried as
suite faces (D10): the destroy-mirror behavior and the post-bind
connect dmesg law.
