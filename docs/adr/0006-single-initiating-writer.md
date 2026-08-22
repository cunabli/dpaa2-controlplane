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
