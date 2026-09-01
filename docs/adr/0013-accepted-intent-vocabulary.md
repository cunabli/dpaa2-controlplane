# ADR-0013: The accepted intent vocabulary, and what it derives, refuses, and guarantees

- **Status:** Accepted (2026-09-02) — the gate of OpenSpec change
  `intent-layer` (phase 1, design D7) is closed: the §8 questions are
  decided in place below (bead `dpaa2-controlplane-gqf.14`). The
  vocabulary, derivation, refusals, and invariants are settled in the
  model (`models/intent/`, seed 20260831). This record reissues the
  phase-1 gate: the earlier standalone `docs/intent.md` is retired in its
  favour.
- **Date:** 2026-09-02
- **Supersedes / relates to:** ADR-0005 (the intent-layer decision this
  record elaborates in place, §§1–5); ADR-0012 (companion sizing by regime
  and thread count — the numbers this record prices by reference); ADR-0011
  (three-valued resource ceilings — feasibility checks against them);
  ADR-0003 §3 (reserved dpmacs); ADR-0001 §§3–4 (DPMAC-anchored identity,
  DPL ownership); ADR-0010 (keys are identities, labels a projection);
  OpenSpec change `intent-layer` design D1–D13 + D6a.

## Context

An operator provisions the DPAA2 Management Complex (MC) on the reference
LX2160A board by writing an intent, not by naming MC objects. This record is
the reference for what they write and what the tool derives from it, written
before any Rust exists — working backwards from the file the operator types
(design D7). The Quint model is the source of truth: every construct, count,
and refusal below is transcribed from `models/intent/` (`types.qnt`,
`derive.qnt`, `refuse.qnt`, `invariants.qnt`) and cites the ADR or baseline
that anchors it. Where this prose and the model could drift, the model wins.

The operator states *capacity at L1 and who consumes it* — a tenant's core
budget, a port's rate, a crypto block's flow demand — and never an
implementation count: no dpio, dpbp, dpcon, dpmcp, dpsw, queue, or worker
number appears in an intent. `compile(intent, inventory)` is a pure, total
function (design D5): given the same intent and inventory it returns either a
complete plan — every object keyed and every count carrying its provenance —
or the *complete* list of refusals. It never returns the first error and
stops, never emits a partial plan, and never reads the board.

## Decision

### 1. What an intent is: the `[intent]` table

A TOML file opens with an `[intent]` table that anchors the document-level
properties. `schema` is mandatory — the version hook the next breaking change
needs (design D1) and where a future document-level property (a name, an
inventory pin) would sit:

```toml
[intent]
schema = 1
```

### 2. The constructs

Field names are those of `models/intent/types.qnt` and the scenario `.toml`
files. Five declarative constructs plus the additive override channel.

**`[[tenant]]`** — a consumer of hardware capacity, a network dataplane in a
container. Anchors to a DPRC.

```toml
[[tenant]]
name = "router"
dataplane = "userspace-poll"    # kernel-netlink | userspace-poll | userspace-event
max_cores = 16
isolation = "isolated"          # public | restricted | isolated (default isolated)
pool = ""                       # a restricted tenant's public holder; "" otherwise
```

