# dpdmai baseline

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

The DPDMAI fronts the LX2160A's **qDMA engine** (in the firmware line
since MC 10.7.0, queue count scaled to core count — 16 on LX2160 — since
10.8.0; DPAA2UM rev 53 Table 2-1 lists LX2160 among DPDMAI platforms).
Tier C question answered up front: **board-exercisable, yes,
high confidence** — creation is not DPC-gated (no LX2160A DPC mentions
qdma; absence from the reference DPC is expected, dpdmai is a
DPL/runtime-created object, not a DPC resource), the root container
allows OBJ_CREATE, and the kernel ships `fsl-dpaa2-qdma` as a module
that binds `obj_type "dpdmai"` unconditionally after a rescan [read].

## Command surface

restool v2.4: 4 verbs (`dpdmai_commands.c:585-603`) [read]. `create`
takes `--priorities=p1,p2` (exactly 2 values, each 1–8; defaults `1,2`),
`--num-queues` (1–16 client-side; the real bound is "number of platform
cores"), `--options` (a single bit: `DPDMAI_OPT_CG_PER_PRIORITY`, used
by nothing in any corpus), `--container`. `info` prints id, priority
count, queue count, decoded options. restool binds its dpdmai table only
when MC reports object version **3** — any other major yields "no
commands" rather than a version-mismatch message [read].

Runtime API (enable/disable/reset, per-queue rx binding, congestion
notification — added 10.26.0) exists only in the MC flib; no restool
verb [read].

## Option inventory: used vs available

**Used by ls-main: nothing.** The corpus recipe is DPDK's
`dynamic_dpl.sh`: create with `--num-queues=$N --priorities=1,1`, then
`dprc assign --plugged=1` into the consumer container (defaults: 64
objects, 1 queue each). Available-unused: `--options` (zero users
anywhere — no script, no DPL, no driver), the congestion-notification
API (no restool verb, no kernel/DPDK caller) [read].

## Attribute mutability

All attributes (`num_of_priorities`, `num_of_queues`, `options`) are
**create-time only** — no set-attributes exists; resizing means
destroy + re-create. Runtime-mutable state lives below the attribute
level (per-queue destination/user-ctx, enable state, congestion config)
and is consumer-side only [read].

## MC API notes

The dpdmai flib is **byte-identical from MC 10.32.0 through 10.39.0**
(v3 CREATE/GET_ATTR in place since ≤ 10.32; only the header-layout move)
[read, mc-utils diff]. The firmware moved inside the window
(`qoriq-mc-binary/CHANGELOG.md`): **10.38.1** fixed the **DPL parser** to
"process the 'num_queues' and 'options' fields from the DPL — without
this fix, the requested parameters ... would not be actually taken into
consideration". Scoping matters: the bug was DPL-path only — the
`DPDMAI_CREATE` command path (restool/DPDK/kernel) always carried both
fields — so on pre-10.38.1 firmware a DPL-declared dpdmai silently got
MC defaults while a restool-created one got what was asked. The pinned
10.39.0 is post-fix. Nothing at 10.39/10.40 [read, MC changelog].

## Kernel-side behavior (Linux 6.6.52)

**Driver-bound, not allocatable-pool**: `fsl-dpaa2-qdma` (=m, built)
matches `obj_type "dpdmai"` with no version filter; probe needs a free
**dpmcp** (portal) and **dpio**s in the same container, else silent
`-EPROBE_DEFER`. On success it registers a dmaengine device
(DMA_MEMCPY/SLAVE) — **no userspace node**; exercising it needs an
in-kernel client or `dmatest` [read].

Two structural quirks worth carrying:
- The driver **sizes its queue walk from the priority count**
  (`num_pairs = min(num_of_priorities, 2)`) and then indexes
  `queue_idx = 0..num_pairs-1`, while `num_of_queues` is parsed and
  never used — a `--num-queues=1 --priorities=1,2` object makes it
  request queue_idx 1 on a one-queue object (outcome unknown, board
  item). Conversely queues beyond 2 are unreachable via dmaengine —
  only DPDK's dmadev honors the full queue count
  (`max_vchans = num_of_queues`) [read].
- `dpaa2_qdma_shutdown` calls `dpdmai_destroy` **after closing the
  token, passing the wrong token type** — either it errors silently on
  every shutdown/kexec, or it removes a provisioned object across a
  reboot cycle [read].

The kernel speaks V2 CREATE/GET_ATTR against the V3-capable MC — layout-
compatible, but the kernel can never see the `options` attribute word
[read].

## Lifecycle ordering and dependencies

