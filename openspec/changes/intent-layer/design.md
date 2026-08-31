## Context

Change #2 left the sizing rules verified and executable in one place
(`models/core/companions.qnt`, ADR-0012) and the pool ceilings measured
(ADR-0011), but nothing consumes them: `topology.toml` is a list of
ports, `DesiredTopology` is that list typed, and `reconcile` knows one
edge (dpni↔dpmac). Every other object the board runs today was created
by a shell script that counts by port — correct for the kernel regime,
silently wrong for the poll-mode child (ADR-0012's opening finding).

ADR-0005 fixed the shape of the answer on 2026-08-22: operators declare
network constructs anchored in hardware; a pure compiler derives the
object plan; each rule is a model invariant; dry-run shows provenance;
overrides are per object and visible. The 2026-08-30 proposal session
sharpened what the operator actually states — *capacity at L1 and who
consumes it*, never a count — and added the inventory as the compiler's
second input and a hard model-review gate before any Rust. The decisions
below record that session so tasks transcribe rather than re-litigate.

## Goals / Non-Goals

**Goals:**

- A frontend-neutral intent vocabulary an operator can write without
  knowing a single sizing rule, expressive enough for the three named
  scenarios and extensible without reworking what is already written.
- A pure, total derivation from intent plus inventory to the complete
  object plan, with every derived object and number carrying its rule,
  its source construct, and its evidence anchor.
- "Possible or not" as a first-class answer: a refusal names the rule
  and the shortfall; the tool proves a request impossible instead of
  emitting a plan that fails on the data path.
- Plan relationships (containment, connect edges, companion coupling,
  ordering) unrepresentable-if-wrong, with the type surface discovered
  from the Quint model rather than designed ahead of it.
- The vocabulary triple-checked in the operator's hands — model,
  traces, and paired TOML — before a line of compiler Rust exists.

**Non-Goals:**

- No executors for the derived families beyond dpni↔dpmac; the plan is
  data, and changes #4–#8 add the executors and per-object lifecycle
  typestates.
- No shared-uplink (dpdmux) construct — its regime/method law is
  unsettled and ADR-0009 refuses the uplink peer; it joins at #12 with
  baseline backing.
- No traffic-characterization → intent automation and no named
  profiles (ADR-0005 §5 keeps them as later sugar with a revisit
  trigger); no YANG/gNMI frontend.
- No live-census refusal: the compiler checks against the inventory's
  declared ceilings; drift against a live census is `reconcile`'s and
  lands with #6 (`pool-objects`).
- No mutation on the board. The single sitting is a read-only fit
  check.

## Decisions

### D1. The operator states capacity and tenants, never counts

The schema carries five constructs — tenant (name, dataplane
`kernel|userspace-poll|userspace-interrupt`, `max_cores`,
`crypto_flows`), port (dpmac, rate, tenant), link (two tenant ends),
fabric (members — ports, tenants, or other fabrics — with a `switching`
qualifier: `hardware`, a dpsw only the kernel can drive, or `software`,
the forwarding tenant bridging its own dpnis, which the MC never sees; a
chain of switches is a software fabric that lists a hardware fabric as a
member — DPAA2 User Manual §2.2.2 figure 6a–c; `forwarded_by` names the
tenant that runs the fabric's forwarding plane), crypto (per tenant) —
and no field for a dpio, dpbp, dpcon, dpmcp, queue or worker count.
`max_cores` is a budget the derivation must fit under; `crypto_flows` is
a tenant-visible quantity, not an object count; `rate` is what the port
must deliver. The file carries a mandatory `schema = 1` key — the
`apiVersion` idiom — so the next breaking change has a hook this one did
not. The nouns are the 2026-08-31 taxonomy review's: *tenant* is RFC
9316's resource-consumer term (a tenant may produce traffic, which
"consumer" misread); *dataplane* names where the tenant's dataplane runs
and the delivery mechanism that drives sizing — `userspace-interrupt` is
declared but refused (`UnpricedDataplane`) until a scenario prices its
companion draws; *forwarded_by* replaces the second "owner" — switching
and routing are not the same, forwarding covers both.
Alternatives rejected in session: `workers`/`threads`/`processes` as
inputs (implementation numbers smuggled into intent — the operator
knows them only because a script once told them), and a raw-object
escape hatch as a first-class construct (ADR-0005 §5: raw objects are
the last resort, expressed here only as raise-only overrides).

### D2. Two inputs: intent and inventory; inventory is observed, never written

`compile(intent, inventory)` takes the hardware's offer as data: dpmacs
with `max_rate`, `eth_if`, `link_type` (immutable per DPMAC-I3, read by
`dpmac info`) and an availability — `Free`, `Reserved { why }` (the
ADR-0003 safety matrix: dpmac.3 total-deny, dpmac.17 management),
`Foreign { owner }` (DPL-owned objects, ADR-0001 §4) — so the compiler
refuses a reserved or foreign anchor by name and the fit check
classifies DPL objects as foreign rather than drift; and the pool
ceilings as a three-valued `Ceiling` — `Counted(n)` (dpbp, the listing
is the ceiling), `Observed { n, provenance }` (dpni 18, unlisted but
measured), `Unknown` (every family the cap ended) — ADR-0011's own
distinction as a type. Feasibility refuses against `Counted` and
`Observed`, and warns on `Unknown`; it never invents a number. `ensure`
reads it from the board; tests and the model read it from change #2's
reference snapshot (`models/board/baselines/reference.json`), which
already carries the pool counts. Feasibility across tenants — the
sum of every derived draw against every ceiling — is therefore a
compile-time property and a named invariant, not a census read; the
refusal names the family and the shortfall. Alternative rejected: an
operator-written inventory file (a second source of truth the board
would contradict).

### D3. T is derived from a declared, visibly unmeasured worker table

The one derivation the corpus cannot anchor is capacity → thread count.
T — the thread count of a userspace tenant's dataplane — and the
per-port worker count are named quantities of the taxonomy itself
(`docs/intent.md` defines them; they are not ADR archaeology), and the
formula is explicit: T = 1 main + Σ workers(rate) over the ports the
tenant terminates. The compiler carries a `rate-class →
workers-per-port` table seeded from the reference board: 10G ⇒ 2
workers, decomposed from the verified configuration (two 10G ports in
poll-mode ran T = 5 = 1 + 2·2, the configuration the companion numbers
were verified against); 25G ⇒ 5 workers, declared linear-in-rate from
the 10G row, unmeasured, signed off at the gate. A rate class with no
row is refused (`UnknownRateClass`); a mix of seeded classes compiles
and provenance carries an *unmeasured combination* warning — the
review's escape-hatch-warns rule: cross-class pricing is flagged, never
silent. Provenance prints the row and the mark *unmeasured* on every T;
`max_cores` bounds the result, and exceeding it is a refusal. ADR-0005
is amended to record the gap and the trigger that replaces the table
with a measured model: an NXP-citeable per-worker figure, or a board
suite of our own that measures it (which would be a traffic-bearing
sitting under ADR-0003, not part of this change). Alternatives rejected:
importing the poll-mode consumer's own throughput figures (the repo does
not cite that effort), and exact-multiset rows (every future mix would
need its own hand-declared row while the formula stayed implicit —
review decision, 2026-08-31).

