# pool-objects — proposal

## Why

Everything delivered so far configures MC objects, but nothing has ever
come alive: a dpni only carries traffic once its consumer can draw the
allocation-pool families — dpbp (buffer memory), dpio (frame I/O), dpcon
(notification channels), dpmcp (command portals) — and those four
families have no Rust surface, no adapter verbs, and a COVERAGE ledger
full of rows deferred to this change (roadmap #6). Delivering them as
one change — they live and die together as the driver's allocation
pool — produces the first MVP workable setup: a kernel-owned live
interface in dprc.1 and a populated, VFIO-bound child container
(dprc.N) any userspace dataplane can consume, replacing the
ls-addni/dynamic-DPL script path for real.

## What Changes

- The four pool families gain their model surface per ADR-0019's Quint
  module architecture: one pattern-owned focused machine
  (`models/families/pool_lifecycle.qnt` — allocator custody,
  count-convergence and prune rules, dpio seat arithmetic, invariants
  and directed runs), member files staying type/parameter modules (the
  dpcon→dpio notification edge, dpio seat regime), core gaining only
  corpus-wide census extensions — closing or advancing the COVERAGE
  rows routed to `pool-objects` (#6).
- `dpaa2-api` gains the P3 counted-companion implementation (ADR-0019):
  one generic shape over `FamilyParams` for the allocator trio
  (dpbp/dpmcp/dpcon), a seat-typed dpio variant, and the count-drift
  disposition — grow and shrink to intent-derived counts, only free
  individuals destroyed, in-use shortfall a refusal. No per-object
  identity types; no cross-pattern trait framework; structural
  isomorphism to the Quint model preserved (ADR-0002 §3).
- dpio enters as table-pure P3; whether its create-cfg earns a P2 facet
  is an explicit judgment point with a marker (bead + acceptance
  criterion), amending ADR-0019 only if evidence demands.
- `dpaa2-mc` gains create/destroy chains for the four families over the
  restool shim, the kernel root-bind face (dpaa2-eth attach in dprc.1),
  and child-container population for the VFIO path.
- Convergence becomes eventually consistent for pool capacity:
  undeclared, non-DPL-born, free objects are pruned; declared deficits
  are created; the planner reasons in counts, never companion identity.
- Board suites deliver the two MVP scenarios: a live kernel interface
  in dprc.1 (dpni + companions + dpmac, kernel-attached) and a
  populated, VFIO-bound dprc.N. The DPL-defined-child mechanism (a boot
  config edit touching the recovery baseline) fires only if a named
  invariant proves unreachable at root scope.
- The intent vocabulary is untouched: companion counts are already
  derived per consumer regime (ADR-0012); this change consumes the
  compiled plan.

## Capabilities

### New Capabilities

None — the four families land inside the existing capability surfaces.

### Modified Capabilities

- `formal-models`: the four family modules grow lifecycle actions and
  named invariants over the pool substrate; count-convergence and prune
  laws; the dpio seat/dpmcp probe-draw edges.
- `reconciler`: count-drift disposition for P3 families (grow/shrink/
  refuse) and the anonymous-capacity prune rule join the plan surface.
- `mc-backend`: restool-shim create/destroy verbs for dpbp/dpio/dpcon/
  dpmcp; kernel bind/unbind of a root dpni; child-container population
  and VFIO handoff observation.
- `mbt-harness`: suite generation for pool walks (census, exhaustion at
  the ceiling, free/drain, prune convergence) and the two MVP board
  scenarios inside the safety envelope.
- `system-integration`: the end-to-end MVP acceptance — dprc.1 live
  kernel interface and dprc.N VFIO-bound populated container converge
  from one intent file, idempotently, with no persisted state.

## Impact

- `models/families/{dpbp,dpio,dpcon,dpmcp}.qnt`, `models/core/`
  (pools/create_allocate/companions edges), `models/COVERAGE.md` rows
  routed to #6.
- `crates/dpaa2-api/src/families/` (new P3 module(s)), `plan/`
  (disposition), `core/` as needed; `crates/dpaa2-mc` (shim verbs, bind
  faces); `crates/dpaa2-verify` (suites, frozen traces).
- ADR record updates follow decisions as they fire (candidate hosts:
  ADR-0011 capacity judgment, ADR-0012 sizing, ADR-0019 P3 reference
  implementation and possible dpio facet); none pre-committed.
- No intent/TOML schema change; no new external dependencies; restool
  remains the only backend (ioctl portal stays #10).
