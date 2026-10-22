# fsl-mc does not auto-bind a runtime-created child dprc

Finding 54 of `docs/upstream/findings.md`; the full control-plane
diagnosis lives on ADR-0013 (§4, the isolation and container-tree
passage) and bead `dpaa2-controlplane-dbf.19`. This file is the
self-standing kernel-facing description. It corrects the long-held
assumption that the fsl-mc bus binds any plugged child dprc on its own:
it matches the obj_type, but attempts no probe for a child added after
boot by a container sync.

## Environment

- LX2160A-class board (DT), Linux 6.6.52 LF tree, MC firmware 10.39.0,
  restool v2.4.
- A kernel-owned tenant declared as a kernel-netlink namespace, whose
  objects live in a runtime-created child container (`dprc.2`) the kernel
  is meant to drive.

## Reproduction

The control plane creates a child dprc with restool and issues a bus
sync. The container appears on the bus but no driver ever attaches to it.
Any flow that creates a child container at runtime and expects the kernel
to drive its residents reproduces it.

## Observed traces

After the create and sync, the child is present on the bus with an empty
driver link and no probe attempt in the log:

```
/sys/bus/fsl-mc/devices/dprc.2            <- present after sync
/sys/bus/fsl-mc/devices/dprc.2/driver     <- absent (empty driver link)
```

dmesg shows zero probe attempts for `dprc.2`, and its residents are never
scanned (banked diagnostics: `results/V-DPSW-5/rev3/member-bind-sysfs.txt`
and `results/V-DPSW-5/rev3/member-bind-dmesg.txt`).

A bare bind write returns `ENODEV` with zero probe activity in the log. A
runtime-created child dprc reads back `unplugged` in its parent, and restool
refuses to plug a dprc by design:

```
restool dprc show dprc.1                                 <- dprc.2 ... unplugged
restool dprc assign dprc.1 --object=dprc.2 --plugged=1   <- "Cannot change plugged state of dprc"
echo dprc.2 > /sys/bus/fsl-mc/drivers/fsl_mc_dprc/bind   <- ENODEV, no probe attempt
```

Writing `driver_override` on the device first, then the bind, attaches it and
scans its residents (board-proven, V-DPSW-5 rev 7 operator probe):

```
echo fsl_mc_dprc > /sys/bus/fsl-mc/devices/dprc.2/driver_override   <- bypasses the plugged check
echo dprc.2 > /sys/bus/fsl-mc/drivers/fsl_mc_dprc/bind              <- DPRC device bound to driver
```

Either write still races the asynchronous post-sync scan over the same device:
before the kernel registers `dprc.2`, the `driver_override` write returns
`ENOENT` (no device node yet) and the bind write returns `ENODEV`.

## Analysis pointers

- `fsl_mc_dprc` does match the obj_type: `match_id_table` accepts any
  obj_type "dprc" (`drivers/bus/fsl-mc/dprc-driver.c` l.882-885), and once
  a container is bound it rescans and adds discovered child dprcs
  (`dprc_scan_container` l.746, `dprc_add_new_devices` l.202-224). The gap
  is upstream of all of that: the post-sync scan attempts no probe for a
  child the sync added after boot, so a runtime-created child dprc sits on
  the bus matched-but-unbound, its residents never scanned.
- The deeper refusal is the plugged state: a runtime-created child dprc is
  `unplugged`, and the fsl-mc bus match skips unplugged objects (the root
  dprc is the only exception). A bare bind write therefore returns `ENODEV`
  with no probe at all — the bus match refusing, not a probe failure. restool
  cannot clear the condition: it refuses to change a dprc's plugged state by
  design ("Cannot change plugged state of dprc").
- Writing `driver_override = fsl_mc_dprc` on the device bypasses the plugged
  check and makes the subsequent bind attach the container and scan its
  residents — the same mechanism `vfio-fsl-mc` already uses on child dprcs,
  which are never plugged either. The override is mandatory, not optional.
- Either write still races the asynchronous post-sync scan over the same
  device; before the scan registers `dprc.2` the `driver_override` write
  returns `ENOENT` and the bind write returns `ENODEV`. The race is timing,
  not authority — a later pass wins.
- Related noise: `fsl_dpaa2_switch` logs `dpsw_vlan_add_if` err `-119`
  while unregistering ports whose dpsw is being destroyed underneath it
  (an MC refusal during driver cleanup; harmless, rev 3 console).

## Mitigation

A kernel-owned tenant's child container needs the control plane to write
`driver_override = fsl_mc_dprc` and then the driver's `bind` file, not left
to the bus. The override is mandatory: it bypasses the plugged check an
unplugged child can never clear through restool. The pair is
level-triggered: re-kicked each convergence pass, with `ENOENT` on the
override write or `ENODEV` on the bind read as deferral (the write raced the
scan) rather than an error. A later pass wins the race and the container
binds; its residents are then scanned and driven as usual. Recovery on a
running board is the same override-then-bind sequence.

## Status

Candidate. Bus-level code (`drivers/bus/fsl-mc/dprc-driver.c`), so
upstream-relevant beyond the LF tree. Evidence: suite V-DPSW-5 rev 3
(banked diagnostics), rev 4 (the bind race), and rev 7 operator probe (the
plugged-state refusal and the override-then-bind working path), bead
`dpaa2-controlplane-dbf.19`, artifacts under `results/V-DPSW-5/`.
