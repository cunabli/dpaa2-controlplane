# ADR-0012: Companion objects are sized by the consumer's regime and thread count, not by port count

- **Status:** Accepted — verified on the board during the consumer
  bring-up that preceded this series; the ledger carries it as DPIO-I2,
  DPBP-I6 and DPNI-I10, all `verified`, and the family documents cited
  this record before it was written down here
- **Date:** 2026-08-30
- **Supersedes / relates to:** ADR-0001 (intent is reified into
  objects; companions are the compiler's to derive); ADR-0011 (pool
  ceilings the census can see);
  `docs/baseline/dpio.md`, `docs/baseline/dpbp.md`,
  `docs/baseline/dpmcp.md`, `docs/baseline/dpni.md`,
  `docs/baseline/object-model.md` §4 (allocation pools: sizing couplings)

## Context

A dpni does not work alone. It draws companion objects from the
container's pools: dpios to reach the hardware queues, dpbps to hold
buffers, dpcons to be told about traffic, dpmcps to talk to the
firmware. The shipped `ls-addni` script counts those companions by
port: one dpbp per dpni, one dpmcp per dpio and per dpni, dpios once
per CPU. That count is correct for the regime the script was written
for — the kernel's own network driver — and silently wrong for the
other regime this control plane must serve: a poll-mode userspace
consumer that runs on the DPDK bus inside a child container.

The two regimes use the same object families differently. The kernel
binds every dpio to its own driver and offers one shared portal
service per CPU; any kernel consumer borrows from it. A poll-mode
consumer takes portals exclusively, two per thread, and keeps them for
its lifetime. The same is true of buffer pools: the kernel driver draws
one per interface, the poll-mode consumer holds a fixed pair for the
whole process. A control plane that counts by port gets the poll-mode
container wrong, and the failure does not surface at create time — it
surfaces mid-run, looking like something else.

## What the board answered

- **dpio.** A kernel container accepts at most one dpio per online CPU;
  the next one is refused with `-ERANGE`. A poll-mode child container
  needs exactly **2 × T**, where T is the consumer's thread count
  (main plus workers): the bus keeps a general portal and a receive
  portal per thread, each an exclusive dpio. One short, and the
  consumer runs out of portals in the middle of bringing up its
  queues, with errors that read as buffer or silicon faults rather than
  as a count. The board runs 16 in the kernel container for 16 cores
  and 10 in the child for T = 5.
- **dpbp.** A poll-mode child container needs exactly **2**, independent
  of how many ports it drives: one for the consumer's own buffer pool,
  one for the driver's scatter-gather pool. Fewer fails buffer setup;
  more is waste the census will later miss (ADR-0011: dpbps are the one
  family whose pool the firmware counts to the object).
- **dpni transmit queues.** A consumer driving transmit from T threads
  needs **at least T** transmit queues on the dpni. A queue shared by
  two threads drops enqueues silently: the firmware raises no error and
  the dpni exposes no counter for it; only the consumer's own
  interface-side drop counter moves. This is a floor, not a tunable —
  it cannot be rationed down to save buffer memory.
- **dpmcp.** Every portal-consuming object draws one, *including each
  dpio* — the draw the port-count model forgets — and the management
  plane itself needs headroom for concurrent openers
  (`docs/baseline/dpmcp.md`).

## Decision

Companion counts are derived quantities of the consumer's declared
regime and thread count. The operator states the regime and T in the
intent; the intent compiler emits the companion set; the operator
never states a dpio, dpbp or transmit-queue number directly.

- Kernel regime: dpios are a container-wide per-CPU service, at most one
  per online CPU; dpbps and dpmcps one per consuming object.
- Poll-mode regime, per child container: dpio = 2 × T, dpbp = 2,
  dpni transmit queues ≥ T, dpmcp = one per portal-consuming object
  including each dpio, plus headroom.

When the container's pools cannot back the derived set, the reconciler
refuses the intent and names the shortfall. It does not ration a
count down to fit: every one of the three numbers above fails late and
quietly when short, so a "best effort" plan is a plan to fail on the
data path.

## Consequences

- The sizing paragraphs of `dpio.md`, `dpbp.md`, `dpmcp.md` and
  `dpni.md`, and §4 of `object-model.md`, cite
  this record for the numbers; the numbers live here once.
- DPIO-I2, DPBP-I6 and DPNI-I10 keep their ledger disposition — deferred
  to `pool-objects` (#6) and `dpni-typestate` (#5) as typestate work —
  with this record as the law those changes encode. The regime becomes
  a type the compiler carries, not a comment beside a constant.
- The `ls-addni` count is recorded as an upstream finding
  (`docs/upstream/findings.md`, ls-* scripts): correct for one regime,
  unannounced as such.

## Open questions and revisit triggers

1. The board's poll-mode child carries **3 dpmcps** as a script
   constant, not a derived count. Deriving it from the rule above
   (portal consumers + headroom) may give a different number; the first
   `pool-objects` suite that creates a poll-mode container from intent
   settles which is right.
2. The 2 × T portal shape is board-observed, not source-cited from the
   consumer's bus code in this corpus (`dpio.md`, unknown #3). If a
   consumer ships a different portal model, the regime table gains a
   row; the decision that counts derive from regime and T stands.
3. Whether a non-DPDK poll-mode consumer shares the dpbp pair and the
   2 × T shape is unverified. Until one exists, "poll-mode" means the
   DPDK bus.