`dataplane` selects the sizing regime and names the ownership mechanism, not
just kernel-vs-userspace: `kernel-netlink` (the kernel's own driver over
netlink, priced by ADR-0012's kernel draws — leaving room for a future
XDP/BPF kernel dataplane beside it), `userspace-poll` (a poll-mode process —
VPP, DPDK — priced by ADR-0012's poll draws), or `userspace-event`, which
ADR-0012 does not price and the compiler refuses (`UnpricedDataplane`, §5)
until a scenario prices its draws. `max_cores` is the budget the derived
thread count T must fit under (design D3). `isolation` and `pool` place the
tenant in the container tree (§4). The tenant name `kernel` is **reserved**
(design D1): the kernel's own driver in the root container (dprc.1), never a
named tenant, implicitly `public`, and materialised only when a port omits a
tenant or a link end names it — the operator never writes a
`[[tenant]] name = "kernel"` block.

**`[[port]]`** — a physical port the tenant must deliver at `rate`. Anchors to
a dpmac (ADR-0001 §3). Derives a dpni terminating the dpmac (DPAA2 User
Manual §2.2.2 figure 6a) and its dpni↔dpmac edge, and — through `rate` — the
port's contribution to T (§4).

```toml
[[port]]
name = "wan0"
dpmac = "dpmac.9"
rate = 10000                    # Mbps, the unit `dpmac info` reports maxima in
tenant = "router"               # omit ⇒ the reserved kernel terminates it
```

**`[[link]]`** — a point-to-point dpni↔dpni pseudo-wire between two tenants
(object-model.md §2, DPNI-I9; figure 6b). Each end names the tenant whose
*interface* terminates the wire (an interface, not a port — room for
tunnels). A link end may name the reserved `kernel` without declaring it; the
derivation materialises the reserved kernel's dpni in Root (design D6a).
Derives one dpni at each named end and the edge between them — no dpmac, no
hardware.

```toml
[[link]]
name = "uplink"
interface_a = "ns1"
interface_b = "kernel"          # the reserved kernel end, undeclared
```

**`[[fabric]]`** — one switched domain over its members (ports, tenants, or
other fabrics). `forwarded_by` names the tenant that runs the forwarding
plane. `switching = "hardware"` derives one dpsw (figure 6c) owned by the
forwarder, `num_ifs` = its interface count; only the kernel can drive a dpsw,
so a hardware fabric not forwarded by the kernel is refused. `switching =
"software"` emits no dpsw — the forwarding tenant bridges its own dpnis, which
the MC never sees. Members are ordered: declaration order numbers the dpsw
interfaces and the dpni ordinals. A chain of switches is a software fabric
listing a hardware fabric; a hardware fabric listing a hardware fabric is
refused until a baseline verifies dpsw↔dpsw.

```toml
[[fabric]]
name = "lan"
switching = "hardware"
forwarded_by = "kernel"
members = ["lan0", "lan1", "router"]
```

**`[[crypto]]`** — an accelerator for one tenant, anchored to a dpseci
(dpseci.md). One block sizes **one** dpseci by its own `flows`, with the
`DPSECI_OPT_HAS_CG` safety bit. A tenant may declare several `[[crypto]]`
blocks; the array is ordered, so a tenant's Nth block numbers its Nth dpseci
(ordinal N). `flows` must land in one device's realizable range
`1..DPSECI_MAX_QUEUE_NUM` (16 queue pairs — `drivers/crypto/caam/dpseci.h:25`
and `drivers/crypto/dpaa2_sec/mc/fsl_dpseci.h:25`): below 1 is
`CryptoFlowsNotPositive`, above 16 is `CryptoFlowsOverDevice`, refused not
clamped, because one block is one device — the remedy is splitting the demand
across blocks (task 2.6e).

```toml
[[crypto]]
tenant = "router"
flows = 2
```

**`[[extra]]`** — the additive raise-only override channel (design D5). Every
derived count is a *request*; a per-(tenant, family) extra adds its `count` on
top, so the effective count is request + count — raise-only by construction,
no floor comparison to get wrong. Only the four companion families
dpio/dpbp/dpmcp/dpcon accept an extra (any other is `ExtraNotCompanion`), and
`count` must be ≥ 1 (`ExtraNotPositive`). Extras are matched by (tenant,
family), unordered, never by position.

```toml
[[extra]]
tenant = "kernel"
family = "dpmcp"
count = 2                       # e.g. provision a secondary-process portal
```

### 3. The two inputs: intent and the observed inventory

`compile` takes the intent above and the **inventory** — what the hardware
offers, observed and never operator-written (design D2; the rejected
alternative was an operator-written inventory the board would contradict).
`ensure` reads it from the board; the model reads it from change-#2's
reference snapshot (`models/board/baselines/reference.json`, regenerated into
`intent/inventory.qnt`). It carries three things:

- **dpmac offers** — each dpmac with the attributes `dpmac info` reports,
  immutable for the object's life (DPMAC-I3): `max_rate` (Mbps), `eth_if`, and
  an **availability**: `Free` (may anchor a port), `Reserved(why)` (the
  ADR-0003 §3 safety matrix — dpmac.3 total-deny, dpmac.17 management plane),
  or `Foreign(owner)` (a DPL-owned object, ADR-0001 §4 — dpni.0's dpmac.17 on
  the reference board). The compiler refuses a reserved or foreign anchor by
  name.
- **foreign DPL objects** — objects a plan must never claim, with their owner
  label (ADR-0001 §4).
- **three-valued ceilings** — one per derived family (ADR-0011):
  `Counted(n)` (the listed pool is the ceiling — dpbp = 63), `Observed({n,
  provenance})` (an unlisted ceiling the board measured — dpni = 18), or
  `Unknown` (the cap ended without a refusal, or the family was never
  measured). Feasibility refuses against `Counted`/`Observed` and warns on
  `Unknown` — it never invents a number.

### 4. The derived quantities and placement rules

Every number carries provenance: its rule, the inputs it consumed, the
construct it bottoms out in, and the ADR/baseline anchor (design D6),
printable as a tree — `dpio ×10 ← 2·T ← T=5 ← 1+Σ workers ←
workers-table(10G⇒2, unmeasured) ← ports wan0/wan1 rate=10G`.

**The worker table and T (design D3).** The one derivation the corpus cannot
anchor is capacity → thread count, so it is a declared, *visibly unmeasured*
table (`derive.qnt` `WORKER_TABLE`):

| rate class | workers/port | basis |
|------------|--------------|-------|
| 10G (10000) | 2 | **seeded** — decomposed from the verified configuration: two 10G ports in poll-mode ran T = 5 = 1 + 2·2, the configuration ADR-0012's companion numbers were verified against |
| 25G (25000) | 5 | **declared** — linear-in-rate from the 10G row, unmeasured, signed off at gate close (§8) |
| anything else | — | refused `UnknownRateClass` |

**T = 1 main + Σ workers(rate)** over the ports the tenant terminates. A
portless tenant is main-only (T = 1). `max_cores` bounds T; T > `max_cores` is
`CoreBudgetExceeded`. Every T carries the mark **unmeasured**. A tenant whose
terminated ports span more than one seeded class compiles but carries an
`UnmeasuredCombination` warning — cross-class pricing is flagged, never
silent.

**Companion draws by regime (ADR-0012, `companionDraw`).** The numbers live in
ADR-0012; this record prices by reference:

| family | userspace-poll (per child dprc) | kernel-netlink (root kernel) | kernel-netlink namespace (child-resident) |
|--------|--------------------------------|------------------------------|-------------------------------------------|
| dpio | 2·T | one per online CPU (`cpus`) | **0** — dpio services are one kernel-global per-CPU list every container shares |
| dpbp | 2 | one per dpni (+1 per dpsw, per dpseci) | one per dpni |
| dpmcp | 1 (one MC portal per process) | `cpus + dpnis` (+1 per dpsw, per dpseci) | one per dpni |
| dpcon | dpnis·T | dpnis·min(cpus, queues) = dpnis·cpus | dpnis·cpus |
| dpni tx queues | ≥ T | `cpus` | `cpus` |

- **dpseci**: one per `[[crypto]]` block, `num_queues` = that block's `flows`,
  `DPSECI_OPT_HAS_CG` set.
- **dpsw**: one per hardware fabric under the kernel-bindable predicate
  (dpsw.md, read-not-verified): `num_ifs` = interface count, `max_fdbs` =
  `num_ifs`, PER_FDB flooding and broadcast, control interface enabled.
- **dprc**: one child DPRC in Root per non-kernel tenant that owns one (an
  isolated tenant or a public holder), restool-default options (Spawn, Alloc,
  ObjCreate, IrqCfg).
- **dprtc.0**: pinned in Root, exactly one, kernel-owned (DPRTC-I1/I2). dpdbg
  is never derived.

The kernel `dpmcp = cpus + dpnis` is ADR-0012's forgotten draw: every kernel
dpio draws its own MC portal, so dpmcp ≥ dpio there; on the DPDK bus it is one
portal per process regardless of dpio count.

**Isolation and the container tree (design D6a, task 2.6c).** The MC container
tree already enforces a private-VLAN shape, and the vocabulary names it —
parent authority is promiscuous (dprc.1 drives its whole subtree), sibling
child dprcs are MC-isolated, co-residency inside one dprc is community
visibility (no per-object ACL):

- **isolated** (the default) — its own child dprc, MC-isolated from siblings.
  Both a userspace-poll process and a *declared kernel-netlink namespace* land
  here; the latter gets a kernel-bound child dprc that the kernel drives
  (`drivers/bus/fsl-mc/dprc-driver.c`: `match_id_table` binds any obj_type
  "dprc" l.882-885, `dprc_scan_container` rescans l.746,
  `dprc_add_new_devices` adds discovered child dprcs l.202-224). Its draw is
  the child-resident kernel draw above — **dpio 0** (the kernel-global per-CPU
  dpio list, `drivers/soc/fsl/dpio/dpio-service.c` l.54), dpbp/dpmcp one per
  dpni, dpcon dpnis·cpus — drawn from its parent container's pool
  (`drivers/bus/fsl-mc/fsl-mc-allocator.c` l.106: a consumer draws companions
  from its PARENT container's pool).
- **restricted** — community co-residency: the tenant's objects are created in
  its `pool` holder's dprc and it derives no dprc of its own, though it keeps
  its own keys and its own regime draw (a DPDK secondary still draws its own
  dpmcp). The concrete case is a DPDK secondary pooling a userspace-poll
  primary.
- **public** — a holder that accepts legal drawers into its own dprc. The
  reserved kernel is implicitly public; a userspace-poll primary may declare
  it.

**Companion pricing and feasibility.** Companion pricing is by reference to
ADR-0012 (the numbers live there once). Feasibility is a cross-plan
sum-vs-ceiling check against the ADR-0011 ceilings: the summed derived count
per family against `Counted`/`Observed` refuses `Infeasible(family, needed,
available)`, and a non-zero `Unknown` family warns `UnknownCeiling`.

### 5. The refusal vocabulary

`compile` runs every rule unconditionally and returns their union — the
*complete* refusal set, never the first violation, so the operator fixes a
file in one pass. An empty set yields the plan and its warnings; a non-empty
set is the whole answer. All 24 variants of `refuse.qnt`, grouped by rule; the
Rust enum spells the anchor pair `Reserved`/`Foreign` (see §11).

*Undeclared references*
- `TenantAbsent` — a construct (port, link end, fabric owner, crypto, extra, a
  restricted tenant's `pool`) names a tenant not declared → declare it or fix
  the name.
- `MemberUnresolved` — a fabric member names a port/tenant/fabric not declared
  → declare it or fix the member list.
- `SelfMember` — a fabric member resolves to the fabric's own owner → remove
  the self-reference.

*Port anchor (design D2; ADR-0003 §3; ADR-0001 §4)*
- `Unanchored` — the port's dpmac is not in the inventory → pick a dpmac the
  board offers.
- `Reserved` — the dpmac is Reserved by the safety matrix → pick another; this
  one must never carry traffic.
- `Foreign` — the dpmac is Foreign, owned by a DPL object → do not claim it.
- `OverRate` — `rate` exceeds the dpmac's `max_rate` → lower the rate or move
  the port.

*Double claim*
- `DoubleClaimed` — two ports on one dpmac, or one port in two fabrics →
  remove the duplicate claim.

*Fabric rules*
- `FabricNotKernelForwarded` — a hardware fabric whose `forwarded_by` is not
  the kernel → only the kernel drives a dpsw.
- `PortTenantMismatch` — a member port whose `tenant` differs from the
  fabric's forwarder → align them.
- `UnsupportedEdge` — a hardware fabric listing a hardware fabric →
  unsupported until dpsw↔dpsw is verified.

*Sizing (design D3)*
- `UnknownRateClass` — a userspace-poll tenant terminates a rate class with no
  seeded worker row → see §8 OQ3.
- `CoreBudgetExceeded` — the derived T exceeds `max_cores` → raise the budget
  or shed ports.

*Extras (design D5)*
- `ExtraNotCompanion` — an extra on a family that is not one of the four
  companions → only dpio/dpbp/dpmcp/dpcon accept extras.
- `ExtraNotPositive` — an extra whose count is below 1 → raise it or remove it.

*Crypto (design D1; dpseci.md)*
- `CryptoFlowsNotPositive` — a crypto block whose flows are below 1 (carries
  the block's 1-based ordinal so two bad blocks stay distinct).
- `CryptoFlowsOverDevice` — a block whose flows exceed one dpseci's 16 queue
  pairs → split the demand across blocks.

*Feasibility (ADR-0011)*
- `Infeasible` — the summed derived count for a family exceeds a
  Counted/Observed ceiling → the request does not fit the board (names family,
  needed, available).

*Dataplane pricing (design D3)*
- `UnpricedDataplane` — a tenant whose dataplane ADR-0012 does not price
  (today `userspace-event`) → use a priced dataplane.

*Isolation and pooling (task 2.6c)*
- `PoolWithoutRestricted` — a `pool` on a non-restricted tenant is a
  contradiction.
- `RestrictedWithoutPool` — a restricted tenant that names no pool holder.
- `HolderNotPublic` — a restricted tenant's pool holder is not public.
- `PoolChain` — a restricted tenant's holder itself has a pool → no chains.
- `PoolDataplaneMismatch` — a restricted drawer's dataplane differs from its
  holder's (the reserved kernel counts as kernel-netlink).

Two warnings attach to an accepted compile: `UnknownCeiling` (a derived
family's ceiling is Unknown, so feasibility could not check it — accepted, not
refused) and `UnmeasuredCombination` (a userspace-poll tenant mixes more than
one seeded rate class — the formula prices it, but cross-class pricing is
unmeasured).

### 6. The invariants the plan type makes unrepresentable

The plan relationships design D6 wants unrepresentable in Rust, first stated
as named predicates over the derived `Plan` (`invariants.qnt`, ids
INTENT_I1–I9); the Rust type surface (task 3.1) transcribes what they prove.

- **INTENT_I1 `containmentByTenant`** — every object sits in a real container:
  the kernel's own in Root, a child dprc marker in Root at ordinal 1, no
  non-kernel object floating in Root, and no dpmac or dpdbg ever a derived
  object.
- **INTENT_I2 `edgesTypedAndSingle`** — typed connect ends, no double connect:
  a dpni at port 0, a dpsw interface within `num_ifs`; link/wire ends are
  dpni↔dpni, a port-edge is dpni↔dpmac, a fabric-edge is dpsw↔(dpmac|dpni);
  no end is connected twice, no dpmac↔dpmac edge.
- **INTENT_I3 `companionsOnlyDerived`** — a dpio/dpbp/dpmcp/dpcon exists only
  as its tenant's count node, its ordinal run exactly the node's value, never
  free-standing.
- **INTENT_I4 `emissionOrderLawful`** — emission order covers the objects
  once; dprtc.0 first; a child dprc precedes its tenant's other keys; every
  pool companion precedes the dpni/dpseci/dpsw that draw it; in the kernel
  every dpio precedes every dpmcp (its forgotten draw).
- **INTENT_I5 `keysAreIdentities`** — no two objects share a key, every
  ordinal is 1-based; the key is the identity, the label a projection
  (ADR-0010).
- **INTENT_I6 `provenanceClosed`** — every prov reference resolves, every node
  carries an anchor, and the only rule that may be `Unmeasured` today is "T".
- **INTENT_I7 `feasibleAgainstCeilings`** — a compile is either a plan within
  every Counted/Observed ceiling (with an `UnknownCeiling` warning per
  non-zero Unknown family) or a non-empty refusal set — never an empty
  refusal.
- **INTENT_I8 `companionCountsByRegime`** — every tenant's companion counts are
  the sizing's effective values, each = request + declared extra, the dpio
  request is exactly `companionDraw`'s field (drawCpus 0 for a child-resident
  namespace, online CPUs for the root kernel), every dpni carries ≥ T
  (poll-mode) or exactly `cpus` (kernel) transmit queues, and every object of
  a tenant lives in that tenant's container.
- **INTENT_I9 `isolatedContainerPrivate`** — an isolated tenant's objects live
  only in its own child dprc, and no other tenant's objects appear there; a
  holder must be public, so an isolated container is never a pool target.

### 7. The scenarios as worked witnesses

Each scenario is an intent `.toml` an operator types, paired with a `.qnt`
asserting the derived plan (design D8); numbers are the model's.

- **fabric** (`scenarios/fabric.*`) — two 10G ports (dpmac.7/8) in a
  kernel-forwarded hardware fabric, with a userspace-poll `router` joined as a
  tenant member terminating dpmac.9/10 at 2×10G. Derived: one kernel dpsw with
  `num_ifs` = 3 (two member dpmacs + the router's attach dpni); the switched
  member ports yield no kernel dpni; the router takes 3 dpnis at T = 5, dpio =
  10, dpbp = 2, dpmcp = 1, dpcon = 15. **Twin:** `max_cores` = 4 sits below T
  = 5 and is refused `CoreBudgetExceeded(router, 5, 4)` and nothing else — the
  kernel owner's budget never binds under the online-CPU reading of OQ2, so
  the poll-mode member is what makes the refusal reachable.
- **vfabric** (`scenarios/vfabric.*`) — two userspace-poll tenants
  `routerA`/`routerB` joined by one link, no dpmac, no kernel tenant. Portless
  ⇒ T = 1: each side one dpni (`numQueues` = 1), dpio = 2, dpbp = 2, dpmcp =
  1, dpcon = 1, joined by the single dpni↔dpni pseudo-wire. **Twin
  (model-only):** a third portless `routerC` draws 6 dpbps against a ceiling
  lowered to `Counted(5)` and is refused `Infeasible(dpbp, 6, 5)`; the same
  twin against the reference pool (63) fits — the ceiling refuses, not the
  shape.
- **router** (`scenarios/router.*`) — one userspace-poll `router` over 2×10G
  (dpmac.9/10) + 1×25G (dpmac.4), with one `[[crypto]]` `flows` = 2. Derived:
  T = 1 + 2·2 + 5 = 10, 3 dpnis at 10 queues, dpio = 20, dpbp = 2, dpmcp = 1,
  dpcon = 30, one dpseci with `num_queues` = 2 and HAS_CG. The 10G+25G mix
  carries `UnmeasuredCombination(router, {10000, 25000})`. **Twin
  (model-only):** a second tenant `router2` claims dpmac.9 — refused
  `DoubleClaimed(9, {wan0, clash0})` and nothing else.
- **vwire** (`scenarios/vwire.*`) — kernel-namespace pseudo-wires over all
  three link shapes: `ns1`/`ns2` (kernel-netlink, isolated) and `vpp`
  (userspace-poll, portless), joined by veth (ns1↔ns2), uplink (ns1↔kernel,
  kernel undeclared), and app (ns2↔vpp). Each namespace is child-resident with
  two dpnis: {dpio 0, dpbp 2, dpmcp 2, dpcon 32}, `numQueues` = 16, in
  Child(nsX) — the first witness of the dpio-0 child-resident draw. The
  undeclared reserved kernel is materialised in Root with the full per-CPU
  draw {dpio 16, dpbp 1, dpmcp 17, dpcon 16}; `vpp` is portless at T = 1 {dpio
  2, dpbp 2, dpmcp 1, dpcon 1} in Child("vpp"). **Twin (model-only):** `vpp`
  turned into a restricted drawer pooling the kernel — refused exactly
  `{ PoolDataplaneMismatch }` (a userspace-poll drawer pooling the
  kernel-netlink kernel).
- **reference — the fit check** (`scenarios/reference.*`) — the reference
  board's deployed provisioning (the reserved kernel plus one userspace-poll
  tenant terminating dpmac.7/9 at 10G). It has **no twin**: its counterpart is
  the recovered boot census (`reference.json`), and the diff — not a second
  compile — is what the runs assert. The census is the BOOT state (root dprc.1
  only), so the whole child side is `DeferredToLive` (the 4.1/4.2 live
  sitting). Root-side dispositions: dpni.0/dpbp.0/dpseci.0/dpcon/dpmcp are
  ForeignDpl or BootResident (DPL statics — the kernel consumes 16 of the 52
  dpmcps, the rest idle boot portals, DPMCP-I6); dpio 16 and dprtc.0 match.
  The child dpmcp (derived 1 vs board 3) is `DpmcpThreeVsOne` — OQ1 (§8).

## Consequences

- The paid-for provisioning knowledge lives in one pure, model-checked
  derivation instead of shell scripts and human memory; the operator states
  what they mean, and the derivation explains itself with per-object
  provenance.
- The vocabulary is a commitment: a construct that cannot be anchored in
  hardware is not in it, and a fourth construct comes through a change with
  baseline backing, not a field added in passing.
- Fabric and crypto are plan-only until their executors land (#11, #8); the
  dpsw predicate is copied from `dpsw.md`'s read-not-verified rules with
  kernel-source anchors, and the ledger keeps DPSW-I1/I2 board-pending.
- ADR-0005 §§1–5 are elaborated here in place (see §11); its numbered section
  references resolve through this record.
- This record's enumerations — the 24 refusal variants (§5), INTENT_I1–I9
  (§6), and the five scenarios (§7) — are hand-maintained copies of what
  `models/intent/*.qnt` states, and copies drift (the `COVERAGE.md` narrative
  drifted exactly this way across tasks 2.6b/2.6c until 2.6d caught it). They
  are slated for the `dpaa2-verify` ledger lint — the design-D9 cross-check of
  the archived `verify-foundation` change — in phase 3, tracked as bead
  `dpaa2-controlplane-gqf.34`, alongside the `.qnt`/`.toml` pairing test the
  scenario files already promise (task 3.4). Until that lands, the models are
  the source of truth and this record is the reader's copy.

## 8. Open questions — decided at gate close (2026-09-02)

Each entry keeps the question as posed and records the decision.

- **OQ2 — the kernel tenant's `max_cores`.** Two readings carried as a
  parameter (`derive.qnt` `KERNEL_BUDGET_IS_DPIO_COUNT`): *online CPUs* (the
  current default, `= false`) — `max_cores` is not a bound on the kernel
  dataplane, its dpio count is the online-CPU count, and the budget never
  binds (pinned by `fabric.qnt`'s `kernelBudgetNeverBindsTest`); or *a budget
  below the dpio ceiling* — `max_cores` rations the kernel's dpio draw.
  **Decided: online CPUs.** `max_cores` is not a bound on the kernel
  dataplane; `KERNEL_BUDGET_IS_DPIO_COUNT` stays `false` as the decided
  reading, pinned by `kernelBudgetNeverBindsTest`. The parameter remains in
  the model as the recorded seam, not an open alternative.
- **OQ3 — rate classes beyond 10G/25G.** Refused-not-derived is the standing
  behaviour: a rate class with no seeded worker row is refused
  `UnknownRateClass`, never extrapolated. A third class enters only with a
  board that has one. **Decided: confirmed — refused, never derived.** The
  rate-class set is a property of the observed inventory, not of the
  derivation: the reference board's boot configuration splits one 100G cage
  into 4×25G with a single usable lane. An unknown class is an
  inventory-reconciliation gap to surface, not a row to extrapolate. A future
  entry point that regenerates the boot configuration may relax the
  constraint — recorded as a revisit shape in §10.
- **The 25G ⇒ 5-worker sign-off.** The 25G row is declared, not measured —
  linear-in-rate from the seeded 10G row (§4). Every T it prices carries the
  mark *unmeasured* and a cross-class `UnmeasuredCombination` warning;
  `max_cores` bounds the result. The gap closes when a per-class measurement
  lands (an NXP-citeable per-worker figure, or a traffic-bearing board suite
  under ADR-0003). **Signed off.** The declared row stands with its
  *unmeasured* mark and `UnmeasuredCombination` warning until the §10
  measurement trigger closes the gap.
- **OQ1 — the 3-vs-1 child dpmcp — largely dissolved by the pool model.** The
  derivation gives one MC portal per process; the deployed board's child
  carried 3 = 1 needed + 1 possible secondary + 1 idle boot portal (DPMCP-I6).
  The `restricted` isolation now makes the secondary *expressible* (a
  restricted secondary pooling the primary draws its own dpmcp), and an
  `[[extra]]` of count 2 on the kernel would raise the derived 1 to 3.
  **Residual:** whether the reference intent should declare that secondary (so
  the plan derives 3) or whether the fit-check diff reports the board's two
  extra portals as explained divergence. The model derives 1 and reports the
  two extra as `DpmcpThreeVsOne`. **Decided: acknowledged as-is.** The
  board's three portals are read as boot defaults; the reference intent
  declares no secondary, and the fit-check divergence report is the standing
  explanation.

## 9. Honest relaxations

- **Restricted co-residency is a hardware-imposed, opt-in relaxation of
  sole-tenancy.** A container has no per-object ACL, so co-residents in one
  dprc see one another; `restricted` names that visibility rather than hiding
  it. It is opt-in (default `isolated`), and INTENT_I9 keeps every isolated
  container sole-tenant.
- **INTENT_I1's corner.** A restricted kernel-netlink drawer pooling the
  reserved kernel would place its objects in Root, which the plan-only
  INTENT_I1 cannot distinguish from a floating object. INTENT_I8 asserts that
  placement intent-aware, and no exercised intent derives one — the alphabet
  tenants are never kernel-netlink — so the corner is on record, not reached.

## 10. Revisit shapes recorded, not implemented

- **Crypto `max_cores` split** — a future `[[crypto]]` core budget, so an
  accelerator's threads ration against a per-block budget rather than sizing
  purely by `flows`. Not built.
- **Port commitment = `line-rate` | `best-effort`** — a future `[[port]]`
  qualifier stating whether `rate` is a hard commitment or best-effort,
  feeding the worker table. Not built.
- **Rate classes from the boot configuration.** The rate-class set is
  inventory: the SerDes split the reference board boots with (one 100G cage as
  4×25G, one usable lane) defines which classes exist, and that configuration
  is changeable. A future entry point that regenerates the boot configuration
  (DPL/DPC) may derive or relax the class set from that source instead of the
  seeded worker table; until it lands, an unknown class refuses (§8 OQ3).
- **The quota/cap → solver trigger.** Feasibility is a sum-vs-ceiling check,
  not a solver: it answers "does this set fit", never "how many fit". An
  intent that asks the latter turns feasibility into an ILP (Z3/MiniZinc
  territory); recorded so nobody bends the checker into a solver.
- **CEL for extras** (design D5; ADR-0005 amendment) — a policy-expression
  language for extras, as Kubernetes uses for validation rules. Recorded, not
  built.
- **The worker-table measurement trigger** — an NXP-citeable per-worker figure
  or a traffic-bearing board suite replaces the declared 25G row with a
  measured model.
- **YANG/gNMI northbound** (ADR-0005 §5) — if intent expression outgrows this
  tool, gNMI over OpenConfig-style paths is the pre-identified candidate; the
  construct vocabulary is designed to survive that translation. No northbound
  wire interface is built ahead of it.

## 11. Model-encoding notes

- **QNT101 anchor-refusal naming.** The model spells the reserved/foreign
  anchor refusals `ReservedAnchor` / `ForeignAnchor` to avoid a name clash
  with the `Availability` constructors `Reserved` / `Foreign` it matches on
  (`refuse.qnt`); the Rust enum keeps the spec names `Reserved` / `Foreign`.
- **QNT404 flattener workarounds.** The `REFUSALS_USE` / `WARNINGS_USE` /
  `COMPILED_USE` value blocks in `refuse.qnt`, `ETH_IFS` and its siblings in
  `types.qnt`, and the analogous `*_USE` blocks in `derive.qnt` exist only so
  the verify-path flattener does not drop a sum-type constructor whose only
  value use sits in another module. They are deletable when upstream Quint
  fixes constructor flattening, and carry no meaning for the vocabulary.

## References

- OpenSpec change `intent-layer`: design D1–D13 + D6a; `tasks.md` (phase 1,
  the 2.6 gate); `models/intent/` (`types.qnt`, `derive.qnt`, `refuse.qnt`,
  `invariants.qnt`, `alphabet.qnt`, `scenarios/`, `models/COVERAGE.md`).
- ADR-0005 (intent layer, elaborated here), ADR-0012 (companion sizing),
  ADR-0011 (three-valued ceilings), ADR-0003 §3 (reserved dpmacs), ADR-0001
  §§3–4 (DPMAC identity, DPL ownership), ADR-0010 (keys, not labels).
- `docs/baseline/object-model.md` (the edge table) and the per-family
  baselines under `docs/baseline/`.
- External anchors (design D7): RFC 9315 / RFC 9316 (intent is a declarative
  outcome plus constraints — `max_cores` is a constraint, `workers` would have
  been configuration), and the ONOS intent framework (per-type compilers
  producing installable intents, installers kept separate, recompiled on
  topology events — this change's compile/executor split).
