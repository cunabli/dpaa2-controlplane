# phylink NULL-PCS Oops and refcount underflow under dpni create/destroy churn

Finding 49 of `docs/upstream/findings.md`; the full control-plane
diagnosis lives on bead dpaa2-controlplane-960.12 and ADR-0008 §9.
This file is the self-standing description for a later kernel fix
attempt.

## Environment

- LX2160A-class board (DT), Linux 6.6.52 LF tree, MC firmware
  10.39.0, restool v2.4.
- Wired 10G port: dpmac.7, PHY-typed, `inband/10gbase-r`, terminated
  by a kernel dpni bound to `fsl_dpaa2_eth`; the vendor standalone
  `fsl_dpaa2_mac` driver holds the dpmac when no dpni is bound.

## Reproduction

A management-tool loop repeatedly created a dpni, connected it to the
wired dpmac, then disconnected and destroyed it, at roughly 1.3 s per
cycle (a control-plane reconcile defect made the loop unbounded; the
churn itself is the trigger, and any restool script can produce the
same sequence). Each cycle logged:

```
fsl_dpaa2_eth dpni.2: Adding to iommu group 9
qbman: wqchan config failed, no response        <- probe fails, cycle repeats
```

with earlier cycles also showing the attach ping-pong between the two
drivers:

```
phy phy-1ea0000.phy.3: phy_power_on was called before phy_init
fsl_dpaa2_mac dpmac.7: Error in attaching the fsl_dpaa2_mac driver
```

After ~3 cycles the kernel Oopsed, and ~1.7 s later warned a refcount
underflow and tainted; the board needed a power cycle. Reproduced on
both boots of the same sitting.

## Observed traces

Oops — NULL-ish dereference at offset 0x29 inside
`phylink_mac_pcs_get_state`, on the phylink resolve worker:

```
Unable to handle kernel NULL pointer dereference at virtual address 0000000000000029
Internal error: Oops: 0000000096000004 [#1] PREEMPT SMP
CPU: 15 PID: 101 Comm: kworker/u40:0 Not tainted 6.6.52 [board tree]
Workqueue: events_power_efficient phylink_resolve
pc : phylink_mac_pcs_get_state+0x5c/0xac
lr : phylink_resolve+0x398/0x604
Call trace:
 phylink_mac_pcs_get_state+0x5c/0xac
 phylink_resolve+0x398/0x604
 process_one_work+0x138/0x25c
```

Follow-up (next churn cycle, same boot):

```
refcount_t: underflow; use-after-free.
WARNING: CPU: 1 PID: 216 at lib/refcount.c:28 refcount_warn_saturate+0xf4/0x144
```

Full transcript: `results/V-MVP-1-rev1/console-transcript.txt`
(operator console paste; the suite's own dmesg capture is absent — the
run was killed mid-loop before its trap completed).

## Analysis pointers

- `phylink_resolve` runs on the `events_power_efficient` workqueue and
  calls `phylink_mac_pcs_get_state`, which dereferences the attached
  PCS. The dpaa2-mac connect/disconnect path creates and destroys the
  Lynx PCS per attach; a teardown that destroys the PCS without
  cancelling or flushing the queued resolve worker lets the worker
  dereference freed or cleared PCS state. The faulting offset (0x29)
  sits inside the first words of the PCS/ops structure.
- The per-cycle `phy_power_on was called before phy_init` and the
  standalone-driver attach failure show the `fsl_dpaa2_eth` ↔
  `fsl_dpaa2_mac` driver ping-pong on the dpmac
  (`dpaa2_eth_connect_mac` → `dpaa2_mac_driver_detach`, remove path →
  `dpaa2_mac_driver_attach`; see finding 34 and ADR-0008 §8) racing
  each churn cycle.
- The refcount underflow one cycle later is consistent with a
  double-put on a phy/PCS/netdev reference in the same churned
  teardown path.
- Related prior finding 22: the standalone dpmac driver installs its
  MC interrupt handler with no phylink guard — a link event on a
  half-torn-down dpmac dereferences NULL.
- The `qbman: wqchan config failed, no response` probe failure is a
  distinct symptom (it followed an ADR-0008 §4 rescan-race disruption
  of the boot dpcon) that kept each cycle short; failing probes widen
  the race window but are not required — the phylink race exists for
  any fast create/connect/disconnect/destroy sequence on a wired port.

## Status

Candidate. `phylink_resolve` vs PCS-teardown lifetime is potentially
upstream-relevant (phylink core plus `dpaa2-mac`); the standalone
`fsl_dpaa2_mac` driver is vendor-tree-only. The control plane now
refuses to churn (same-run destroy-then-create is a typed refusal), so
the trigger is fenced tool-side regardless of the kernel fix.

## Local fix attempt (not part of the upstream report)

