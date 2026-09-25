# pool-objects — design

## Context

Roadmap #6. The intent layer already derives every pool count per
consumer regime (ADR-0012; `intent/derive.rs`: poll-mode dpio 2·T, dpbp
2, dpmcp per process; kernel dpio per online CPU, plus the per-dpio
dpmcp probe draw), and the Quint core already models allocation pools
generically (`core/pools.qnt`, `create_allocate.qnt`, `companions.qnt`
over `FamilyParams`). What does not exist is everything between the
compiled plan and the board: the Rust family surface, the adapter
verbs, and the suites. ADR-0019 classifies all four families as P3
(counted companion) — a pattern with no reference implementation yet.
This change is P3's trial: it either validates the pattern's clauses or
amends them with evidence in hand.

The grilling session of 2026-09-20 fixed the frame: business outcome is
the first MVP workable setup — a live kernel interface in dprc.1 and a
populated, VFIO-bound dprc.N for any userspace dataplane — and the
posture throughout is "rework as we learn, no early variance for
undiscovered gains."

## Goals / Non-Goals

**Goals:**

- P3's first implementation: one generic shape over `FamilyParams` for
  the allocator trio (dpbp/dpmcp/dpcon), a seat-typed dpio variant,
  structurally isomorphic to the Quint model (ADR-0002 §3).
- Full count convergence: grow and shrink to derived counts; eventual
  consistency by pruning undeclared, non-DPL-born, free objects.
- The two MVP board scenarios green: kernel root-bind live interface;
  DPDK-shaped child container populated and VFIO-bound.
- The COVERAGE rows routed to #6 closed or advanced with board
  evidence (DPBP-I2/I3/I4/I5/I6/I7, DPIO-I2/I3, DPCON-I1/I3/I4/I5,
  DPMCP rows).

**Non-Goals:**

- No dpni/dprc retrofit onto any shared seam — P1/P2 are different
  patterns by design (ADR-0019); no cross-pattern trait framework.
- No intent/TOML vocabulary change — counts stay derived, never
  declared.
- No sustained traffic — dpmac constraints hold until #9; "live" means
  kernel-attached and link-connected, not soak-tested.