### D4. Companion draws and family predicates come from the baseline, by reference

Userspace-poll per child dprc: dpio = 2·T, dpbp = 2, dpmcp = one per
process (the primary; a secondary adds one), dpni transmit queues ≥ T;
kernel: dpio one per online CPU, dpbp and dpmcp one per consuming
object plus one per dpio (ADR-0012, `companions.qnt`); dpcon one per
polled queue (`dpcon.md`); dpseci `num_queues ≥ crypto_flows` with the
`HAS_CG` safety bit (`dpseci.md`); dpsw for a hardware fabric
only, `num_ifs` = its interface count (member ports as dpmac endpoints,
member tenants and attached software fabrics as dpni endpoints —
figure 6c), `max_fdbs ≥ num_ifs`, PER_FDB flooding and broadcast,
control interface enabled — the kernel-bindable predicate (`dpsw.md`) —
and kernel-forwarded only; a software fabric emits no dpsw: its
forwarding tenant terminates member ports (figure 6a) and reaches member
tenants by dpni↔dpni pseudo-wire (figure 6b), the boundary connector
that needs no netlink exposure; a hardware fabric inside a hardware
fabric is refused until a baseline verifies dpsw↔dpsw; one child DPRC
per non-kernel tenant with restool-default
options (`dprc.md`); dprtc.0 pinned in root; dpdbg not derived. Each
rule is one named invariant whose comment cites the section it comes
from; the numbers live in the ADRs and the model, never twice.

