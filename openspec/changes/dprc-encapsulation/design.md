# dprc-encapsulation design

## Context

Roadmap tile #4, proposed just-in-time after #3 (intent-layer, delivered
2026-09-06 with vocabulary-v2 hardening). The dprc baseline
(`docs/baseline/dprc.md`) is unusually mature: the option-bit permission
matrix, eviction law, lock semantics, visibility law, and plug gating are all
board-settled with distinct refusal statuses recorded, and the remaining
unknowns are precisely the restool-unreachable portal faces. The `McControl` /
`KernelControl` seams (`dpaa2-api/src/port.rs`) exist with restool as the sole
southbound implementation. The intent layer can declare consumers and derive
object sets but nothing realizes containers.

Scoping was settled in the proposing session (grilling, 2026-09-08): restool-
bound, sans-io emphasized, end-to-end intent wiring, container-only
population, board fully in scope with scratch-first suites.

## Goals / Non-Goals

**Goals:**

- D-shaped for #5–8: the pure reconcile core carries the change; the board
  suite verifies what replay cannot (VFIO, pool boundary, live convergence).
- Child-DPRC lifecycle as Quint-isomorphic Rust typestates; containment laws
  as plan guards.
- End-to-end: declared consumer → converged, VFIO-bindable container on the
  board, idempotent, with provenance.
- First online-MBT discovery sessions (containment/pool semantics).

**Non-Goals:**

- Any ioctl/portal transport (tile #10) — including read-only query paths; a
  precursor was considered and explicitly dropped in scoping.
- Companion population/sizing (tile #6), dpni option surface (tile #5),
  cross-DPRC links (tile #9), DPL pinning of icid/portal ids (tile #14).
- Probing restool-unreachable faces: child-portal unlock (DPRC-I11),
  OBJ_CREATE_ALLOWED gate, DPRC-I8 batch ordering — deferral rows to #10.

## Decisions

### D1 — Restool-bound, portal faces deferred

The change drives every MC mutation through the existing restool shim.
Alternative considered: a parallel thin change with a sans-io command codec +
read-only ioctl transport (byte-capture differential harness) to pre-stage
#10. Rejected for now: nothing to differentiate before #5–8 land families,
raw commands bypass restool's client guards on a live board, and two active
changes contend for operator sittings. The sans-io leaning is captured
instead by D2, at zero board risk.

### D2 — Sans-io emphasis: the pure core carries the change

All lifecycle/containment logic lands as pure functions and typestates in
`dpaa2-api`, verified by ITF replay off-board; `dpaa2-mc` grows only thin verb
plumbing. Consequence: the conformance gate (DoD gate 3) covers the
permission matrix, eviction, visibility, and ordering logic with no operator
in the loop, and #5–8 inherit the same shape. This is the deliberate lean
toward the rest of tier A without starting transport work.

### D3 — Typestates mirror the Quint lifecycle sum

One sum: Declared → Created(unplugged) → Populated (assign-only face) →
Plugged | Locked(hierarchy) → Emptied → Destroyed, with VFIO bind state
carried on the plugged face (kernel-side, not MC-side, per the baseline: a
DPRC's plugged state is not restool-mutable; binding is the kernel lever).
Quint authored first (quint-is-the-spec); Rust twin is structurally
isomorphic; the ADR-0002 law binds them. Alternative — separate MC-state and
kernel-state machines — rejected: the baseline shows they interlock (plug
gating IS bind gating), and one sum keeps illegal interleavings
unrepresentable.

### D4 — Refusal discrimination is a plan-level concern

The three refusal shapes (0x6/0x8/0x4) and restool's client-side guards are
typed in `dpaa2-mc` and interpreted in `dpaa2-api` (drift attribution,
permission-gap reporting). Rationale: V-DPRC-2/6 proved the statuses carry
causal information a reconciler must not collapse; burying them as strings in
the shim would repeat ls-main's scraping sin.

### D5 — Container-only population, minimal residents for laws

Suites and discovery sessions use the cheapest sufficient residents (dpbp-
class objects, as prior V-DPRC suites did) to exercise DPRC-I1/I9 and the
eviction law. The intent path emits no residents at all. Keeps #4 one kind of
learning (containment), per the tiling contract.

### D6 — VFIO exercised actively, scratch-first

The board milestone performs a real driver_override + bind/unbind on a
scratch child, including override propagation to a subsequently-added child.
The live VPP container (`dprc.2`) is not fenced off — the board is dedicated
to this effort — but suites default to scratch children and self-clean;
deliberate VPP-container use is named in the suite plan. Alternative
(typestates verified read-only against dprc.2's observed state) rejected:
ships the VFIO typestate board-unverified.

### D7 — Discovery sessions are the online-MBT debut

`dpaa2-verify`'s online driver runs model-guided sessions against
containment/pool faces still marked candidate (DPRC-I1, I5, I7, I9-under-
reconciler, I10, lock remainder of I11). Divergences amend model + baseline
in-change (DoD gates 1–2). Deferred faces emit deferral rows, never probes.

## Risks / Trade-offs

- [Two-pass interlock: VFIO binding a scratch child may interact with
  autorescan/hot-plug in unmodeled ways] → sessions run under the existing
  operator-supervised envelope; the recovery guarantee (verified in #2) gates
  all mutating suites; hooks never destroy in-run (standing rule).
- [End-to-end convergence path touches the live bus (rescan visibility)] →
  convergence verdicts come from MC re-observation only (DPRC-I6 requirement);
  no sync-based success claims.
- [Container-only scope may feel thin as a "use the intent layer" milestone] →
  accepted deliberately; the payoff is a small #4 and pre-shaped #5/#6.
- [Deferral rows accumulate against #10] → they are the differential test
  plan for #10's portal bring-up; recorded loss, planned recovery.

## Migration Plan

No deployed state to migrate: the reconciler is level-triggered and
stateless. Rollback of a converged consumer container is the reconciler's own
teardown path (destroy after evict), exercised by DPRC-I9 suites.

## Open Questions

- Whether override propagation to subsequently-added children is observable
  with container-only population (a child must be *added* while bound) — the
  suite may need a resident create post-bind; settle at suite authoring.
- Label semantics under lock (set-label is accepted on locked children,
  V-DPRC-3): does the reconciler treat label drift on a locked container as
  repairable? Default: yes (the board says the verb works); confirm in model.