Kernel sources for the fix attempt live in the workspace's sibling
build tree, `../../.build/src/linux/` relative to this repo's root.
Candidate files:
`drivers/net/phy/phylink.c` (resolve worker vs PCS lifetime),
`drivers/net/ethernet/freescale/dpaa2/dpaa2-mac.c` (PCS
create/destroy, attach/detach ordering), `dpaa2-eth.c` (connect/
disconnect endpoints), and the vendor standalone dpmac driver.
Exploration is deferred to a separate session (2026-09-25 direction);
reproduction needs no special tooling — a restool loop of
create/connect/disconnect/destroy on a wired dpmac at ~1 s cadence.

It also takes finding 50 (`drivers/bus/fsl-mc/fsl-mc-allocator.c` —
the in-use guard in `fsl_mc_resource_pool_remove_device` cannot be
enforced from a `void` remove callback, so an in-use allocatable
object unbinds anyway and the pool bookkeeping goes inconsistent).
Minimal fix shape: check the free list before `device_release_driver`
reaches the allocator, or give the allocator a removal that can fail.
Landing it re-opens root-scope reclaim for the control plane
(ADR-0020) and is the highest-value of the three for this tool.

The same session also takes finding 39 (`drivers/bus/fsl-mc/
dprc-driver.c` — `dprc_scan_objects` reads descriptors per index with
nothing holding the firmware still, and a stale plugged bit reaches
`device_release_driver` unverified and unlogged; ADR-0008 §4, four
firings observed, two on add bursts). Minimal fix shape: re-read the
descriptor before releasing a bound driver on a plugged→unplugged
transition, so a torn read becomes a no-op. Decided 2026-09-25
(ADR-0008 §9): the control plane does not pace around it — the kernel
fix is the mitigation.

## Single fire without churn (2026-09-27, V-MVP-1 rev 5)

The churn framing above is sufficient but not necessary. One ordered
teardown of a single bound dpni — disconnect while bound, sysfs
unbind of `fsl_dpaa2_eth`, destroy (the ADR-0008 §8 order) — lost
the same race once: `dpmac.7: Error in attaching the fsl_dpaa2_mac
driver` as the standalone mac driver re-attached while the dpni side
went away, then one second later a data-abort Oops on the
`events_power_efficient` `phylink_resolve` worker
(`phylink_resolve → dpaa2_mac_link_up → mc_send_command+0x78`): the
worker drove an MC command against a mac whose backing state was
already torn down. The kworker exited with irqs disabled and
preempt_count 1, the console carried the taint trace, and the board
otherwise kept serving — the suite completed every read-back after
the Oops. The window is therefore per-transition, not per-rate:
pacing cannot close it, which confirms the ADR-0008 §9 decision that
the kernel fix, not control-plane throttling, is the mitigation.
Evidence: the V-MVP-1 rev 5 sitting capture (dmesg: attach error at
116.1 s, Oops at 117.2 s; board power-cycled after the sitting).

## Second signature in the same window (2026-10-02, V-DPMAC-3 rev 1)

The per-transition window can also fail milder and elsewhere: one
ordered sever-then-unbind teardown logged the usual ping-pong pair
(attach error at 119.3 s), then at 120.1 s a `WARNING` at
`kernel/time/timer.c:1738` in `__run_timers` on an idle CPU's timer
softirq with `x22 = dead000000000122` (`LIST_POISON2`) — a detached
timer reached from the expiry list. No Oops, no refcount underflow,
no taint, no kworker death; the suite completed and a plain reboot
recovered. The owning timer is unidentified from the trace, so this
is recorded as finding 52, a sibling signature of the same
dpaa2-mac/phylink teardown lifetime family, not an amendment to the
PCS mechanism above. Evidence:
`~/dpaa2-board-evidence/V-DPMAC-3-rev1/dmesg.txt`.

## Pinned trigger window (2026-10-18, V-DPSW-5 rev 5)

The switched-domain suite pins the trigger to one window, caught live
on the serial console. The control plane severs dpni↔dpmac.7; the
post-mutation bus sync invites the kernel to rescan while the dpmac is
endpoint-free, so the standalone `fsl_dpaa2_mac` driver attaches:

```
fsl_dpaa2_mac dpmac.7 mac7: configuring for inband/10gbase-r
```

Roughly 300 ms later the dpsw connect forces that attachment to
unregister, and link transitions land *during* the unregister:

```
fsl_dpaa2_mac dpmac.7 mac7 (unregistering): Link is Down
fsl_dpaa2_mac dpmac.7 mac7 (unregistering): Link is Up
```

