# ADR-0017: VFIO override propagation is real, and deferred to the next container scan

- **Status:** Accepted — board sitting 2026-09-13 (suite V-DPRC-8
  rev 1, dprc-encapsulation task 5.2)
- **Date:** 2026-09-14
- **Supersedes / relates to:** OpenSpec change `dprc-encapsulation`
  (design D6, task 5.2); ADR-0006 (visibility is established by
  re-observation, never by issuing sync — DPRC-I6's loudness law);
  ADR-0008 (kernel hot-bind availability is a reference-pair property);
  `docs/baseline/dprc.md` (Kernel-side VFIO bullet, silent-failure notes)

## Context

The consumer path binds a child DPRC to `vfio-fsl-mc` via
`driver_override` + `bind` — the driver has no match table
(`vfio_fsl_mc.c:423-452`). The pinned kernel source promises that once a
DPRC is bound, a bus notifier propagates the override to every
subsequently added child device (`vfio_fsl_mc.c:523-526, 600-607`), and
change #4's typestates lean on that promise: a consumer container
converged while bound must hand every resident to VFIO. Whether the
propagation is observable at all under container-only population was the
change's design open question — a resident must be *added* while the
container is bound for the notifier to have anything to do.

## What the board answered

V-DPRC-8 rev 1 (trace 2/2; every acceptance face held) created a dpbp in
the scratch child while the child was bound to `vfio-fsl-mc`:

- The MC accepted the create and `dprc show` listed the resident
  immediately — but no bus device appeared, and no restool verb made one
  appear while the container stayed bound.
- The next bind-time container scan — the suite's re-bind face —
  surfaced the resident with the override propagated and the device in
  the container's IOMMU group (the sitting's dmesg carries both lines
  for dpbp.2).
- The suite's one hook FAIL was the propagation oracle judged one face
  early; the evidence landed at the later face. The board did not
  diverge.

## Decision

1. **Propagation is trusted; visibility is not.** The
   override-propagation notifier works as the source promises, but it
   fires at a container *scan*, and no restool verb triggers a scan of a
   bound container. The reconciler treats an MC-accepted create into a
   bound container as invisible to the kernel and to VFIO until a
   bind cycle (unbind + bind) or another scan-bearing event runs.
2. **Convergence claims extend DPRC-I6's law to the VFIO face.**
   "Converged" for a bound consumer container is judged by
   re-observation after a scan — never by create acceptance, and never
   by `dprc sync`, which reaches root containers only.
3. **Population order for the consumer typestates: populate, then
   bind.** A consumer container is populated first and bound last. The
   pure core makes the alternative *unrepresentable* today: the create
   and assign faces exist only on the unplugged container typestate and
   are absent once the container is `Plugged`, so a post-bind create
   cannot be planned and buys no deferred-visibility obligation. Should
   population-after-bind ever land (tile #6), that create must carry the
   deferred-visibility obligation explicitly — the obligation is filed
   there, not represented here (PASS4-F8).

## Consequences

- `docs/baseline/dprc.md`'s Kernel-side VFIO bullet carries the verified
  claim and the silent-failure notes carry the trap (amended with this
  ADR, dprc-encapsulation task 6.1).
- Board oracles that judge bus-visible effects anticipate event-driven
  landings at *later faces*, not just later instants — the V-DPRC-8
  oracle-placement lesson.

## Open questions and revisit triggers

- **What besides a bind-time scan surfaces the resident.** Whether the
  bound container's own IRQ rescan path can fire under `vfio-fsl-mc`
  was not discriminated: the sitting also recorded (not judged) that the
  fresh child's bus node was already present at hook time, so neither
  `dprc sync` nor the bus rescan was discriminated as the surfacing
  route. A dedicated probe separates the routes; revisit at
  `mc-portal-backend` (#10)'s raw command path if not before.
