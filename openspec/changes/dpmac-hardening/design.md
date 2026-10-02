# dpmac-hardening — design

## Context

The dpmac-typestate review (synthesis at
openspec/changes/archive/2026-10-03-dpmac-typestate/review/synthesis.md) left eight merged
findings and two explicit disposition forks. This design records the forks
so the parcels do not re-litigate them. The protected decisions of
dpmac-typestate (D1–D7, the board outcomes, the #10/#13 deferral routings)
bind this change exactly as they bound the review.

## Decisions

### D1 — The proof binds; the docs are not softened

MERGED-1 offered two dispositions: bind `SeveredProof` to its edge, or
ponytail-mark the ceiling and soften the transition.rs claims. The owner
picked the binding (2026-10-03): the change's own core promise is "the
teardown order holds by type" (c4d95d0), and the repo's premise is that
invalid topology is unrepresentable, not documented-as-representable. The
proof therefore stores its `DpniId` privately, loses `Clone`/`Copy`, stops
being extractable from `Unbind`, and `unbind` derives its target from the
proof. The model is already this strong (`severAt` consumes
KernelOwned∧KernelFaceBound and yields the witness, ADR-0002/ADR-0008 §8);
the Rust side rises to it rather than the docs falling to the Rust side.
The ripple (reconcile call sites, board/replay.rs, doctests) is one
parcel's API fan-out; the ADR-0016 §4 one-writer exemption covers it if the
commit cannot split cleanly.

### D2 — Counter names travel as data, not as a second vocabulary

MERGED-2's fix carries the verbatim restool row names inside
`CounterReadout::Vocabulary` (the shim already has them and drops them at
restool.rs:328). Rejected alternative: render positional `counter-N`
labels — honest but useless to an operator, and it leaves the 10.40
extension unrenderable. The D4 split stays untouched: the model keeps its
representative slice, the adapter keeps the 28-row verbatim vocabulary as
the only observable; the names ride the readout as observed data, so no
third vocabulary table is born (the fake's literal 28 resolves to a named
const from the same change). The spec-side wording (formal-models delta,
COVERAGE:106) says the split out loud instead of implying 28 model rows.

### D3 — The zero sentinel is settled in one story, two beads

B3 (hoist the shell inference) and B4 (bind-transient exclusion) are the
same judgment — "an absent or zeroed MAC read-back is not an observation" —
landing on the display path and the plan path respectively. They run
together so the two paths cite the same pure predicate
(`MacAddr::is_zero` via one family judgment), not two local conventions.
B4's Actuate posture (skip vs defer the SetMac) is decided inside the bead
with its test, model side consulted first per house ordering.

### D4 — B6 is contingent; check the record before the refactor

The `LinkType` triplication may already be an acknowledged seam
(dpni-typestate or dprc-encapsulation designs). The bead first searches the
designs/ADRs; if the triplication is recorded deliberate, B6 shrinks to a
doc pointer naming the collapse trigger (next P4 family). Only an
unrecorded triplication earns the `From`-seam commit now. Either way the
family sum (the qnt twin) is the one that survives — ADR-0014 protects the
model↔Rust lockstep, not intra-crate convenience copies.

### D5 — Ordering

B7 ∥ B1 first (B7 is pure docs with no dependency; B1 is the lead defect).
Then B2 (its doc tail lands on B7's amended files). B3+B4 together (D3).
B5/B6 anytime after B2 settles parse.rs's shape. One bead at a time through
acceptance per repo rule; parcels go to the pinned developer agent.

## Risks / Trade-offs

- [B1 changes a public plan-surface API] → the fan-out is contained to
  in-repo consumers (reconcile.rs, replay.rs, tests); no external consumer
  exists; the one-writer exemption is the recorded escape if atomicity
  demands it.
- [B2 widens `CounterReadout`] → additive; the fake and fixtures update in
  the same parcel; render tests pin the one operator-visible behavior
  (correct name on a non-uniform readout).
- [B7 touches sealed board records] → forward pointers only; verdict prose
  is never rewritten (review hard rule, PASS1-F5 disposition).

## Open Questions

None blocking — B4's Actuate posture and B6's contingency are decided
inside their beads by construction.
