## ADDED Requirements

### Requirement: Intent is a frontend-neutral vocabulary of network constructs
The `dpaa2-api` crate SHALL define an `Intent` type composed of five
constructs — tenant (name, dataplane `kernel-netlink`, `userspace-poll`, or
`userspace-event`, a `max_cores` budget),
port (dpmac anchor, rate, owning tenant), link (two tenant ends),
fabric (members — ports, tenants, or fabrics — and a `switching`
qualifier, hardware or software), and crypto (per tenant, with a `flows`
count) — carrying no serialization derives and no field for a dpio, dpbp,
dpcon, dpmcp, queue or worker count. The tenant name `kernel` SHALL be
reserved for the root-container `kernel-netlink` dataplane. (ADR-0005 §1, ADR-0012)

#### Scenario: A port without a tenant belongs to the kernel
- **WHEN** an intent contains a port that names no tenant
- **THEN** the port is owned by the reserved `kernel` tenant in the
  root container

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

### Requirement: The inventory is the observed hardware offer
The compiler SHALL take an `Inventory` value describing what the
hardware offers: each dpmac with its `max_rate`, `eth_if`, `link_type`,
and an availability of `Free`, `Reserved` with a reason (the ADR-0003
safety matrix), or `Foreign` with its owner (DPL-owned objects,
ADR-0001 §4); and each pool ceiling as a three-valued `Ceiling` —
`Counted`, `Observed` with provenance, or `Unknown` (ADR-0011). The
inventory SHALL be produced by reading the board or by loading the
reference snapshot; it SHALL NOT be operator-written.

#### Scenario: Inventory from the reference snapshot
- **WHEN** a test loads `models/board/baselines/reference.json`
- **THEN** it yields an `Inventory` whose dpmacs and pool counts match
  the snapshot, with no board attached

#### Scenario: Inventory from the board
- **WHEN** `ensure` runs
- **THEN** the inventory is read from `dpmac info` and the container's
  resource listing before compilation

#### Scenario: Reserved and foreign anchors are inventory facts
- **WHEN** the inventory is built for the reference board
- **THEN** dpmac.3 reads `Reserved`, dpmac.17 reads `Reserved`, the
  DPL-provisioned dpni.0 reads `Foreign`, the dpbp ceiling reads
  `Counted`, the dpni ceiling reads `Observed` citing ADR-0011, and
  dpcon's reads `Unknown`

### Requirement: Compilation is pure, total, and deterministic
`compile(intent, inventory)` SHALL be a pure function returning
`Result<Plan, Refusal>`: same inputs, same output; no I/O; every intent
either compiles to a complete plan or is refused with a named rule.
(ADR-0005 §2)

#### Scenario: Same intent, same plan
- **WHEN** the same intent and inventory are compiled twice
- **THEN** the two plans are equal object-for-object, including
  emission order and provenance

#### Scenario: Every outcome is a plan or the complete refusal list
- **WHEN** any intent is compiled
- **THEN** the result is a plan or a non-empty list of `Refusal`
  values, one per violated rule, each naming the rule and the offending
  construct; no panic, no partial plan, and no violation hidden behind
  an earlier one

