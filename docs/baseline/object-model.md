# Object model: cross-family relationship map

Synthesis of the 16 family baselines (task 6.1; spec: object-baseline,
"A cross-object relationship map exists"). Every claim here is distilled
from a family document and carries its anchor — `(dpni.md, DPNI-I4)` means
the claim, its evidence, and its verification status live there. This doc
adds no new evidence; it arranges the per-family facts into the five views
a typestate design and a Quint lifecycle model need: containment, connect
edges, create-vs-allocate, allocation pools, and lifecycle ordering. The
change series that consumes this map is sequenced in
[../ROADMAP.md](../ROADMAP.md); the process rules the views lean on are
ADR-0003 (board envelope) and ADR-0006 (single-initiating-writer) under
[../adr/](../adr/).

## 1. Containment: the DPRC tree

Every object lives in exactly one DPRC at a time. The tree on the
reference environment: `dprc.0` (the MC's own container, aliased
`mc.global`) → `dprc.1` (the root container Linux sees) → `dprc.2` (the
VPP child) (dprc.md [verified]).

State a container contributes to the model:

- **Identity, fixed at create**: container id, `icid` (the IOMMU isolation
  context — VFIO groups by container), and its own MC command portal.
  All pool-assigned; restool cannot pin them, only a DPL can (dprc.md).
- **Permission mask, fixed at create**: `SPAWN | ALLOC | OBJ_CREATE |
  IRQ_CFG` is the restool default and the observed VPP-child mask
  (DPRC-I4 [verified]); the root additionally has `TOPOLOGY_CHANGES`.
  Which bit gates which operation is an open permission matrix
  (dprc.md unknown #3) — and a permission bit is **not** the last gate:
  the MC's create handler can refuse on orthogonal grounds (platform gate
  for dpaiop, root-container gate for dpdbg) even where `OBJ_CREATE` is
  allowed (DPAIOP-I2).
- **Membership, mutable**: `dprc assign --child` moves an object
  parent↔child, but **only while unplugged** (DPRC-I3). Crossing a
  container boundary changes the object's icid domain and which pools and
  drivers can reach it — allocation and driver binding never cross the
  boundary (DPRC-I1, DPMCP-I4).
- **Locking**: `set-locked 1` strips create/destroy/assign/lock from the
  entire sub-hierarchy (DPRC-I11, board open after V-DPRC-3 rev 1).

Placement constraints that are laws, not conventions: dpdbg exists only in
the root container (firmware-refused elsewhere; DPDBG-I1); dprtc is a
firmware-enforced system-wide singleton normally DPL-born in the root
(DPRTC-I1); dpmacs are DPC-born in the root and stay there — the runtime
lifecycle of a dpmac is connect/disconnect, not create/move (DPMAC-I1).