An armed phylink-side timer or delayed work outlives the free, and a
panic follows in under a second. The window is the same per-transition
lifetime race the churn and single-fire sections describe, now with a
precise sequence: a wired port left endpoint-free for a few hundred
milliseconds is enough to attach, arm, and then free the standalone
driver underneath its own queued work. Evidence: suite V-DPSW-5 revs
1–5, bead dpaa2-controlplane-dbf.19, console capture under
`results/V-DPSW-5/`.

## Third signature: timer-wheel corruption turns fatal (2026-10-18, V-DPSW-5 revs 1–5)

Finding 52's `kernel/time/timer.c:1738` `__run_timers` WARNING recurs
in this suite (revs 1, 3) and then escalates to fatal Oopses in
`__run_timers` (revs 2, 5): an hlist unlink faults on a garbage `next`
pointer, with `LIST_POISON2` (`0xdead000000000122`) visible in the
registers. One instance wrote to read-only memory (rev 5), another
dereferenced a near-NULL address (rev 2). Each fires on an idle CPU
(`swapper`) in softirq context, so each is a panic, not a recoverable
warning. This confirms finding 52's detached-timer signature as the
same teardown-lifetime family and shows it can be fatal, not only a
WARNING; the owning timer is still unidentified from the traces.
Evidence: suite V-DPSW-5 revs 1–5, bead dpaa2-controlplane-dbf.19,
captures under `results/V-DPSW-5/`.

## Fourth signature: delayed slab-corruption fallout (2026-10-18, V-DPSW-5 rev 3)

One run shows the corruption outliving the teardown by more than a
minute. Rev 3 first took the original workqueue Oops byte-identical to
the trace above (`phylink_mac_pcs_get_state+0x5c/0xac` via
`phylink_resolve`), then logged later, periodic Oopses in
`kmem_cache_free` via `skb_free_head` on netlink paths, ending in a
fatal panic roughly 80 s after teardown. One use-after-free surfaces on
many downstream allocators: the freed object is handed back to the slab
and the corruption is observed only later, when an unrelated netlink
free walks the poisoned cache. Evidence: suite V-DPSW-5 rev 3, bead
dpaa2-controlplane-dbf.19, capture under `results/V-DPSW-5/`.

## Control-plane mitigation: no post-mutation sync on disconnect (2026-10-18, V-DPSW-5)

The restool shim's dprc disconnect verb no longer issues the
post-mutation bus sync. With the sync removed, the kernel is never
pointed at a transiently endpoint-free wired port, so the standalone
`fsl_dpaa2_mac` driver is not invited to attach into the trigger
window above; the next mutating verb's sync carries the disconnected
state instead. This fences the trigger tool-side, as the churn refusal
and the single-fire framing already do for their paths. The kernel
patch remains the real fix — the resolve worker and the armed timer
must be cancelled or flushed before the PCS and the standalone driver
are freed — and this doc is its seed. Evidence: suite V-DPSW-5 revs
1–5, bead dpaa2-controlplane-dbf.19, captures under `results/V-DPSW-5/`.

## Fifth signature: the sync-hold narrows the window but does not close it; device-links refcount underflow (2026-10-22, V-DPSW-5 revs 6–7)

The disconnect sync-hold above narrows the trigger window; it does not
close it. Rev 6 ran the full suite clean on the sync-hold tree — the
first complete run — but rev 7 on the *same tree* fired the step-3
phylink Oops again (`phylink_mac_pcs_get_state+0x5c` via
`phylink_resolve`, byte-identical) at the dpmac.7 handover. A mutating
verb's own sync still carries the transiently endpoint-free state into
the window for a short interval, so the standalone `fsl_dpaa2_mac`
driver can still attach, arm, and be freed under its own queued work.

Rev 7 also produced a new signature: the `deferred_probe` kworker died
with a refcount-underflow use-after-free Oops in
`device_link_drop_managed`, after the child dprc was bound through the
`driver_override` path and its `dpaa2-eth` probe failed qbman channel
setup. A device-links refcount dropped below zero and the managed link
was freed while still referenced.

The candidate structural lever is a plan-order change so the wired
uplink never passes through a standalone `fsl_dpaa2_mac` attach during
the handover — the dpmac stays driver-occupied by the switch across the
rewire rather than momentarily endpoint-free. This is deferred to the
`resource-aware-orchestration` change (#15) driver-occupancy ordering
guard (ADR-0023).

Two notes from the same window. Finding 52's `kernel/time/timer.c:1738`
`__run_timers` WARNING recurred across the rev-1–7 teardown windows, the
same detached-timer sibling signature. And a suite-operational gotcha:
suspending the suite with Ctrl+Z at the burst prompt kills it *without*
running its `EXIT` teardown trap, leaving the board dirty — any future
mid-run probe needs a teardown-safe pause instead. Evidence: suite
V-DPSW-5 revs 6–7, bead dpaa2-controlplane-dbf.19, captures under
`results/V-DPSW-5/`.
