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
