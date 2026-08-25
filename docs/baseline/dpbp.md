# dpbp baseline

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

The DPBP is a QBMan buffer pool. It is the simplest object family — no
create options at all — and the archetype of the **allocatable pool
object**: consumers (dpni via `fsl_dpaa2_eth` or the DPDK PMD, dpsw)
*allocate* dpbps from their container's pool rather than creating them
(ADR-0001 C1). This doc also hosts the shared kernel pool-allocator
mechanics referenced by `dpio.md`, `dpcon.md`, and `dpmcp.md`.

## Command surface

restool v2.4: 4 verbs (`dpbp_commands.c:440-458`) [read]. `info` prints
id, API version, plugged state, **`buffer pool id` (bpid)**, label.
`create` takes only `--container`; `destroy` refuses driver-bound
objects — and since scanned dpbps auto-bind to `fsl_mc_allocator`, a
plugged dpbp must be unbound before restool can destroy it [read].

The MC object id and the **bpid are distinct id spaces**: acquire/release
traffic addresses the bpid through the QBMan portal, bypassing MC
entirely [read].

## Option inventory: used vs available

`dpbp_cfg.options` is a documented **placeholder**: restool never sets it
and the flib explicitly discards it (`(void)(cfg)`, `mc_v10/dpbp.c:121`)
— the family has zero create-time options [read]. Used by: ls-addni
(1 dpbp per dpni), ls-addsw (conditionally — with an **inverted
condition**, silent-failure notes), ls-append-dpl (generic); our
dprc-script creates `NDPBP=2` in the VPP child: one for the VPP buffer
pool, one for the PMD's scatter-gather pool [verified, ADR-0012].

## Attribute mutability

Nothing is configurable at create or runtime through this family's MC
API (open/close/enable/disable/reset/get_attributes). The bpid is
MC-assigned and immutable. Pool *content* (buffers) is data-path state
owned by whoever seeds it [read].

## MC API notes

The dpbp flib is **byte-identical between MC 10.32.0 and 10.39.0** —
no wire delta for the pinned pair [read, mc-utils diff]. The firmware
behind it did move, though (`qoriq-mc-binary/CHANGELOG.md`): 10.33
made the hardware depletion threshold (HWDET) user-configurable, and
10.34 **changed the default depletion thresholds on LX2160A** to
achieve a lossless 10G link — identical create parameters yield
different depletion behavior across the window, on exactly our
platform [read, MC changelog].

## Kernel-side behavior (Linux 6.6.52)

**Shared pool-allocator mechanics** (apply to dpbp/dpcon/dpmcp; dpio is
NOT allocatable — see `dpio.md`) [read, `fsl-mc-allocator.c`]:

- Pool types are exactly {dpmcp, dpbp, dpcon, irq}. Objects enter their
  container's pool when the `fsl_mc_allocator` driver binds them —
  which requires the object to be **plugged**; an unplugged pool object
  is visible in `dprc show` but invisible to the allocator (the
  looks-converged-isn't trap behind DPRC-I2/I8).
- The DPRC scanner probes all allocatables before any consumer, by
  design (ADR-0006 fold, DPRC-I8).
- Exhaustion: `-ENXIO` + `"No more resources of type dpbp left"`;
  consumers convert it to a **silent** `-EPROBE_DEFER` retry loop.
- `fsl_mc_object_free` issues **no MC command** — it is Linux-side
  bookkeeping only; double-free and cross-pool free are silently
  no-oped.
- Pool entry preconditions fail silently (`-EINVAL`, no log); removal
  of an in-use resource cannot be refused (void remove callback) — the
  `-EBUSY` is discarded and the resource goes stale.

**dpbp-specific**: consumers are dpaa2-eth (1 at probe + 1 per AF_XDP
channel, max 9) and dpaa2-switch (1). Allocate side runs
`open → reset → enable → get_attributes`; free side runs
`drain → disable → close → object_free` — **no reset on the way out**,
so a dirty dpbp circulates through the pool and is cleaned only by the
*next* allocator's reset. A consumer that dies between enable and
disable leaves the dpbp enabled with stale buffers and the pool none
the wiser [read]. `dpaa2_io_service_acquire` returning 0 buffers is a
legitimate empty-pool result, not an error [read].

Two in-tree bugs worth carrying: dpaa2-switch uses the **object id as
the bpid** (dpaa2-eth correctly uses `.bpid`) — numerically identical
on many boards, wrong in general; and `dpaa2_eth_free_dpbps` leaks
every second dpbp when more than one is held (compact-while-iterating)
[read].

## Lifecycle ordering and dependencies

Create + plug **before** any consumer probes (C1/DPRC-I8) [verified].
One dpbp per kernel-bound dpni/dpsw is drawn automatically; the DPDK
path draws none from the kernel pool — the child container's dpbps are
VFIO-owned and claimed by the PMD [verified]. Teardown: consumer frees
→ pool return (no MC reset) → unbind from `fsl_mc_allocator` →
`dpbp destroy`.

## Intent mapping

Sizing rule (ADR-0012 [verified]): **two dpbps per VPP child container**
(VPP pool + PMD SG pool), regardless of port count; kernel-side
consumers draw one each from the parent pool. The intent compiler emits
dpbps as derived companions of the consumer set, never as user-facing
objects.

## Silent-failure notes

- ls-addsw's dpbp condition is inverted: it *skips* the dpbp exactly
  when the switch control interface needs one; the dpsw then never
  probes, with no error anywhere (`-ENXIO` → silent defer) [read].
- Empty-pool exhaustion is a silent infinite defer loop for every
  consumer; the only loud form is dpaa2-eth's non-ENXIO
  "Not enough DPCONs"-style info line (dpcon only) [read].
- No reset on pool return (above) — pool membership does not imply
  clean state [read].
- restool `info` prints "dpbp version: 0.0" before checking the version
  query error; `destroy` in a child container overwrites the destroy
  error with the close result (family-wide pattern) [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPBP-I1 | Zero configuration surface: two dpbps are interchangeable at create; identity that matters downstream is the MC-assigned bpid, distinct from the object id | `dpbp info` bpid vs id | candidate |
| DPBP-I2 | Pool-entry precondition: allocatable ∈ kernel pool ⟺ plugged ∧ allocator-bound; unplugged objects are dprc-visible but allocator-invisible | `dprc show` plugged column vs driver symlink vs consumer probe outcome | candidate |
| DPBP-I3 | **Breaking:** the model must NOT assume pool membership ⇒ clean object state — free does no MC reset; cleanliness is established by the next allocator's reset, or never (non-kernel consumers) | buffer count of a re-allocated dpbp before its consumer resets it | candidate |
| DPBP-I4 | Exhaustion liveness: consumer probe blocks (silent defer) exactly while `free_count = 0`; adding + plugging one dpbp unblocks it | dmesg defer loop; probe completes after top-up | verified (C1 class, ADR-0001) |
| DPBP-I5 | **Breaking:** the model must NOT equate bpid and object id (an in-tree consumer does, latently wrong) | `dpbp info` on a board where the two diverge | candidate |
| DPBP-I6 | Two-dpbp rule: a VPP child container needs exactly 2 dpbps (VPP pool + PMD SG); fewer fails buffer setup, more is waste | VPP "HW buffer pool" log lines; PMD probe | verified (ADR-0012) |

## Unknown / unverified register

1. What `dpbp_reset` actually clears — config only, or does it drain
   buffers? (Flib says nothing; feeds DPBP-I3's board check.)
2. Whether bpid can ever diverge from object id on this board (decides
   DPBP-I5's observability here, and the dpaa2-switch bug's reach) — a
   runtime dpbp read back `buffer pool id` equal to its object id (1 = 1)
   [board-observed 2026-08-25, V-READBACK-1]; no divergence observed, so the law stays a prohibition the
   board cannot yet falsify.
3. The placeholder `dpbp_cfg.options` — reserved by MC or truly dead?
