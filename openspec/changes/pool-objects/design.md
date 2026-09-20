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