**Visibility is per-observer, not global** (the model must carry it as
separate state): MC-side existence, kernel-bus existence, and driver
binding are three distinct predicates. Bus rescan reaches root containers
only and discards errors (DPRC-I6, breaking); most creates trigger no
rescan at all (DPCI-I3); child containers refresh only via their own IRQ
path when `autorescan` is on (dprc.md unknown #12).

## 2. Connect edges

Connections are owned by the **dprc** family: `dprc connect` on a common
ancestor of both endpoints, both currently disconnected, endpoints
allowed to live in different containers (DPRC-I5, DPNI-I9 [verified
cross-container]). restool is type-agnostic; the MC validates pair
legality. Multi-port objects address endpoints as `type.id.port`.

Edge inventory supported by the corpus:

| Edge | Meaning | Anchor |
|---|---|---|
| dpni ↔ dpmac | physical port; MAC inherited dpmac→dpni at connect+bind | dpni.md DPNI-I3 [verified] |
| dpni ↔ dpni | point-to-point pair, incl. cross-container (kernel↔VPP pseudo-wire) | dpni.md DPNI-I9 [verified] |
| dpni ↔ itself | loopback (ls-addni `--loopback`) | dpni.md [read] |
| dpni ↔ dpsw.N.M | switch port membership | dpsw.md [read] |
| dpni ↔ dpdmux.N.M | demux downlink | dpdmux.md [read] |
| dpsw.N.M ↔ dpmac | switch uplink | dpsw.md [read] |
| dpdmux.N.0 ↔ dpmac | demux uplink; MC ≥ 10.37 refuses non-dpmac uplinks that older firmware accepted | dpdmux.md [read, MC changelog] |
| dpci ↔ dpci | inter-partition link; the only same-family-only edge | dpci.md [read] |

Families with **no endpoint surface at all**: dpbp, dpio, dpcon, dpmcp,
dpseci, dpdmai, dpdcei, dprtc, dpdbg. Their consumer relationships are
allocation or binding, never connection. dpaiop's `aiop_container_id` is
a create-time reference to an AIOP-flagged dprc, not a connect edge
(DPAIOP-I4).

Semantics the model must carry per edge:

- **Cardinality one**: an endpoint has at most one peer; reconnect
  requires disconnect first (DPNI-I9, DPCI-I1).
- **Asymmetry is legal** where port counts differ: a dpci's Tx priority
  capacity is the *peer's* count, not its own (DPCI-I1).
- **Connection ≠ link**: `dprc` connection state, MAC link state, and
  consumer-enabled link state are three different variables. dpmac
  `info`'s "link is up" is connection state (DPMAC-I5, breaking); a
  connected-but-never-enabled dpci may never leave link-down (DPCI-I5);
  the dpmac link API is a directional pair — peer requests flow down
  (`get_link_cfg`), PHY reality flows up (`set_link_state`) (DPMAC-I4).
- **Connect events are one-sided**: only the peer object (dpni) gets an
  ENDPOINT_CHANGED interrupt; the dpmac has none — connect-then-plug and
  plug-then-connect are both legal orders because the late side absorbs
  the event (dpni.md, dpmac.md).
- Kernel endpoint lookup encodes container scope: `-ENOTCONN` when
  unconnected, `-EPERM` when the peer exists in another container — the
  latter is what makes the standalone dpmac driver keep the PHY while VPP
  owns the datapath cross-container (dprc.md, DPMAC-I6 [verified]).

## 3. Create vs allocate

Two disjoint ways an object reaches a consumer, and the model must never
conflate them (spec scenario "Create-vs-allocate is unambiguous"):

- **Created**: an actor (restool, DPL, MC at boot) makes a new object in
  a container; a driver then binds it 1:1 when plugged.
- **Allocated**: a driver claims an *existing* object from its
  container's kernel pool. Pools exist for exactly four resource types:
  **`FSL_MC_POOL_DPMCP`, `FSL_MC_POOL_DPBP`, `FSL_MC_POOL_DPCON`, and
  the IRQ pool** (dprc.md, dpbp.md). Nothing else is pooled — notably
  **dpio is not** (no `FSL_MC_POOL_DPIO`; DPIO-I1).

Per family:

| Family | Reaches its consumer by | Pool (if allocated) |
|---|---|---|
| dprc | created by parent (SPAWN); bound by `fsl_mc_dprc` automatically | — |
| dpmac | MC-created at boot from the DPC; consumed by *connection*, standalone driver binds the leftovers | — |
| dpni | created; `fsl_dpaa2_eth` or VFIO/PMD binds on plug | — |
| dpbp | created + plugged → enters pool; consumer allocates | `FSL_MC_POOL_DPBP` |
| dpcon | created + plugged → enters pool; consumer allocates | `FSL_MC_POOL_DPCON` |
| dpmcp | created + plugged → enters pool; claimed **only** via `fsl_mc_portal_allocate` (the object-allocate path refuses dpmcp) | `FSL_MC_POOL_DPMCP` |
| dpio | created + plugged; kernel binds each to the dpio driver (per-CPU shared service); DPDK claims exclusively per thread — never pooled | — |
| dpseci | created; `dpaa2_caam` or VFIO/PMD binds | — |
| dpsw | created; `dpaa2-switch` binds (kernel-only — DPDK discards dpsw; DPSW-I6) | — |
| dpdmux | created; staging `dpaa2-evb` binds, or DPDK claims via VFIO | — |
| dpdmai | created; `fsl-dpaa2-qdma` binds after rescan, or DPDK dmadev | — |
| dpci | created + `dprc connect`; no kernel driver — userspace/DPDK consumers only | — |
| dprtc | DPL-created singleton; `fsl-dpaa2-ptp` binds | — |
| dpdbg | runtime-created singleton (root only); driver-less by design | — |
| dpdcei | creatable (hardware present), no consumer in the corpus | — |
| dpaiop | not creatable on this platform (DPAIOP-I1) | — |

The MC additionally distinguishes **objects from resources** — pooled
primitives (e.g. `mcp`) enumerable via `dprc_get_res_count/ids`; restool's
help hides the split but the model must keep it (dprc.md).

**Consumer draw census** — what each binding driver pulls from its own
container's pools at probe (the C1 class of failures when missing;
allocation never crosses containers, DPRC-I1):

