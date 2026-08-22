# dpcon baseline

<!-- Instantiated from _template.md; every section mandatory, empty sections
     state so explicitly (spec: object-baseline). -->

Claim markers, used per claim throughout: **[read]** = derived from source or
manual, not yet observed; **[verified]** = observed on the board against the
pinned reference environment (see `reference-environment.md`).

Findings are written to be provable: behavioral claims name their
observables and are distilled into the Invariant candidates section as
propositions a Quint model can carry — invariant-bearing or
invariant-breaking.

Cross-family relationships are mapped in [object-model.md](object-model.md);
the board scenarios that will settle this document's open items are
classified in [traffic-inventory.md](traffic-inventory.md).

The DPCON is a QBMan concentrator channel: it aggregates frame queues so
one consumer (a dpni queue's servicing core) dequeues them through one
channel with priority levels. It is an allocatable pool object (shared
mechanics: `dpbp.md`); the kernel eth driver draws **one dpcon per
channel** — the C1 resource that, when missing, produced the original
"No more resources of type dpcon left" probe failure (ADR-0001)
[verified].

## Command surface

restool v2.4: 4 verbs (`dpcon_commands.c:475-493`) [read]. `info` prints
id, version, plugged state, **`qbman channel id`** (the id used by
dequeue operations — distinct from the object id), num-priorities,
label.

## Option inventory: used vs available

One create option: `--num-priorities` (1–8, **default 2** — unlike
dpio's default of 8), plus `--container` [read]. Used by: ls-addni
(`--num-priorities=2`, `min(num_queues, ncores)` dpcons per dpni);
our dprc-script mirrors the recipe for kernel-linked dpnis [verified].
ls-addsw/ls-addmux create none (their consumers use ANY-CPU FQDANs, no
concentrator) [read].

Gap in the vendor recipe [read]: ls-addni's dpcon count follows
`--num-queues` and **ignores `--num-channels`** — a multi-channel dpni
is silently under-provisioned; and a mid-loop dpcon create failure just
stops the loop and proceeds with fewer.

## Attribute mutability

`num_priorities` is the only create field, immutable. Runtime:
`dpcon_set_notification` (re)targets the channel at a DPIO with a
priority and user context; `DPCON_INVALID_DPIO_ID` (-1) disables
notifications. The priority ceiling in that call is the **DPIO's**
priority count, not the dpcon's own [read].

## MC API notes

The dpcon flib is **byte-identical between MC 10.32.0 and 10.39.0** —
no delta for the pinned pair [read, mc-utils diff].

## Kernel-side behavior (Linux 6.6.52)

- Sole in-tree pool consumer: dpaa2-eth, one per channel =
  `min(CPUs with affine dpio, dpni.num_queues)`. Lifecycle:
  allocate → open → **reset** → enable → get_attributes; free:
  disable → close → object_free — **no reset on release** (dirty
  objects circulate; DPBP-I3's pattern) [read].
- The **qbman channel id** (`qbman_ch_id`), not the object id, goes
  into the notification context and CDAN setup — two id spaces, same
  trap as dpbp's bpid [read].
- dpaa2-eth always registers the io service **before**
  `dpcon_set_notification` (the documented ordering contract) and
  always uses **priority 0** — the created priority levels (2 by
  recipe) are unused capacity in the kernel path [read].
- Exhaustion: first dpcon missing → silent `-EPROBE_DEFER`; shortage
  *after* the first → "Not enough DPCONs, will go on as-is" and a
  degraded probe with dark queues (dpni.md DPNI-I5) [read].

## Lifecycle ordering and dependencies

Create + plug before the consumer dpni (C1) [verified]; count = one per
polled queue per consumer. The DPDK/VPP child container follows the
same one-per-queue shape via its own claim path [verified in use].
Teardown returns to pool without reset; `dpcon destroy` requires
unbinding from `fsl_mc_allocator` first [read].

## Intent mapping

Derived companion of dpni sizing: `#dpcon = min(num_queues, consumer
cores)` per dpni (kernel regime), one per polled queue (VPP regime).
`--num-priorities=2` is the deployed constant; nothing in the corpus
consumes priority > 0, so the intent layer treats priorities as an
opaque vendor default until a consumer needs them [read/verified].

## Silent-failure notes

- Post-first-dpcon exhaustion degrades the consumer quietly (dark
  queues, info-level log) — the strongest pool-family instance of
  probe-success ≠ full-function [read].
- ls-addni proceeds after a mid-loop dpcon create failure [read].
- No reset on pool return [read].
- restool `destroy` error overwritten by close in child containers
  (family-wide) [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPCON-I1 | Companion cardinality: a kernel-bound dpni consumes exactly `min(affine-dpio CPUs, num_queues)` dpcons; consumer probe outcome is monotone in pool free-count with threshold 1 (defer below, degrade between 1 and full, complete at full) | pool free-count vs probe result vs channel count | verified (C1 + shortfall path [read]) |
| DPCON-I2 | Two id spaces: dequeue/notification addressing uses `qbman_ch_id`, never the object id; the model must carry both per dpcon | `dpcon info` channel id vs object id | candidate |
| DPCON-I3 | **Breaking:** the model must NOT assume created priority capacity is used — every in-corpus consumer drives priority 0 only; priorities are create-immutable dead weight until proven otherwise | CDAN configs; consumer source | candidate |
| DPCON-I4 | Notification retargeting is the one mutable edge: dpcon→dpio binding can change at runtime (set_notification), and disabling is expressed as dpio id −1, not a separate state | `set_notification` sequences + dequeue behavior | candidate |
| DPCON-I5 | **Breaking:** pool membership ⇒ clean state is false (no reset on free) — shared with DPBP-I3 | re-allocated dpcon state before consumer reset | candidate |

## Unknown / unverified register

1. What MC does with priority values 1–7 on dequeue when the consumer
   never configures them (scheduling semantics unobserved).
2. Whether `dpcon_reset` clears a live notification target.
3. The DPDK child-container dpcon claim path's exact count expression
   (behaviorally one-per-queue [verified]; source outside corpus).