create (parent-dprc token; container needs OBJ_CREATE) → optionally
`dprc assign --plugged=1` into the consumer container → **bus rescan**
(create does not trigger one) → kernel probe (dpmcp + dpio present) or
VFIO scan for DPDK. Destroy: refused while a kernel driver is bound
("unbind it first"); sent on the parent token; MC requires all object
tokens closed [read].

## Intent mapping

The **DMA-offload construct**: derived only when an intent names a qdma
consumer, sized `num_queues = min(consumer queue demand, cores)` and
`priorities=1,2`. For the kernel consumer the safe shape is
`--num-queues=2 --priorities=1,2` (both axes ≥ the driver's walk); for
DPDK, queue count is the real capacity lever. Not emitted by default —
the VPP port series has no qdma consumer today.

## Silent-failure notes

- **`--num-queues` omitted sends 0 on the wire**; what MC substitutes
  (1, 2, core count?) is undocumented in every corpus — the effective
  default of the most important sizing knob is unknown [read].
- **generate-dpl emits a wrong dpdmai node**: `priorities` gets the
  priority *count*, not the two values (hand-written DPLs say
  `<2 5>`), and `num_queues`/`options` are omitted — a round-trip both
  mangles and drops config [read].
- **ls-append-dpl breaks on real dpdmai DPL nodes twice over**: a
  multi-cell `priorities = <2 5>` is spliced space-separated into
  `--priorities=2 5` (restool wants `p1,p2`; exit status unchecked),
  and any property valued 0 is dropped before reaching restool [read].
- Kernel probe deferral on missing dpmcp/dpio is the same silent
  infinite-defer loop as the pool families [read].
- Kernel queue walk vs queue count mismatch and shutdown-destroy quirk
  (above) [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPDMAI-I1 | Create-frozen sizing: priorities/queues/options are immutable; reconciler repairs drift only by destroy+recreate (which unbinds the consumer) | attr readback vs desired | candidate |
| DPDMAI-I2 | **Breaking:** the model must NOT assume the DPL path equals the create path — on pre-10.38.1 firmware DPL num_queues/options were silently defaulted while CREATE honored them; version-gate any DPL equivalence claim | firmware version vs DPL-created attr readback | candidate (corpus-proven for the window) |
| DPDMAI-I3 | Consumer-shape coupling: a kernel-consumable dpdmai needs num_queues ≥ min(num_priorities, 2) — the driver walks queue_idx sized by the *priority* count | probe outcome of a 1-queue/2-priority object | board-pending |
| DPDMAI-I4 | **Breaking:** the model must NOT assume generate-dpl round-trips config — priorities value→count mangling plus dropped fields (worse than dpdcei/dpaiop: actively wrong, not just lossy) | regenerated node vs create args | verified 2026-08-29 (V-GENDPL-1 rev 1): `--priorities=2,4` came back as `priorities = <0x2>` — the count in the list's place |
| DPDMAI-I5 | Unknown-default hazard: create without `--num-queues` yields an MC-chosen count no corpus documents; the model must treat it as unspecified, never as 1 | `info` after a bare create | verified 2026-08-29 (V-READBACK-1 rev 2, hook 10/10): the corrected hook confirms MC 10.39.0 chooses 1 queue and 2 priorities — the model keeps it unspecified, the number is now known |

## Unknown / unverified register

1. ~~What `num_queues = 0` yields on MC 10.39.0 (decides DPDMAI-I5).~~ —
   resolved: 1 queue, 2 priorities, API 3.4 [board-observed 2026-08-25, V-READBACK-1].
2. Kernel probe outcome for `num_of_queues < num_of_priorities`
   (decides DPDMAI-I3): loud MC error at get_rx_queue, or bogus FQID
   failing at first transfer?
3. The MC-enforced queue ceiling on this SoC (changelog says core count
   = 16; restool clamps 16 client-side; unconfirmed).
4. Whether `dpaa2_qdma_shutdown`'s wrong-token destroy ever succeeds —
   i.e. does a created dpdmai persist across kernel shutdown? **Half
   answered** [V-DPDMAI-2 rev 1, 2026-08-29]: the *persistence* half is
   settled — a bare, unplugged root dpdmai is absent after a reboot
   (recovery diff clean at the 97-object reference), so a runtime dpdmai
   does not outlive a reboot. The *shutdown-path* half is unanswerable on
   this BSP: no qdma driver ever binds a dpdmai (V-LIFE-DPDMAI-1,
   ADR-0008), so `dpaa2_qdma_shutdown` never runs and the wrong-token
   destroy is never exercised.
5. Whether `DPDMAI_OPT_CG_PER_PRIORITY` is accepted and reflected in
   `info` (exercised by zero code anywhere).
6. Free-dpmcp headroom in the root container for a qdma probe after
   existing drivers claim theirs.