| Consumer | dpmcp | dpbp | dpcon | dpio |
|---|---|---|---|---|
| dpni (`fsl_dpaa2_eth`) | 1 | 1 | `min(affine-dpio CPUs, num_queues)` | borrowed from shared service |
| dpio driver (per dpio) | 1 | — | — | (is the dpio) |
| dpsw | 1 | 1 (ctrl-if) | — | borrowed |
| dpdmux (`dpaa2-evb`) | 1 | — | — | — |
| dpseci (`dpaa2_caam`) | 1 | — | — | borrowed |
| dpdmai (`fsl-dpaa2-qdma`) | 1 | — | — | needs dpios present |
| dprtc / standalone dpmac | 1 each | — | — | — |
| `/dev/dprc.N` opener beyond the first | 1 each (concurrent, returned on close) | — | — | — |

The DPDK/VPP regime draws **zero** from kernel pools: the child
container's objects are VFIO-owned and claimed directly (2 exclusive
dpios per thread, dpbps/dpcons/dpmcps by the PMD) (dpio.md, dpbp.md
[verified]).

Exhaustion is silent by default: the first missing pool object of a type
is a quiet `-EPROBE_DEFER` retry loop (`-ENXIO "No more resources of type
%s left"` underneath); dpcon shortage *after* the first degrades the
consumer instead of deferring it (DPNI-I5, breaking). Pool free does no
MC reset — membership never implies clean state (DPBP-I3, DPMCP-I3,
breaking).

## 4. Allocation pools: sizing couplings

The paid-for rules, all evidence-anchored in the pool family docs:

- **dpio**: regime-typed, the most miscounted number on the board
  (DPIO-I2 [verified]). Kernel container: one per online CPU, ceiling
  enforced (`-ERANGE` beyond). Poll-mode child: **exactly `2 × T`**, T
  the consumer's thread count (main plus workers) — a general and a
  receive portal per thread, each an exclusive dpio (ADR-0012).
- **dpmcp**: one per portal-consuming object *including one per dpio*
  (the forgotten draw), plus concurrency headroom for the management
  plane itself — N simultaneous uapi openers need N−1 free dpmcps
  (DPMCP-I1/I2). Poll-mode child: **one per process** — the bus maps a
  single dpmcp per process and every object's commands go through it
  (ADR-0012, DPMCP-I7 [verified]); the 3 the board's child carries is a
  script constant, two of them idle.
- **dpbp**: one per kernel dpni/dpsw; **exactly two per poll-mode
  child** (the consumer's own pool + the driver's scatter-gather pool)
  regardless of port count (DPBP-I6, ADR-0012 [verified]).
- **dpcon**: one per polled queue per consumer —
  `min(num_queues, consumer cores)` per kernel dpni, one per queue in
  the poll-mode regime (DPCON-I1 [verified]).
- **irq**: 256 per container, `-ENOSPC` on exhaustion (dprc.md).

Derivation direction for the intent compiler: dpni sizing *triggers* the
companion math, the pool families *carry* it. The dependency arrows
bottom out at dpmcp, which depends only on container membership
(DPMCP-I1) — with one twist: dpio, itself unpooled, **draws a dpmcp** at
kernel probe, so dpio→dpmcp is an ordering edge inside the "pool
companion" phase (DPIO-I1).

## 5. Lifecycle ordering

The canonical forward order, board-validated for the dpni case
(dpni.md/dprc.md, C1 [verified]) and structurally identical for every
created-then-bound family:

1. **Pool companions first**: create + plug dpmcp/dpbp/dpcon (and
   dpios) in the consumer's container. The kernel scanner itself
   enforces the same order — allocatables probe before consumers in
   every scan pass (DPRC-I8).
2. **Create the consumer object** in (or move it to) the target
   container. Placement moves are legal only while unplugged (DPRC-I3).
3. **Pre-plug mutation window**: the only restool-expressible one is
   `dpni update --mac-addr` — and it is futile against a kernel
   consumer, because **bind resets the object** (next point).
4. **Plug** (`assign --plugged=1`) — the kernel's bind trigger
   (DPRC-I2). Binding **resets the object** for dpni, dpdmux, and dpsw
   (dpsw additionally dismantles MC-default state; DPNI-I2, DPDMUX-I3,
   DPSW-I4, all breaking): only the immutable cfg block survives into a
   bound consumer. dpdmux alone lets a prior owner store skip-reset
   policy (`set_resetable`) that a later reset honors.
5. **Connect** endpoints from a common ancestor — before or after plug,
   both legal (ENDPOINT_CHANGED absorbs the late side).
6. **Rescan** where the consumer needs bus visibility and the mutation
   didn't trigger one (dpci/dpdmai creates don't; `dprc create` does).
   Sync is not visibility (DPRC-I6, breaking).
