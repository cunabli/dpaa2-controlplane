# mc-backend delta — pool-objects

## ADDED Requirements

### Requirement: The restool shim drives the four pool families
The `dpaa2-mc` restool shim SHALL grow create and destroy verbs for
dpmcp, dpbp, dpcon, and dpio, resolving the disposition's count deltas
to concrete object ids: creates carry the family's cfg (dpcon/dpio
priorities, dpio channel mode) computed from the typed block, destroys
target the free individual the adapter selected — reclaim inside a
child container, or a never-plugged undeclared root object under
prune; plugged root capacity is never a destroy target (ADR-0020) —
and read-back is the only observation (exit status never is).
Order-sensitive sequencing that the typed surface does not carry (the
dpio→dpmcp probe draw) SHALL live procedurally in the adapter, per the
dpni set-MAC-before-plug precedent.

#### Scenario: A grow delta becomes N creates
- **WHEN** the reconciler dispatches a deficit of 2 dpcon in a
  container
- **THEN** the shim issues two dpcon creates in that container and the
  post-dispatch census reads back the new count

#### Scenario: A destroy targets only the adapter-selected free object
- **WHEN** the reconciler dispatches a surplus destroy for dpbp in a
  child container
- **THEN** the shim destroys exactly one free dpbp id and re-observes
  the census

### Requirement: The kernel root-bind face brings a dpni alive
The adapter SHALL drive the dpaa2-eth bind of a root-container dpni
whose pool companions are in place, and observe the outcome through
read-back (bus binding state, interface presence): probe success is
judged per-target, never inferred from the bind write alone (DPIO-I5).
A probe deferral for want of companions (-EPROBE_DEFER, the C1 silent
exhaustion class) SHALL map to a typed observation, not an error
swallow.

#### Scenario: Bind with satisfied draw produces a live interface
- **WHEN** a root dpni with its derived companions present is bound to
  dpaa2-eth
- **THEN** the observation reports the binding and the kernel network
  interface exists

#### Scenario: Bind with dry pool is observed as deferral
- **WHEN** a root dpni is bound while a companion family's free pool
  cannot satisfy the draw
- **THEN** the adapter observes and reports the deferred probe with
  the shortfall family, and no object is destroyed or created in
  response

### Requirement: Child-container population serves the VFIO handoff
The adapter SHALL populate a child dprc with a dpni and its derived
pool companions and drive the child's VFIO binding through the
typestates dprc-encapsulation delivered, so the container is
consumable by a userspace dataplane; the population SHALL be
observable as a census of the child matching the derived counts.

#### Scenario: A populated child binds to VFIO
- **WHEN** the reconciler converges an intent declaring a userspace
  tenant's container
- **THEN** the child dprc holds the dpni and the regime-derived
  companion counts and is bound to vfio-fsl-mc, read back from the bus

## MODIFIED Requirements

### Requirement: KernelControl actuates VFIO binding for child DPRCs
`KernelControl` SHALL observe and actuate the vfio-fsl-mc binding path for a
child DPRC — `driver_override` write, bind, unbind — and SHALL expose the
observed propagation of the override to subsequently-added children of a bound
container. Restool-unreachable portal faces (child-portal unlock, the
OBJ_CREATE_ALLOWED gate) are explicitly out of scope, deferred to tile #10.
The `pool-objects` (#6) DPL-defined-child window closed NOT FIRED (bead
dpaa2-controlplane-960.13, 2026-09-27), so DPRC-I8 batch ordering rides bead
dpaa2-controlplane-5y7 on the raw command path (`mc-portal-backend`, #10).

#### Scenario: Bind a scratch child to VFIO
- **WHEN** KernelControl sets `driver_override` to vfio-fsl-mc on a plugged scratch child and binds it
- **THEN** the container is observed bound to vfio-fsl-mc and its IOMMU group exists

#### Scenario: Unbind restores the unbound state
- **WHEN** KernelControl unbinds the scratch child and clears the override
- **THEN** the container is observed unbound and eligible for fsl_mc_dprc again