### Requirement: Companion counts derive from dataplane and budget, never from the operator
The compiler SHALL derive the thread count T from the declared per-class
workers-per-port table (T = 1 main + Σ workers over the terminated
ports) bounded by `max_cores`, and the companion set from the
dataplane per ADR-0012: userspace-poll per child dprc dpio = 2·T, dpbp =
2, dpmcp = one per process, dpni transmit queues ≥ T; kernel dpio one
per online CPU, dpbp and dpmcp one per consuming object plus one dpmcp
per dpio; dpcon one per polled queue; dpseci `num_queues ≥` the crypto
construct's `flows`, with `HAS_CG`; dpsw `num_ifs` = port count, `max_fdbs ≥
num_ifs`, PER_FDB flooding and broadcast, control interface enabled.

#### Scenario: Userspace-poll router derives its companions
- **WHEN** a userspace-poll tenant owns two 10G ports and the table
  gives T = 5
- **THEN** the plan holds one child DPRC, two dpnis with at least 5
  transmit queues, 10 dpios, 2 dpbps, 1 dpmcp, and one dpcon per polled
  queue, each keyed (tenant, family, ordinal) with its label rendered
  from the key, and each with a provenance tree naming the rule and the
  tenant

#### Scenario: Thread count is marked unmeasured
- **WHEN** provenance for a derived T is printed
- **THEN** it names the rate-table row and the mark `unmeasured`

### Requirement: The compiler refuses by name
The compiler SHALL refuse, with a variant naming the rule and the
offending construct, on: tenant absence (a dpci or dpdcei intent with
no userspace tenant named, DPDCEI-I1); an unanchored dpmac (not in
the inventory); a reserved or foreign dpmac; a dpmac claimed by two
constructs; a port rate above its dpmac's `max_rate`; a hardware fabric
forwarded by a tenant other than the kernel; a member port whose tenant
is not its fabric's forwarder; a hardware fabric inside a hardware
fabric; a derived T above the tenant's `max_cores`;
a limit below its request; a limit on a family whose count the
constructs fix; a userspace-poll tenant terminating a rate class with no
seeded worker row; a tenant whose dataplane has no companion pricing
(`userspace-event` today); a construct naming an undeclared tenant,
port or fabric; and cross-tenant infeasibility, where the
sum of derived draws exceeds a `Counted` or `Observed` ceiling — naming
the family, the amount needed, and the amount available. An `Unknown`
ceiling SHALL produce a warning in provenance, never a refusal. The
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

### Requirement: Derived counts are requests; overrides are limits
Every derived count SHALL be a *request*; an override, declared per
family under a tenant, SHALL be a *limit*; `limit ≥ request` SHALL
hold, and provenance for the affected objects SHALL print both values.

#### Scenario: A limit above the request
- **WHEN** a userspace-poll tenant with T = 5 declares the limit `dpio =
  12`
- **THEN** the plan holds 12 dpios whose provenance reads request 10,
  limit 12

#### Scenario: A limit below the request
- **WHEN** the same tenant declares the limit `dpbp = 1`
- **THEN** compilation is refused with `LimitBelowRequest` naming dpbp,
  request 2, limit 1

### Requirement: Every derived value carries a provenance tree
Each object and each sized attribute in the plan SHALL carry a
provenance tree: the rule that produced it and the values it consumed,
each with its own provenance, down to the declared construct and the
evidence anchor (baseline section or ADR) the rule cites.

#### Scenario: Provenance names the anchor
- **WHEN** the plan's dpbp entries for a userspace-poll tenant are
  inspected
- **THEN** each reads rule `dpbp-pair`, construct the tenant's name,
  anchor `ADR-0012`

#### Scenario: Provenance reaches the declared construct
- **WHEN** a userspace-poll tenant's dpio count is explained
- **THEN** the tree reads `dpio ×10 ← 2·T ← T = 5 ← 1 + Σ workers ←
  workers-table(10G ⇒ 2, unmeasured) ← port wan0 rate = 10G, port wan1
  rate = 10G`, and each node names its anchor

### Requirement: Derived objects are keyed, labels are rendered
Each derived object SHALL be identified by the key (tenant, family,
ordinal); its DPRC label SHALL be rendered from that key; and
desired↔observed matching SHALL be by key, never by object name
(ADR-0010).

#### Scenario: Labels are a projection of the key
- **WHEN** the plan is emitted for a tenant named `router`
- **THEN** its third dpio carries key (`router`, dpio, 3) and label
  `router/dpio/3`, and the same intent compiled again yields the same
  key and label

### Requirement: Plan relationships are unrepresentable if wrong
The plan type SHALL admit edges, container memberships, and companions
only through constructors that take the deriving construct as a
witness, so that a free-standing companion, a dpmac at a link end, a
dpni connected twice, or a tenant object in the root container cannot
be constructed. Emission order SHALL follow `object-model.md` §5 (pool
companions before tenant objects; a kernel-dataplane dpio before its
dpmcp) as a property of construction.

#### Scenario: No free-standing companion
- **WHEN** code attempts to add a dpio to a plan without a tenant
- **THEN** it does not compile

#### Scenario: Order is a property of construction
- **WHEN** any plan is emitted
- **THEN** every companion precedes the tenant object that draws it,
  without a post-hoc sort

### Requirement: The intent layer is additive over api, mc and hal
`Intent`, `Inventory` and `compile` SHALL be an additive layer:
`reconcile`, `McControl`, `KernelControl` and the HAL SHALL NOT depend
on them, and the plan's constructors SHALL be public so a
`DesiredTopology` can be built without `Intent` while keeping the
relationship locks. Raw object-level escape hatches SHALL be addable at
the plan layer without changing the compiler; none is defined here.

#### Scenario: Library use without intent
- **WHEN** a program builds a plan through the constructors and calls
  `reconcile`, then drives `McControl` with the transitions
- **THEN** it compiles and runs with no `Intent`, no TOML, and no
  `dpaa2-config` dependency

#### Scenario: The compiler is not a gate on the MC
- **WHEN** `dpaa2-mc` is used directly
- **THEN** no type from the intent layer is required

### Requirement: The intent model is executable and gated by the user manual
`models/intent/` SHALL carry the vocabulary as Quint types, the
derivation as pure definitions, every rule as a named invariant citing
its evidence anchor, the three scenarios (hardware-switched fabric under
a core budget; virtual fabric between two child dprcs; userspace router
with crypto flows) each with a refused negative twin, a random-simulation
run over a finite intent alphabet, and the reference board's provisioning
as a fit-check scenario diffed against the snapshot. The gate artefact
SHALL be `docs/intent.md` — constructs, inputs, derived quantities,
refusals, the scenarios as worked examples — and the README example,
written before any Rust; a decision bead *intent vocabulary accepted*
SHALL gate every Rust task and SHALL be closed by the operator after
reviewing that document, which then ships as the reference; the
decision is recorded as an ADR-0005 amendment.

#### Scenario: A negative twin is refused in the model
- **WHEN** scenario (1) is run with `max_cores` below the derived T
- **THEN** the model's compile yields the `CoreBudgetExceeded` refusal
  and the invariant run stays green

#### Scenario: The fit check finds the idle portals
- **WHEN** the reference board's provisioning is compiled and diffed
  against the snapshot
- **THEN** the diff reports the userspace-poll child dprc's two extra
  dpmcps and nothing else unexplained

#### Scenario: Rust waits for the gate
- **WHEN** the gate bead is open
- **THEN** no task touching `dpaa2-api` or `dpaa2-config` is ready

#### Scenario: The manual is the reviewed artefact
- **WHEN** the gate bead is closed
- **THEN** `docs/intent.md` exists, every scenario in it matches a
  `scenarios/<name>.toml`, and the ADR-0005 amendment cites it
