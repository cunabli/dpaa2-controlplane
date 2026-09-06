# intent-compiler delta: vocabulary-v2

## MODIFIED Requirements

### Requirement: Intent is a frontend-neutral vocabulary of network constructs
The `dpaa2-api` crate SHALL define an `Intent` type composed of five
constructs — tenant (name, dataplane `kernel-netlink`, `userspace-poll`, or
`userspace-event`, a `max_cores` budget, and an `isolation` of `public`,
`restricted` — which carries the `pool` holder name as its payload —
or `isolated`, defaulting to `isolated`),
port (dpmac anchor, rate, and an owning tenant stated as a two-case sum:
the reserved kernel — the default when the operator names none — or a
declared tenant name), link (two tenant ends),
fabric (members — ports, tenants, or fabrics — and a `switching`
qualifier, hardware or software), and crypto (per tenant, with a `flows`
count) — carrying no serialization derives and no field for a dpio, dpbp,
dpcon, dpmcp, queue or worker count. A `restricted` tenant without a pool,
and a pool on a non-restricted tenant, SHALL be unrepresentable in the
type; no empty-string sentinel or optional SHALL stand for an absent
pool or for the default port tenant — the kernel default is its own
case of the tenant sum. The tenant name `kernel` SHALL be
reserved for the root-container `kernel-netlink` dataplane, is implicitly
`public`, and MAY be named at a link interface end without being declared.
(ADR-0005 §1, ADR-0012)

#### Scenario: A port without a tenant belongs to the kernel
- **WHEN** an intent contains a port that names no tenant
- **THEN** the port's tenant is the sum's kernel case, the port is owned
  by the reserved `kernel` tenant in the root container, and no refusal
  can name `kernel` as an absent tenant — the kernel case carries no name
  to refuse

#### Scenario: A chain of switches is stated as composition
- **WHEN** a software fabric forwarded by a userspace-poll tenant lists
  a hardware fabric as a member
- **THEN** the plan holds the kernel's dpsw with that tenant's dpni on
  one of its interfaces, no second dpsw, and no pseudo-wire

#### Scenario: No count field exists
- **WHEN** the `Intent` type is inspected
- **THEN** no construct exposes a dpio, dpbp, dpcon, dpmcp, queue or
  worker count; the only numbers are `max_cores`, a crypto `flows`, and
  port `rate`

#### Scenario: A declared kernel-netlink namespace is child-resident
- **WHEN** an intent declares an `isolated` `kernel-netlink` tenant other
  than the reserved kernel
- **THEN** the plan holds its own kernel-bound child DPRC, its dpnis at
  cpus transmit queues, and the child-resident kernel draw — dpio 0
  (the per-CPU dpio service is kernel-global), dpbp and dpmcp one per
  dpni, dpcon one per online CPU per dpni

#### Scenario: A restricted tenant co-resides in its holder's container
- **WHEN** an intent declares a `restricted` tenant whose `pool` names a
  `public` holder of the same dataplane
- **THEN** the tenant's objects are created in the holder's child DPRC,
  the tenant derives no DPRC of its own, and it keeps its own dataplane
  companion draw

#### Scenario: An illegal pool shape is unrepresentable
- **WHEN** Rust code attempts to state a `restricted` tenant with no
  pool, or a pool on a `public` or `isolated` tenant
- **THEN** the program does not compile — the shape has no constructor —
  and no `Refusal` variant exists for either shape

#### Scenario: A pool holder is refused when illegal
- **WHEN** a `restricted` tenant's `pool` names a holder that is absent,
  not `public`, itself pooled, or of a different dataplane than the
  drawer
- **THEN** the compile refuses by name and never derives the drawer into
  an illegal container

#### Scenario: The kernel is nameable at a link end
- **WHEN** a link names `kernel` at one end and the intent never declares
  the kernel tenant
- **THEN** the compile does not refuse the end as absent, and the
  kernel's link-end dpni is materialised in the root container at cpus
  transmit queues

### Requirement: The compiler refuses by name
The compiler SHALL refuse, with a variant naming the rule and the
offending construct, on: a construct naming an undeclared tenant
(`TenantAbsent`, DPDCEI-I1 generalised), whose payload SHALL identify the
referencing site as a typed enum (port, link end, fabric forwarder,
crypto, extra, pool drawer) rather than a construct-name string, so no
reserved token can collide with a declared construct name; a tenant
declared under the reserved name `kernel` (`KernelDeclared`); a link
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
`Refusal` and `Dataplane` types SHALL be `#[non_exhaustive]`, and a
`PoolShortfall` variant SHALL be reserved for the reconciler's
live-census refusal.

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
- **THEN** compilation is refused with `KernelDeclared`

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