- No ioctl transport — restool shim only (#10).
- Actually launching a userspace dataplane (DPDK/VPP) against dprc.N is
  a manual one-shot validation, outside the automated suite envelope.

## Decisions

### D1 — The shape is P3, applied not invented

The four families implement ADR-0019 P3: plain counts and sizing
functions; pool custody as membership for the trio (`pooled: true`);
regime-typed per-CPU seats for dpio (`pooled: false`, DPIO-I1/I2). The
trio differs only by `FamilyParams` data, so it lands as one generic
implementation instantiated three times — genuine reuse inside one
pattern. Macros are admitted only where they buy semantic/structural
sharing that generics cannot (e.g. stamping the per-family linted-enum
copies of ADR-0014); a macro that merely saves typing is refused, and
every shape stays isomorphic to the Quint sums so the model remains the
thing the code is validated against.

*Alternative rejected:* a universal `McObject` lifecycle trait across
families — it forces P4 to advertise a create that must not exist, P3
to carry per-object identity that lies, and P1 to surrender its verb
gates (the ADR-0019 idiom stance names this the per-object style the
catalog exists to refuse).

### D2 — The count→individual boundary sits in the plan, not the family

P3's untested clause. The planner and disposition reason exclusively in
counts per (container, family): observed census vs derived requirement.
Where counts become individuals is the dispatch edge: a grow delta
emits N creates (no names — companions wear their consumer's name,
ADR-0015); a shrink delta selects victims from the *free* set only,
arbitrarily (anonymity is the pattern's truth; picking is not a policy
surface). The family module exposes census/sizing types and the
disposition; the adapter owns turning a delta into concrete
create/destroy verbs against concrete ids. If this boundary grows into
an agreement none of ADR-0011/0012/0019 can host without distortion, a
new record is minted at that moment — not before.

### D3 — Convergence rules for anonymous capacity

- Deficit → create to the derived count.
- Surplus → destroy, free individuals only, never a drawn one.
- Intent below current draw → refusal surfaced to the operator
  (`ShrinkBelowDraw` shape), never a forced teardown of a live
  consumer.
- Prune: an object that is undeclared in intent AND not DPL-born
  (boot-baseline objects are foreign, roadmap #14 standing rule) AND
  free is deleted — pool families join the prune discipline
  dprc-hardening established. DPBP-I3's dirty-return law holds: a pool
  free is no reset; the drain is the kernel's, observed not driven.

### D4 — dpio is table-pure P3 with an earned-amendment marker

dpio enters exactly as ADR-0019's table says: P3, seat arithmetic, not
pool custody. Its create-cfg (channel mode, priorities) smells of P2's
hazard class, but the facet is not pre-legislated: the phase-2 dpio
task carries the acceptance criterion "judge the cfg hazard class
against a concrete refusal or MBT probe; amend ADR-0019 with a cfg
facet only if earned," and a bead marks the revisit so it cannot
silently lapse to epic-review. Same treatment for the dpio→dpmcp
probe-draw ordering edge: procedural in the adapter (the dpni
set-MAC-before-plug precedent) unless the typed surface gains
order-hazardous verbs.

### D5 — Suite reach: root-bind floor, DPL-child only if earned

The kernel face is exercised where it is already proven reachable: a
root-container dpni's dpaa2-eth bind (V-DPRC-10 witnessed the allocator
draw and its exhaustion refusal live). The child-container kernel face
needs a plugged, bound child dprc, which restool refuses; the
DPL-defined-child mechanism edits the boot configuration — the recovery
baseline the whole safety envelope (ADR-0003) is anchored to — so it
fires only when a named invariant (candidate: DPBP-I2's kernel-pool
half) is demonstrated unreachable at root scope, as its own gated task
(bead dpaa2-controlplane-5y7 tracks the window). The VFIO path needs no
plugging at all: dprc-encapsulation (#4) already delivered child
lifecycle + VFIO binding; this change adds the population.

### D6 — Records follow decisions

No ADR is pre-committed. Named decision points — the D2 boundary, the
D4 facet, P3's reference-implementation line — produce a new ADR or an
amendment (candidate hosts: ADR-0011, ADR-0012, ADR-0019) when they
fire, per the modus operandi: evidence updates the worldview, the
record follows (ADR-0016).

### D7 — Model layering: pattern-owned mechanisms, family-owned machines (ADR-0019 Quint module architecture, re-amended)

*Re-amended 2026-09-20 during phase 1 (bead dpaa2-controlplane-960.3
review): the first cut put every P3 machine transition and directed
run inside one shared module, and the file immediately accreted
family-particular blocks (dpio seat records and runs beside trio
custody) with nothing to do with one another — unreadable at sixteen
families.*

The Quint work follows ADR-0019's re-amended Quint module
architecture: `models/families/pool_lifecycle.qnt` keeps ONLY what is
genuinely common to P3 — the parameterized custody substrate
(census/ceiling predicates, custody-cycle mechanics, count
convergence, free-only shrink, `ShrinkBelowDraw`, prune with the
DPL-born exemption) — and exists to contextualize each family. Every
member file (`dpbp.qnt`/`dpmcp.qnt`/`dpcon.qnt`/`dpio.qnt`) owns its
stateful modules in the `dprc.qnt`/`dpni.qnt` shape: `module
<f>_lifecycle` — thin for the trio, each instantiating the pattern
substrate contextualized to its family; dpio's standalone (unpooled,
reusing the shared core transforms) with its seat particulars —
seat record, probe draw, DPIO-I3 surface — plus `module
<f>_scenario` when scenario content exists. Family-specific types
stay family types (dpcon's notification-edge, dpio's seat-regime
vocabulary). `models/core/` gains only what is corpus-wide;
`main.qnt` keeps the baseline-id runs. The review defect is
re-deriving pattern mechanisms locally, not owning a machine:
substantial family dynamics on top of thin instantiation are facet
evidence feeding the D4 judgment.

*Alternatives rejected:* per-family lifecycle copies that re-derive
the custody cycle — near-identical machines whose drift nothing
checks; one shared module holding every member's transitions and runs
— the accretion this re-amendment removes; and pool depth in `core/`
— dissolving the tier-2 encapsulation boundary that keeps core
answering "how does the board behave" and a families/ file answering
"how does this family behave".

### D8 — Phasing is layer-major with a family traversal inside

Five phases, strictly ordered: models → dpaa2-api → dpaa2-mc → suites →
close-out. Inside each, the family traversal runs dpmcp → dpbp → dpcon
→ dpio: dependency bottom first (everything draws a dpmcp; dpio itself
draws one at probe), the odd seat-typed member last so three data
points of the generic shape exist before its judgment fires. A
per-family vertical slice is refused: P3 makes the trio one
implementation, and the MVP scenarios are consumer-shaped — no single
family converges anything testable alone.

### D9 — One provider per pool: the pool construct owns root capacity

Two mechanisms provision the same functional pool at root: the
restool shim's per-port chain in `create_dpni` (an ls-addni-shaped
draw — dpio top-up plus dpbp/dpmcp/dpcon companions stamped with the
port's name) and the intent-derived pool construct (`converge_pools`,
D3). The kernel's fsl-mc allocator ignores labels: every plugged
pool-family object in a container is one functional pool, so the
per-port companions double-feed the root pool and break census
idempotence. The label is control-plane custody bookkeeping only
(ADR-0015), never an allocation input — a dpmcp labeled `kern0`
counts into the pool exactly like a drawer-labeled one. Board
evidence names the break: V-POOL-6 rev3 observed drawn 20 against a
derived requirement of 19 — the port's own dpmcp companion counted
into the census and turned a converged state into a refusal.

