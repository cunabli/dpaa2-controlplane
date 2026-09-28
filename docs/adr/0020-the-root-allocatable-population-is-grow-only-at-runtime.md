# ADR-0020: The root container's allocatable population is grow-only at runtime

- **Status:** Accepted — pool-objects phase 4 (bead
  dpaa2-controlplane-960.12, the 4.3 board sitting; the reclaim path it
  narrows is the pool-objects design D10 custody decision)
- **Date:** 2026-09-26
- **Supersedes / relates to:** ADR-0003 §7 (the reboot-required
  recovery this record widens from dpio seats to every root pool
  family); ADR-0011 (resource ceilings are pool-bound where the census
  can see — this record names a second thing the census cannot see);
  ADR-0008 §4 (the rescan race that punishes root-container object
  churn from the other direction); ADR-0007 (creator-bound destroy —
  the MC-side authority limit, where this is the kernel-side one)

## Context

Every allocatable object plugged in the root container is bound by the
kernel's `fsl_mc_allocator` driver, which owns the resource pools the
rest of the kernel draws from (`fsl_mc_object_allocate`). This holds
for `dpbp`, `dpmcp`, `dpcon` and `dpio` alike, and it holds regardless
of provenance: an object created by this tool at runtime and plugged
into the root is claimed exactly as a DPL-born one is. The boundary is
the container and the plugged bit, not the object's origin.

Reclaiming such an object requires unplugging it, and restool refuses
a plugged-state change on any driver-bound object client-side, before
issuing an MC command at all (`cannot be changed plugged state because
it is bound to driver ... unbind it first`). The refusal is identical
for a free object and a drawn one, so it carries no information about
drawn-ness.

Removing the driver first does not open the path, and this is the
decisive point. `fsl_mc_resource_pool_remove_device` does carry the
right guard — a resource absent from the pool's free list is in use and
the removal returns `-EBUSY` — but its caller
`fsl_mc_allocator_remove` is a `void` remove callback, and the Linux
driver model does not let a driver refuse removal: `device_release_
driver` proceeds whatever the callback does. A sysfs unbind of an
in-use allocatable object therefore *succeeds* from userspace, emits
one `dev_err` line, and leaves the device detached from the allocator
while a consumer still holds its resource. Probing drawn-ness by
unbinding would mean corrupting the allocator's bookkeeping on every
wrong guess, detectable only by scraping a log line after the damage.

No read-only substitute exists either: the free list is internal to the
allocator with no sysfs attribute, and the MC reports pool counts but
nothing per-object. At root scope there is consequently no safe
drawn-ness signal, by probe or by read.

Inside a child container the situation is different and unaffected:
VFIO owns the residents, no allocator competes for them, and the
unplug-probe reclaim law works as designed.

## Decision

### 1. Root capacity grows at runtime and shrinks only across a reboot

The pool construct remains the sole provider of root pool-family
objects (the single-provider decision) and sizes root capacity to
intent by growing. It does not reclaim root capacity at runtime.
Surplus — whether left by a departing port, a narrowed intent, or an
emptied one — persists until the next boot, when the DPL baseline is
restored.

### 2. Unreclaimed root capacity is typed residue, never silence

Surplus the tool must not reclaim renders on every operator surface
(`ensure`, `dry-run`, `status`) as a reboot-required disposition
stating observed versus required and naming the reconciliation path,
exactly as grow-only dpio seats already do. Intent stays the expressed
truth; inventory the tool cannot move is reported against it, never
carved out of it.

### 3. The unplug-probe reclaim law is child-scoped

The reclaim law — unplug of a drawn object refused, the refusal being
the drawn signal; a plugged-free object unplugged then destroyed —
applies where the tool's own handoff owns the objects, which is inside
a child container. It makes no claim at root scope.

### 4. Prune keeps the reach the hardware allows

An undeclared root object that was never plugged carries no allocator
binding and is destroyable, so prune still reaches it; the double gate
and the one-label law are unchanged. An undeclared object that *is*
plugged falls under decision 2 and is reported as residue.

## Consequences

The change's system-integration claim narrows accordingly: drift at
root scope heals upward only, and teardown returns the DPL baseline
modulo reboot-required residue. Reclamation inside a child container
is unaffected and complete. A tool run therefore never returns root
capacity, and an operator sizing a board across successive intents
should expect the high-water mark to stand until reboot.

## Open questions and revisit triggers

- **The kernel fix is the real remedy, and it is small.** Either the
  fsl-mc bus refuses the unbind before `device_release_driver` runs
  (a pre-removal check against the same free list), or the allocator
  stops being a driver whose removal cannot fail. Landing either one
  re-opens root-scope reclaim and re-anchors decisions 1–3 in this
  record; the local-tree attempt is scoped alongside the other two
  kernel findings in `docs/upstream/phylink-dpni-churn-crash.md`.
- **Whether the firmware would permit the unplug at all is unmeasured.**
  restool refuses client-side, so the MC is never asked. If a future
  transport asks it directly (the ioctl portal, or VFIO), the answer
  becomes observable and decision 1 is worth re-testing — bearing in
  mind that unplugging an object Linux has plugged is itself the
  ADR-0008 §4 trigger, so a permissive firmware answer would not by
  itself make runtime reclaim safe.
- **A per-object drawn-ness attribute would settle it without a policy
  change.** Nothing in sysfs exposes whether an allocatable object is
  currently lent out. If a kernel version adds one, the census gains a
  read-only signal and the probe stops being the only route.
- **The dpmcp portal leak is a separate irreversibility with the same
  practical shape** (a destroyed dpmcp does not return its portal until
  reboot, ADR-0011 §3). Both are reboot-bounded; a fix to one does not
  imply a fix to the other.