### D5. The compiler is total and refuses by name

`compile` returns `Result<Plan, Refusal>`; `Refusal` is an enum whose
variants are the rules: `TenantAbsent` (DPDCEI-I1 generalised; a
construct names an undeclared tenant), `Unanchored` and
`DoubleClaimed` (dpmac not in the inventory / claimed by two constructs),
`OverRate` (rate above `max_rate`), `FabricNotKernelForwarded` (a
hardware fabric not forwarded by the kernel), `PortTenantMismatch` (a
member port whose tenant is not its fabric's forwarder),
`UnsupportedEdge` (a hardware fabric inside a hardware fabric),
`CoreBudgetExceeded` (T > `max_cores`), `UnknownRateClass` (a userspace
tenant terminating a rate class with no seeded worker row — D3 refuses
unseeded classes; a new row enters through a scenario that needs it),
`UnpricedDataplane` (a tenant whose dataplane has no companion pricing —
`userspace-interrupt` today), `LimitBelowRequest`, `LimitNotDerived` (a
limit on a family whose count the constructs fix — never silently
ignored, D7), `MemberUnresolved`, `SelfMember`, and `Reserved`,
`Foreign`, and `Infeasible { family, needed, available }`.
`compile` returns *every* violation, not the first — the compiler idiom:
the operator fixes a file in one pass — so the error side is a non-empty
`Refusals` list. `Refusal` and `Dataplane` are `#[non_exhaustive]`: a
`PoolShortfall` variant is reserved for `reconcile` (#6), and a
passthrough value (a VFIO child whose guest dataplane the host cannot
see) is #4's. Overrides follow the Kubernetes/cgroups request/limit idiom:
every derived count is a *request*, a per-family override under the
consumer is a *limit*, `limit ≥ request` is the rule, and provenance
prints both. A policy-expression language for limits (CEL, as
Kubernetes uses for validation rules) is recorded as a revisit trigger
in the ADR-0005 amendment and not built.

### D6. Plan relationships are locked by construction; types follow the model

The plan is built through constructors that take witnesses: a `Port`
yields the dpni and its dpni↔dpmac edge; a `Tenant` yields its child
dprc and its companion set; a `Link` takes two tenants and
yields the pair; a `Fabric` takes its forwarding tenant and its ports
and yields the dpsw and its endpoints. Nothing else can produce an edge,
a membership, or a companion, so a free-standing dpio, a dpmac at a link
end, a double-connected dpni, or a tenant in the root dprc is
not a value the plan type admits. Emission order is a property of the
constructors (`object-model.md` §5: pool companions before consumer
objects, dpio before its dpmcp in the kernel regime), not a sort. Each
derived object is keyed by `(tenant, family, ordinal)` and its label
is rendered from the key — ADR-0010: names are not identities — so
desired↔observed matching is by key and the label namespace is a
projection. Provenance is a tree, not a triple: every derived value
points at its rule and at the values it consumed, recursively, down to
the declared construct and the evidence anchor (`dpio ×10 ← 2·T ← T=5 ← 1+Σ workers ←
workers-table(10G ⇒ 2, unmeasured) ← ports wan0/wan1 rate=10G`); the
build-system
idiom (`nix why-depends`, Bazel `--explain`) that makes a later
`dry-run --explain <object>` a print, not a feature. The
Quint model (`models/intent/`) is written first and the Rust type
surface transcribes what the model proved ergonomic — the session's
explicit instruction. Per-object lifecycle typestates (created → plugged
→ bound → enabled, VFIO, link-up) stay with #4–#8.

### D7. Model first, hard gate, then Rust

Phase 1 is Quint only: vocabulary as types, derivation as pure defs,
every rule a named invariant, three scenarios written as intent with a
negative twin each, random simulation over the intent space, and the
reference-board fit check. It ends in a decision bead — *intent
vocabulary accepted* — that the operator closes after reviewing the gate
artefact, which is the user manual itself: `docs/intent.md` (constructs,
inputs, derived quantities, refusals, the scenarios as worked examples)
and the rewritten README example, written before any Rust — working
backwards from the operator's experience, and shipping the reviewed
document as the reference rather than a throwaway. Every Rust task
depends on that bead. The decision is recorded as the ADR-0005
amendment, which also cites the external anchors: RFC 9315/9316 (intent
is outcome-level with constraints; `max_cores` is a constraint,
`workers` was configuration) and the ONOS intent framework (per-type
compilers producing installable intents, installers separate,
recompiled on topology events — this change's compile/executor split).
Counterexamples found by simulation become rules or recorded unknowns,
never silent adjustments. Alloy is recorded as an escalation trigger
beside TLA+ (ADR-0002) for a relational property that proves awkward in
Quint; not taken up front.

### D8. Every scenario is a `.qnt` beside the `.toml` it means

`models/intent/scenarios/<name>.qnt` holds the intent value, the
expected plan shape, and the invariant run; `<name>.toml` beside it is
what an operator would type for the same intent. Quint cannot read
TOML, so the pairing is by convention in phase 1 and by test in phase
2: `dpaa2-verify` parses the TOML, compiles it, and asserts the plan
equals the ITF trace's. The three scenarios and their twins: (1)
hardware-switched fabric over dpmac.7/8 at 10G, kernel-forwarded, with a
userspace-poll router as a tenant member terminating dpmac.9/10 at
2×10G — the seeded rate class, so the router's T derives from the table
under `max_cores` = M (twin: M below the derived T, refused as
`CoreBudgetExceeded`; the kernel tenant's budget never binds, so the
twin needs the userspace member to make the refusal reachable); (2)
virtual fabric between two userspace-poll tenants
with no dpmac (twin: the same dpbp pool overdrawn by a third
tenant); (3) userspace router over N×10G + 1×25G with `crypto_flows`
≤ N (twin: a port claimed by two tenants). The reference board's
actual provisioning — kernel root with dpmac.7/9 and the userspace-poll
child
— is the fourth scenario, whose compiled plan is diffed object-for-object
against the snapshot; the 3-vs-1 dpmcp is the expected finding.

### D9. Rust tests beside the model: property, snapshot, and trace replay

Quint's random simulation is the oracle; three laws are cheap enough to
restate in Rust with `proptest` (dev-dependency) so the transcription
stays honest: `compile` is deterministic, a limit never lowers a
request, every companion precedes the consumer that draws it. The
dry-run text — provenance trees included — is snapshotted with `insta`.
Trace replay stays on the ITF replayer #2 built; `quint-connect`
(Informal's Rust MBT crate that drives a Rust driver from Quint traces)
is evaluated in the replay task and adopted only if it retires more
code than it adds — the few-dependencies tenet decides, not novelty.

### D10. Pipeline shape and the breaking schema

`topology.toml → Intent (dpaa2-config) → compile (dpaa2-api) →
DesiredTopology → reconcile`. `Intent` and `Inventory` carry no serde;
`ConfigSource::load` returns `Intent`; `DesiredTopology` becomes the
compiled plan (objects, edges, memberships, provenance) and `reconcile`
keeps its signature, executing the objects it has executors for and
reporting the rest as plan-only — not drift, not error. The port-only
schema is replaced: a file with only `[[port]]` entries still parses
(ports default to the `kernel` tenant in root, today's behavior), but
`DesiredTopology`'s old shape is not preserved and the README example
is rewritten. Alternative rejected: keeping the port list as a parallel
desired type — two "desired" values the reconciler would have to keep
consistent.

### D11. The intent layer is additive; api, mc and hal stay directly usable

The layering `dpaa2-config → dpaa2-api → dpaa2-mc/dpaa2-hal` is not
collapsed by this change. `Intent` and `compile` live in `dpaa2-api`
but nothing below them depends on them: `reconcile` takes a
`DesiredTopology` and does not know where it came from, `McControl`,
`KernelControl` and the HAL keep their surfaces, and the plan's
constructors are public — a library user can build a `DesiredTopology`
programmatically through the same witness-taking constructors (so the
relationship locks of D6 still hold) and reconcile it, or drive the MC
directly through `dpaa2-mc`, without `Intent` or a TOML file. This is
what keeps raw object-level escape hatches (ADR-0005 §5's last resort)
addable later at the plan layer without touching the compiler; none is
built here. Alternative rejected: making `compile` the only producer of
`DesiredTopology` (private constructors) — it would turn the intent
vocabulary from a frontend into a gate on the library.

### D12. The board milestone is one read-only sitting

The compiler is board-free by construction (ADR-0005). The single
sitting re-runs the fit check against a live census — `dprc show
mc.global --resources`, `dpmac info` on the lifecycle-safe ports, the
container listing — and diffs the compiled plan against it; no object
is created, moved or destroyed. It fires after the gate and after the
Rust compiler exists, so the diff exercises the shipped code path. The
first execution of a compiled plan is change #4's.

### D13. Tracking

Beads epic `intent-layer`; `tasks.md` is the entry point; one bead at a
time through acceptance; follow-ups discovered by a task are child
beads and `tasks.md` lines completed before the epic closes; the gate
bead and the sitting bead are the two sync points with the operator in
the critical path. Commits: `intent-layer: spec-init` for the
artifacts, then one commit per standing law or module with the
`Change: intent-layer` trailer.

## Risks / Trade-offs

- [The rate-class → workers table is a guess dressed as a rule] →
  provenance prints *unmeasured* on every T it produces, `max_cores`
  bounds it, and ADR-0005's amendment names the trigger that replaces
  it; the table has one seeded row (10G) plus one declared row (25G),
  refuses rate classes it does not know, and a cross-class mix carries
  an *unmeasured combination* warning rather than passing silently.
- [The vocabulary is a commitment; a scenario later needs a construct
  that is not there] → the gate exists to catch it with three shaped
  scenarios and random simulation before Rust; a fourth construct comes
  through a change with baseline backing (ADR-0005 consequences), not a
  field added in passing.
- [Fabric is plan-only until #11; a dpsw rule may be wrong for years
  before an executor tests it] → the predicate is copied from
  `dpsw.md`'s `[read]` rules with their kernel-source anchors, marked
  read-not-verified in the invariant comment, and the ledger row keeps
  DPSW-I1/I2 board-pending under #11.
- [The dpni ceiling of 18 is unlisted; the inventory cannot see it] →
  the inventory carries it as `Ceiling::Observed` with its ADR-0011
  provenance, so the feasibility invariant still fires; a firmware
  change re-anchors it (ADR-0011 revisit trigger).
- [Feasibility is a sum-vs-ceiling check, not a solver] → it answers
  "does this set fit", never "how many fit"; the day an intent asks
  for the latter it is an ILP and the honest tool is a solver (Z3,
  MiniZinc), recorded here so nobody bends Apalache into one.
- [`DesiredTopology` reshaping touches the retro-model's ITF replay
  from #2] → the replay targets `reconcile`'s inputs; the task that
  reshapes the type re-runs the ladder and updates the retro-model's
  adapter in the same bead.
- [Random simulation over the intent space is unbounded] → the
  scenario generator draws from a finite alphabet (the inventory's
  dpmacs, two rate classes, tenants ≤ 3, cores ≤ 16); coverage is
  counted, not assumed.

## Open Questions

- Whether the fit check's 3-vs-1 dpmcp finding is reported as a
  divergence of the board from intent (the board carries two idle
  portals) or as an override in the reference intent — decided at the
  gate, recorded in the ADR-0005 amendment.
- Whether the kernel tenant's `max_cores` means online CPUs (the
  dpio ceiling) or a budget below it; the model carries both readings
  as a parameter until the gate picks one.
- Rate classes beyond 10G and 25G (the board's two) are refused, not
  derived; a third class enters with a board that has one.