Decision: the pool construct is the sole provider of pool-family
objects at root. Per-port creation of dpmcp/dpbp/dpcon and the
per-port dpio top-up cease; the derivation instead folds each
declared port's draw into the pool requirement — +1 dpmcp, +1 dpbp,
+num_queues dpcon per port, the ADR-0012 `companionDraw` arithmetic
the derivation already anchors on. A departing port leaves surplus
capacity, which D3's free-only shrink reclaims level-triggered — no
per-port rollback, no companion teardown. This composes with D2's
count→individual boundary (the fold is a count edit; the adapter
still turns a delta into ids) and D3's convergence laws unchanged.

Child-container population is the carve-out: `create_dpni_in` and
`populate_child` are unchanged. There the consuming construct's
chain is the only provider and companions wear the consumer's name —
one provider per pool holds in the child too, and consumer-name
custody is correct because the consumer is the provider.

*Alternative rejected:* a census "other-owned" arm that specially
counts root pool-family objects wearing another construct's label.
Under single provider such objects do not exist at root by
construction, so the arm would count nothing real; foreign and
DPL-born handling (D3, the roadmap #14 standing rule) stand
unchanged and already cover boot-baseline objects. Building the arm
would legitimize the double-feed the decision removes instead of
removing it.

### D10 — Custody carries the plug facet; reclaim probes, residue is typed

*Added 2026-09-24 during phase 4 (the 4.3 authoring audit, beads
dpaa2-controlplane-960.22/.23/.27).*

The board exposes two custody observables the model had collapsed into
one: *plugged* — the object sits in the kernel's allocatable pool
(DPBP-I2: allocatable ⟺ plugged ∧ allocator-bound) — and *drawn* — a
consumer actually holds it. The Rust census bridged them with a
conservative proxy (plugged ⇒ drawn), and the reconciler plugs
everything it creates so the kernel can draw it; together those make
managed surplus structurally unreclaimable — D3's free-only shrink and
D9's "a departing port leaves surplus for free-only shrink" both
assert a law the shipped census cannot execute (V-POOL-6 rev 1–3, the
2026-09-24 audit). The divergence lived in the observation mapping
between the model and the board, a seam trace-replay cannot check
until the model's state space carries the facet explicitly.

Decision, in three parts. (1) The model splits the facets:
`pool_lifecycle` gains plug/unplug transitions distinct from
draw/return, and the twins' observation mapping carries plugged
explicitly, so census divergences of this class fail offline first —
the model stays the behavior oracle the Rust twin is validated
against. (2) Reclaim is the *unplug-probe* law: a shrink or prune of a
managed individual attempts the unplug first; the MC refusing it
(object in use) is the drawn signal read from the board itself and
surfaces as the ShrinkBelowDraw face; a successful unplug is followed
by destroy. Drawn-ness is discovered, never inferred from a proxy.
(3) A divergence the tool must not reconcile live is *typed residue*,
never silence: the grow-only dpio seats (D4; the twice-observed
ADR-0008 §4 race) render in ensure/dry-run/status as a
reboot-required disposition — observed vs. required stated, the
reconciliation path named (ADR-0003 §7). Intent stays the expressed
truth; inventory the tool cannot move is reported against it, not
carved out of it.

The two label judges are reconciled under the same decision:
`judge_label` and `PoolMembership` disagree on the empty label
(Foreign("dpl") vs DplBorn — the board twice showed the out-of-band
empty-label dpbp escaping prune), and one law replaces them.

*Alternatives rejected:* creating surplus unplugged — it leaves the
kernel unable to draw the capacity the pool exists to provide;
keeping the proxy and dropping shrink from scope — it abandons D9's
own premise; destroying dpio seats live — it re-opens the §4 race on
every teardown for no witness the reboot does not already give.

### D11 — Actuation is planned from the compiled plan, per container

*Added 2026-09-24 during phase 4 (the 4.3 authoring audit, beads
dpaa2-controlplane-960.24/.25/.26).*

Task 3.3 delivered `populate_child`/`vfio_handoff` as primitives, but
nothing in the imperative shell called them: the port loop actuated
every terminated port in the root (a userspace tenant's dpni would be
created in dprc.1 and handed to dpaa2-eth), the child was created
empty, and `populate_child` hardcoded one dpni per child while the
reference intent derives two — a compile-vs-actuation arity mismatch
between two individually verified layers.

Decision: every actuation pass takes its shape from the compiled plan,
which the derivation invariants already verify. Ports route by their
planned dpni's container (Root ports feed the port loop and
link::apply through a root-only projection; child port-edges feed the
population plan) — the container, not the dataplane, is the key, so a
`Restricted { pool: kernel }` userspace tenant lands where its plan
says. `converge_population` runs after `converge_containers`: child id
resolved by label, per-port dpnis and derived companions populated
from the plan, connect issued from the common ancestor (DPNI-I9 form,
without the root plug step), and `vfio_handoff` guarded by a
`bound_driver` read so a re-run is a no-op. Population renders its own
dry-run block and joins the status census, so "empty plan" quantifies
over every pass. Drift inside a bound child is a typed refusal
(ADR-0017: residents added while bound stay invisible until a rebind
cycle); the healing policy needs a live dataplane to schedule the
disruption and is roadmap #9's decision (bead dpaa2-controlplane-w01).