7. **Enable** — always consumer-side. restool enables nothing in any
   family; a restool-only object is created-but-never-enabled (dpci,
   dpdcei, dpseci docs).

Create-time gates orthogonal to this order: platform (dpaiop refused on
AIOP-less silicon regardless of permissions, DPAIOP-I2), placement
(dpdbg root-only), cardinality (dprtc singleton, second create refused),
and firmware-version behavior swings inside an "additive" API window
(dpdmai's DPL-vs-CREATE divergence pre-10.38.1, DPDMAI-I2; dpdmux's
10.37 uplink restriction).

**Teardown** is the mirror with weaker guarantees:

1. Unbind the driver (restool `destroy` refuses driver-bound objects —
   but the `in_use` check reads root-container sysfs only, so
   child-container and VFIO-bound objects pass it; dpdmux.md).
   Unbind-side resets are best-effort (dpni) or version-gated (dpseci
   ≤ 5.3 leaves armed state behind, DPSECI-I5); dpbp/dpcon return to
   the pool dirty; dpmcp is never reset at all.
2. Destroy on the **parent dprc token** with **all object tokens
   closed**. Connected endpoints are not checked by restool for any
   family; whether MC demands disconnect first is an open item
   (dpci.md unknown #2).
3. Recurse into now-unconsumed suppliers (companions). The shipped
   tooling derives this set from the **kernel device-link graph**, which
   exists only for kernel-bound consumers — a VFIO-owned object has no
   such links, so the port must derive teardown order from this map,
   not from sysfs (dprc.md, dpni.md).
4. Trust nothing about destroy exit codes: the destroy-error-overwritten-
   by-close bug is family-wide in child containers, and `ls-delete`
   discards outcomes wholesale (DPMAC-I8, DPSW-I7, DPDMUX-I7).

Linux-side removal never destroys MC objects (DPRC-I7, breaking); MC-side
destroy under a bound driver is an unguarded race (dpni.md).

## 6. Cross-cutting laws for the typestate and Quint seeds

Distilled once here; each is proven per-family in the anchored docs:

1. **The cfg block is the type parameter.** Every family's create config
   is immutable post-create — drift is refuse-and-report, never repair.
   The exhaustive exceptions across all 16 families: dpdmux
   `default_if`, dpmac `eth_if` + IPG (DPNI-I1, DPDMUX-I4, DPMAC-I3).
2. **Read-back is the only convergence observable.** Exit status is
   systematically weaker than state (create-then-fail, destroy-error
   overwrite, 0-exit failure paths); some attributes are write-only and
   admit *no* drift detection (dpni `dist_key_size`, dpci `options`,
   dpseci options via restool, dpdbg's entire SET surface) (DPNI-I6,
   DPCI-I2, DPSECI-I3, DPDBG-I2).
3. **Visibility is tri-state per object**: MC existence, bus visibility,
   driver binding — advanced by different actions, never inferred from
   each other (DPRC-I6, DPCI-I3, DPRC-I2).
4. **Two id spaces.** Datapath addressing uses MC-assigned hardware ids
   (dpbp `bpid`, dpcon `qbman_ch_id`), not object ids; in-tree code that
   conflates them works by coincidence (DPBP-I5, DPSW-I5, DPCON-I2).
5. **Ownership exclusivity is an assumption, not an MC property.** The
   API permits concurrent tokens everywhere; single-initiating-writer is
   ADR-0006's modeling assumption, and the dprtc kernel-MMIO vs DPDK-MC
   split shows hardware allowing the silent fight (DPSECI-I7, DPRTC-I2).
6. **Version is per-action state.** Nothing negotiates: every client
   emits statically-versioned commands, and firmware behavior moves
   inside "additive" flib windows — the model carries the emitted
   command version and the firmware behavior version as part of each
   action (DPNI-I11, DPDMAI-I2).
7. **Object ids are reused, not retired.** The MC hands out the lowest
   free id of a family, in one namespace across every container: a
   destroyed object's id is reissued by the next create of that family
   [board, 2026-08-29: every 5.10 suite got `dprc.2` again; V-DPRC-5
   released `dpci.0`/`dpci.1` and V-DPCI-2 was handed `dpci.0` first;
   `dpmcp.53` was reissued six times across earlier sittings]. The
   Quint models' monotone `nextNum` is an abstraction that keeps names
   unique inside one trace — it is not a claim about the firmware. A
   name therefore identifies an object only while that object is known
   to be alive; across a destroy the same name may be someone else, the
   ABA hazard of ADR-0010.
