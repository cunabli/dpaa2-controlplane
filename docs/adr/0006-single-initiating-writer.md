# ADR-0006: Single initiating writer; the kernel is a reactive environment

- **Status:** Accepted — scoping session 2026-08-22
- **Date:** 2026-08-22
- **Supersedes / relates to:** OpenSpec change `restool-baseline` (design D9);
  ADR-0001 §2 (level-triggered reconciliation); ADR-0002 (what the models
  do and do not model)

## Context

Formal models need a concurrency stance before anything else: who can mutate
the MC bus, and when? Modeling the MC, the kernel drivers, udev, and the tool
as free-running concurrent actors would make every model an interleaving
explosion — and would not describe this system, where provisioning runs at
boot before workloads and no other agent issues MC commands. The stance has
to be stated once, precisely, so every model in the series can assume it
instead of hedging around it.

## Decision

### 1. The tool is the only initiator of MC-bus mutations during a pass

During a convergence pass, this tool is the single actor that *originates*
MC-bus mutations. Operators do not run restool concurrently with a pass;
no daemon competes. This is an operational contract, stated in the docs,
assumed by every model.

### 2. The kernel is not a second actor; its reactions fold into our transitions

The kernel mutates state — `fsl_dpaa2_eth` allocates DPBP/DPCON/DPMCP/DPIO
from container pools at probe (ADR-0001 C1), udev renames netdevs — but only
*in reaction to* the tool's own actions (plugging a DPNI, retriggering a
device). Models therefore treat each tool action's transition function as
including its kernel consequences: "plug DPNI" transitions to a state where
pool counts have dropped and a netdev exists. The kernel needs no separate
process in the model; it is the environment's deterministic response,
observed through the same level-triggered re-read as everything else.

### 3. Atomicity within a pass is an assumption; healing across passes is a guarantee

Within a pass we assume no interleaved foreign mutation. Across passes no
assumption is needed: the level-triggered design (ADR-0001 §2) recomputes
the plan from full observed state, so anything that changed between passes —
crashes, partial applies, even a foreign mutation — is converged or surfaced
as drift on the next pass.

### 4. The violation mode is recorded, not modeled

"Someone else mutated the bus mid-pass" is a documented operational
violation, not a modeled transition. We do not build detection machinery or
interleaving models for it. If it happens, the symptom is a failed or
drift-reporting pass, and the level-triggered next pass is the recovery.
Revisit trigger: any real deployment where a second initiator becomes
legitimate (e.g. another management agent on the same bus).

## Amendment 2026-08-29 — the kernel enforces the single writer (task 5.11)

The stance above was written as an operational contract: nothing stops a
second restool from running, so the docs ask operators not to. The
board sitting for task 5.11 (suites V-POOL-3 and V-CONC-1) found the
kernel already enforcing it, and more strictly than the contract asks.

`/dev/dprc.1` admits exactly one opener at a time on the reference
kernel. The first opener is handed the root container's own portal; any
later opener while the first is still open fails `open()` with `EINVAL`.
The path is `fsl_mc_uapi_dev_open` → `fsl_mc_portal_allocate(root
dprc)`, which allocates a dpmcp from the root pool and then records the
root dprc as that dpmcp's *consumer* with `device_link_add`. The dpmcp is
the root dprc's own child, so the link would make a device depend on its
descendant; the device core refuses such links, the allocator maps the
refusal to `EINVAL`, and the open fails. No dpmcp is ever short: the pool
had well over a hundred free portals and `ENXIO` — the errno the baseline
predicted for exhaustion — never appeared. Every concurrent restool the
sitting tried met the same errno: 119 of 120 held openers, 27 of 32
concurrent reads, one of two concurrent create loops. This is kernel
behaviour, not firmware, and it holds for any process that opens the
device, restool or not.

Three consequences for this decision record:

1. **§1 is no longer only a contract.** A second initiator through the
   uapi cannot exist while the first holds the device open. The tool must
   serialize its own restool (or portal) use, and an operator's restool
   run during a pass fails rather than interleaving. The "violation mode"
   of §4 therefore surfaces as `EINVAL` on the loser, not as drift.
2. **The MC-level question stays open.** Whether the firmware itself
   serializes commands from two portals correctly is what V-CONC-1 set
   out to learn and could not: the kernel stopped the second writer
   before a single command reached the MC. Answering it needs a second
   portal the uapi does not grant — the online driver with its own dpmcp
   (`mc-portal-backend`), or a kernel where the consumer link is taken on
   the opener's behalf rather than the root's.
3. **Multi-opener designs are out.** Any plan to run reconciler workers,
   a watcher and a CLI as concurrent openers of `/dev/dprc.N` is unsound
   on this kernel; one opener owns the device for its lifetime.

Revisit trigger: a kernel where `fsl_mc_uapi_dev_open` allocates the
dynamic portal without the root-as-consumer link (or takes the link from
a non-ancestor device). The check is V-POOL-3: more than one opener held
at once with `ENXIO`, not `EINVAL`, at exhaustion.

## Consequences

**Positive**

- Models stay linear-per-action: one initiating process, deterministic
  environment response — tractable for Apalache, honest about the system.
- Kernel reactions become *checkable postconditions* of tool actions (pool
  deltas, netdev appearance) instead of unmodeled noise.

**Negative / to watch**

- The models are only as valid as the operational contract; a concurrent
  restool invocation during a pass produces divergences the model calls
  impossible. The contract is therefore stated wherever board procedures
  are documented (ADR-0003 scripts run under it).
- Folding kernel reactions into transitions means each family's baseline
  document must actually capture those reactions (the kernel-side section
  is mandatory in the template) — an incomplete kernel-side section becomes
  a model gap.

## References

- OpenSpec change `restool-baseline`, `design.md` D9 (and D10 for the
  kernel-as-second-state-machine evidence lens).
- ADR-0001 §2, C1 — level-triggered reconciliation; the driver allocation
  reaction that motivated §2 above.
- `docs/baseline/_template.md` — mandatory kernel-side section per family.
