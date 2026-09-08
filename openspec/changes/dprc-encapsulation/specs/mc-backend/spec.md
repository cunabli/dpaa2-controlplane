# mc-backend delta: dprc-encapsulation

## ADDED Requirements

### Requirement: The restool shim exposes the dprc verb surface
`McControl` SHALL expose the dprc verbs the reconciler plans — create, destroy,
assign (child placement and plugged state), unassign, set-label, set-locked —
at MC-command granularity, surfacing MC status codes distinctly (0x4/0x6/0x8
refusal shapes preserved) and surfacing restool's own client-side refusals
(e.g. the plugged-move guard) as typed errors, never as scraped text.

#### Scenario: Create returns the created identity
- **WHEN** the shim creates a child DPRC under a parent
- **THEN** the result carries the child's id for re-observation, and the new child reads back unplugged (board-verified default)

#### Scenario: Client-side refusal is typed
- **WHEN** restool refuses a plugged-object move before issuing any MC command
- **THEN** the shim returns a typed refusal distinguishable from an MC status refusal

### Requirement: KernelControl actuates VFIO binding for child DPRCs
`KernelControl` SHALL observe and actuate the vfio-fsl-mc binding path for a
child DPRC — `driver_override` write, bind, unbind — and SHALL expose the
observed propagation of the override to subsequently-added children of a bound
container. Restool-unreachable portal faces (child-portal unlock, the
OBJ_CREATE_ALLOWED gate, DPRC-I8 batch ordering) are explicitly out of scope,
deferred to tile #10.

#### Scenario: Bind a scratch child to VFIO
- **WHEN** KernelControl sets `driver_override` to vfio-fsl-mc on a plugged scratch child and binds it
- **THEN** the container is observed bound to vfio-fsl-mc and its IOMMU group exists

#### Scenario: Unbind restores the unbound state
- **WHEN** KernelControl unbinds the scratch child and clears the override
- **THEN** the container is observed unbound and eligible for fsl_mc_dprc again