*Alternatives rejected:* routing by tenant dataplane — it contradicts
the plan for restricted-pool tenants; one dpni per child for the MVP —
it closes the change with the flagship reference intent unactuatable;
folding population into `converge_containers` — it breaks the
container-only tile #5/#6 boundary and the vdprc9 pin that guards it.

### D12 — The converge loop is structurally unable to churn hardware

*Added 2026-09-25 after the 4.3 sitting failure (bead
dpaa2-controlplane-960.12; ADR-0008 §9; upstream finding 49).*

The reconcile cfg-drift branch judged a just-created kernel dpni as
DestroyThenCreate on every pass — the kernel-profile read-back
projection was never board-pinned — and the loop either died on
restool's bound-destroy refusal (the Unbind transition was a logged
no-op) or hot-cycled create/destroy at ~1.3 s until the kernel Oopsed
(phylink NULL-PCS, then a refcount underflow UAF; board tainted, both
boots).

Decision, in three parts. (1) *Loop-breaker, modeled first:* a
DestroyThenCreate planned for a port dpni created in the same run is a
typed refusal naming the diverging observation fields, never an
actuation — dpni.qnt carries the law, the reconciler takes the
run-created set as an input and emits the refusal, and ensure exits
with it rendered. The refusal is also the diagnostic: it pins the
mispredicted field without touching hardware. (2) *Real unbind, §8
order:* the Unbind transition becomes a sysfs unbind of `fsl_dpaa2_eth`
(mirror of the child VFIO path), and every teardown emission orders
disconnect while bound → unbind → destroy (ADR-0008 §8: unbind-first
strands the dpmac; destroy-while-bound is refused client-side). (3)
*Board-pin before correcting:* `DpniObservation::project` is corrected
only against read-back evidence — the named refusal field plus one
manual restool probe on a rebooted board — never adjusted to make the
loop converge.

*Alternatives rejected:* actuating DestroyThenCreate with a retry cap —
bounded churn still crossed the kernel's crash threshold in three
cycles; skipping cfg-drift for kernel-profile blocks — it blinds the
reconciler to real drift the moment the projection is pinned; unbind
without the §8 sever-first order — board-verified to strand the port
until reboot.

## Risks / Trade-offs

- [P3's count-only claim fails contact with real dispatch — e.g.
  shrink victim selection turns out to matter (dpcon bound to a
  specific dpio)] → the D2 boundary keeps identity in the adapter;
  DPCON-I4's mutable dpcon→dpio edge is modeled in phase 1 so the
  hazard is known before Rust lands; worst case is a facet amendment,
  priced by ADR-0019 as an amendment not a rewrite.
- [Root-scope suites cannot reach a routed invariant] → D5's gated
  DPL-child task is the named escape; the spec fails loudly (task
  blocked with the invariant named) rather than silently narrowing
  coverage.
- [Kernel bind suites destabilize the board (dpaa2-eth probe loops,
  -EPROBE_DEFER)] → suites stay scratch-first and self-cleaning inside
  the ADR-0003 envelope; the recovery guarantee (reboot restores the
  DPL baseline) is the standing backstop, and exhaustion tests judge
  against the census (ADR-0011) before creating.
- [Prune deletes something an out-of-band consumer held] → prune's
  free-only precondition is judged from the same observation the
  census uses; DPL-born objects are structurally exempt; the refusal
  path (not deletion) covers everything drawn.
- [dpio facet judgment drags] → the marker bead pins it to phase 2's
  dpio task, before epic-review.

## Open Questions

- Which existing invariant ids the count-convergence and prune laws
  land under (new POOL-* ids vs extending DPBP/DPCON rows) — resolved
  in phase 1 against `models/COVERAGE.md` conventions.
- Whether dpmcp's destroyed-portal leak (DPMCP-I6 complement, COVERAGE
  OI-3 note) constrains shrink for dpmcp specifically — resolved by a
  phase-4 probe.
