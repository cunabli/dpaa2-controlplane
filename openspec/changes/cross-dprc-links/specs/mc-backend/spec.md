# mc-backend delta: cross-dprc-links

## ADDED Requirements

### Requirement: McControl actuates dpni↔dpni connect, disconnect, and observe, ancestor-explicit
`McControl` SHALL actuate the dpni↔dpni edge — connect, disconnect, and
observe — with the issuing container named explicitly: the connect and
disconnect SHALL be issued at a common ancestor of both endpoints (the
root constant today, `CONNECT_ANCESTOR`), rendering the cross-container
form `connect_in(ancestor, dpni, peer)` (design D2/D6). A child-issued
connect the MC refuses `No privilege (0x4)` without
`TOPOLOGY_CHANGES_ALLOWED` (V-DPCI-1) SHALL surface as a typed refusal,
not an untyped backend error.

#### Scenario: A cross-container connect names the ancestor
- **WHEN** the executor connects two dpnis resident in different child
  containers
- **THEN** it issues one `McControl` connect scoped to their common
  ancestor, not to either child

#### Scenario: Child-issued connect without the gate is a typed refusal
- **WHEN** a connect is attempted from a child container lacking
  `TOPOLOGY_CHANGES_ALLOWED`
- **THEN** the shim returns the typed `No privilege (0x4)` refusal,
  distinguishable from a backend error and from an MC pair-legality
  refusal

### Requirement: The connection verbs are named one-to-one with whitelisted MC commands and return typed values
The `McControl` connection verbs SHALL be named one-to-one with the
whitelisted MC commands they carry — `DPRC_GET_CONNECTION`,
`DPRC_CONNECT`/`DPRC_DISCONNECT`, and `DPNI_GET_LINK_STATE` — and SHALL
return typed values (`ObjectRef`, a `LinkState` type), never scraped
restool text through the trait (design D6). restool-text parsing SHALL
stay an implementation detail behind the trait so the `mc-portal-backend`
(#10) portal implementation drops in under the differential gate. This
change SHALL add no portal read-slice: the `/dev/dprc.N` read vocabulary
is unchanged and #10 owns every addition (ADR-0021).

#### Scenario: Observe returns a typed link state
- **WHEN** the shim observes a dpni↔dpni connection
- **THEN** it returns the typed `LinkState`/`ObjectRef` result, and no
  restool text crosses the trait boundary

#### Scenario: Each verb maps to one whitelisted MC command
- **WHEN** the connection verbs are inspected against the MC command set
- **THEN** each names exactly one of `DPRC_GET_CONNECTION`,
  `DPRC_CONNECT`/`DPRC_DISCONNECT`, `DPNI_GET_LINK_STATE`, so a future
  ioctl backend maps one-to-one behind the same trait

### Requirement: Child populate resolves link peers and issues the connect
The child-population adapter's peer resolution SHALL be generalized
beyond port-edges: a child-resident dpni carrying a dpni↔dpni link edge
SHALL resolve its peer (`planned_peer` no longer resolves port-edges
only) and the connect SHALL be issued at the common ancestor (design
D2). A link edge that previously resolved, dry-ran, and stayed
`plan_only` SHALL now drive an actual `dprc connect`.

#### Scenario: A child-resident link peer resolves and connects
- **WHEN** the reconciler populates a child container holding a dpni with
  a declared link edge
- **THEN** the adapter resolves the edge's peer and issues the connect at
  the ancestor, rather than leaving the edge plan-only

### Requirement: A post-bind drift refusal is plumbed as a typed DeferredVisibility obligation
When a create into an already-bound container is MC-accepted but
kernel-invisible, the adapter SHALL plumb the `DriftRefused` outcome into
the typed `DeferredVisibility` obligation the core carries (design D5),
judged by re-observation after a scan. The outcome SHALL NOT be swallowed
nor reported as an untyped backend error.

#### Scenario: An accepted-but-invisible create yields the typed obligation
- **WHEN** a post-bind create is accepted by the MC yet reads back
  kernel-invisible
- **THEN** the adapter surfaces the typed `DeferredVisibility` obligation,
  and convergence is judged by re-observation, never by the create's
  acceptance
