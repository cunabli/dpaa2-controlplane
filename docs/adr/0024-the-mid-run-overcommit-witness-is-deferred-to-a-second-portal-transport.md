# ADR-0024: The mid-run overcommit witness is deferred to a second portal transport

Status: accepted

## Context

The rollback-on-overcommit machinery of the orchestration model (ADR-0023)
is proven offline. The `plan-rollback` change carries its oracles — the
guarded-action model's rollback action, ORCH-I3 restated as net identity up
to the typed dpio-seat residue with no apply/rollback oscillation reachable,
and the frozen overcommit-rollback trace replayed through the Rust `admit` —
and the `vrollback1_intents` pin tests fix the operator-facing render of a
mid-run overcommit disposition in observed data terms (openspec change
`plan-rollback`; ADR-0023).

What offline proof cannot supply is the live witness: a concurrent drain that
provably lands *inside the run window* — between admission reading the pool
free counts and the engine's own create verb — so the firmware's no-resources
status `0x8` fires mid-run and the executed prefix unwinds against a real
board. Suite V-ROLLBACK-1 is that witness, and it needs a drainer whose draw
is orderable relative to the engine's verbs. Two drainer shapes exist:

- **(a) a restool writer** — a suite-owned process that opens the root
  container and drains the raced pool on a deterministic signal.
- **(b) the kernel probe's own allocator draw** — the bus-side consumer that
  pulls from the same pools as a bound driver probes.

Each shape is blocked by a standing hardware law:

- Shape (a) collides with the kernel's single-opener rule on `/dev/dprc.N`: a
  second opener fails at the `open()` syscall before any command reaches the
  MC while the first opener is held (V-CONC-1; ADR-0006). The `dpaa2-mc`
  shim's bounded portal-open retry (bead `dpaa2-controlplane-f59`) makes that
  collision *benign* — the loser of the race backs off and re-opens rather
  than aborting the run — but it does not make the drain *orderable*: whoever
  loses waits for the other to release the portal, so the writer's draw cannot
  be placed deterministically between two of the engine's verbs.
- Shape (b) draws asynchronously and defers-not-fails (the DPSW-I9 class: a
  missing allocatable defers the probe, an automatic retry re-fires only on
  bus events, and the completion lands seconds later), so it carries no
  decisive ordering story relative to the engine's dispatch.

Neither shape can therefore place a draw inside the run window on the restool
transport, which is the only MC transport this control plane drives today
(the ioctl MC-portal transport is roadmap `mc-portal-backend`, ADR-0004 §8,
ADR-0018).

## Decision

The mid-run overcommit witness is recorded as **not reachable on the restool
transport**. It is deferred, not abandoned.

The verification surface for the `plan-rollback` change is the offline proofs
above together with the landed live witnesses that exercise the capacity face
and the global-ledger rows on real hardware:

- V-ADMIT-1 — the live admission refusal (an over-budget intent refused
  pre-dispatch in operator data terms).
- V-CEIL-1 and V-DPRC-7 — the global `bp` budget row joining the capacity
  face, proven by a dpbp create refusing exactly at the `bp` free count with
  symmetric credit-back.
- V-DPCON-2 — the `dpcon` draw on the MC-global hardware ledger (the
  `swpch.2wq` draw at one unit per dpcon for the engine's fixed
  `--num-priorities=2` operand). Its verdict lives in the hardware ledger
  only: no dpcon row is in the capacity face until it joins under the
  monotonic-growth rule (ADR-0023).

Suite V-ROLLBACK-1 keeps its **machinery verdict** — the precondition gate,
the drain writer, convergence under drain, and the teardown cascade are
board-proven (rev 3). That verdict is recorded by the suite's ledger row in
the board-suite README and by this record, deliberately with no
`VERDICTS.json` entry: the suite's latest revision ended at a precondition,
and the latest-revision-only ingest convention yields nothing ingestible
from it. Its **re-sit requirement is retired** (bead
`dpaa2-controlplane-275`): no further sitting is required of it on the restool
transport, because the witness it was staged to land is unreachable there.

## Revisit triggers

The expected remedy is a second MC transport, not a change to the rollback
machinery.

1. **The MC-portal ioctl transport joining `dpaa2-hal`** (roadmap
   `mc-portal-backend`, ADR-0004 §8, the ADR-0018 southbound seam). A second
   portal path may admit a concurrent drain without the `open()` collision of
   the single-opener rule. When it lands, re-derive the opener semantics under
   the new transport and re-stage the V-ROLLBACK-1 witness.
2. **A kernel change relaxing the single-opener rule on `/dev/dprc.N`**, which
   would make shape (a) orderable on the restool transport directly.
3. **Any decisive ordering mechanism for shape (b)** — a way to place the
   kernel probe's allocator draw inside the run window — which would make the
   kernel-probe drainer a sound witness.

## Consequences

- The one-rollback-per-drain bound for a *hardware-side* drain (a drain of the
  MC-global ledger that the per-container capacity face cannot see, ADR-0023
  Consequences) stays **live-unwitnessed** on the restool transport. The
  offline re-admission behavior and the global-row machinery are proven; only
  the mid-run live landing of that bound awaits a revisit trigger.
- Bead `dpaa2-controlplane-b4c` (the `admits_create` ceiling-versus-live-free
  conflation) is adjacent but separate from this deferral; the capacity-census
  delta (`plan-rollback` task 12.2) resolves it: a live free count is
  headroom, never a population bracket.

## References

- openspec change `plan-rollback` (the rollback machinery and its oracles);
  openspec change `resource-aware-orchestration` (the capacity face and
  admission)
- ADR-0023 (orchestration as a guarded-action convergence model; the
  overcommit concept and the census-scoped bound)
- ADR-0006 (single initiating writer; the single-opener rule V-CONC-1
  witnesses), ADR-0018 (the sans-io southbound seam), ADR-0004 §8 (the
  ioctl-portal route, roadmap `mc-portal-backend`)
- bead `dpaa2-controlplane-f59` (the shim's bounded portal-open retry that
  makes the collision benign), bead `dpaa2-controlplane-275` (the retired
  V-ROLLBACK-1 re-sit requirement), bead `dpaa2-controlplane-3qv` (the shape
  question this record settles), bead `dpaa2-controlplane-b4c` (the adjacent
  ceiling conflation, resolved by the `plan-rollback` capacity-census delta)
- suites V-ROLLBACK-1, V-ADMIT-1, V-CEIL-1, V-DPRC-7, V-DPCON-2
