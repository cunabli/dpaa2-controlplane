# intent-compiler delta — pool-objects

## MODIFIED Requirements

### Requirement: The compiler refuses by name
The compiler SHALL refuse, with a variant naming the rule and the
offending construct, on: a construct naming an undeclared tenant
(`TenantAbsent`, DPDCEI-I1 generalised), whose payload SHALL identify the
referencing site as a typed enum (port, link end, fabric forwarder,
crypto, extra, pool drawer) rather than a construct-name string, so no
reserved token can collide with a declared construct name; a tenant
declared under the reserved name `kernel` in any shape other than the
exact reserved kernel value, which the shell legitimately injects
(`KernelDeclared`); a link
whose two ends resolve to the same tenant (`LinkSelfLoop`); a rename
`from` claiming a construct that is currently declared and not itself
renamed (`RenameDoubleClaim`); an unanchored dpmac (not in
the inventory); a reserved or foreign dpmac; a dpmac claimed by two
constructs; a port rate above its dpmac's `max_rate`; a hardware fabric
forwarded by a tenant other than the kernel; a member port whose tenant
is not its fabric's forwarder; a hardware fabric inside a hardware
fabric; a derived T above the tenant's `max_cores`;
an extra on a family that is not one of the four companions, or an extra
whose count is below 1; a crypto block whose flows are below 1, or above
one dpseci's 16 queue pairs (`DPSECI_MAX_QUEUE_NUM`) — one block is one
device, so the demand is refused, not clamped, and split across blocks; a
userspace-poll tenant terminating a rate class with no
seeded worker row; a tenant whose dataplane has no companion pricing
(`userspace-event` today); a pool holder that is absent, not
`public`, or itself pooled (no chains), or a drawer whose dataplane
differs from its holder's (the reserved kernel counting as
kernel-netlink); a member naming an undeclared port or fabric, or a
fabric listing itself as a member; and cross-tenant infeasibility, where the
sum of derived draws exceeds a `Counted` or `Observed` ceiling — naming
the family, the amount needed, and the amount available. An `Unknown`
ceiling SHALL produce a warning in provenance, never a refusal. The
compile-side `KernelDeclared`, `LinkSelfLoop`, and `RenameDoubleClaim`
rules SHALL mirror their parse-side twins verbatim, each site carrying a
doc note naming its twin (deliberate duplication across the config→api
seam, design D11). The
`Refusal` and `Dataplane` types SHALL be `#[non_exhaustive]`, justified
by the change-#4 passthrough. No corpus-wide `PoolShortfall` variant is
reserved: the reconciler's live-census refusal role went to the
family-namespaced `ShrinkBelowDraw` refusal plus the ADR-0020 typed
reboot-required residue — a deliberate no-corpus-wide-refusal choice
living in `crates/dpaa2-api/src/families/pool_lifecycle.rs` (review
PASS1-F14/PASS4-F3).

#### Scenario: Core budget exceeded
- **WHEN** a userspace-poll tenant's table row gives T = 5 and
  `max_cores` = 4
- **THEN** compilation is refused with `CoreBudgetExceeded` naming the
  tenant, T, and the budget

#### Scenario: Two tenants overdraw the buffer-pool ceiling
- **WHEN** three userspace-poll tenants need 6 dpbps and the inventory
  lists 5
- **THEN** compilation is refused with `Infeasible` naming dpbp, 6,
  and 5

#### Scenario: A dpmac claimed twice
- **WHEN** a port and a fabric both name `dpmac.7`
- **THEN** compilation is refused with `DoubleClaimed` naming the dpmac
  and both constructs

#### Scenario: A hardware fabric forwarded by a userspace-poll tenant
- **WHEN** a hardware-switched fabric names a userspace-poll tenant as
  its forwarder
- **THEN** compilation is refused with `FabricNotKernelForwarded`

#### Scenario: A reserved anchor
- **WHEN** a port names `dpmac.17`
- **THEN** compilation is refused with `Reserved` carrying the
  inventory's reason

#### Scenario: An unseeded rate class is refused, not extrapolated
- **WHEN** a userspace-poll tenant terminates a port whose rate class
  has no workers-per-port row (a 40G port against the table's 10G and
  25G rows)
- **THEN** compilation is refused with `UnknownRateClass` naming the
  tenant and its port rates

#### Scenario: All violations at once
- **WHEN** an intent claims `dpmac.17` and also exceeds `max_cores`
- **THEN** the refusal list holds both `Reserved` and
  `CoreBudgetExceeded`

#### Scenario: A programmatic self-loop link is refused
- **WHEN** an `Intent` built in Rust (never parsed) contains a link
  whose two ends name the same tenant
- **THEN** compilation is refused with `LinkSelfLoop` naming the link,
  and the link never reaches derivation

#### Scenario: A programmatic kernel declaration is refused
- **WHEN** an `Intent` built in Rust declares a tenant named `kernel`
  whose shape differs from the exact reserved kernel value
- **THEN** compilation is refused with `KernelDeclared`; an intent
  carrying the exact reserved value is accepted, because the shell
  injects that value when completing the kernel and compile cannot
  distinguish the injection from a declaration

#### Scenario: A programmatic rename double-claim is refused
- **WHEN** an `Intent` built in Rust carries a construct whose rename
  `from` names another construct that is currently declared and not
  itself renamed
- **THEN** compilation is refused with `RenameDoubleClaim` naming the
  claiming construct and the contested name

#### Scenario: A port named pool cannot collide in refusal rendering
- **WHEN** an intent declares a port named `pool` and a restricted
  tenant whose pool holder is undeclared
- **THEN** the `TenantAbsent` refusal for the missing holder identifies
  the drawing tenant through the typed referrer, distinguishable from
  any refusal referring to the port `pool`

### Requirement: A declared consumer derives its container
The compiler SHALL derive, for each declared consumer runtime, its child-DPRC
realization: restool-default options mask ({SPAWN, ALLOC, OBJ_CREATE,
IRQ_CFG}_ALLOWED — the board-verified VPP-container mask), placement under the
root container, and a label carrying the consumer's name-keyed identity
(ADR-0015). The kernel tenant remains the root container and never derives a
child (ADR-0005).

#### Scenario: Consumer container derivation
- **WHEN** intent declares a consumer runtime
- **THEN** the derived model contains one child DPRC with exactly the default options mask, root placement, and the consumer's name as label, with per-object rule provenance citing the baseline anchor

#### Scenario: Kernel tenant derives no container
- **WHEN** intent declares the kernel tenant
- **THEN** the derived model contains no child DPRC for it

#### Scenario: Derivation folds the companion draws the pool construct consumes
- **WHEN** a consumer is derived under this change
- **THEN** derivation emits the folded per-port companion draws the pool construct consumes (design D9) — tiles #5 and #6 are both delivered — while the consumer derives no inline companion objects (DPIO/DPBP/DPCON/DPMCP) or DPNIs of its own, because the pool passes own them (single provider)
